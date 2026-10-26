# Purple OS / Aegis64  (Warning!!!- This project is still in development)

**Purple OS** is an independent, native-first 64-bit operating system designed from scratch.
Its kernel is named **Aegis64**, its scheduler is **AegisSched_64 / PurpleSched**, its native filesystem is **PurpleFS v2**, its native driver package architecture is **APDRV v3**, its application bundle format is **`.apapplication`**, its hardware discovery engine is **PurpleDetect**, and its hardware trust mechanism is **AegisDetect**.

---

## 1. Architectural Highlights

- **Target Architecture:** x86-64 (AMD64 Long Mode with 4-level paging and CR0.WP write-protection).
- **Dual Boot Targets:**
  - `build/kernel.elf`: Multiboot 1 compliant 32-bit ELF bootstrap (`0x101000` entry point).
  - `build/kernel64.elf`: Pure 64-bit native ELF kernel with 64 KiB boot stack.
  - `build/aegis64-uefi.img`: GPT dual-partition UEFI disk image booted via OVMF firmware and GRUB.
- **Physical Memory Manager (PMM):** 4 KiB frame allocator managing physical memory up to 4 GiB using a 128 KiB dual-bitmap (alloc/reserved) layout, reserving the first 16 MiB.
- **Virtual Memory Manager (VMM):** 4-level page tables (PML4, PDPT, PD, PT) with 2 MiB huge page splitting, canonical address validation, rollback on allocation failure, and strict W^X enforcement.
- **Scheduler (AegisSched_64 / PurpleSched):** Preemptive multi-core scheduler supporting 5 priority tiers (`IDLE`, `LOW`, `NORMAL`, `HIGH`, `REALTIME`), per-core CPU affinity masks, 100 Hz Local APIC timer ticks, and 16 KiB task stacks.
- **Symmetric Multiprocessing (SMP):** ACPI MADT enumeration, real-mode AP bootstrap trampoline (`src/kernel/smp_trampoline.S`), INIT/SIPI inter-processor interrupt sequence, per-CPU runtime state and idle tasks.
- **Native Filesystem (PurpleFS v2):** Journaled filesystem supporting 4096-byte blocks, GPT partition layout, directory tables, direct/indirect data blocks, and transaction redo logging with crash recovery.
- **Security & Integrity (AegisDetect):** Hardware Root of Trust authentication using HMAC-SHA256, baseline verification of boot and filesystem components, and tamper detection.
- **Device & Bus Discovery (PurpleDetect):** Real-time hardware device lifecycle manager covering ISA, PCI, Serial, Timers, and Storage peripherals with background monitoring daemon.
- **Native Driver Subsystem (APDRV v3 & KSYM):** Dynamic kernel module loading (`.apdrv`), kernel symbol table resolution, relocations (RELOC_64, 32, PC32, PLT32), W^X memory enforcement, and PCI auto-binding.
- **Application Bundle Runtime (`.apapplication`):** Authenticated application packaging with HMAC-SHA256 signatures, permission-based capabilities, and scheduled concurrent execution.
- **Desktop Windowing Environment:** Floating window manager, mouse pointer tracking (PS/2 Aux IRQ 12), text window compositor, and dual-output (serial COM1 + GOP / Bochs VBE linear framebuffer).
- **Command Shell (AegisShell):** Interactive shell with 28+ system management commands (`sysinfo`, `cpuinfo`, `mem`, `tasks`, `smp`, `smptest`, `disks`, `df`, `remount`, `sync`, `touch`, `write`, `peek`, `ls`, `aegisdetect`, `devices`, `lspci`, `desktop`, `mouse`, `ksym`, `insmod`, `lsmod`, `rmmod`, `ifconfig`, `apps`, `app`, `video`, `setmode`).

---

## 2. Quick Start & Build Instructions

### Prerequisites (Arch Linux)
```bash
sudo pacman -S gcc binutils make qemu-system-x86 edk2-ovmf grub python
```

### Build Everything
```bash
make all
```

### Run Host Test Suite (93 Tests, 11 Suites)
```bash
make test
```

### Run AddressSanitizer & UndefinedBehaviorSanitizer (9 Suites)
```bash
make test-asan
```

### Run Automated QEMU Verification Suites
```bash
make check-scheduler-qemu          # AegisSched_64 preemption and priority slices
make check-smp-qemu                # SMP 4-core concurrency & APIC IPI test
make check-writable-storage-qemu   # PurpleFS v2 dynamic rw, file creation & persistence
make check-storage-qemu            # 44/44 end-to-end integration assertions (APDRV, APAPP, Desktop)
```

### Launch Interactive Environments
```bash
make run-qemu                      # Multiboot QEMU serial terminal
make run-uefi                      # Full UEFI / OVMF graphical boot
```
