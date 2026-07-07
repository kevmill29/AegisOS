# Aegis Core Agent — Architecture (v0.1 scaffold)

Scope of this document: the `/core-agent` Rust workspace only. Compositor and
front-end (Phase 2) are out of scope until this layer is solid.

## Crate layout

```
core-agent/
  process-monitor/   pure detection logic: process snapshots, Steam/Proton
                      heuristics, the safety-critical ProtectedSet. No I/O
                      beyond reading /proc (via sysinfo). No policy decisions.
  cgroup-ctl/         cgroup v2 mechanism: create groups, set weights/limits,
                      move PIDs. Depends on process-monitor only for the
                      ProtectedSet type, so the "never touch a protected PID"
                      check is enforced at the point of the actual write, not
                      just by caller discipline.
  agent-daemon/       the binary (`aegis-agent`): poll loop, state machine,
                      wires monitor + cgroup-ctl together, emits events.
```

Dependency direction is strictly `agent-daemon -> cgroup-ctl -> process-monitor`.
`process-monitor` never depends on `cgroup-ctl` — detection must stay usable
(and testable) with zero privileges, since most of it will run in contexts
(dev laptop, CI) that never get real cgroup write access.

## Why cgroups v2, not `nice`/`ionice`/`cpulimit`

`nice` only affects CPU scheduling priority for a single process, doesn't
cover memory/IO, and doesn't compose: if the agent ever spawns helper
processes (local LLM inference, telemetry collectors), each one needs its own
`nice` call and there's no single knob to move "the agent and everything it
spawns" up or down at once. A cgroup is that single knob — moving one PID
into a cgroup moves its entire subtree with it (children inherit membership
unless they're explicitly moved elsewhere), and the same primitive covers
CPU (`cpu.weight`, `cpu.max`), memory (`memory.high`), and IO (`io.max`) —
one for the frontend broadcast state to poll if a "how throttled am I right
now" dial is ever needed.

We target **cgroup v2 (unified hierarchy)** exclusively, not v1. v1's
per-controller separate hierarchies are legacy at this point (deprecated
upstream, and most current-gen distros the target hardware would run — Bazzite,
SteamOS 3, Arch — default to v2-only). `require_cgroup_v2_mounted` fails fast
with a clear error rather than silently no-op'ing on a v1 system, since a
throttle that silently doesn't apply is worse than one that loudly refuses to
start.

## Permission model — the part that actually gates deployment

Writing to `/sys/fs/cgroup/**` requires either:

1. **Running as root**, with the daemon managing a subtree directly
   (e.g. `/sys/fs/cgroup/aegis.slice/*`), or
2. **systemd delegation**: a unit file with `Delegate=yes` under a `Slice=`,
   which hands the *entire subtree* under that slice to the unit's own UID —
   this is what lets a non-root systemd service safely create and manage its
   own child cgroups.

Verified empirically in the WSL2 Ubuntu 24.04 dev environment: cgroup v2 is
mounted with `cpuset cpu io memory hugetlb pids rdma` controllers active, but
`/sys/fs/cgroup/init.scope` (the default cgroup for an interactive shell) is
`root:root`-owned and a non-root `mkdir` under it is rejected with
`Permission denied`, as expected. `sudo` in that environment requires an
interactive password (no passwordless sudo configured), so this codebase does
not attempt any command that assumes privilege it hasn't been granted —
`SelfThrottle::setup` catches the permission failure and the daemon falls
back to **monitor-only mode**, logging a warning, rather than crashing or
retrying with elevated privileges it wasn't given. That fallback is
deliberate: on real target hardware the agent will run as a systemd *system*
service (root), where this isn't a concern, but during development it should
never come as a surprise that non-root iteration doesn't get throttling.

**Production setup, implemented**: `systemd/aegis-agent.service` +
`systemd/install.sh`. The unit uses `Slice=aegis.slice` + `Delegate=yes`, plus
`NoNewPrivileges`/`ProtectSystem=strict`/`ProtectHome`/`PrivateTmp` as
defense-in-depth (systemd is trusted to punch the one hole this daemon
actually needs — its own delegated cgroup subtree — while `ProtectSystem=strict`
locks down everything else). This keeps the daemon itself non-setuid and
avoids writing any privilege-escalation logic into Rust at all.

Critically, the daemon does **not** assume a fixed top-level path like
`/sys/fs/cgroup/aegis.slice` is where it lives and is writable — that was the
v0.1 design and it's wrong: systemd decides the actual cgroup path (which,
depending on version/config, may nest the service under `aegis.slice` in a way
that doesn't match a hand-guessed path), and delegation only grants write
access to *that* subtree, not to an arbitrary sibling the daemon invents.
Instead, `cgroup_ctl::own_cgroup_dir` reads `/proc/self/cgroup` at startup —
the unified-hierarchy `0::<path>` line — and treats *that* as the root to
subdivide. This was verified against the WSL2 dev environment: before the fix,
the daemon looked for `/sys/fs/cgroup/aegis.slice` (which doesn't exist here)
and produced a misleading permission error; after the fix, it correctly
discovers `/sys/fs/cgroup/init.scope` (the actual cgroup an interactive WSL2
shell runs in) and fails there instead, with an error that points at the real
path.

`SelfThrottle::setup` then creates two leaf children, `normal` and
`background`, under that discovered path, and — this ordering is
load-bearing, not incidental — moves the daemon's own PID into `normal`
*before* enabling `cpu`/`memory` in the parent's `cgroup.subtree_control`.
Cgroup v2's "no internal process" rule forbids a cgroup from simultaneously
holding member processes directly and distributing to children via
`subtree_control`; since our own PID starts out sitting directly in whatever
cgroup we were delegated, we have to vacate it into a leaf *first*, or the
kernel rejects the `subtree_control` write with `EBUSY`. Getting this
backwards is exactly the kind of bug that only shows up once you have real
delegated write access to test against — which is why `cgroup-ctl` now also
has file-write-level unit tests (format of `cpu.weight`, `cpu.max`,
`cgroup.subtree_control`, `cgroup.procs`, and the `/proc/self/cgroup` parsing)
run against a plain tempdir. Those tests can't catch a kernel rejecting the
sequence (no real controller semantics in a tempdir), but they do pin the
exact bytes written and the ordering of operations, which is what a future
refactor is most likely to break silently.

## Process detection — inference, not integration

The agent has no Steam Web API key, no VDF-parsing of Steam's local config,
and no IPC hook into the Steam client. Deliberately: those are extra failure
surfaces and extra permissions for information the process tree already
exposes for free. Detection is pure inference over `/proc`:

- **Steam client presence**: process named `steam` or `steamwebhelper`.
- **Active game session**: Proton wraps every launch in a small supervisor
  process named exactly `reaper`, invoked as
  `reaper SteamLaunch AppId=<id> -- <proton> waitforexitandrun <game>`.
  `reaper`'s only job is to clean up the game's process tree on exit, so its
  *presence* is the signal — not any fuzzy "wine"/"proton" substring match on
  argv, which also shows up in unrelated library/shim paths.
- **AppID**: parsed straight out of reaper's own argv (`AppId=730` etc).
- **Game binary**: found by walking down the process tree from `reaper`
  through wine's plumbing layers (`wineserver`, `wine64-preloader`) to the
  first descendant that isn't wine infrastructure itself. This is a
  best-effort label for the UI, not something correctness depends on —
  throttling and session-lifetime tracking key off `reaper`'s PID, which is
  stable for the session's whole lifetime, not the guessed game PID.

This means non-Proton (native Linux) games aren't detected yet — there's no
`reaper` for those. That's a known gap, tracked as follow-up: native titles
need a different signal (most likely: known Steam library path prefix in
`exe`, i.e. `.../steamapps/common/**`, combined with the process being a
child of `steam` itself rather than a system service).

## Safety guardrails (`process-monitor::ProtectedSet`)

A hardcoded, always-checked-immediately-before-any-mutation deny-list:
`systemd`, `init`, `kthreadd`, `NetworkManager`, `dhcpcd`/`dhclient`,
`wpa_supplicant`, `sshd`, and the current display stack (`Xorg`,
`Xwayland`, `cage`, `kwin_wayland`, `gnome-shell`, `weston`, `sway`), plus
PID 1 unconditionally, plus the agent's own PID and any runtime-supplied
extras (the active compositor binary, resolved at startup rather than
hardcoded — Phase 2 swaps `cage` for a custom Smithay compositor and this
list needs to follow that swap without a code change).

This check lives in `process-monitor` (the detection crate) but is *enforced*
in `cgroup-ctl::CgroupHandle::add_process`, which requires both a
`ProcessInfo` and a `&ProtectedSet` as arguments — there's no code path to
move a PID into a cgroup without passing through the check, by construction,
not by convention. `SelfThrottle` (moving the agent's *own* PID) skips this
check deliberately: the agent can't accidentally select a different,
protected PID when the only PID in play is `std::process::id()`.

Note: v0.1 only *moves* PIDs between cgroups (for self-throttling of the
agent). It does not yet kill or freeze arbitrary processes — that's future
scope once there's an actual policy for "which non-essential OS tasks get
throttled during a game," and the guardrail above is where that policy's
blast radius gets capped regardless of what the policy engine (rule-based or
LLM-assisted) decides.

## What's stubbed, not implemented

- **Frontend broadcast**: `agent-daemon::events::emit` currently prints
  `AEGIS_EVENT: {json}` to stdout. This is a placeholder for whatever IPC
  transport Phase 2 picks (Unix socket, D-Bus signal) — the call sites don't
  change, only `emit`'s body does.
- **LLM orchestration**: no model integration yet. The state machine in
  `main.rs` is intentionally simple (rule-based: reaper present/absent) so
  that a future LLM-based policy layer has a well-defined, already-tested
  signal (`GameSession`, protected-process list, resource snapshots) to
  consume rather than needing to re-derive process semantics itself.
- **Throttling non-agent OS tasks**: today `SelfThrottle` only throttles the
  agent itself. Broader "deprioritize non-essential OS tasks" needs an
  explicit allowlist/policy (which background services are safe to
  throttle) before it's implemented — this is deferred specifically because
  getting that policy wrong has real blast radius, unlike self-throttling.

## Validation status — what's actually been verified vs. assumed

Worth being explicit about, since it's easy for "compiles and passes unit
tests" to quietly stand in for "works":

- **Verified for real**: cgroup v2 detection, `/proc/self/cgroup` discovery
  and parsing, all `cgroup-ctl` file-write formats (via tempdir tests), the
  full detection pipeline (via synthetic process-tree unit tests), and the
  daemon's permission-failure fallback path (ran the actual binary in WSL2,
  confirmed it discovers the real cgroup path, hits the expected
  `Permission denied`, logs a warning, and keeps running in monitor-only mode
  rather than crashing).
- **Not yet verified**: `SelfThrottle::engage`/`disengage` against a real,
  writable delegated cgroup — this needs either root or an actual
  `systemd --user`/`Delegate=yes` grant, neither of which is available
  non-interactively in the current dev sandbox (no passwordless `sudo`
  configured). Installing `systemd/aegis-agent.service` via
  `systemd/install.sh` on real target hardware (or WSL2 with an interactive
  `sudo` password) is the next concrete step to close this gap. Also
  unverified: detection against a *real* Steam+Proton game launch (all
  `steam.rs` tests use synthetic process lists) — that needs an actual Steam
  install with a Proton title, which isn't present in this dev environment.

## Dev environment

Built and tested against WSL2 (Ubuntu 24.04.2 LTS, kernel
`6.6.87.2-microsoft-standard-WSL2`), not native Windows — there is no cgroup
v2, no `/proc` process semantics, and no Proton on Windows, so this is the
closest available approximation of the eventual Linux target. Toolchain:
`rustc`/`cargo` 1.96.1 installed via rustup inside the WSL distro. The
workspace lives on the Windows-side path (`/mnt/c/...`) for editor
convenience; if incremental build times become painful on 9p/DrvFs, the fix
is to build from a native WSL path (`~/aegis`) instead, not to change the
code.

## Phase status (all charter phases implemented)

| Phase | Component | Status |
|---|---|---|
| 1 | Core Agent (this workspace) | ✅ built, tested, live-verified incl. real cgroups |
| — | Aegis OS v0.1 bootable ISO | ✅ built (28MB), QEMU-verified, net+audio+DHCP |
| 2 | Electron/React/Three.js sphere UI + Ctrl+Alt+T terminal | ✅ built, event stream verified across WSL→Windows |
| 3 | Compositor: cage kiosk (working) + Smithay scaffold | ✅ launcher works; `../compositor` scaffold compiles+runs |
| 4 | Steam/Proton integration | ✅ `steam-integration/` — scanned real library (10 games) |
| 5 | LLM orchestration (Ollama + cloud + rules) | ✅ `llm-orchestrator/` — live-verified w/ llama3.2:1b |

`steam-integration` and `llm-orchestrator` (behind the daemon's optional `llm`
feature) feed the throttle brain; the default/ISO build excludes the LLM tier
so the static musl binary stays lean. The brain's recommended `cpu.weight` is
currently advisory (logged); wiring it into the live cgroup is the one
remaining connect-the-dots step, deferred so a slow model can never delay the
throttle `SelfThrottle` already applies with its safe default.
