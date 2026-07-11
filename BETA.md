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

## Boot menu

Two entries (UEFI and BIOS), **"Install Aegis OS" is the default**:

- **Install Aegis OS** — runs the guided installer straight away (see below).
- **Aegis OS (live / try the sphere)** — boots the live sphere without touching
  your disk.

## Install to disk

Just pick **"Install Aegis OS"** at the boot menu. It runs:

1. Arch's own **archinstall** — you choose the disk + base system.
2. The **Aegis overlay** — the `aegis` package plus the gaming stack (Steam,
   Proton/Xwayland, Vulkan) from the repo baked onto the ISO.

When archinstall finishes, **decline its final prompts (don't reboot from
inside it)** — you return to the Aegis installer, which applies the overlay,
verifies it, and then asks *you* to press Enter to reboot. A shutdown
inhibitor blocks stray reboots while only half the install is on disk.

If a machine ever ends up with base Arch but no Aegis (boots to a bare CLI
login), boot **"Install Aegis OS"** again and type `overlay` at the first
prompt — it skips archinstall and just applies the overlay to the existing
install.

The installed disk boots into the same sphere. (You can also run
`sudo aegis-installer` by hand from a console.)

## What you should see (live or installed)

- The **sphere kiosk** on tty1, running as the unprivileged `aegis` user (cage +
  Electron). It pulses to system/game audio and reacts to the agent's sleep-mode
  events. On a multi-monitor setup it uses a single output (no split).
- Drop to a console with **Ctrl-Alt-F2**. Useful checks:
  - `systemctl status aegis-agent` — the core agent
  - `systemctl status greetd` — the kiosk display manager
  - `sudo -u aegis pactl list short sinks` — audio devices seen by the kiosk
  - `journalctl -u aegis-agent -f` — watch `AEGIS_EVENT` lines
- In-kiosk terminal overlay: **Ctrl-Alt-T**.

## Known beta limitations

- **Electron 43:** the frontend was authored against Electron 33. Core APIs are
  stable; flagged for a boot check.
- **LLM throttle brain:** built but dormant on the image (the `llm` cargo
  feature is off). The deterministic self-throttle is active.
- **GPU:** open-source drivers only (mesa/radeon/intel + software Vulkan). No
  proprietary NVIDIA yet.

## Report

Issues → https://github.com/kevmill29/AegisOS/issues
