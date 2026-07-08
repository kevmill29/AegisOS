#!/usr/bin/env bash
# Build the Aegis OS ISO on top of Arch's official `releng` archiso profile.
#
# Why this replaces core-agent/iso/build-iso.sh: the old path hand-rolled a
# custom kernel, a busybox initramfs, and a bespoke gum/pacstrap installer —
# every install issue we hit (the UEFI blind console, boot-medium hangs) came
# from that homemade plumbing. archiso gives us the *stock* Arch kernel (all
# console/GPU/USB/net drivers built in — boots reliably on UEFI and BIOS) and a
# proper systemd live environment. Aegis rides on top as an overlay: the Rust
# agent + its service, our packages, branding, and archinstall for disk installs.
#
# Run as root inside the WSL Arch build chroot (which has archiso installed):
#   wsl -d Ubuntu -u root -- bash core-agent/archiso/build.sh
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../core-agent
OVERLAY="$REPO_ROOT/archiso"                                       # our overlay tree
AGENT="$REPO_ROOT/target/x86_64-unknown-linux-musl/release/aegis-agent"
OUT_DIR="$(dirname "$REPO_ROOT")/dist"

# Arch build host chroot (mkarchiso must run on Arch; we already have this from
# the old installer and installed `archiso` into it during the viability probe).
BS=/root/aegis/iso-build/iso/arch-bootstrap
PROFILE="$BS/root/aegis-profile"        # profile lives on the chroot's ext4, NOT
WORK="/root/archwork"                    # /mnt/c — mksquashfs/loop need real fs
CHROOT_OUT="/root/archout"

# --- preflight ---------------------------------------------------------------
[ -x "$AGENT" ] || { echo "missing musl agent: $AGENT (build it first)" >&2; exit 1; }
[ -d "$BS/usr/share/archiso/configs/releng" ] || { echo "archiso not installed in $BS" >&2; exit 1; }

echo "==> Fresh Aegis profile from official releng"
rm -rf "$PROFILE"
cp -a "$BS/usr/share/archiso/configs/releng" "$PROFILE"

echo "==> Branding profiledef"
sed -i \
  -e 's/^iso_name=.*/iso_name="aegis"/' \
  -e 's/^iso_label=.*/iso_label="AEGIS"/' \
  -e 's/^iso_publisher=.*/iso_publisher="Project Aegis"/' \
  -e 's/^iso_application=.*/iso_application="Aegis OS Live\/Installer"/' \
  "$PROFILE/profiledef.sh"

echo "==> Enable [multilib] in the profile pacman.conf (lib32 gaming libs)"
if ! grep -qE '^\[multilib\]' "$PROFILE/pacman.conf"; then
  printf '\n[multilib]\nInclude = /etc/pacman.d/mirrorlist\n' >> "$PROFILE/pacman.conf"
fi

echo "==> Append Aegis packages"
cat "$OVERLAY/packages.aegis" >> "$PROFILE/packages.x86_64"

echo "==> Overlay airootfs (services, branding, session, installer launcher)"
cp -a "$OVERLAY/airootfs/." "$PROFILE/airootfs/"

echo "==> Stage the Aegis agent into the live root"
install -Dm755 "$AGENT" "$PROFILE/airootfs/usr/local/bin/aegis-agent"
install -Dm755 "$REPO_ROOT/iso/fake-game" "$PROFILE/airootfs/usr/local/bin/fake-game"

echo "==> Enable the agent service in the live image"
mkdir -p "$PROFILE/airootfs/etc/systemd/system/multi-user.target.wants"
ln -sf /etc/systemd/system/aegis-agent.service \
       "$PROFILE/airootfs/etc/systemd/system/multi-user.target.wants/aegis-agent.service"

# file ownership/permissions inside the image (append to profiledef's array).
# archiso applies these after copying airootfs; scripts must be executable.
python3 - "$PROFILE/profiledef.sh" <<'PY'
import sys, re
p = sys.argv[1]
s = open(p).read()
adds = [
  '  ["/usr/local/bin/aegis-agent"]="0:0:755"',
  '  ["/usr/local/bin/fake-game"]="0:0:755"',
]
# insert our entries just before the closing ) of file_permissions=(...)
s = re.sub(r'(file_permissions=\([^)]*)', lambda m: m.group(1) + "\n" + "\n".join(adds) + "\n", s, count=1)
open(p, "w").write(s)
PY

echo "==> Build ISO with mkarchiso (inside the Arch chroot)"
mount -t proc proc "$BS/proc" 2>/dev/null || true
mount -t sysfs sys "$BS/sys" 2>/dev/null || true
mount -o bind /dev "$BS/dev" 2>/dev/null || true
cp /etc/resolv.conf "$BS/etc/resolv.conf" 2>/dev/null || true

chroot "$BS" bash -c "set -e; rm -rf $WORK $CHROOT_OUT; mkarchiso -v -w $WORK -o $CHROOT_OUT /root/aegis-profile"

umount -l "$BS/dev" "$BS/sys" "$BS/proc" 2>/dev/null || true

echo "==> Copy ISO out"
mkdir -p "$OUT_DIR"
cp "$BS$CHROOT_OUT"/*.iso "$OUT_DIR/aegis-arch.iso"
echo "==> Done: $OUT_DIR/aegis-arch.iso"
ls -la "$OUT_DIR/aegis-arch.iso"
