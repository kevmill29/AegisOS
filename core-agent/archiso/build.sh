#!/usr/bin/env bash
# Build the Aegis OS ISO on top of Arch's official `releng` archiso profile.
#
# Why this replaces core-agent/iso/build-iso.sh: the old path hand-rolled a
# custom kernel, a busybox initramfs, and a bespoke gum/pacstrap installer —
# every install issue we hit (the UEFI blind console, boot-medium hangs) came
# from that homemade plumbing. archiso gives us the *stock* Arch kernel (all
# console/GPU/USB/net drivers built in — boots reliably on UEFI and BIOS) and a
# proper systemd live environment.
#
# Aegis rides on top as a single pacman package (`aegis`, built here from
# ./package into a local repo baked onto the ISO). The live env installs it and
# boots straight into the sphere kiosk; `aegis-installer` installs the very same
# package onto a disk after archinstall lays down the base system. One overlay,
# two consumers.
#
# Run as root inside the WSL Arch build chroot (which has archiso installed):
#   wsl -d Ubuntu -u root -- bash core-agent/archiso/build.sh
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"      # .../core-agent
OVERLAY="$REPO_ROOT/archiso"                                       # our overlay tree
AGENT="$REPO_ROOT/target/x86_64-unknown-linux-musl/release/aegis-agent"
FRONTEND="$(dirname "$REPO_ROOT")/frontend"
OUT_DIR="$(dirname "$REPO_ROOT")/dist"

# Arch build host chroot (mkarchiso + makepkg must run on Arch).
BS=/root/aegis/iso-build/iso/arch-bootstrap
PROFILE="$BS/root/aegis-profile"        # profile lives on the chroot's ext4, NOT
WORK="/root/archwork"                    # /mnt/c — mksquashfs/loop need a real fs
CHROOT_OUT="/root/archout"
PKGSTAGE="$BS/home/builder/aegis-pkg"    # makepkg workspace (must be a real user)
BUILD_REPO="$BS/opt/aegis/repo"          # local repo, file:///opt/aegis/repo in chroot

# --- preflight ---------------------------------------------------------------
[ -x "$AGENT" ] || { echo "missing musl agent: $AGENT (build it first)" >&2; exit 1; }
[ -f "$FRONTEND/dist/index.html" ] || { echo "missing frontend build: $FRONTEND/dist (npm run build)" >&2; exit 1; }
[ -d "$BS/usr/share/archiso/configs/releng" ] || { echo "archiso not installed in $BS" >&2; exit 1; }

# chroot mounts up-front: needed by pacman (base-devel), makepkg, and mkarchiso.
mount -t proc proc "$BS/proc" 2>/dev/null || true
mount -t sysfs sys "$BS/sys" 2>/dev/null || true
mount -o bind /dev "$BS/dev" 2>/dev/null || true
cp /etc/resolv.conf "$BS/etc/resolv.conf" 2>/dev/null || true
cleanup(){ umount -l "$BS/dev" "$BS/sys" "$BS/proc" 2>/dev/null || true; }
trap cleanup EXIT

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

echo "==> Enable [multilib] + [aegis] repo in the profile pacman.conf"
if ! grep -qE '^\[multilib\]' "$PROFILE/pacman.conf"; then
  printf '\n[multilib]\nInclude = /etc/pacman.d/mirrorlist\n' >> "$PROFILE/pacman.conf"
fi
if ! grep -qE '^\[aegis\]' "$PROFILE/pacman.conf"; then
  printf '\n[aegis]\nSigLevel = Optional TrustAll\nServer = file:///opt/aegis/repo\n' >> "$PROFILE/pacman.conf"
fi

# --- build the `aegis` package ----------------------------------------------
echo "==> Preparing the Arch build user + base-devel"
chroot "$BS" bash -c '
  set -e
  id builder >/dev/null 2>&1 || useradd -m builder
  pacman -Sy --needed --noconfirm base-devel >/dev/null
'

echo "==> Staging the aegis package tree"
rm -rf "$PKGSTAGE"; mkdir -p "$PKGSTAGE/root"
cp "$OVERLAY/package/PKGBUILD" "$OVERLAY/package/aegis.install" "$PKGSTAGE/"
install -Dm755 "$AGENT"                              "$PKGSTAGE/root/usr/bin/aegis-agent"
install -Dm755 "$OVERLAY/package/aegis-session"      "$PKGSTAGE/root/usr/bin/aegis-session"
install -Dm755 "$REPO_ROOT/iso/fake-game"            "$PKGSTAGE/root/usr/bin/fake-game"
install -Dm644 "$OVERLAY/package/aegis-agent.service" "$PKGSTAGE/root/usr/lib/systemd/system/aegis-agent.service"
install -Dm644 "$OVERLAY/package/greetd-config.toml"  "$PKGSTAGE/root/usr/share/aegis/greetd-config.toml"
install -Dm644 "$OVERLAY/package/sysusers-aegis.conf" "$PKGSTAGE/root/usr/lib/sysusers.d/aegis.conf"
install -Dm644 "$OVERLAY/package/tmpfiles-aegis.conf" "$PKGSTAGE/root/usr/lib/tmpfiles.d/aegis.conf"
mkdir -p "$PKGSTAGE/root/opt/aegis/frontend"
cp "$FRONTEND/package.json" "$PKGSTAGE/root/opt/aegis/frontend/"
cp -r "$FRONTEND/electron" "$FRONTEND/dist" "$PKGSTAGE/root/opt/aegis/frontend/"
chroot "$BS" chown -R builder:builder /home/builder/aegis-pkg

echo "==> Building the package with makepkg"
chroot "$BS" bash -c '
  set -e
  cd /home/builder/aegis-pkg
  runuser -u builder -- makepkg -f --nodeps --skipinteg
'

echo "==> Publishing to the local repo"
rm -rf "$BUILD_REPO"; mkdir -p "$BUILD_REPO"
cp "$PKGSTAGE"/aegis-*.pkg.tar.zst "$BUILD_REPO/"
chroot "$BS" bash -c 'cd /opt/aegis/repo && repo-add aegis.db.tar.gz aegis-*.pkg.tar.zst >/dev/null'

# --- assemble the profile ----------------------------------------------------
echo "==> Package lists (Aegis userland + the aegis package itself)"
cat "$OVERLAY/packages.aegis" >> "$PROFILE/packages.x86_64"
printf '\n# Aegis overlay package (local repo)\naegis\n' >> "$PROFILE/packages.x86_64"

echo "==> Overlay airootfs (motd, installer, install-mode units) + ship repo"
cp -a "$OVERLAY/airootfs/." "$PROFILE/airootfs/"
mkdir -p "$PROFILE/airootfs/opt/aegis"
cp -a "$BUILD_REPO" "$PROFILE/airootfs/opt/aegis/repo"

echo "==> Enable the install-mode service (gated on the aegis.install cmdline)"
mkdir -p "$PROFILE/airootfs/etc/systemd/system/multi-user.target.wants"
ln -sf /etc/systemd/system/aegis-install.service \
       "$PROFILE/airootfs/etc/systemd/system/multi-user.target.wants/aegis-install.service"

echo "==> Boot menu: add a default 'Install Aegis OS' entry + relabel live"
# UEFI (systemd-boot): a new install entry (sort-key 00 -> first/default) that
# adds aegis.install, and relabel the stock live entry.
ARCHISO_OPTS='archisobasedir=%INSTALL_DIR% archisosearchuuid=%ARCHISO_UUID%'
cat > "$PROFILE/efiboot/loader/entries/00-aegis-install.conf" <<EOF
title    Install Aegis OS
sort-key 00
linux    /%INSTALL_DIR%/boot/%ARCH%/vmlinuz-linux
initrd   /%INSTALL_DIR%/boot/%ARCH%/initramfs-linux.img
options  $ARCHISO_OPTS aegis.install
EOF
sed -i 's/^title .*/title    Aegis OS (live \/ try the sphere)/' \
    "$PROFILE/efiboot/loader/entries/01-archiso-linux.conf"
sed -i 's/^default .*/default 00-aegis-install.conf/' "$PROFILE/efiboot/loader/loader.conf"

# BIOS (syslinux): prepend an install label, make it the default, relabel live.
SL="$PROFILE/syslinux/archiso_sys-linux.cfg"
python3 - "$SL" "$ARCHISO_OPTS" <<'PY'
import sys
p, opts = sys.argv[1], sys.argv[2]
s = open(p).read()
install = (
    "LABEL aegisinstall\n"
    "MENU LABEL Install Aegis OS\n"
    "LINUX /%INSTALL_DIR%/boot/%ARCH%/vmlinuz-linux\n"
    "INITRD /%INSTALL_DIR%/boot/%ARCH%/initramfs-linux.img\n"
    "APPEND " + opts + " aegis.install\n\n"
)
s = s.replace("MENU LABEL Arch Linux install medium (%ARCH%, BIOS)",
              "MENU LABEL Aegis OS (live / try the sphere)")
open(p, "w").write(install + s)
PY
sed -i -e 's/^DEFAULT .*/DEFAULT aegisinstall/' -e 's/^TIMEOUT .*/TIMEOUT 100/' \
    "$PROFILE/syslinux/archiso_sys.cfg"

echo "==> File permissions for the installer launcher"
python3 - "$PROFILE/profiledef.sh" <<'PY'
import sys, re
p = sys.argv[1]
s = open(p).read()
add = '  ["/usr/local/bin/aegis-installer"]="0:0:755"\n'
s = re.sub(r'(file_permissions=\()', r'\1\n' + add, s, count=1)
open(p, "w").write(s)
PY

# --- build the ISO -----------------------------------------------------------
echo "==> Build ISO with mkarchiso"
chroot "$BS" bash -c "set -e; rm -rf $WORK $CHROOT_OUT; mkarchiso -v -w $WORK -o $CHROOT_OUT /root/aegis-profile"

echo "==> Copy ISO out + checksum"
mkdir -p "$OUT_DIR"
cp "$BS$CHROOT_OUT"/*.iso "$OUT_DIR/aegis-arch.iso"
# Emit a checksum next to the ISO so a flash can be verified (a SQUASHFS read
# error at boot means a bad USB write, not a bad build — this lets you tell).
( cd "$OUT_DIR" && sha256sum aegis-arch.iso > aegis-arch.iso.sha256 )
echo "==> Done: $OUT_DIR/aegis-arch.iso"
ls -la "$OUT_DIR/aegis-arch.iso"
cat "$OUT_DIR/aegis-arch.iso.sha256"
