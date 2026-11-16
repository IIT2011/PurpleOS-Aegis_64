# Purple OS / Aegis64: Verification and Test Guide

> [!WARNING]
> **Active Development Warning:**
> Purple OS and the Aegis64 kernel are under heavy active development.

---

## 1. Test & Build Commands

| Verification Target | Command | Expected Outcome |
| :--- | :--- | :--- |
| **Cargo Check** | `cargo check --target x86_64-unknown-none` | Exit code 0, 0 compilation errors. |
| **Kernel Build** | `cargo build --target x86_64-unknown-none` | Produces `target/x86_64-unknown-none/debug/aegis64`. |
| **Multiboot ELF Bootstrap** | `rust-objcopy -O elf32-i386 target/x86_64-unknown-none/debug/aegis64 target/kernel.elf` | Generates 32-bit ELF wrapper with magic `0x1BADB002`. |
| **APDRV Package Build** | `python scripts/build_apdrv.py` | Generates 17 verified `.apdrv` modules with machine code & PCI tables. |
| **QEMU Multi-Core Boot** | `qemu-system-x86_64 -kernel target/kernel.elf -serial stdio -m 256M -smp 4` | Boots 4 SMP cores, initializes drivers, passes AegisDetect (12/12), reaches shell. |

---

## 2. Kernel Self-Tests via AegisShell 2.0

Once booted into AegisShell, the following automated kernel commands verify hardware subsystems:

* `smptest`: Verifies atomic synchronization and concurrency across all 4 CPU cores simultaneously.
* `gpu3d`: Verifies 3D hardware command submission pipeline for Intel RCS, AMD PM4, or NVIDIA Nouveau pushbuffers.
* `aegisdetect`: Verifies SHA-256 cryptographic hashes for 12 system components under $W \oplus X$ memory protection.
* `lsmod`: Confirms loaded APDRV v3 kernel modules with memory region access permissions.
* `devices`: Inspects peripheral discovery via PurpleDetect.
* `lspci`: Enumerate PCI devices and verify BAR configuration.
