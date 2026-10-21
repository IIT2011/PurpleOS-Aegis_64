# Aegis64 Architecture

## 1. System Overview

Aegis64 is a monolithic, 64-bit native operating system kernel developed for x86-64 hardware.

```mermaid
graph TD
    Firmware[UEFI Firmware / OVMF] --> Bootloader[GRUB BOOTX64.EFI]
    Bootloader --> Bootstrap[kernel.elf Entry 0x101000]
    Bootstrap --> Paging[PAE, EFER.LME, CR0.PG Bootstrap Page Tables]
    Paging --> LongMode[Long Mode Entry 64-bit]
    LongMode --> Kernel[Aegis64 kernel_main_64]
    Kernel --> PMM[PMM: Dual-Bitmap 4 GiB Manager]
    Kernel --> VMM[VMM: 4-Level Page Tables & W^X]
    Kernel --> Interrupts[IDT & PIC 100 Hz Timer]
    Kernel --> Scheduler[AegisSched_64 Preemptive Engine]
    Scheduler --> Shell[AegisShell Command Environment]
```

## 2. Core Protection Boundaries

- **Privilege Levels:**
  - Kernel core, scheduler, and memory management execute in **Ring 0**.
  - Future application bundles execute in **Ring 3** with narrow syscall interfaces.
- **Memory Protection (W^X):**
  - Code sections (`.text`, `.boot`) are mapped `Read + Execute` (No Write).
  - Data, BSS, and stacks are mapped `Read + Write` with the No-Execute (`NX`) bit enforced.
  - Page table modifications trigger `invlpg` invalidation.
- **Hardware Isolation:**
  - Interrupt vectors 0..31 are dedicated to CPU exceptions with full register dumping and CR2 decoding.
  - Hardware interrupts are mapped cleanly to IRQ 32..47 via the 8259 PIC.
