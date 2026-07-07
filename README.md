# Aegis OS

Aegis OS is a hyper-minimal, custom-built Linux operating system designed from the ground up for dedicated gaming (Steam/Proton) and maximum performance. 

Instead of relying on bloated desktop environments, Aegis OS runs a custom Rust-based system agent as PID 1 (or immediately after init), managing game processes, system resources via cgroups, and booting directly into a custom Wayland compositor and a sleek React/Electron frontend.

## 📂 Repository Structure

The project is structured as a monorepo containing three core components:

* **`/core-agent`**: The brains of the OS. A Rust daemon that monitors system resources, integrates with Steam/Proton, and manages cgroups. This folder also contains the scripts to build the bootable ISO and the interactive `gum`-based NetInstaller.
* **`/compositor`**: A custom, lightweight Wayland kiosk compositor designed solely to display the Aegis frontend without the overhead of a traditional desktop environment.
* **`/frontend`**: The user interface. A sleek, modern Electron/Vite app built with React that features the "Aegis Sphere" UI.

---

## 🛠️ Prerequisites for Building

To compile the code and build the ISO from source, you will need a Linux environment (WSL2 Ubuntu works perfectly). 

**Required Packages (Ubuntu/Debian):**
```bash
sudo apt update
sudo apt install build-essential curl git xorriso cpio zstd \
                 grub-pc-bin grub-efi-amd64-bin musl-tools
```

**Required Toolchains:**
1. **Rust**: Install via [rustup](https://rustup.rs/).
2. **Node.js**: Install via [nvm](https://github.com/nvm-sh/nvm) or your package manager.
3. **Custom Linux Kernel**: You must have a compiled Linux Kernel `bzImage`. By default, the build script looks for it at `/root/aegis/linux-6.12/arch/x86/boot/bzImage`, but you can override this by setting the `KERNEL_BZIMAGE` environment variable.

---

## 🏗️ Building from Source

### 1. Build the Frontend
```bash
cd frontend
npm install
npm run build
cd ..
```

### 2. Build the Compositor
```bash
cd compositor
cargo build --release
cd ..
```

### 3. Build the Core Agent (Statically Linked)
Because the core agent runs in an extremely barebones initramfs during installation, it must be statically linked against `musl`.
```bash
cd core-agent
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
cd ..
```

### 4. Assemble the Bootable ISO
Once the agent is compiled, you can assemble the hybrid UEFI/BIOS NetInstaller ISO. 
*(Note: You must run this as `root` or with `sudo` because `cpio` and `mount` require elevated privileges to package the rootfs).*

```bash
sudo bash core-agent/iso/build-iso.sh
```

If successful, the final ISO will be output to `dist/aegis-0.1.iso`.

---

## 🚀 Installation (Baremetal or VM)

1. **Flash to USB**: Use a tool like [Balena Etcher](https://etcher.balena.io/), [Rufus](https://rufus.ie/), or [Ventoy](https://www.ventoy.net/) to write `dist/aegis-0.1.iso` to a USB flash drive.
2. **Boot the Target Machine**: Insert the USB into the target computer or Virtual Machine and boot from it. Ensure UEFI mode is enabled.
3. **The Aegis NetInstaller**: You will be greeted by the interactive, sleek `gum` frontend. 
4. **Follow the Prompts**: 
   - Choose to Erase an Entire Disk, or Dual-Boot onto a specific partition.
   - The installer will automatically format the drive, connect to the internet, and download the latest gaming dependencies directly onto your hardware.
5. **Reboot**: Remove the USB stick and boot directly into your brand new Aegis OS installation!
