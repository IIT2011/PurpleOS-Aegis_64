# Purple OS / Aegis64

> [!WARNING]
> **Active Development Warning:**
> Purple OS and the Aegis64 kernel are under heavy active development. System architectures, driver ABIs, memory layouts, and filesystem formats are subject to frequent changes. Not intended for production use.

---

**Purple OS** is an independent, native-first 64-bit operating system engineered from scratch in **pure bare-metal Rust** (`#![no_std]`, `#![no_main]`, `target = "x86_64-unknown-none"`, with zero external crate dependencies and zero legacy C/C++ runtime code).

Its core kernel is **Aegis64**, its multi-core scheduler is **AegisSched_64**, its mounted native filesystem is **PurpleFS v2**, its modular driver package architecture is **APDRV v3**, its hardware discovery system is **PurpleDetect**, and its cryptographic boot verification system is **AegisDetect**.

---

## 1. Architectural Overview

```
+--------------------------------------------------------------------------+
|                       AegisShell 2.0 (User Console)                      |
+--------------------------------------------------------------------------+
|          PurpleFS v2 (Filesystem)  |  APAPP Subsystem (Applications)     |
+--------------------------------------------------------------------------+
|   APDRV v3 Dynamic Kernel Modules (W^X Enforced, Hardware PCI Matching)   |
|  [e1000] [r8139] [xhci] [hdaudio] [bochs_gpu] [gpu-intel/amd/nvidia]     |
+--------------------------------------------------------------------------+
|         3D Acceleration (Intel RCS | AMD PM4 | NVIDIA Nouveau FIFO)      |
+--------------------------------------------------------------------------+
|   AegisSched_64 (Preemptive Multi-Core Scheduler, 5 Priority Tiers)       |
+--------------------------------------------------------------------------+
|   SMP Engine (4/4 Cores Online, ACPI MADT Enumeration, APIC IPIs)        |
+--------------------------------------------------------------------------+
|   VMM (4-Level Paging, CR0.WP, W^X)  |  PMM (Dual-Bitmap Frame Allocator)|
+--------------------------------------------------------------------------+
|                  x86-64 Long Mode Bare-Metal Hardware                     |
+--------------------------------------------------------------------------+
```

### Key Subsystems

- **Pure Bare-Metal Rust:** Built entirely in Rust 2021 Edition for `x86_64-unknown-none` with custom memory allocators, interrupt service routines, and hardware drivers.
- **Physical Memory Management (PMM):** Dual 128 KiB bitmap allocator managing 4 GiB of physical address space in 4 KiB frames with Multiboot memory map parsing and reserved hardware regions.
- **Virtual Memory Management (VMM):** 4-level paging (`PML4`, `PDPT`, `PD`, `PT`) supporting 2 MiB huge page splitting, canonical address validation, and strict $W \oplus X$ (Write XOR Execute) memory protection with supervisor `CR0.WP` enforcement.
- **Symmetric Multiprocessing (SMP):** ACPI MADT topology discovery, real-mode AP trampoline bootstrap (`smp_trampoline.s`), INIT/SIPI inter-processor interrupts, and per-CPU runqueues across 4 concurrent cores.
- **Preemptive Multi-Core Scheduler (AegisSched_64):** 5 priority tiers (`IDLE`, `LOW`, `NORMAL`, `HIGH`, `REALTIME`), per-CPU task queues, time-slice quantums driven by 100 Hz Local APIC timer ticks, and hardware context switching.
- **Integrity Checks (AegisDetect):** RSA-2048 signature verification for supported signed payloads and SHA-256 checks for the configured storage manifest. Manifest coverage is limited to the entries defined in `src/security/aegis_detect.rs`.
- **Native Filesystem (PurpleFS v2):** GPT-backed volumes with 4096-byte blocks, directory entries, direct-block writes, and reads through singly indirect blocks. PurpleFS v3 is a design in progress and has no mount support.
- **Fast Hardware Syscalls:** MSR-based `SYSCALL`/`SYSRET` handler (`EFER.SCE`, `STAR`, `LSTAR`, `SFMASK`) with user pointer validation.

---

## 2. Authentic Hardware Drivers & 3D Acceleration

The kernel features authentic, bare-metal hardware drivers derived directly from Linux Kernel driver sources:

### Network Controllers
- **Intel 82540EM Gigabit Ethernet (`src/drivers/e1000.rs`):** Full MMIO register map (`CTRL`, `STATUS`, `EERD`, `RCTL`, `TCTL`, `TIPG`), EEPROM MAC address reading via `EERD`, and coherent DMA ring buffers (32 RX / 32 TX descriptors).
- **Realtek RTL8139 Fast Ethernet (`src/drivers/rtl8139.rs`):** Port I/O register architecture (`CR`, `CAPR`, `CBA`, `TSD`), 8 KiB continuous RX DMA ring buffer, 4 TX descriptor ring, and CAPR erratum compensation.

### Audio & USB Subsystems
- **Intel High Definition Audio (`src/drivers/audio.rs`):** Full controller state machine (`CRST` reset sequencing), 256-entry CORB and RIRB DMA rings, codec verb dispatch engine (`send_verb`), and 44.1 kHz 16-bit stereo PCM stream descriptors.
- **xHCI USB 3.0 Host Controller (`src/drivers/usb.rs`):** Capability/Operational MMIO register maps, `HCRST` reset, Device Context Base Address Array (DCBAA), 64-entry TRB command ring with cycle toggle, and doorbell register signaling.

### Display & Modesetting
- **Bochs / QEMU DRM Video (`src/drivers/gpu.rs`):** VBE Dispi 1024x768 @ 32bpp modesetting via ports `0x1CE`/`0x1CF`, PCI BAR0 16 MiB VRAM binding (`0xFD000000`), hardware double-buffering page-flipping (`VBE_DISPI_INDEX_Y_OFFSET`), 2D solid rect fill, and linear framebuffer fallback.

### 3D Hardware Acceleration Engine (`src/drivers/gpu_accel.rs`)
- **Intel Gen9/Gen11 Graphics (`i915` DRM Reference):** Render Command Streamer (RCS) ring buffer, `INTEL_CMD_3D_PIPELINE_SELECT`, `INTEL_CMD_3D_PRIMITIVE` draw calls, `INTEL_MI_FLUSH_DW` cache invalidation, and tail updates via `INTEL_RCS_RING_TAIL` (`0x02030`).
- **AMD Radeon GCN/RDNA (`amdgpu` DRM Reference):** Command Processor (CP) PM4 packet processor, Type-3 packet encoding (`PACKET3`), `AMD_PACKET3_INDEX_BUFFER_SIZE`, `AMD_PACKET3_DRAW_INDEX_AUTO`, and CP Ring 0 write pointer dispatch via `AMD_CP_RB0_WPTR` (`0xC0C8`).
- **NVIDIA GeForce via Nouveau (`nouveau` DRM Reference):** Pushbuffer FIFO channel, NV9097/NVC0 3D class method dispatch (`nv_method`), viewport bounding (`NV9097_SET_VIEWPORT_HORIZ/VERT`), `NV9097_DRAW_ELEMENTS`, and FIFO kicks via user aperture `NOUVEAU_USER_PUT` (`0x0040`).

---

## 3. APDRV v3 Native Module Subsystem

The native driver package format (**APDRV v3**) packages kernel extensions into packed binary modules with cryptographic verification and $W \oplus X$ memory protection:

- **Specification:** [docs/APDRV_FORMAT_SPECIFICATION.md](docs/APDRV_FORMAT_SPECIFICATION.md)
- **Header:** 240-byte packed header (`APDRV100` magic, module metadata, section offsets, hardware PCI IDs).
- **Execution:** Modules contain genuine x86-64 machine code (`entry_init`, `entry_fini`, `entry_probe`, `entry_ioctl`) with relocations resolved against the kernel symbol table (`KSYM`).
- **Standard Modules:**
  - `e1000.apdrv`, `r8139.apdrv`, `xhci.apdrv`, `hdaudio.apdrv`, `bochs_gpu.apdrv`
  - `gpu-intel.apdrv`, `gpu-amd.apdrv`, `gpu-nvidia.apdrv`
  - `Storage.apdrv`, `Network.apdrv`, `WiFi.apdrv`, `USB.apdrv`, `Audio.apdrv`, `Graphics.apdrv`, `Input.apdrv`, `Bluetooth.apdrv`, `Camera.apdrv`

---

## 4. Building and Running

### Prerequisites
- **Rust Toolchain:** Stable or Nightly with `x86_64-unknown-none` target:
  ```bash
  rustup target add x86_64-unknown-none
  cargo install cargo-binutils
  rustup component add llvm-tools-preview
  ```
- **QEMU Emulator:** `qemu-system-x86_64`
- **Python 3.10+** (for APDRV module packaging scripts)

### Build Kernel
```bash
# Compile pure Rust kernel binary
cargo build --target x86_64-unknown-none

# Generate Multiboot-compliant 32-bit ELF bootstrap image
rust-objcopy -O elf32-i386 target/x86_64-unknown-none/debug/aegis64 target/kernel.elf

# Build loadable APDRV modules with the trusted signing key
python scripts/build_apdrv.py build/drivers_apdrv --signing-key /path/to/trusted-private-key.pem
```

### Launch in QEMU
```bash
# 4 vCPUs, 256 MiB RAM, serial console on stdio
qemu-system-x86_64 -kernel target/kernel.elf -serial stdio -vga std -m 256M -smp 4
```

Purple Desktop starts automatically when the kernel finds an active framebuffer.
The Purple Bar and Purple Dock render over the wallpaper, and AegisShell stays
available over serial. Dock icons provide hover and click feedback for the shell
design. To include the font files and filesystem diagnostics, generate and attach
a disk with `python build/mkpfs2_disk build/purple_gpt.img` and
`-drive file=build/purple_gpt.img,format=raw,index=0,media=disk`.
The desktop uses compiled Inter Regular and Bold glyph coverage from the bundled
font files. Regenerate the coverage with `python scripts/generate_inter_font.py`.

---

## 5. Dedicated Storage Hierarchy & VFS Namespace

Storage devices in Aegis64 are decoupled from generic `/dev` character devices and categorized into dedicated, type-safe hardware paths:

| Storage Mount Path | Hardware Category | Supported Controllers & Protocols |
| :--- | :--- | :--- |
| `/hdd/hdd1`, ... | Hard disks | Legacy ATA PIO (LBA28) and SATA AHCI (LBA48) |
| `/ssd/ssd1`, ... | Solid-state storage | SATA AHCI and the first active namespace on each NVMe controller |
| `/usb`, `/sdxc`, `/sdhc` | Removable storage | USB mass storage and SD block I/O are unavailable |
| `/dev` | Non-Storage Devices | Character & pseudo devices (`null`, `zero`, `console`) |
| `/proc` | Introspection | Virtual procfs (`version`, `meminfo`, `uptime`, `cpuinfo`, `tasks`) |
| `/tmp` | Volatile Storage | In-memory RAM disk (TmpFS) |
| `/` | Root Storage | Mounted PurpleFS v2 volume from GPT partition |

AHCI and NVMe support logical blocks of 512 to 4096 bytes. GPT and PurpleFS v2 currently require 512-byte logical blocks. Storage paths expose read-only sector-zero inspection. Writes to raw device paths are rejected to protect partition metadata; persistent file writes are available only through the mounted filesystem.

---

## 6. Interactive Console: AegisShell 2.5 (MS-DOS & Linux POSIX Hybrid)

AegisShell 2.5 provides a complete terminal environment combining the ergonomics of **MS-DOS**, the utility of **Base Linux CLI**, and **Natural Expressive English Phrasing**:

### Command Matrix

| MS-DOS Command | Linux POSIX Command | Natural English Syntax | Description |
| :--- | :--- | :--- | :--- |
| `cd <path>` / `chdir` | `cd <path>` | `go to <path>` | Changes current working directory (`cd /proc`, `cd /ssd`, `cd ..`) |
| `cd` | `pwd` | `show current folder` | Displays current working directory |
| `dir [path]` | `ls [-l] [path]` | `show files [in path]` | Lists directory entries with file size, directory flag, and dates |
| `tree` | `tree` | `show tree` | Visual ASCII hierarchy of root, virtual mounts, and storage nodes |
| `type <file>` | `cat <file>` | `read <file>` / `peek` | Reads and outputs file contents via unified VFS |
| — | `head [-n lines] <file>`| `show first lines` | Displays the first N lines of a file (default 10) |
| — | `tail [-n lines] <file>`| `show last lines` | Displays the last N lines of a file (default 10) |
| `copy <src> <dst>` | `cp <src> <dst>` | `copy <src> to <dst>` | Copies files across filesystems (PurpleFS, TmpFS, ProcFS) |
| `ren` / `move` | `mv <src> <dst>` | `move <src> to <dst>` | Moves or renames files |
| `del <file>` / `erase` | `rm <file>` | `delete <file>` | Deletes a file from storage |
| — | `touch <file>` | `create <file>` | Creates an empty file |
| — | `echo <text> > <file>` | `write [to] <file> <text>` | Overwrites file with specified text content |
| — | `echo <text> >> <file>`| `append [to] <file> <text>`| Appends text content to file |
| `md <dir>` / `mkdir` | `mkdir <dir>` | `create folder <dir>` | Creates a new directory |
| `rd <dir>` / `rmdir` | `rmdir <dir>` | `delete folder <dir>` | Removes a directory |
| `find <str> <file>` | `grep <pattern> <file>` | `search <str> in <file>` | Filters and outputs matching lines in text file |
| — | `wc <file>` | `count lines in <file>` | Counts lines, words, and byte size of a file |
| `debug / dump` | `hexdump` / `xxd` / `hex` | `dump <file\|device>` | 16-byte hex + ASCII memory dump of files or storage nodes |
| `ver` | `uname -a` | `show version` | Displays kernel version, edition, and compiler architecture |
| `date` / `time` | `date` | `show time` / `show date` | Reads real CMOS Real-Time Clock (RTC) chip timestamp |
| `mem` | `free` | `show memory` | Displays total, used, and free physical memory and frame counts |
| — | `whoami` / `id` | `show user` | Displays current security context (`root` Ring 0 Supervisor) |
| — | `ps` / `tasks` | `show tasks` | Lists active threads, scheduling states, and CPU affinities |
| `cls` | `clear` | `clear screen` | Clears video framebuffer and serial console |
| `mount` | `mount` | `show mounts` | Lists active VFS mountpoints and backing block devices |
| — | `sync` | `sync storage` | Flushes dirty block buffers and commits journal transactions |
| — | `remount <ro\|rw>` | `remount volume as` | Toggles volume between Read-Only and Read-Write mode |
| — | `ifconfig` / `ip` | `show network` | Displays Ethernet interfaces, IP addresses, and MAC addresses |
| — | `arp` | `show arp` | Displays kernel dynamic ARP table entries |
| — | `ping <ip>` | `ping to <ip>` | Transmits ICMP Echo packets through pure Rust network stack |
| — | `posix [status\|test]` | `show posix` | Displays POSIX subsystem status & runs 8-test validation suite |
| — | `reboot` / `restart` | `restart system` | Reboots machine via 8042 keyboard controller |

---

## 7. Native POSIX Subsystem (IEEE Std 1003.1-2024 / Linux x86_64 ABI)

Aegis64 contains a Rust POSIX compatibility layer for future applications. The kernel is written in Rust, while required CPU, device, and ABI constants remain part of the implementation. The project is under active development; security and conformance must be assessed feature by feature:

- **Hardware Syscall Vector:** Standard x86_64 Linux ABI (`SYSCALL`/`SYSRET` with syscall ID in `RAX`, arguments in `RDI`, `RSI`, `RDX`, `R10`, `R8`, `R9`).
- **Standard Syscalls Supported:**
  - File I/O: `open` (2), `close` (3), `read` (0), `write` (1), `lseek` (8), `stat` (4), `fstat` (5), `pipe` (22), `dup` (32), `dup2` (33).
  - Process Control: `wait4` (61), `kill` (62), `getpid` (39), `getppid` (110), `getuid` (102), `getgid` (104). `fork` and `execve` currently return `ENOSYS`.
  - Virtual Memory: `brk` (12), `mmap` (9), `munmap` (11).
  - Time & Clock: `clock_gettime` (228), `nanosleep` (35) with astronomical Gregorian leap-year CMOS RTC epoch calculation.
  - Directories & Path: `getcwd` (79), `chdir` (80), `mkdir` (83), `rmdir` (84), `unlink` (87).
- **Per-Task File Descriptor Table:** 32 slots per task, initialized with standard console streams:
  - FD 0: `stdin` (keyboard/serial)
  - FD 1: `stdout` (serial/video framebuffer)
  - FD 2: `stderr` (serial/video framebuffer)
  - FD 3..31: Dynamically mapped to VFS file descriptors and anonymous IPC pipes.
- **Signal Handling:** POSIX.1-1990 signals 1..31 (`SIGHUP`, `SIGINT`, `SIGKILL`, `SIGTERM`, `SIGCHLD`, etc.) with atomic signal masks and pending vectors.
- **In-Kernel Verification:** Run `posix test` in the AegisShell to validate all 8 core subsystems in real time.

---

## 8. License

This project is licensed under the Aegis64/PurpleOS License (APPL) v3.0 - see the [LICENSE.md](LICENSE.md) file for details.
