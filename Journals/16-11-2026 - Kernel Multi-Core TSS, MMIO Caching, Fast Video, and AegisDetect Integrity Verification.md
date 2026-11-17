# Engineering Journal: Multi-Core TSS, MMIO Caching Flags, Fast Double-Buffered Video, and AegisDetect SHA-256 Verification

**Date:** November 16, 2026  
**Subsystems Affected:** GDT / TSS (`gdt.rs`), SMP (`smp.rs`), Task Scheduler (`task.rs`), IDT (`idt.rs`), Storage (`storage.rs`), MMIO Paging (`vmm.rs`, `e1000.rs`, `audio.rs`, `usb.rs`, `gpu.rs`, `gpu_accel.rs`), Video Console (`video.rs`), PurpleFS v3 (`purplefs_v3.rs`), AegisDetect (`aegis_detect.rs`), GitHub Linguist (`.gitattributes`).

---

## 1. Problem Analysis & Discovered Deficiencies

1. **Per-CPU TSS & Double Fault Separation:**
   - Previous GDT had a single TSS descriptor at selector `0x28`. In x86-64 long mode, loading a TSS via `ltr` marks the descriptor as "Busy" (Type 11). When AP cores executed `ltr`, this caused `#GP` (General Protection Fault) or shared the same interrupt stack table (IST1), introducing double-fault stack corruption during concurrent exceptions.
2. **SMP-Aware Scheduling:**
   - The scheduler in `task.rs` hardcoded `cpu_id = 0` in yield, sleep, and timer ticks. As a result, AP cores yielded or context-switched on CPU 0's task queue instead of their own.
3. **MMIO Caching Invalidation:**
   - Peripheral registers across E1000, Intel HDA, xHCI/EHCI, Bochs GPU, Intel/AMD/NVIDIA 3D accelerators, AHCI, and NVMe were previously accessed via identity mapping without page caching flags, risking stale reads and out-of-order writes.
4. **Console Framebuffer PCIe Stalls:**
   - Graphical console scrolling previously read back pixels from VRAM across the slow PCIe bus to shift rows up, introducing severe frame delays.
5. **Cryptographic Integrity & Filesystem Driver Inodes:**
   - PurpleFS v3 lacked inode definitions for secondary driver modules (`wifi.apdrv`, `camera.apdrv`, `bluetooth.apdrv`, `input.apdrv`, `network.apdrv`, `Graphics.apdrv`).
   - `AegisDetect` verification was previously static; it required genuine SHA-256 cryptographic hashing across filesystem components.
6. **GitHub Language Statistics Cache Artifact:**
   - GitHub's repository language card displayed `C: 92.3%` due to cached Linguist statistics from the pre-rewrite commit `b1945a78` (which held 13,098,377 bytes of legacy C files). The working tree now contains zero `.c` or `.h` files.

---

## 2. Implemented Architecture & Fixes

1. **GDT & Per-CPU TSS Expansion (`src/arch/x86_64/gdt.rs`, `src/kernel/smp.rs`):**
   - Expanded `GdtTable` to 21 entries accommodating up to 8 logical CPUs with per-CPU 16-byte TSS descriptors (`0x28 + cpu_id * 16`).
   - Allocated distinct 8 KiB Double Fault stacks and 16 KiB default stacks per CPU.
   - `init_ap(cpu_id)` dynamically issues `ltr {tss_sel}` with its dedicated selector.
2. **Local APIC Core Identification in Scheduler (`src/kernel/task.rs`):**
   - Implemented `get_current_cpu()` querying `crate::drivers::apic::get_id()`.
   - Wired `schedule()`, `sleep()`, `exit()`, and `timer_tick()` to dynamically operate on the current core's runqueue.
3. **Explicit MMIO Range Paging with Uncacheable Flags:**
   - Mapped MMIO apertures with `PAGE_MMIO` (`PAGE_CACHE_DISABLE | PAGE_WRITE_THROUGH`) across:
     - E1000 NIC (128 KiB)
     - Intel HDA Audio (16 KiB)
     - xHCI USB (64 KiB)
     - Bochs / QEMU DRM VBE (16 MiB linear aperture)
     - 3D Acceleration Rings (Intel RCS, AMD PM4, NVIDIA Pushbuffer - 64 KiB each)
     - AHCI Controller (4 KiB at BAR5)
     - NVMe Controller (16 KiB at BAR0)
4. **Fast RAM-Buffered Console (`src/drivers/video.rs`):**
   - Introduced `text_buf: [u8; 128 * 64]` in RAM.
   - Replaced PCIe VRAM readbacks during scrolling with in-memory buffer shifts (`copy_within`) and a clean write-stream redraw.
5. **AegisDetect SHA-256 Cryptographic Verification (`src/security/aegis_detect.rs`, `src/fs/purplefs_v3.rs`):**
   - Populated complete inode mappings for all 16 driver packages in `PurpleFsV3`.
   - Connected `AegisDetect` to `crate::crypto::sha256::hash()` to cryptographically hash each system component on boot and verify integrity against trusted baseline digests.
6. **GitHub Linguist Overrides (`.gitattributes`):**
   - Configured `*.rs linguist-language=Rust`, excluded auxiliary Python build utilities with `scripts/** linguist-vendored=true`, and marked binary packages.

---

## 3. Verification & Acceptance Testing

- **Compilation:** `cargo build --target x86_64-unknown-none` and `cargo build --release` compiled with **0 warnings, 0 errors**.
- **ELF Image Conversion:** `rust-objcopy -O elf32-i386 target/x86_64-unknown-none/debug/aegis64 target/kernel.elf` completed successfully (2.18 MB binary).
- **QEMU Bare-Metal SMP Boot:**
  - 4/4 SMP cores initialized and operational.
  - Local APIC timer calibrated via PIT (63.26 MHz base, 100 Hz tick).
  - Storage & PurpleFS mounted volume `PurpleSystem`.
  - AegisDetect: 12/12 components SHA-256 verified and trusted.

---

## 4. PurpleDetect Hardware Inventory Subsystem & Auxiliary Bug Fixes

1. **Full Pure Rust PurpleDetect Subsystem (`src/drivers/purple_detect.rs`):**
   - Transformed previously hollow 7-line stub into a live, thread-safe hardware inventory registry (`REGISTRY`).
   - Defined `DeviceCategory` (`Storage`, `Input`, `Display`, `Audio`, `Network`, `Serial`, `Timer`, `Interrupt`, `Bridge`, `Bus`) and `DeviceState` (`Discovered`, `Active`, `Supported`, `Unsupported`).
   - Automatic hardware discovery dynamically registers both ISA peripherals (COM1, PIT, PS/2 Keyboard/Mouse, LAPIC, ATA Primary & Secondary) and all discovered PCI devices via `pci::for_each_device`.
   - Wired `devices` and `hwinfo` AegisShell commands directly to `purple_detect::print_table()`.
2. **VMM Spinlock Recursion Elimination (`src/mm/vmm.rs`):**
   - Fixed deadlock in `map_mmio_range()` which previously acquired `VMM_LOCK.lock()` before invoking `map_page()`, which itself acquired `VMM_LOCK.lock()`.
3. **ACPI Parser Hardening (`src/kernel/acpi.rs`):**
   - Fixed unaligned memory reads across packed ACPI tables (`core::ptr::read_unaligned`).
   - Added RSDP checksum verification and boundary sanitization to eliminate panics on uniprocessor configurations.
