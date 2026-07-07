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
qemu-system-x86_64 \
    -m 2048 \
    -enable-kvm \
    -cpu host \
    -hda "$DISK" \
    -cdrom "$ISO" \
    -boot d \
    -nographic \
    -serial mon:stdio
