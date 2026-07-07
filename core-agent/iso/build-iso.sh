#!/usr/bin/env bash
# Assembles the Aegis OS v0.1 bootable ISO. Run as root inside the WSL distro
# (or any Linux with the same tools):
#   wsl -d Ubuntu -u root -- bash /mnt/c/Users/Kevin/projectsclaude/core-agent/iso/build-iso.sh
#
# Inputs (must already exist):
#   - kernel bzImage:   $KERNEL_BZIMAGE (default: /root/aegis/linux-6.12/arch/x86/boot/bzImage)
#   - static agent:     <repo>/target/x86_64-unknown-linux-musl/release/aegis-agent
#   - busybox:          /usr/bin/busybox (from busybox-static)
# Output:
#   - <repo-parent>/dist/aegis-0.1.iso
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KERNEL_BZIMAGE="${KERNEL_BZIMAGE:-/root/aegis/linux-6.12/arch/x86/boot/bzImage}"
AGENT_BIN="$REPO_ROOT/target/x86_64-unknown-linux-musl/release/aegis-agent"
OUT_DIR="$(dirname "$REPO_ROOT")/dist"
# Staging on the native Linux FS, not /mnt/c — cpio/xorriso need real permission
# and symlink semantics that DrvFs doesn't reliably provide.
WORK=/root/aegis/iso-build

for f in "$KERNEL_BZIMAGE" "$AGENT_BIN" /usr/bin/busybox; do
  [ -f "$f" ] || { echo "missing input: $f" >&2; exit 1; }
done

echo "==> Staging initramfs tree"
# Ensure previous bind mounts from old failed runs are cleanly unmounted
# so rm doesn't fail or try to delete the host's virtual file systems!
for mnt in mnt/rootfs dev/pts dev sys proc; do
    if mountpoint -q "$WORK/arch-bootstrap/$mnt" 2>/dev/null; then
        umount -l "$WORK/arch-bootstrap/$mnt"
    fi
done
rm -rf "$WORK"
mkdir -p "$WORK"/initramfs/{bin,sbin,usr/bin,proc,sys,dev,tmp,run,etc}
cp /usr/bin/busybox "$WORK/initramfs/bin/busybox"
# /bin/sh must exist BEFORE init runs — init's own shebang needs it; without
# it the kernel panics with "Failed to execute /init (error -2)" (found the
# hard way in the first QEMU boot). getty is expected at /sbin/getty by init;
# everything else self-installs via `busybox --install` at boot.
ln -sf busybox "$WORK/initramfs/bin/sh"
ln -sf ../bin/busybox "$WORK/initramfs/sbin/getty"
cp "$AGENT_BIN" "$WORK/initramfs/usr/bin/aegis-agent"
cp "$REPO_ROOT/iso/fake-game" "$WORK/initramfs/usr/bin/fake-game"
cp "$REPO_ROOT/iso/init" "$WORK/initramfs/init"
cp "$REPO_ROOT/iso/aegis-install" "$WORK/initramfs/usr/bin/aegis-install"
cp "$REPO_ROOT/iso/udhcpc-script" "$WORK/initramfs/etc/udhcpc.script"

echo "==> Downloading Gum for sleek installer UI..."
if [ ! -f "$WORK/gum" ]; then
    curl -sL "https://github.com/charmbracelet/gum/releases/download/v0.14.3/gum_0.14.3_Linux_x86_64.tar.gz" | tar -xzC "$WORK" --strip-components=1 "gum_0.14.3_Linux_x86_64/gum"
fi
cp "$WORK/gum" "$WORK/initramfs/usr/bin/gum"

chmod 755 "$WORK/initramfs/init" "$WORK/initramfs/usr/bin/fake-game" \
          "$WORK/initramfs/usr/bin/aegis-agent" "$WORK/initramfs/bin/busybox" \
          "$WORK/initramfs/usr/bin/aegis-install" "$WORK/initramfs/usr/bin/gum" \
          "$WORK/initramfs/etc/udhcpc.script"

echo "==> Packing initramfs (cpio newc, gzip)"
( cd "$WORK/initramfs" && find . | cpio -o -H newc --owner root:root 2>/dev/null | gzip -9 ) \
  > "$WORK/initramfs.gz"

echo "==> Staging installer payload (arch-bootstrap) onto ISO..."
mkdir -p "$WORK/iso"
if [ ! -f "$WORK/iso/arch-bootstrap/bin/pacstrap" ]; then
    echo "Downloading Arch Linux bootstrap for the USB installer..."
    mkdir -p "$WORK/iso/arch-bootstrap"
    curl -L "https://mirrors.kernel.org/archlinux/iso/latest/archlinux-bootstrap-x86_64.tar.zst" | tar --zstd -xC "$WORK/iso/arch-bootstrap" --strip-components=1
fi

# Ensure the pacman mirrorlist has at least one active mirror
sed -i 's/^#Server = https:\/\/mirrors.kernel.org/Server = https:\/\/mirrors.kernel.org/' "$WORK/iso/arch-bootstrap/etc/pacman.d/mirrorlist"

echo "==> Laying out ISO tree"
mkdir -p "$WORK"/iso/boot/grub
cp "$KERNEL_BZIMAGE" "$WORK/iso/boot/bzImage"
cp "$WORK/initramfs.gz" "$WORK/iso/boot/initramfs.gz"
cp "$REPO_ROOT/iso/grub.cfg" "$WORK/iso/boot/grub/grub.cfg"

echo "==> Building hybrid BIOS/UEFI ISO with grub-mkrescue"
grub-mkrescue -o "$WORK/aegis-0.1.iso" "$WORK/iso" 2>&1 | tail -2

mkdir -p "$OUT_DIR"
cp "$WORK/aegis-0.1.iso" "$OUT_DIR/aegis-0.1.iso"
echo "==> Done: $OUT_DIR/aegis-0.1.iso"
ls -la "$OUT_DIR/aegis-0.1.iso"
