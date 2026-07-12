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

**Testing in a VM?** COPY the ISO out of `dist/` first (e.g. to
`aegis-test.iso`) and attach the copy — rebuilds overwrite `dist/aegis-arch.iso`
in place, and changing the medium under a running VM produces phantom
"SQUASHFS data corrupt" errors. VirtualBox settings that matter: **EFI
enabled** (System), Graphics Controller **VMSVGA** + **128 MB** video memory
(Display), ≥4 GB RAM, ≥30 GB disk. Without a KMS-capable virtual GPU the
kiosk can't start — it will tell you so on screen and drop to a text login.

Every ISO must pass the automated boot gate before shipping:
`bash core-agent/archiso/test-boot.sh` boots it in QEMU, asserts the kiosk
session survives, and screendumps the display to confirm the sphere actually
rendered (`AEGIS_TEST_VGA=VGA` re-runs it with an unaccelerated GPU).

## Boot menu

Two entries (UEFI and BIOS), **the live sphere is the default**:

- **Aegis OS (live · install from inside)** — boots the live sphere without
  touching your disk. Installation happens from *inside* it (see below).
- **Aegis OS installer (text mode / recovery)** — console installer, for BIOS
  machines and recovery.

## Install to disk (the normal way)

Boot the default entry. The sphere comes up live — audio, monitors, the works —
with an **"Install Aegis OS →"** button in the corner (it only exists when
running from the USB). Click it and answer four questions:

1. **Which disk** (everything on it is erased — it warns you twice)
2. **Username + password**
3. **Computer name**

Behind the panel, Arch's own `archinstall` runs fully unattended from a
generated config (systemd-boot, ext4, NetworkManager, multilib), then the
**Aegis overlay** (agent, sphere kiosk, Steam/Proton/Vulkan gaming stack) goes
on top and is verified before the "Restart now" button appears. The GUI path
requires **UEFI boot** and a **30 GB+ disk**; ethernet is the easy network
option for now.

## Install to disk (text mode / recovery)

Pick **"Aegis OS installer (text mode / recovery)"** at the boot menu — the
console installer walks the same archinstall → overlay sequence interactively.
When archinstall finishes, decline its final prompts (a shutdown inhibitor
blocks stray reboots until the overlay is applied and verified).

If a machine ever ends up with base Arch but no Aegis (boots to a bare CLI
login), boot the text installer and type `overlay` at the first prompt — it
skips archinstall and just applies the overlay to the existing install.

The installed disk boots into the same sphere. (You can also run
`sudo aegis-installer` by hand from a console.)

## What you should see (live or installed)

- The **sphere kiosk** on tty1, running as the unprivileged `aegis` user (cage +
  Electron). It pulses to system/game audio and reacts to the agent's sleep-mode
  events. On a multi-monitor setup the sphere sits on the primary screen and
  its light clouds across the others (Windows-style extend, no seam split).
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
