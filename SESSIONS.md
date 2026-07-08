# Aegis — Session Handoff

Last updated: 2026-07-08 (beta ISO: sphere kiosk + gaming + archinstall overlay, all integrated)

## MILESTONE: Full beta ISO — sphere kiosk + gaming stack + disk installer, one package

Built the complete beta on the archiso base. Aegis is now a single pacman
package (`aegis`, built by build.sh from `core-agent/archiso/package/` into a
local repo baked onto the ISO). The live env installs it and boots straight
into the sphere; `aegis-installer` installs the *same* package to disk.

What went in (`core-agent/archiso/`):
- `package/PKGBUILD` + `aegis.install` — binary package: agent, `aegis-session`
  (cage → Electron sphere), frontend, `fake-game`, and the systemd units.
  Post-install scriptlet enables `seatd`, `aegis-agent`, `aegis-kiosk`.
- `package/aegis-session` — kiosk launcher, adapted for systemd/seatd (drops the
  old hand-rolled udev; keeps the DRM wait, PipeWire monitor routing, and the
  ANGLE/SwiftShader Electron flags for GPU-less/VM WebGL).
- `package/aegis-kiosk.service` — runs the session on tty1, Conflicts the getty,
  restarts on failure.
- `airootfs/usr/local/bin/aegis-installer` — guided: runs `archinstall` for the
  base, then applies the Aegis overlay to the target (enables multilib + the
  `[aegis]` repo, copies the repo onto the target, `pacman -Sy aegis steam
  xorg-xwayland lib32-*`). Handles archinstall leaving /mnt mounted or not.
- `packages.aegis` — gaming/graphics/audio userland (steam, xorg-xwayland,
  Vulkan incl. lib32, pipewire, cage, electron).
- `build.sh` — now also: makepkg the package (as a `builder` user in the chroot),
  `repo-add` a local repo, wire `[multilib]` + `[aegis]` into the profile
  pacman.conf, ship the repo in the airootfs.

Build result (this session): `mkarchiso` resolved 689 packages (aegis + steam +
electron + lib32 Vulkan + archinstall + cage + …), the package scriptlet fired
("Aegis enabled — seatd, the core agent, and the sphere kiosk will start on
boot"), no conflicts → **1.9 GB `dist/aegis-arch.iso`, exit 0**. Shipped squashfs
verified to carry the enabled service symlinks (aegis-agent/aegis-kiosk/seatd),
`aegis-session`, the frontend, the `[aegis]` repo, steam, cage, and archinstall.

**NOT yet runtime-tested** (deferred to the debug phase per direction): booting
the sphere kiosk (cage→Electron under seatd; SwiftShader in a VM is slow), and a
full `aegis-installer` disk install. Likely first debug targets: Electron 43 vs.
the app (built for 33), the kiosk seat handoff (getty↔aegis-kiosk on tty1), and
archinstall's /mnt mount state at the overlay step.

## MILESTONE: Installer pivoted off the hand-rolled stack onto archiso + archinstall

Decision (user's call): stop hand-rolling the parts that are hardest to get
right — the custom stripped kernel, the busybox initramfs, and the bespoke
`gum`/`pacstrap` net-installer — since *every* install problem we hit (the UEFI
blind console, the boot-medium hang risks) came from that homemade plumbing.
Rebuild the ISO on Arch's **official `releng` archiso profile** so we inherit
the **stock Arch kernel** (all console/GPU/USB/net drivers built in) and a real
systemd live env, with **archinstall** for disk installs. Aegis rides on top as
an overlay, not a bespoke OS.

New pipeline (`core-agent/archiso/`, replaces `core-agent/iso/`):
- `build.sh` — clones `releng`, enables `[multilib]`, appends `packages.aegis`,
  overlays `airootfs/`, stages the musl `aegis-agent` binary + `fake-game`,
  enables `aegis-agent.service`, then runs `mkarchiso` inside the WSL Arch
  build chroot. Output: `dist/aegis-arch.iso`.
- `packages.aegis` — audio (pipewire/wireplumber/libpulse), graphics
  (mesa + swrast/radeon/intel Vulkan), kiosk infra (cage/seatd/greetd). Steam,
  electron, lib32 Vulkan, and the sphere kiosk are the next layer.
- `airootfs/etc/systemd/system/aegis-agent.service` (Delegate=yes for the
  cgroup self-throttle) + branded `airootfs/etc/motd`.

**Built + tested this session (thorough):**
- `mkarchiso` runs in WSL (loop/squashfs/overlay all present) → 1.6 GB
  `dist/aegis-arch.iso`, exit 0.
- **Boots under native UEFI (OVMF/QEMU)**: systemd-boot menu renders, then the
  full framebuffer console — the blind-black-screen that hung the old installer
  is *gone at the source* (stock kernel). Autologin root shell + the AEGIS OS
  motd render. Screenshots captured.
- Shipped squashfs verified to contain: `vmlinuz-linux` (stock kernel),
  `/usr/local/bin/aegis-agent` + its **enabled** service symlink, `archinstall`,
  cage/greetd/seatd/pipewire/wireplumber, and our motd.

Next on this track: layer the sphere kiosk (greetd → cage → electron frontend)
and an `aegis-installer` wrapper that runs archinstall with an Aegis config +
post-install overlay (agent + kiosk enablement), ideally as a pacman package in
a local repo baked into the ISO. The old `core-agent/iso/` tree is retained for
reference but deprecated.

## MILESTONE: ISO now boots the installer under native UEFI (the real-hardware blocker)

The user flashed the ISO (Etcher, not Ventoy) and it froze right after `Run
/init` on real UEFI hardware — kernel log visible, then nothing — while it
booted fully in VirtualBox. Root cause: our custom kernel had been stripped of
**every EFI framebuffer console driver** (`FB_EFI`, `SYSFB_SIMPLEFB`,
`DRM_SIMPLEDRM`, `DRM_FBDEV_EMULATION`, `FRAMEBUFFER_CONSOLE` all off) and kept
only `VGA_CONSOLE` — which exists only in BIOS text mode. So VirtualBox (BIOS)
had a console but native UEFI had none: the installer ran blind and blocked on
its first `gum` menu forever (looked like a hang, wasn't). Arch installs fine
on the same box because its kernel ships those drivers.

Fix (this session):
- Rebuilt Linux 6.12 with `CONFIG_SYSFB_SIMPLEFB`, `DRM_SIMPLEDRM`,
  `DRM_FBDEV_EMULATION`, `FB`, `FRAMEBUFFER_CONSOLE` all `=y` (bzImage #3,
  22:44). simpledrm owns the EFI GOP framebuffer → real interactive `tty0`
  under UEFI, and it also hands the installed kiosk a `/dev/dri/card0`.
- Removed `nomodeset` from the installer/live GRUB entries — it would have
  *disabled* simpledrm and re-broken the console. (`iso/grub.cfg`.)
- **Verified under UEFI/OVMF in QEMU**: the magenta "AEGIS OS NetInstaller"
  gum menu renders and eth0 gets a DHCP lease (screenshot delivered to user).
  This is the exact screen that was blank before.
- Also (next-step from the plan): started PipeWire + wireplumber +
  pipewire-pulse in `aegis-session` (non-fatal) so the installed OS has a real
  audio sink; sphere-reactivity-to-system-audio (monitor source routing) still
  TODO. Rebuilt ISO 23:23 carries all of the above.

Built per README first (frontend `npm run build`, compositor `cargo build
--release`, agent musl) — note the literal whole-workspace `cargo build
--target musl` fails on `llm-orchestrator` (needs an `x86_64-linux-musl-gcc`
cross-compiler for its TLS stack); irrelevant to the ISO, whose agent is
`-p agent-daemon` (pure Rust). Kernel is a README prerequisite, not built by it.

Uncommitted on disk: kernel `.config` (WSL side), `iso/grub.cfg`,
`iso/aegis-session`, the brain-wiring Rust (`self_throttle.rs`, `monitor.rs`,
`agent-daemon/src/main.rs`), and this file. Nothing committed/pushed yet this
session.

## MILESTONE: Installed OS boots into the working kiosk (cyan sphere, "aegis online")

Verified twice over: on the user's VirtualBox VM and in an automated QEMU/KVM
lab inside WSL (`/root/aegis/lab/` — lab-mkdisk/lab-boot/lab-screen/lab-cmd
scripts drive install→boot→screenshot without a human).

**Finished this session:**
- Built everything (frontend, compositor, musl agent) in WSL; compiled Linux
  6.12 from source (`/root/aegis/linux-6.12`, WSL) with vfat/iso9660/squashfs/
  overlayfs + DRM (simpledrm, vmwgfx, virtio-gpu, bochs) + FB_SIMPLE.
- Fixed installer/boot scripts end to end: CRLF everywhere (git `autocrlf`
  was reintroducing it — `.gitattributes` now forces LF), agent missing from
  the ISO payload, `root=LABEL=AEGIS` instead of raw /dev/sdX, `fail()`
  undefined in init, pacstrap cache OOM, single-mirror pacstrap failures,
  switch_root racing the agent launch.
- Phase 3 integration: new `aegis-init` (installed-system PID 1: mounts, agent
  cgroup, session supervisor w/ crashloop→console fallback), `aegis-session`
  (udevd → seatd → cage → Electron, pixman renderer, SwiftShader WebGL),
  `aegis-update` (zero-typing "Update Aegis OS" GRUB entry on the ISO),
  `aegis.ui=debug` boot mode that dumps the session log to the console.
- Frontend bugs found via lab (all fixed, committed): `vec3 active` — GLSL ES
  reserved word, desktop drivers tolerate it, ANGLE rejects → no sphere;
  uncaught AegisSphere constructor error unmounted the whole React tree →
  the original black screen; agent link/Hello events raced page load →
  "link lost" forever — replaced with an `aegis:ui-ready` pull handshake.
- All work committed on `main` (through `2c3950d` + this file). ISO:
  `dist/aegis-0.1.iso` (2026-07-07 20:12 build) — this exact build is
  lab-verified AND running on the user's VM.

**Broken / incomplete right now:** nothing known-broken. Incomplete (by
design, not regressions): PipeWire starts in `aegis-session` (game audio sink
works) and the default capture source is best-effort routed to the sink's
`.monitor` so the sphere reacts to system audio (frontend `audio.js` now
captures with the voice-DSP off) — unverified on real audio hardware, but
non-fatal if it doesn't take; Steam client installed but unlaunchable (no
xorg-xwayland
in the kiosk); LLM tier compiled out of the ISO agent (so on-target the brain
never runs and the background weight stays at the safe default 10 — the
brain→cgroup wiring itself is done and lab-verified, it just needs the llm
feature enabled on the image to take effect); Smithay compositor still
winit-only (cage stands in); updater syncs files but not packages nor the
installed grub.cfg; WSL note — `wsl.exe` strips quotes/joins args, so run
multi-command work via script files in /root/, never inline `bash -c` with
$vars or inner quotes.

**DONE (this session):** the throttle brain now writes its recommended
`cpu.weight` to the LIVE background cgroup — no longer advisory.
`SelfThrottle::apply_background_weight()` (cgroup-ctl) writes the clamped
weight to the background leaf; `engage()` resets to the safe default (10)
first so a new game never inherits a stale weight; the daemon refines it
*after* the immediate throttle so a slow/absent model can't delay
deprioritization. `ProcessMonitor` now exposes real total/available memory so
the brain decides on live telemetry, not zeros. Lab-verified as root against a
fake Proton reaper: cycling 4 launches, llama3.2:1b returned 70/75/55/50 and
the live `background/cpu.weight` file read back exactly those values each time
(default 10 with no game); PID moved normal↔background and restored on exit.
26 workspace tests pass; both the `--features llm` build and the default
musl/ISO release still compile (ISO agent stays lean — llm feature is off
there, so on-target the weight stays at the safe default 10).

**Exact next step on resume:** real-hardware validation — the user reflashes
the 23:35 ISO (UEFI fix + PipeWire + sphere audio routing) and confirms it
installs and boots the kiosk on their AMD rig. Pending that, in order: add
`xorg-xwayland` + a supervised Steam launch (needs a reinstall, not an update)
— the actual point of a gaming OS; LLM tier on the target image; voice/STT;
Smithay DRM backend. Also still open: commit + push this session's uncommitted
work (kernel config, grub.cfg, aegis-session, aegis-install, frontend
audio.js, brain-wiring Rust, SESSIONS.md).

---

# Previous milestone (2026-07-07, all charter phases implemented)

## MILESTONE: All charter phases implemented and live-verified

Every phase from the charter now has working, tested code — 25 workspace tests
pass; each phase verified against real systems, not just unit tests.

**Phase 3 — Compositor.** `compositor/` (separate workspace, kept out of the
core-agent/ISO dependency graph). `launch-kiosk.sh` runs the frontend under
cage (the working kiosk path today). `aegis-compositor` is a Smithay scaffold
that COMPILES against the real Smithay dep (all native wayland/EGL/calloop deps
resolved) and runs — auto-detects Winit (nested, dev) vs Udev (bare metal)
backend; the Wayland-display/shell/event-loop integration points are stubbed
and documented in src/state.rs as the next targets.

**Phase 4 — Steam/Proton.** `core-agent/steam-integration/` crate: a tolerant
VDF/ACF parser + library scanner. Verified against the REAL Steam install
(discovered 10 games — Cyberpunk, Elden Ring, PoE2, etc. — with correct AppIDs
and install dirs from actual appmanifest files). Also finds Proton versions.
`examples/scan_library.rs` is the live demo. 8 unit tests.

**Phase 5 — LLM orchestration.** `core-agent/llm-orchestrator/` crate: 3-tier
hybrid brain (local Ollama llama3.2:1b → cloud Anthropic API → deterministic
rule-based floor). Verified live: real llama3.2:1b classified telemetry and
returned a clamped cpu_weight + rationale; confirmed graceful degradation
(Ollama stopped → automatic rule-based fallback). Cloud tier uses
claude-haiku-4-5 (cheapest/fastest, right call for bounded JSON classification)
via the Messages API; inert without ANTHROPIC_API_KEY. `decide()` is
infallible by construction — the floor never fails, so throttling never
depends on an LLM being up. The LLM can ONLY emit a clamped cpu.weight for the
agent's own cgroup — it cannot name PIDs or touch ProtectedSet paths.
Ollama installed as a systemd service in WSL (needed zstd first).

**Integration.** agent-daemon now depends on steam-integration (logs library
at startup) and optionally llm-orchestrator behind a `--features llm` flag
(kept OFF by default so the static-musl ISO binary stays lean and network-
free). Full end-to-end verified: daemon found 10 Steam games, detected a fake
Elden Ring launch, the real local LLM recommended a throttle weight, sleep/wake
events fired. The brain's number is advisory this milestone (logged, not yet
written to cpu.weight) — that wiring is the one remaining connect-the-dots step,
deliberately deferred so a slow/wrong model can't delay the throttle that
SelfThrottle already applies with its safe default.

## Full crate map (core-agent workspace)

- process-monitor — /proc scanning, Steam/Proton detection, ProtectedSet guard
- cgroup-ctl — cgroup v2 self-throttle, delegation discovery
- steam-integration — VDF/ACF library + Proton scanner (Phase 4)
- llm-orchestrator — 3-tier throttle brain (Phase 5)
- agent-daemon — the aegis-agent binary tying it together
Separate: compositor/ (Phase 3), frontend/ (Phase 2 Electron sphere + terminal)

---

# Previous milestone (2026-07-07, post-ISO)

## MILESTONE: Aegis OS v0.1 bootable ISO — built and demo'd

`dist/aegis-0.1.iso` (27.6MB, hybrid BIOS/UEFI via grub-mkrescue): custom-
compiled Linux 6.12 kernel (defconfig + cgroup controllers MEMCG/PIDS/
CFS_BANDWIDTH + AHCI/NVMe/USB-storage/EFI-stub/ISO9660 built-in, no modules),
busybox initramfs, static musl aegis-agent. Verified in QEMU end-to-end:
boots in ~2s, agent starts with FULL cgroup self-throttling enabled (no
fallback — it's root on its own OS), typing `fake-game` at the OS shell
triggered real detection (AppId=730), self-throttle to cpu.weight 10, restore
to 100 on exit. Build inputs/scripts live in `core-agent/iso/` (init,
grub.cfg, fake-game, build-iso.sh); kernel tree at WSL-side /root/aegis/.
Two boot bugs found & fixed: missing /bin/sh symlink (kernel panic), and
double shell on ttyS0 eating alternate keystrokes.

## MILESTONE: SelfThrottle verified against real kernel cgroups

Via `wsl -u root` (password-free root — this unlocked everything): simulated
systemd Delegate=yes by launching the daemon inside /sys/fs/cgroup/aegis-
delegated. Daemon subdivided it (normal weight=100 / background weight=10),
and its PID observably moved normal→background→normal in lockstep with a fake
game session. The vacate-before-subtree_control ordering worked (no EBUSY).

## MILESTONE: Frontend complete and verified end-to-end

`npm run build` green. Electron ran 30s against the simulate-mode daemon in
WSL: link established, Hello sync + full SleepModeInitiated/Ended cycle
received over TCP 127.0.0.1:48620 (WSL→Windows localhost forwarding works).
Also added: **Ctrl+Alt+T bash terminal overlay** (global shortcut + renderer
fallback; commands exec in real bash — native on Linux, via WSL in dev;
streamed stdout/stderr; Esc closes, Ctrl+C kills). Exec path verified
headlessly via AEGIS_TERM_SELFTEST=1. Voice-open is a documented stub
pending local STT (whisper.cpp) — mic pipeline already exists in audio.js.

## Hardware enablement (added post-ISO, all QEMU-verified)

Kernel now includes: wired NICs (Intel e1000/e1000e/igc, Realtek r8169, USB
RTL8152/AX88179), HDA audio + Realtek/HDMI/Conexant/generic codecs +
snd-usb-audio, Intel i915 GPU (from defconfig). Init brings up loopback +
DHCP on all wired interfaces (`iso/udhcpc-script` handles IP/route/DNS).
Verified in QEMU: DHCP lease acquired, DNS resolved, real HTTP fetch from the
internet succeeded, sound card enumerated in /proc/asound/cards. ISO now
28.1MB. NOT included (Phase 2, needs firmware blobs + real userland): WiFi
(wpa_supplicant), AMD/NVIDIA GPU, Mesa/Vulkan, any sound server (PipeWire).

## Known minor gaps

- On the booted ISO, detection logged `game=None` (game child name resolution
  differs under busybox process trees) — cosmetic, AppId still parsed.
- Native (non-Proton) game detection still unimplemented.
- ISO has no compositor/frontend yet — that's the next OS milestone (cage +
  Electron requires a real userland base, not busybox).
- Visual check of terminal overlay + sphere is untested by human eyes; all
  plumbing is machine-verified.

---

# Previous session notes (2026-07-07, post-MVP verification)

## MILESTONE: Core Agent MVP verified end-to-end in WSL2

Ran the real daemon (not `--simulate`) against a fake Proton `reaper` process
(bash script named `reaper` with authentic Steam argv). Daemon booted, its
process monitor detected the launch (`app_id=Some("730")`, game child resolved),
emitted `SleepModeInitiated` over stdout + the TCP event bus, then detected the
exit and emitted `SleepModeEnded`. Found and fixed a real bug in the process:
sysinfo's default refresh never populates `cmd`, so AppId parsing silently
returned None — `ProcessMonitor::refresh` now uses an explicit
`ProcessRefreshKind` requesting cmd+exe. A debug helper lives at
`process-monitor/examples/dump_reaper.rs`.

Bare-metal note: nothing in the daemon depends on WSL — it uses only `/proc`,
cgroup v2 file I/O, and localhost TCP. WSL is dev tooling, not a dependency.

Phase 2 (frontend) is scaffolded and mid-flight: `frontend/` has the full
Electron+React+Three.js app (TCP client in `electron/main.cjs`, audio-reactive
sphere in `src/sphere/AegisSphere.js`), `npm install` completed, but
`npm run build` + the Electron end-to-end smoke test have NOT run yet — that's
the immediate next step for Phase 2.

## Status: nothing is broken. Build is green, 17/17 tests pass.

Dev environment: WSL2 Ubuntu 24.04 (`wsl -d Ubuntu`), Rust 1.96.1 via rustup.
Project lives on the Windows-side path, edited from Windows, built via
`wsl -d Ubuntu -e bash -lc 'source $HOME/.cargo/env; cd /mnt/c/Users/Kevin/projectsclaude/core-agent && cargo test'`.

## What's finished

- `core-agent/` Cargo workspace: `process-monitor` (lib), `cgroup-ctl` (lib),
  `agent-daemon` (bin `aegis-agent`).
- **process-monitor**: process snapshotting via `sysinfo`; Steam/Proton game
  detection by finding the `reaper` process and parsing `AppId=` from its
  argv; `ProtectedSet` safety guardrail (systemd, NetworkManager, sshd,
  display stack, PID 1, own PID — hard denylist, checked at point of use).
  6 unit tests, synthetic process trees only.
- **cgroup-ctl**: cgroup v2 file-write wrapper (`cpu.weight`, `cpu.max`,
  `memory.high`, `cgroup.subtree_control`, `cgroup.procs`). `own_cgroup_dir()`
  discovers the daemon's actual delegated cgroup via `/proc/self/cgroup`
  rather than assuming a hardcoded path (this was a real bug, found and
  fixed this session — see ARCHITECTURE.md "Validation status" section).
  `SelfThrottle` moves the daemon's own PID into a low-priority `background`
  leaf cgroup on game launch, back to `normal` on exit, with the ordering
  required by cgroup v2's "no internal process" rule (vacate parent before
  enabling subtree_control). 11 unit tests, all against a tempdir (not real
  cgroupfs).
- **agent-daemon**: 2s poll loop, state machine on game-session
  presence/absence, emits `AEGIS_EVENT: {json}` lines to stdout as a stub for
  the not-yet-built frontend IPC.
- `systemd/aegis-agent.service` + `systemd/install.sh` — packaging for real
  deployment (`Slice=aegis.slice`, `Delegate=yes`, hardening flags). Script
  syntax-checked (`bash -n`), **not executed for real** (needs interactive
  `sudo` password, deliberately not run without you present).
- `ARCHITECTURE.md` — design rationale, permission model, detection
  heuristics, and an explicit "Validation status" section listing what's
  actually verified vs. still assumed.

## What's incomplete (not broken, just not built yet)

- **Never tested against a real writable delegated cgroup.** WSL2 has no
  passwordless `sudo`, so `SelfThrottle::engage`/`disengage` has only been
  exercised via tempdir unit tests — the actual kernel accepting the
  `subtree_control` write sequence is unverified. Confirmed instead that the
  daemon fails *gracefully* (logs a warning, keeps running monitor-only) when
  it lacks cgroup write access — ran the real binary for this.
- **Never tested against a real Steam+Proton game launch.** All `reaper`
  detection logic is validated with synthetic process lists only.
- Native (non-Proton) Linux game detection: not implemented — there's no
  `reaper` process for those, needs a different signal (documented as a known
  gap in ARCHITECTURE.md).
- Frontend (Electron/React/Three.js sphere), Wayland compositor (`cage` /
  Smithay), and LLM orchestration layer: not started at all — Phase 2, out of
  scope until Core Agent is validated on real hardware.

## Exact next step on resume

Pick one:
1. **Install for real** — run `core-agent/systemd/install.sh` yourself (it
   needs your `sudo` password interactively, which is why I didn't run it) on
   either this WSL2 box or real target hardware, then confirm
   `engage`/`disengage` actually work by launching a Proton game and watching
   `journalctl -u aegis-agent -f` for the `AEGIS_EVENT` lines and a
   self-throttle log line.
2. **Or, if no real Steam/Proton available yet**: implement native
   (non-Proton) game detection in `process-monitor/src/steam.rs` (misnamed
   now — would want a rename to something like `game_detection.rs` once this
   lands), using the Steam-library-path heuristic already sketched in
   ARCHITECTURE.md.

No uncommitted risk either way — there is no git repo initialized yet in this
project, so "state" is just the files on disk described above.
