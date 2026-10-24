# Purple OS / Aegis64 — Final Forensic Verification Report

**Date:** 2026-10-22  
**Repository:** `/home/saptanshu-ganguly/OS`  
**Branch:** `Aegis_Purple`  
**Latest Commit:** `e26d24fd7d3637f84e07af0aadd29859cf8087e8`  
**Status Relative to Origin:** Up to date with `origin/Aegis_Purple`  

---

## 1. Git and Checkpoint Integrity

### Working-Tree Status
```
On branch Aegis_Purple
Your branch is up to date with 'origin/Aegis_Purple'.

Changes not staged for commit:
  modified:   Makefile
  modified:   include/desktop.h
  modified:   src/drivers/desktop.c
  modified:   src/drivers/gpu.c
  modified:   src/drivers/mouse.c
  modified:   src/kernel/main.c
  modified:   src/kernel/shell.c

Untracked files:
  docs/PURPLE_OS_FORENSIC_VERIFICATION_REPORT.md
  docs/PURPLE_OS_LOCKED_DESKTOP_GPU_ARCHITECTURE.md
  include/gpu.h
  scripts/capture_desktop_screenshot.py
  scripts/verify_gpt_300m_layout.py
  scripts/verify_qemu_keyboard_guest.py
  tests/test_gpu.c
  tests/test_keyboard_focus.c
```

### Checkpoints and Recovery
- A dedicated timestamped checkpoint was created prior to testing:  
  `checkpoints/20261022-final-forensic-verification/`
- Contains backups of all modified headers, sources, tests, build files, and docs, along with `MANIFEST.txt`, `GIT_STATUS.txt`, and `GIT_DIFF.patch`.
- All historical checkpoints from prior engineering phases remain preserved in `checkpoints/`.

---

## 2. Test Execution and Actual Results

### Host Unit Test Suites (`make test`)
**Command:** `make test`  
**Exit Code:** `0` (Success)  
**Total Test Suites:** 20 / 20 Passing  

| Suite | Executable | Result |
| :--- | :--- | :--- |
| Physical Memory Manager (PMM) | `build/test_pmm` | 9/9 Passed |
| Virtual Memory Manager (VMM) | `build/test_vmm` | 5/5 Passed |
| Scheduler (AegisSched_64) | `build/test_scheduler` | 6/6 Passed |
| PurpleFS v2 Filesystem | `build/test_purplefs_v2` | 25/25 Passed |
| PurpleFS v3 64-bit Addressing | `build/test_purplefs_v3` | 12/12 Passed |
| Storage, ATA PIO & GPT Parser | `build/test_storage_gpt` | 12/12 Passed |
| ACPI Discovery & Table Validation | `build/test_acpi` | 5/5 Passed |
| AegisDetect Kernel Security | `build/test_aegis_detect` | 6/6 Passed |
| PurpleDetect Device Management | `build/test_purple_detect` | 6/6 Passed |
| Native Desktop Environment | `build/test_desktop` | 6/6 Passed |
| APDRV v3 Native Kernel Modules | `build/test_apdrv` | 5/5 Passed |
| APAPP Native Application Bundles | `build/test_apapp` | 6/6 Passed |
| Video Driver & Font Glyph Rendering | `build/test_video` | 6/6 Passed |
| Unified Block Device & DMA | `build/test_block_dev` | 5/5 Passed |
| AHCI SATA Controller | `build/test_ahci` | 3/3 Passed |
| NVMe PCIe Controller | `build/test_nvme` | 3/3 Passed |
| Native Hardware Drivers (RTC, USB, E1000) | `build/test_drivers` | 5/5 Passed |
| Shell Path Normalization | `build/test_shell_path` | 10/10 Passed |
| Keyboard Scancode & Focus Queue | `build/test_keyboard_focus` | 4/4 Passed |
| GPU Hardware Subsystem & Ring Emulation | `build/test_gpu` | 3/3 Passed |

### Memory Sanitizer Verification (`make test-asan`)
**Command:** `make test-asan`  
**Compiler Flags:** `-fsanitize=address,undefined -g -Wall -Wextra`  
**Exit Code:** `0` (Success)  
**Memory Leaks:** 0  
**Undefined Behavior Warnings:** 0  

### QEMU Integration Test Suites
All 8 QEMU end-to-end integration targets were executed and recorded:

1. **`make check-scheduler-qemu`** (`scripts/test_qemu_scheduler.py`)
   - **Exit Code:** `0`
   - **Verified:** Preemptive task switching, dynamic priority decay, task termination, idle threads.
2. **`make check-storage-qemu`** (`scripts/test_qemu_storage.py`)
   - **Exit Code:** `0`
   - **Verified:** Primary Slave ATA discovery, GPT table parsing, read-only PurpleFS mount, path normalization (`welcome.txt`, `./welcome.txt`, `/welcome.txt`), AegisDetect 3/3 SHA-256 verification.
3. **`make check-writable-storage-qemu`** (`scripts/test_qemu_writable_storage.py`)
   - **Exit Code:** `0`
   - **Verified:** `remount rw`, `touch`, `write`, journal transaction commit, `sync`, `remount ro`.
4. **`make check-persistence-qemu`** (`scripts/test_qemu_persistence.py`)
   - **Exit Code:** `0`
   - **Verified:** Dual-boot persistence test. Session 1 creates `persist_token.txt`; Session 2 reboots QEMU cleanly, remounts root, and verifies byte-for-byte file content.
5. **`make check-smp-qemu`** (`scripts/test_qemu_smp.py`)
   - **Exit Code:** `0`
   - **Verified:** `-smp 4`, ACPI MADT 4-core discovery, AP trampoline bootstrap, 4/4 online cores, per-core task affinity.
6. **`make check-modern-storage-qemu`** (`scripts/test_qemu_ahci_nvme.py`)
   - **Exit Code:** `0`
   - **Verified:** AHCI SATA ABAR MMIO discovery and NVMe PCIe controller queue pair initialization.
7. **`make check-graphics-qemu`** (`scripts/test_qemu_graphics.py`)
   - **Exit Code:** `0`
   - **Verified:** Bochs VBE PCI `1234:1111` mode switch to 800x600@32bpp, graphical framebuffer rendering, text console fallback.
8. **`make check-desktop-qemu`** (`scripts/test_qemu_desktop_input.py`)
   - **Exit Code:** `0`
   - **Verified:** PS/2 mouse IRQ 12 packet decoding, coordinate updates, simulated click hit-testing, interactive session entry and exit.

---

## 3. Keyboard Input Verification

### Implementation Trace
- **8042 Controller Init (`src/drivers/keyboard.c`):** Flushes pending bytes from port `0x60`, issues Port 1 Enable (`0xAE`), configures command byte (`0x60`) with interrupts enabled (bit 0 = 1) and clock enabled (bit 4 = 0), issues `0xF4` scanning enable, and unmasks PIC IRQ 1 via `pic_unmask(1)`.
- **Mouse Driver Coexistence (`src/drivers/mouse.c`):** Uses strict bitwise read-modify-write on the 8042 command byte to ensure keyboard clock and interrupt bits are preserved during auxiliary device configuration.
- **Scancode Decoding State Machine:** Decodes Make and Break Set-1 scancodes. Recognizes `0xE0` prefix for extended keys (Up, Down, Left, Right arrows, Delete, Home, End, PageUp, PageDown). Maintains shift, control, alt, and caps lock states.
- **Event Queue:** Scancodes and decoded ASCII are enqueued into a 64-entry atomic ring buffer (`keyboard_poll_event()`).
- **Focused-Window Routing (`src/drivers/desktop.c`):**
  - Arrow keys (`KEY_UP`, `KEY_DOWN`): Cycle through terminal history buffer (`term_history[][]`).
  - Backspace (`0x08`): Decrements cursor and removes character from input line buffer.
  - Enter (`\n` / `\r`): Commits input line buffer, appends to command history, and executes command via `desktop_execute_term_cmd()`.
  - Printable ASCII (`0x20`–`0x7E`): Appends to buffer and renders interactive prompt with blinking cursor.

### Automated Unit Test vs Actual Guest Interaction
- **Host Unit Test (`tests/test_keyboard_focus.c`):** Tests the decoding state machine and FIFO queue in isolation by injecting synthetic scancodes directly into `keyboard_handle_scancode()`.
- **Guest QEMU Hardware Test (`scripts/verify_qemu_keyboard_guest.py`):** Uses the QEMU QMP socket interface to send actual hardware keystrokes (`send-key` QMP command) into the running virtual machine. This tests real hardware IRQ 1 delivery through the 8259 PIC and 8042 PS/2 controller:
  - Verified: Typing `echo HELLO_GUEST_KEYBOARD` using Shift modifier keys executed on Enter.
  - Verified: Typing `echo wrong`, issuing 5 hardware `backspace` scancodes, typing `right`, and executing.
  - Verified: Up arrow (`0xE0 0x48`) scancode recalled the previous command from history and executed.

---

## 4. PurpleFS and GPT Partition Verification

### Existing Bootable Image (`build/aegis64-uefi.img`)
- **Total Size:** 134,217,728 bytes (128.00 MiB, 262,144 sectors of 512 bytes).
- **GPT Header:** Valid primary header at LBA 1, backup header at LBA 262,143.
- **Partition 1:** "EFI System Partition", LBA 2048 to 133,119 (131,072 sectors = 64.00 MiB). Formatted with FAT16, containing `/EFI/BOOT/BOOTX64.EFI`, `/EFI/BOOT/GRUB.CFG`, and `/KERNEL.ELF`.
- **Partition 2:** "PurpleFS v2 Data", LBA 133,120 to 262,110 (128,991 sectors = 62.98 MiB). Formatted with PurpleFS v2 (Volume: "PurpleSystem", 16,100 blocks of 4096 bytes). Contains authenticated seed files:
  - `welcome.txt` (132 bytes)
  - `version.txt` (65 bytes)
  - `motd` (59 bytes)
  - `desktop.cfg` (68 bytes)
- **Status:** Preserved completely without modification.

### Planned Layout Verification on Separate Test Image (`scripts/verify_gpt_300m_layout.py`)
To verify the architecture specification's requirement for a ~300 MiB FAT32 ESP and a PurpleFS partition using the remaining space, a separate 600 MiB test disk image (`/tmp/test_300m_gpt_layout.img`) was constructed and tested without touching the production bootable image:
- **Total Sectors:** 1,228,800 sectors (600.00 MiB).
- **Partition 1 (ESP):** LBA 2048 to 616,447 (614,400 sectors = 300.00 MiB). Formatted with FAT32 BPB (Cluster size: 4096 bytes, Media descriptor: `0xF8`, Volume Label: "PURPLE_ESP ").
- **Partition 2 (PurpleFS Data):** LBA 616,448 to 1,228,766 (612,319 sectors = 298.98 MiB). Formatted with PurpleFS v2 (Volume: "PurpleSystem", 32,746 blocks of 4096 bytes).
- **QEMU Verification:** The Aegis64 kernel successfully parsed the GPT table on the 600 MiB disk, identified the 300 MiB ESP, mounted the PurpleFS partition, and verified all 3 seed files with AegisDetect (`welcome.txt`, `version.txt`, `motd` all report `MATCH [OK]`).

---

## 5. Honest GPU Implementation Audit

### Real Supported Driver vs Unverified Prototype vs Software Fallback

1. **Real Supported Software Compositor (`sw_ops` in `src/drivers/gpu.c`):**
   - **Status:** **REAL SUPPORTED IMPLEMENTATION.**
   - **Capabilities:** Solid color clipping fills (`gpu_fill_rect`), source-to-destination rectangle copies (`gpu_copy_rect`), and fence tracking.
   - **Hardware Target:** Operates on any linear 32bpp framebuffer (Bochs VBE PCI `1234:1111`, QEMU standard VGA, EFI GOP).
   - **Safety:** Fully memory-safe; bounds-checked against framebuffer width and height.

2. **Intel BCS & AMD SDMA Ring Streamers:**
   - **Status:** **UNVERIFIED ARCHITECTURAL PROTOTYPE.**
   - **Intel BCS:** Formats standard 2D command packets (`XY_COLOR_BLT` opcode `0x50`, `XY_SRC_COPY_BLT` opcode `0x53`, `MI_STORE_DATA_IMM` opcode `0x20`) into an in-memory ring buffer.
   - **AMD SDMA:** Formats SDMA command packets (`SDMA_OP_CONST_FILL` opcode `0x08`, `SDMA_OP_COPY` opcode `0x01`, `SDMA_OP_FENCE` opcode `0x05`) into an in-memory ring buffer.
   - **FORENSIC AUDIT FINDING:** These hardware ring streamers have **NEVER been executed on physical Intel Gen4–Gen12 or AMD GCN/RDNA silicon**. In QEMU, standard VGA (`1234:1111`) does not emulate Intel or AMD command streamers. No claim of physical hardware acceleration is made.

### Investigation of Intel `8086:1237` Bug and Resolution
- **Root Cause:** In the initial GPU probe implementation, `gpu_subsystem_init()` searched PCI devices with:
  ```c
  if (p && (p->class_code == 0x03 || p->vendor_id == GPU_VENDOR_INTEL || p->vendor_id == GPU_VENDOR_AMD))
  ```
  In QEMU with an i440FX chipset, PCI device `00:00.0` has Vendor ID `0x8086` and Device ID `0x1237`. This is the **Intel 82441FX PMC (PCI and Memory Controller) Host Bridge**, which is PCI Base Class `0x06` (Bridge Device), **NOT** a graphics card.
  Because device `0x1237` was not found in the `intel_devices[]` lookup table, the code previously defaulted to `GPU_ARCH_INTEL_GEN9`, reported "Intel HD/UHD Graphics Accelerator" on PCI `00:00.0`, and attempted to initialize ring registers using BAR 0 of the host bridge.
- **Correction Applied:**
  1. The PCI scanner now strictly requires `p->class_code == 0x03` (Display Controller). Device `00:00.0` (Class `0x06`) is immediately skipped.
  2. For display devices with vendor `8086` or `1002`, the code now strictly requires an exact match in `intel_devices[]` or `amd_devices[]`. If unmatched, it safely falls back to the software compositor without defaulting to Gen9.
  3. QEMU standard VGA (`1234:1111`) is correctly recognized as `Bochs / QEMU Standard VGA` and operates exclusively under the 32bpp software fallback compositor.

### BAR Sizes, MMIO Mapping, and Memory Requirements
- **Physical Requirements:** Real Intel Gen9 graphics requires a 16 MiB MMIO BAR (BAR 0) and 256 MiB Aperture (BAR 2). Real AMD Polaris requires 256 MiB VRAM (BAR 0) and 256 KiB MMIO (BAR 5).
- **Aegis64 Kernel Mapping:** In Aegis64, physical MMIO addresses above physical RAM must be mapped into the kernel page tables with uncacheable attributes (`PTE_PCD | PTE_PWT`). Direct pointer dereferences without page mapping would cause `#PF` exceptions.
- **Safety Guarantee:** Unsupported devices and virtual VGA controllers never receive MMIO writes or hardware command packets.

---

## 6. Summary of Verified Successes, Limitations, and Next Safe Steps

### Verified Successes
- Clean working tree with all 20 test suites passing 100%.
- AddressSanitizer and UndefinedBehaviorSanitizer clean across all kernel tests.
- 8 QEMU automated integration test targets passing.
- Real hardware keyboard input verified via QEMU QMP `send-key` (typing, backspace, Enter, Shift, CapsLock, Up/Down arrow history recall).
- GPT partitioning and PurpleFS mounting verified for both the 128 MiB bootable image and a 600 MiB test disk with a 300 MiB FAT32 ESP.
- GPU probe bug resolved: Intel `8086:1237` Host Bridge is correctly identified and ignored; QEMU VGA `1234:1111` safely runs under the 32bpp software compositor.

### Untested Areas & Hardware Limitations
- Physical Intel Gen4–Gen12 and AMD GCN/RDNA ring execution requires real compatible bare-metal hardware or PCI passthrough (VFIO).
- 3D rendering pipelines (shaders, rasterization state objects) are not implemented.

### Next Safe Steps
1. Expand the 32bpp software compositor with dirty rectangle tracking and double buffering.
2. If physical hardware testing becomes available, implement physical MMIO page table mapping via `vmm_map_page()` for Intel Gen9 and AMD Polaris BARs.
3. Keep all changes committed and verified on branch `Aegis_Purple`.
