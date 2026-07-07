#!/usr/bin/env bash
set -e

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ISO="$REPO_ROOT/../dist/aegis-0.1.iso"
DISK="$REPO_ROOT/../dist/test-disk.qcow2"

if [ ! -f "$ISO" ]; then
    echo "Error: ISO not found at $ISO"
    exit 1
fi

if [ ! -f "$DISK" ]; then
    echo "Creating a 20GB test disk..."
    qemu-img create -f qcow2 "$DISK" 20G
fi

echo "Starting QEMU test loop..."
echo "Use Ctrl+A then X to exit QEMU."

# Run QEMU with a serial console so we can interact directly in the WSL terminal.
# 4G minimum: the installer copies the arch-bootstrap toolkit (~800MB) into a
# RAM-backed /tmp, which a 2G guest can't hold alongside the initramfs.
qemu-system-x86_64 \
    -m 4096 \
    -enable-kvm \
    -cpu host \
    -hda "$DISK" \
    -cdrom "$ISO" \
    -boot d \
    -nographic \
    -serial mon:stdio
