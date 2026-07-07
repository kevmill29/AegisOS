# Aegis — Footprint Benchmarks & Recommended Specs

Measured 2026-07-07. Methodology caveats up front: daemon numbers from WSL2
(Ubuntu 24.04, 16-core host), boot numbers from QEMU TCG (software emulation —
boot *timing* uses the guest clock and is roughly representative; RAM floor is
exact), Electron numbers from Windows (Linux builds typically run slightly
leaner). Bare-metal validation should repeat the boot tests, but these are the
right order of magnitude.

## Measured results

| Component | Metric | Result |
|---|---|---|
| aegis-agent (idle, 76-proc table) | CPU | 0.8% of one core |
| aegis-agent (idle) | RSS | 1.6 MB |
| aegis-agent (stressed, 1,554-proc table) | CPU | 9.8% of one core |
| aegis-agent (stressed) | peak RSS | 4.7 MB |
| Aegis OS v0.1 ISO | boot to live agent | ~1.8 s (guest clock) |
| Aegis OS v0.1 ISO | RAM floor | **96 MB boots; 64 MB fails (GRUB)** |
| Aegis OS v0.1 ISO | storage | 27.6 MB |
| Electron frontend (sphere @60fps) | working set, 6 processes | ~545 MB |
| Electron frontend | CPU | ~15% of one core (continuous render) |

Notes:
- Agent poll cost scales with process-table size (it's a 2s-interval full
  `/proc` scan). A typical gaming desktop sits at 300–600 processes → expect
  ~2–4% of one core. If that ever matters, the known optimization is switching
  to netlink proc events (kernel pushes process start/exit) instead of polling.
- Electron working-set totals double-count shared memory across its 6
  processes; true private footprint is lower. Still, the frontend is ~100×
  heavier than everything else Aegis combined — any future footprint work
  starts there, not in the Rust.
- The frontend is not yet cgroup-managed; on game launch it should be moved
  into the same `background` group as the agent (planned).

## Recommended specs

**The honest headline: Aegis's own requirements are negligible — recommended
specs for an Aegis gaming machine are set by the games you want to play, not
by Aegis.** The full stack (agent + compositor + Electron sphere) budgets
under ~1 GB RAM and well under one CPU core, versus the 2–4 GB a stock
Windows install idles at. That delta is the pitch: more of the hardware goes
to the game.

| Tier | CPU | RAM | GPU | Storage |
|---|---|---|---|---|
| Aegis OS core only (v0.1 ISO, headless/appliance) | any x86_64 | 128 MB | none | 64 MB |
| Aegis full stack (Phase 2: compositor + sphere UI) | 2 cores | 2 GB | any Vulkan-capable | 8 GB |
| **Recommended gaming build** | **6+ cores** (game + 1 core headroom) | **16 GB** (game budget + <1 GB Aegis) | **per target games** — RDNA2+/RTX class for modern AAA via Proton | **NVMe, 512 GB+** (game library dominates) |

GPU guidance is entirely game-driven: the sphere UI is trivial WebGL (any
integrated GPU renders it); Proton/DXVK wants the same Vulkan-capable GPU
you'd buy for Linux gaming generally (AMD preferred for Mesa driver quality).

## Reproduce

- Daemon idle/stress: see SESSIONS.md history — run the musl release binary,
  sample `/proc/<pid>/stat` utime+stime over 30 s; stress = `for i in $(seq
  1 1500); do sleep 300 & done` first.
- RAM floor: `qemu-system-x86_64 -m <MB> -cdrom dist/aegis-0.1.iso -nographic`,
  select the serial-console GRUB entry, look for the AEGIS banner.
- Electron: launch `frontend/` app, sum `WorkingSet64` across `electron`
  processes after 15 s.
