# Purple OS / Aegis64

> [!WARNING]
> **Active Development Warning:**
> Purple OS and the Aegis64 kernel are under heavy active development. System architectures, driver ABIs, memory layouts, and filesystem formats are subject to frequent changes. Not intended for production use.

---

**Purple OS** is an independent, native-first 64-bit operating system engineered from scratch in **pure bare-metal Rust** (`#![no_std]`, `#![no_main]`, `target = "x86_64-unknown-none"`, with zero external crate dependencies and zero legacy C/C++ runtime code).

Its core kernel is **Aegis64**, its multi-core scheduler is **AegisSched_64**, its native filesystem engine is **PurpleFS v3**, its modular driver package architecture is **APDRV v3**, its hardware discovery system is **PurpleDetect**, and its cryptographic boot verification system is **AegisDetect**.

---

## 1. Architectural Overview

```
+--------------------------------------------------------------------------+
|                       AegisShell 2.0 (User Console)                      |
+--------------------------------------------------------------------------+
|          PurpleFS v3 (Filesystem)  |  APAPP Subsystem (Applications)     |
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
- **Cryptographic Security (AegisDetect):** Hardware Root of Trust and boot integrity engine utilizing SHA-256 and HMAC-SHA256 baselines (12/12 system components verified).
- **Native Filesystem (PurpleFS v3 & v2):** Hierarchical journaled filesystem supporting 4096-byte blocks, GPT partition layout, directory tables, direct/indirect data blocks, and transaction redo logging with crash recovery.
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

# Build APDRV driver modules
python scripts/build_apdrv.py
```

### Launch in QEMU
```bash
# 4 vCPUs, 256 MiB RAM, serial console on stdio
qemu-system-x86_64 -kernel target/kernel.elf -serial stdio -m 256M -smp 4
```

---

## 5. Interactive Console (AegisShell 2.0)

AegisShell provides rich kernel inspection and management commands supporting both **Natural Expressive Phrasing** and classic command shortcuts:

### Natural Expressive Commands

| Natural Command Syntax | Description |
| :--- | :--- |
| `copy <source> to <destination>` | Copies a file from source to destination on PurpleFS |
| `move <source> to <destination>` | Moves/renames a file on PurpleFS |
| `delete <file>` / `remove <file>` | Removes a file and deallocates its blocks on PurpleFS |
| `read <file>` / `peek <file>` | Displays file contents from VFS / PurpleFS |
| `create <file>` / `touch <file>` | Creates a new empty file |
| `write to <file> <content>` | Writes arbitrary text content into a file |
| `show <category>` | Displays subsystem status (`disks`, `filesystem`, `memory`, `cpu`, `system`, `uptime`, `tasks`, `devices`, `pci`, `network`, `modules`, `symbols`, `video`, `3d`, `mouse`, `files`) |
| `load module <name>` | Loads and verifies an APDRV v3 kernel driver module |
| `remount volume as <ro\|rw>` | Switches filesystem mount state between Read-Only and Read-Write |
| `sync storage` | Commits journal transactions and flushes hardware block cache |
| `verify security` | Executes AegisDetect SHA-256 cryptographic baseline authentication |
| `ping to <ip>` / `ping <ip>` | Transmits ICMP Echo request through pure Rust TCP/IP network stack |
| `clear screen` | Clears linear framebuffer and terminal window |
| `restart system` | Reboots hardware via 8042 keyboard controller |

### Command Aliases & Shortcuts

| Command | Category | Description |
| :--- | :--- | :--- |
| `sysinfo` / `show system` | System | Displays OS version, kernel build, and memory architecture |
| `cpuinfo` / `show cpu` | System | CPU vendor, brand string, and feature flags |
| `mem` / `show memory` | Memory | Physical Memory Manager (PMM) total and free frame statistics |
| `tasks` / `show tasks` | Scheduler | Displays all active threads, states, CPU affinity, and priority ticks |
| `smp` | Multi-Core | SMP status and operational status of all online CPU cores |
| `smptest` | Multi-Core | Dispatches concurrent atomic increment workload across all 4 vCPUs |
| `lspci` / `show pci` | Hardware | Scans and lists all devices on the PCI bus hierarchy |
| `devices` / `show devices` | Hardware | PurpleDetect peripheral registry (COM1, PS2, ATA, etc.) |
| `video` / `show video` | Hardware | Displays VBE resolution, color depth, and linear framebuffer base |
| `gpu3d` / `show 3d` | Hardware | Inspects 3D acceleration engine (Intel/AMD/NVIDIA) |
| `mouse` / `show mouse` | Input | Reads current PS/2 mouse coordinates and button flags |
| `ifconfig` / `show network` | Network | Network interface configuration, MAC and IP address |
| `ping <ip>` | Network | Sends ICMP Echo Ping through pure Rust TCP/IP stack |
| `lsmod` / `show modules` | Modules | Lists loaded APDRV v3 modules with W^X enforcement status |
| `insmod` / `load module` | Modules | Dynamically loads and verifies an APDRV driver module |
| `ksym` / `show symbols` | Symbols | Displays kernel export symbol table |
| `aegisdetect` / `verify` | Security | Displays cryptographic boot integrity baseline verification |
| `disks` / `show disks` | Storage | Lists block devices, partitions, and GPT partition table |
| `df` / `show filesystem` | Storage | Displays PurpleFS volume mount state, block counts, and capacity |
| `list files` / `show files` | Filesystem | Directory listing of `/` root filesystem |
| `copy` / `move` / `delete` | Filesystem | File operations with expressive grammar |
| `reboot` / `restart` | Power | Triggers soft reboot via 8042 keyboard controller |

---

## 6. License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
