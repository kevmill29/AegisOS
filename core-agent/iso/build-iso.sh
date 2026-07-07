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

FRONTEND_DIR="$(dirname "$REPO_ROOT")/frontend"
for f in "$KERNEL_BZIMAGE" "$AGENT_BIN" /usr/bin/busybox "$FRONTEND_DIR/dist/index.html"; do
  [ -f "$f" ] || { echo "missing input: $f" >&2; exit 1; }
done

# Fail before any long download, not 10 minutes in. mformat (mtools) matters:
# without it grub-mkrescue SILENTLY emits a BIOS-only image and UEFI boxes
# won't boot the stick.
for tool in cpio gzip curl tar zstd mountpoint chroot grub-mkrescue xorriso mformat; do
  command -v "$tool" >/dev/null 2>&1 \
    || { echo "missing tool: $tool  (apt install cpio gzip curl tar zstd util-linux coreutils grub-common xorriso mtools)" >&2; exit 1; }
done
for d in /usr/lib/grub/i386-pc /usr/lib/grub/x86_64-efi; do
  [ -d "$d" ] || { echo "missing GRUB platform files: $d  (apt install grub-pc-bin grub-efi-amd64-bin) — grub-mkrescue drops that boot mode without complaint" >&2; exit 1; }
done

echo "==> Staging initramfs tree"
# Ensure previous bind mounts from old failed runs are cleanly unmounted
# so rm doesn't fail or try to delete the host's virtual file systems!
for mnt in mnt/rootfs dev/pts dev sys proc; do
    if mountpoint -q "$WORK/arch-bootstrap/$mnt" 2>/dev/null; then
        umount -l "$WORK/arch-bootstrap/$mnt" || true
    fi
    if mountpoint -q "$WORK/iso/arch-bootstrap/$mnt" 2>/dev/null; then
        umount -l "$WORK/iso/arch-bootstrap/$mnt" || true
    fi
done
# Only the generated pieces are rebuilt; $WORK/gum and $WORK/iso/arch-bootstrap
# are caches the download guards below check for — a blanket rm -rf here was
# defeating them and re-fetching ~200MB every run.
rm -rf "$WORK/initramfs" "$WORK/initramfs.gz" "$WORK/iso/boot" "$WORK/iso/usr"
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
# Extract to a temp dir and mv into place so an interrupted download can't
# leave a truncated binary that the cache check then trusts forever.
if [ ! -x "$WORK/gum" ]; then
    rm -rf "$WORK/gum.tmp"
    mkdir -p "$WORK/gum.tmp"
    curl -sL "https://github.com/charmbracelet/gum/releases/download/v0.14.3/gum_0.14.3_Linux_x86_64.tar.gz" | tar -xzC "$WORK/gum.tmp" --strip-components=1 "gum_0.14.3_Linux_x86_64/gum"
    mv "$WORK/gum.tmp/gum" "$WORK/gum"
    rm -rf "$WORK/gum.tmp"
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
    # Same atomic dance as gum: a half-extracted bootstrap that happens to
    # contain bin/pacstrap would pass the cache check and ship a broken
    # installer payload.
    rm -rf "$WORK/iso/arch-bootstrap" "$WORK/iso/arch-bootstrap.partial"
    mkdir -p "$WORK/iso/arch-bootstrap.partial"
    curl -L "https://mirrors.kernel.org/archlinux/iso/latest/archlinux-bootstrap-x86_64.tar.zst" | tar --zstd -xC "$WORK/iso/arch-bootstrap.partial" --strip-components=1
    mv "$WORK/iso/arch-bootstrap.partial" "$WORK/iso/arch-bootstrap"
fi

# Several mirrors, not one: with only mirrors.kernel.org enabled, a single
# throttled mirror ("Operation too slow") killed pacstrap mid-install with no
# fallback. geo.mirror.pkgbuild.com geo-routes to something nearby.
cat > "$WORK/iso/arch-bootstrap/etc/pacman.d/mirrorlist" <<'MIRRORS'
Server = https://geo.mirror.pkgbuild.com/$repo/os/$arch
Server = https://mirror.rackspace.com/archlinux/$repo/os/$arch
Server = https://mirrors.kernel.org/archlinux/$repo/os/$arch
Server = https://mirror.leaseweb.net/archlinux/$repo/os/$arch
MIRRORS
# Don't give up on a slow-but-alive download (pacman's low-speed timeout is
# what produced "Less than 1 bytes/sec"), and fetch in parallel.
sed -i 's/^#ParallelDownloads.*/ParallelDownloads = 5/' "$WORK/iso/arch-bootstrap/etc/pacman.conf"
grep -q '^DisableDownloadTimeout' "$WORK/iso/arch-bootstrap/etc/pacman.conf" \
  || sed -i '/^\[options\]/a DisableDownloadTimeout' "$WORK/iso/arch-bootstrap/etc/pacman.conf"

if [ ! -f "$WORK/iso/arch-bootstrap/usr/bin/mkfs.fat" ]; then
    echo "==> Injecting missing dependencies into arch-bootstrap (dosfstools, e2fsprogs, util-linux)..."
    mount -t proc proc "$WORK/iso/arch-bootstrap/proc"
    mount -t sysfs sys "$WORK/iso/arch-bootstrap/sys"
    mount -o bind /dev "$WORK/iso/arch-bootstrap/dev"
    cp /etc/resolv.conf "$WORK/iso/arch-bootstrap/etc/resolv.conf"
    sed -i 's/^CheckSpace/#CheckSpace/' "$WORK/iso/arch-bootstrap/etc/pacman.conf"
    chroot "$WORK/iso/arch-bootstrap" pacman-key --init
    chroot "$WORK/iso/arch-bootstrap" pacman-key --populate archlinux
    chroot "$WORK/iso/arch-bootstrap" pacman -Sy --noconfirm dosfstools e2fsprogs util-linux
    umount -l "$WORK/iso/arch-bootstrap/dev"
    umount -l "$WORK/iso/arch-bootstrap/sys"
    umount -l "$WORK/iso/arch-bootstrap/proc"
fi

echo "==> Laying out ISO tree"
mkdir -p "$WORK"/iso/boot/grub
cp "$KERNEL_BZIMAGE" "$WORK/iso/boot/bzImage"
cp "$WORK/initramfs.gz" "$WORK/iso/boot/initramfs.gz"
cp "$REPO_ROOT/iso/grub.cfg" "$WORK/iso/boot/grub/grub.cfg"
# aegis-install copies these from the mounted boot media onto the installed
# root (/run/bootmedia/usr/bin/aegis-agent). They only exist inside the
# initramfs unless staged here too — without this the installed system boots
# with no agent at all.
mkdir -p "$WORK/iso/usr/bin"
cp "$AGENT_BIN" "$WORK/iso/usr/bin/aegis-agent"
cp "$REPO_ROOT/iso/fake-game" "$WORK/iso/usr/bin/fake-game"
cp "$REPO_ROOT/iso/aegis-init" "$WORK/iso/usr/bin/aegis-init"
cp "$REPO_ROOT/iso/aegis-session" "$WORK/iso/usr/bin/aegis-session"
chmod 755 "$WORK/iso/usr/bin/aegis-agent" "$WORK/iso/usr/bin/fake-game" \
          "$WORK/iso/usr/bin/aegis-init" "$WORK/iso/usr/bin/aegis-session"
# Frontend app for the installed kiosk: Electron loads the package dir
# (package.json main -> electron/main.cjs -> dist/index.html).
mkdir -p "$WORK/iso/opt/aegis/frontend"
cp "$FRONTEND_DIR/package.json" "$WORK/iso/opt/aegis/frontend/"
cp -r "$FRONTEND_DIR/electron" "$FRONTEND_DIR/dist" "$WORK/iso/opt/aegis/frontend/"

echo "==> Building hybrid BIOS/UEFI ISO with grub-mkrescue"
grub-mkrescue -o "$WORK/aegis-0.1.iso" "$WORK/iso" 2>&1 | tail -2

mkdir -p "$OUT_DIR"
cp "$WORK/aegis-0.1.iso" "$OUT_DIR/aegis-0.1.iso"
echo "==> Done: $OUT_DIR/aegis-0.1.iso"
ls -la "$OUT_DIR/aegis-0.1.iso"
