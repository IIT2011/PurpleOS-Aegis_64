# Purple OS Driver Support Matrix

This matrix reflects the current repository state as verified from the GitHub branch content at `Aegis_Purple`.

## High-level status legend
- Implemented and source-verified
- Implemented and test-described in Makefile
- Partially implemented / design-only
- Blocked by missing specification

## Platform and kernel
| Component | Status | Evidence |
| --- | --- | --- |
| x86-64 kernel entry / init | Implemented | `src/kernel/main.c` |
| PMM | Implemented | `src/mm/pmm.c` |
| VMM | Implemented | `src/mm/vmm.c` |
| IDT / PIC / APIC | Implemented | `src/kernel/idt.c`, `src/kernel/pic.c`, `src/drivers/apic.c` |
| ACPI | Implemented | `src/kernel/acpi.c` |
| SMP | Implemented | `src/kernel/smp.c` |
| Scheduler | Implemented | `src/kernel/task.c` |
| Input: keyboard / mouse | Implemented | `src/drivers/keyboard.c`, `src/drivers/mouse.c` |
| Serial console | Implemented | `src/drivers/serial.c` |
| Video / framebuffer | Implemented | `src/drivers/video.c` |
| Desktop | Implemented | `src/drivers/desktop.c` |
| Shell | Implemented | `src/kernel/shell.c` |

## Storage and filesystem
| Component | Status | Evidence |
| --- | --- | --- |
| PurpleFS v2 | Implemented | `src/fs/purplefs_v2.c` |
| GPT / partition logic | Implemented | `src/fs/gpt.c`, `src/fs/partition_dev.c` |
| ATA PIO | Implemented | `src/drivers/storage/ata_pio.c` |
| AHCI | Implemented | `src/drivers/storage/ahci.c` |
| NVMe | Implemented | `src/drivers/storage/nvme.c` |
| PurpleFS v3 design | Missing / blocked | `docs/PURPLEFS_DESIGN.md` is v2, not v3 |
| Bootable-image architecture plan | Missing / blocked | not found in `docs/` |

## Driver packages and security
| Component | Status | Evidence |
| --- | --- | --- |
| `.apdrv` loader | Implemented | `src/drivers/apdrv.c` |
| Kernel symbol resolution | Implemented | `src/kernel/ksym.c` |
| AegisDetect | Implemented | `src/security/aegis_detect.c` |
| PurpleDetect | Implemented | `src/drivers/purple_detect.c` |
| HMAC trust anchor model | Implemented as design/runtime support | `src/security/aegis_detect.c` |

## Network / devices
| Component | Status | Evidence |
| --- | --- | --- |
| NIC stack | Implemented | `src/drivers/netdev.c`, `src/drivers/e1000.c` |
| USB host controller logic | Implemented | `src/drivers/usb.c` |
| Audio | Implemented | `src/drivers/audio.c` |
| RTC | Implemented | `src/drivers/rtc.c` |
| DMA abstractions | Implemented | `src/drivers/dma.c` |
| PCI enumeration | Implemented | `src/drivers/pci.c` |

## Explicit blockers
- Formal PurpleFS v3 layout and addressing review docs are absent.
- Formal bootable image architecture plan is absent.
- A true independent review of v3 crash/recovery and journal sizing policy is not present in checked-in docs.
- QEMU or UEFI execution was not performed in this GitHub-only environment, so emulator status is not confirmed here.

## Canonical interpretation
The repository is already a substantial kernel/OS implementation with real code and tests, but the mission-level v3 boot and PFS3 specification gate remains unresolved because the required design files are missing from the current branch.
