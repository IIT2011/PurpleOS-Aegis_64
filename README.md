# Purple OS / Aegis64

**Purple OS** is an independent, native-first 64-bit operating system designed from scratch.
Its kernel is named **Aegis64**, its scheduler is **AegisSched_64 / PurpleSched**, its native filesystem is **PurpleFS v2**, its native driver package architecture is **APDRV v3**, its application bundle format is **`.apapplications`**, and its hardware trust mechanism is **AegisDetect**.

---

## 1. Architectural Highlights

- **Target Architecture:** x86-64 (AMD64 Long Mode with 4-level paging).
- **Dual Boot Targets:**
  - `build/kernel.elf`: Multiboot 1 compliant 32-bit ELF bootstrap (`0x101000` entry point).
  - `build/kernel64.elf`: Pure 64-bit native ELF kernel.
  - `build/aegis64-uefi.img`: GPT dual-partition UEFI disk image booted via OVMF firmware and GRUB.
- **Physical Memory Manager (PMM):** 4 KiB frame allocator managing physical memory up to 4 GiB using a 128 KiB dual-bitmap (alloc/reserved) layout, reserving the first 16 MiB.
- **Virtual Memory Manager (VMM):** 4-level page tables (PML4, PDPT, PD, PT) with 2 MiB huge page splitting, canonical address validation, rollback on allocation failure, and strict W^X enforcement.
- **Scheduler (AegisSched_64):** Priority-aware round-robin preemptive scheduler with 4 tiers (`IDLE`, `LOW`, `NORMAL`, `HIGH`), 100 Hz PIT timer ticks, and 16 KiB task stacks.
- **Command Shell (AegisShell):** Interactive dual-output (serial COM1 + GOP linear framebuffer) shell with tokenized argument parsing.

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

### Run Host Test Suite
```bash
make test
```

### Run Verification & Boot Checks
```bash
make check
make check-scheduler-qemu
```

### Launch in QEMU (Direct Multiboot)
```bash
make run-qemu
```

### Launch in QEMU (Full UEFI / OVMF Boot)
```bash
make run-uefi
```
