# Aegis OS — Beta

A gaming-focused agentic OS: an Arch-based live/installer image that boots into
an audio-reactive **sphere** and runs a background **agent** that detects game
launches and self-throttles so it never steals frames from your game.

## Flash it

1. Build (or obtain) the image: `dist/aegis-arch.iso` (~1.9 GB).
   Build from source in the WSL Arch chroot:
   `wsl -d Ubuntu -u root -- bash core-agent/archiso/build.sh`
2. Write it to a USB stick with **Balena Etcher** (or `dd`).
3. Boot the stick. Both **UEFI** and **BIOS** are supported — it uses the stock
   Arch kernel, so the console renders everywhere (no more blind UEFI screen).

## What you should see

- The boot menu ("Aegis OS", UEFI or BIOS), then the stock Arch live boot.
- The **sphere kiosk** takes over tty1 (cage + Electron). The sphere pulses to
  system/game audio and reacts to the agent's sleep-mode events.
- Drop to a shell any time with **Ctrl-Alt-F2** (root autologin). Useful checks:
  - `systemctl status aegis-agent` — the core agent
  - `systemctl status aegis-kiosk` — the sphere session
  - `journalctl -u aegis-agent -f` — watch `AEGIS_EVENT` lines
- In-kiosk terminal overlay: **Ctrl-Alt-T**.

## Install to disk

From the live shell (Ctrl-Alt-F2):

```
sudo aegis-installer
```

Step 1 runs Arch's own **archinstall** (you pick the disk + base system).
Step 2 lays the Aegis overlay on top — the `aegis` package plus the gaming
stack (Steam, Proton/Xwayland, Vulkan) from the repo baked onto the ISO — so the
installed disk boots into the same sphere.

## Known beta limitations (being worked)

- **Steam as root:** the kiosk session currently runs as `root`, and Steam
  refuses to launch as root. Game *detection*, self-throttle, the sphere, and
  audio all work; actually launching Steam needs the kiosk moved to a dedicated
  non-root user (planned via greetd). Until then, launch Steam from a non-root
  login on another VT.
- **Electron 43:** the frontend was authored against Electron 33. Core APIs are
  stable; flagged for a boot check.
- **LLM throttle brain:** built but dormant on the image (the `llm` cargo
  feature is off). The deterministic self-throttle is active.
- **GPU:** open-source drivers only (mesa/radeon/intel + software Vulkan). No
  proprietary NVIDIA yet.

## Report

Issues → https://github.com/kevmill29/AegisOS/issues
