#!/usr/bin/env bash
# Automated boot test: does this ISO actually come up as a sphere kiosk on a
# PC? Boots the ISO in QEMU (KVM if available) with a virtio GPU, watches the
# full journal on the serial console, then screendumps the virtual display and
# checks that the sphere really rendered. This is the pre-ship gate: static
# "the files are in the image" checks kept passing while real boots failed in
# greetd/cage territory — only a real boot catches that class.
#
# Run as root in WSL Ubuntu (or any Linux with qemu-system-x86 + loop mounts):
#   bash core-agent/archiso/test-boot.sh [path/to/aegis-arch.iso]
#
# PASS criteria:
#   A1  boot reaches graphical.target
#   A2  the Aegis core agent starts
#   A3  greetd's initial cage session survives (no fallback greeter session)
#   A4  the virtual screen shows the sphere (non-black + cyan pixels)
set -uo pipefail

ISO_SRC="${1:-/mnt/c/Users/Kevin/projectsclaude/dist/aegis-arch.iso}"
# GPU model matters: virtio-vga is the well-supported default; vmware-svga
# emulates the VMware SVGA II adapter (the same device class VirtualBox's
# VMSVGA presents, driven by vmwgfx) to chase VirtualBox-specific failures.
VGA="${AEGIS_TEST_VGA:-virtio-vga}"
WORK=/root/aegis-boot-test
BOOT_TIMEOUT=420     # seconds to reach graphical.target (TCG is slow)
KIOSK_WAIT=120       # extra seconds for cage+electron to paint
SERIAL="$WORK/serial.log"
QMP="$WORK/qmp.sock"
SCREEN_PPM="$WORK/screen.ppm"
SCREEN_PNG="$WORK/screen.png"

say() { printf '\n== %s\n' "$*"; }
pass=() ; fail=()
check() { # check <name> <0|1> <detail>
  if [ "$2" = 0 ]; then pass+=("$1"); echo "PASS  $1"; else fail+=("$1: $3"); echo "FAIL  $1 — $3"; fi
}

say "Preparing workspace ($WORK)"
mkdir -p "$WORK"
rm -f "$SERIAL" "$SCREEN_PPM" "$SCREEN_PNG" "$QMP"
# ISO on the native fs: QEMU seeking into a drvfs-mounted ISO is painfully slow.
ISO="$WORK/test.iso"
cmp -s "$ISO_SRC" "$ISO" 2>/dev/null || cp "$ISO_SRC" "$ISO"

say "Extracting kernel + initramfs from the ISO"
MNT=$(mktemp -d)
mount -o loop,ro "$ISO" "$MNT"
cp "$MNT/arch/boot/x86_64/vmlinuz-linux" "$WORK/vmlinuz"
cp "$MNT/arch/boot/x86_64/initramfs-linux.img" "$WORK/initrd"
umount "$MNT" && rmdir "$MNT"

KVM_ARGS=()
if [ -w /dev/kvm ]; then
  KVM_ARGS=(-enable-kvm -cpu host)
  say "KVM available — hardware speed"
else
  say "No KVM — falling back to TCG (slow; timeouts are sized for it)"
fi

say "Booting the ISO (GPU: $VGA, journal forwarded to serial)"
# Direct-kernel boot lets us control the cmdline: archisodevice pins the boot
# medium (no label guessing), journald.forward_to_console streams every unit's
# output — including cage/electron stderr — onto the serial log we parse.
qemu-system-x86_64 \
  "${KVM_ARGS[@]}" -smp 4 -m 4096 \
  -kernel "$WORK/vmlinuz" -initrd "$WORK/initrd" \
  -append "archisobasedir=arch archisodevice=/dev/sr0 console=ttyS0 systemd.journald.forward_to_console=1" \
  -drive "file=$ISO,media=cdrom,readonly=on" \
  -device "$VGA" -display none \
  -serial "file:$SERIAL" \
  -qmp "unix:$QMP,server,nowait" \
  -no-reboot &
QEMU_PID=$!
trap 'kill $QEMU_PID 2>/dev/null; wait $QEMU_PID 2>/dev/null' EXIT

wait_for() { # wait_for <regex> <timeout-s> -> 0 found / 1 timeout or qemu died
  local deadline=$((SECONDS + $2))
  while (( SECONDS < deadline )); do
    grep -qE "$1" "$SERIAL" 2>/dev/null && return 0
    kill -0 $QEMU_PID 2>/dev/null || return 1
    sleep 3
  done
  return 1
}

say "Waiting for graphical.target (up to ${BOOT_TIMEOUT}s)"
wait_for 'Reached target.*Graphical Interface' "$BOOT_TIMEOUT"
check "A1 graphical.target reached" $? "never reached graphical target — see $SERIAL"

grep -qE 'Started.*Aegis Core Agent' "$SERIAL"
check "A2 aegis-agent started" $? "agent service never started"

say "Giving the kiosk ${KIOSK_WAIT}s to come up, then inspecting"
sleep "$KIOSK_WAIT"

# A3: if the cage session dies, greetd falls back to its text greeter — the
# presence of a greeter-user session IS the failure signal.
if grep -qE 'Session .+ of [Uu]ser greeter' "$SERIAL"; then
  check "A3 cage session survived" 1 "greetd fell back to the text greeter (cage session died)"
else
  grep -qE 'session opened for user aegis|Session .+ of [Uu]ser aegis|New session .+ of user aegis' "$SERIAL"
  check "A3 cage session survived" $? "no aegis session found in the journal at all"
fi

say "Screendump via QMP"
ANALYSIS=$(python3 - "$QMP" "$SCREEN_PPM" "$SCREEN_PNG" <<'PY'
import json, socket, sys, time, zlib, struct, os

qmp_path, ppm_path, png_path = sys.argv[1], sys.argv[2], sys.argv[3]
try:
    s = socket.socket(socket.AF_UNIX); s.settimeout(20); s.connect(qmp_path)
    f = s.makefile('rw')
    f.readline()                                   # greeting
    f.write(json.dumps({"execute": "qmp_capabilities"}) + "\n"); f.flush(); f.readline()
    f.write(json.dumps({"execute": "screendump",
                        "arguments": {"filename": ppm_path}}) + "\n"); f.flush()
    f.readline()
    time.sleep(2)
except Exception as e:
    print(f"SCREENDUMP-ERROR {e}"); sys.exit(0)

if not os.path.exists(ppm_path):
    print("SCREENDUMP-ERROR no file produced"); sys.exit(0)

with open(ppm_path, 'rb') as fh:
    assert fh.readline().strip() == b'P6'
    line = fh.readline()
    while line.startswith(b'#'): line = fh.readline()
    w, h = map(int, line.split())
    fh.readline()                                  # maxval
    data = fh.read(w * h * 3)

total = w * h
lit = cyan = 0
for i in range(0, len(data), 3):
    r, g, b = data[i], data[i+1], data[i+2]
    if r + g + b > 60:
        lit += 1
        if b > 80 and g > 60 and b > r + 30:
            cyan += 1
print(f"SCREEN {w}x{h} lit={lit/total:.4f} cyan={cyan/total:.5f}")

# Also emit a PNG so a human (or Claude) can look at what the screen shows.
def chunk(tag, payload):
    c = struct.pack('>I', len(payload)) + tag + payload
    return c + struct.pack('>I', zlib.crc32(tag + payload) & 0xffffffff)
raw = b''.join(b'\x00' + data[y*w*3:(y+1)*w*3] for y in range(h))
png = (b'\x89PNG\r\n\x1a\n'
       + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0))
       + chunk(b'IDAT', zlib.compress(raw, 6))
       + chunk(b'IEND', b''))
open(png_path, 'wb').write(png)
print(f"PNG {png_path}")

ok = lit / total > 0.002 and cyan / total > 0.0002
print("SCREEN-OK" if ok else "SCREEN-DARK")
PY
)
echo "$ANALYSIS"
echo "$ANALYSIS" | grep -q 'SCREEN-OK'
check "A4 sphere rendered on the virtual display" $? "screen is dark/blank — cage or the sphere didn't paint"

say "Journal excerpts (greetd / cage / session / electron)"
grep -iE 'greetd|cage|aegis-session|electron|drm|session' "$SERIAL" | tail -40 || true

say "Verdict"
if [ "${#fail[@]}" -eq 0 ]; then
  echo "BOOT-TEST PASS (${#pass[@]} checks) — screendump: $SCREEN_PNG"
else
  echo "BOOT-TEST FAIL:"
  printf '  - %s\n' "${fail[@]}"
  echo "serial log: $SERIAL"
  exit 1
fi
