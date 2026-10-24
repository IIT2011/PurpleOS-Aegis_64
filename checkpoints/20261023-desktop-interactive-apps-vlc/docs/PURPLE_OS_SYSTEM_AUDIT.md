# PURPLE OS / AEGIS64 — FULL SYSTEM ARCHITECTURAL AUDIT & CAPABILITY REPORT

**Date:** October 2026  
**Operating System:** Purple OS  
**Kernel:** Aegis64 (x86-64 monolithic core with dynamic APDRV driver modules)  
**Repository:** `/home/saptanshu-ganguly/OS`  
**Branch:** `Aegis_Purple`  
**Target Architecture:** x86-64 (AMD64 / Intel 64)  
**Firmware Target:** UEFI (x86-64 OVMF / EDK2) & Legacy Multiboot 1 / BIOS  

---

## 1. Executive Summary

This comprehensive forensic audit documents the exact architectural baseline, hardware driver ecosystem, storage pipeline, graphical desktop environment, command-line interface, application runtime, and verification state of Purple OS and the Aegis64 kernel.

Purple OS is engineered around an x86-64 monolithic kernel core (`kernel64.elf`) augmented by native dynamic hardware driver modules (`.apdrv`), unified block device abstractions, a flat fast-booting initial filesystem (`PurpleFS v2`), an upcoming 64-bit hierarchical filesystem (`PurpleFS v3`), a direct linear framebuffer graphical desktop environment with window compositing, and an application package model (`.apapplication`).

All 17 host unit test suites and AddressSanitizer (ASan) / UndefinedBehaviorSanitizer (UBSan) verification targets pass with zero leaks and zero errors:
- **Host Unit Test Suites:** 17 passed / 17 total (100%)
- **AddressSanitizer / UndefinedBehaviorSanitizer:** 100% clean, zero memory leaks, zero buffer overflows
- **QEMU System Emulation Targets:** BIOS/Multiboot and UEFI bootable pipelines verified

---

## 2. Kernel Core Subsystems

### 2.1 Physical Memory Manager (PMM)
- **Source Files:** `src/mm/pmm.c`, `include/pmm.h`
- **Implementation Status:** Operational and verified.
- **Architecture:** Bitmap-based physical frame allocator managing 4096-byte (4 KiB) pages.
- **Capabilities:**
  - Fast first-fit single page allocation (`pmm_alloc_page()`).
  - Contiguous physical page allocation (`pmm_alloc_contiguous(count)`).
  - Page reference tracking and deallocation (`pmm_free_page()`).
  - Reserve ranges for kernel text/data, low memory (0–1 MiB BIOS/BDA/EBDA), APIC/IOAPIC, and ACPI tables.
  - Overflow-safe address arithmetic and boundary enforcement.
- **Verification:** `tests/test_pmm.c` passes 9/9 test cases including double-free rejection, out-of-bounds guards, and unaligned deallocation traps.

### 2.2 Virtual Memory Manager (VMM)
- **Source Files:** `src/mm/vmm.c`, `include/vmm.h`
- **Implementation Status:** Operational and verified.
- **Architecture:** 4-level paging (PML4 -> PDPT -> PD -> PT) on x86-64.
- **Capabilities:**
  - Canonical address enforcement (`vmm_is_canonical()`).
  - 4 KiB and 2 MiB huge page mappings (`vmm_map_page()`, `vmm_map_huge_page()`).
  - W^X (Write XOR Execute) memory permission enforcement.
  - Page fault handler decoding CR2, error codes (US, RW, P, NX).
  - Kernel higher-half direct mapping (`0xFFFF800000000000` base).
- **Verification:** `tests/test_vmm.c` passes 5/5 test cases including huge page splitting and NX protection.

### 2.3 Task Scheduler (AegisSched_64)
- **Source Files:** `src/kernel/task.c`, `src/kernel/switch.S`, `include/task.h`
- **Implementation Status:** Operational and verified.
- **Architecture:** Priority-based preemptive round-robin scheduler with CPU core affinity.
- **Capabilities:**
  - Tasks represent kernel and user threads with dedicated kernel stacks.
  - Multiple priority levels with dynamic time slice assignment.
  - Calibrated preemptive timer switching via Local APIC timer and PIT fallback.
  - Context switching in assembly (`src/kernel/switch.S`) preserving callee-saved registers (RBX, RBP, R12-R15).
  - Cooperative yield (`task_yield()`) and termination (`task_exit()`).
- **Verification:** `tests/test_scheduler.c` passes 6/6 test cases.

### 2.4 Interrupt & Exception Infrastructure
- **Source Files:** `src/kernel/idt.c`, `src/kernel/interrupt.S`, `src/kernel/pic.c`, `src/drivers/apic.c`, `include/idt.h`, `include/apic.h`, `include/pic.h`
- **Implementation Status:** Operational.
- **Architecture:** 256-entry Interrupt Descriptor Table (IDT) with 64-bit interrupt gates.
- **Capabilities:**
  - Dedicated handlers for CPU exceptions (0–31) with register state dumps and panic traces.
  - Dual 8259 PIC initialization and legacy masking.
  - Local APIC (LAPIC) and I/O APIC discovery and configuration via ACPI MADT.
  - MSI (Message Signaled Interrupts) capability probing for PCI devices.

---

## 3. Storage & Partitioning Subsystem

### 3.1 Unified Block Device Layer
- **Source Files:** `src/drivers/block_dev.c`, `include/block_dev.h`
- **Implementation Status:** Operational and verified.
- **Architecture:** Object-oriented block device registration (`block_device_t`) with 64-bit sector addressing (LBA64), standardized read/write/flush operations, and reference tracking.
- **Capabilities:**
  - Device naming (`ata0`, `ahci0p0`, `nvme0n1`, `part0`).
  - Read-only protection flag.
  - Sector size abstraction (default 512 bytes, configurable to 4096 bytes).
  - Capacity reporting in 64-bit sector counts.
- **Verification:** `tests/test_block_dev.c` passes 5/5 test cases.

### 3.2 ATA PIO Legacy Controller
- **Source Files:** `src/drivers/storage/ata_pio.c`, `include/ata_pio.h`
- **Implementation Status:** Operational.
- **Capabilities:** Primary and secondary IDE channel support (ports `0x1F0`/`0x170`), 28-bit and 48-bit LBA sector I/O, device identify commands (`0xEC`).

### 3.3 AHCI SATA Controller
- **Source Files:** `src/drivers/storage/ahci.c`, `include/ahci.h`
- **Implementation Status:** Operational and verified.
- **Capabilities:**
  - PCI class `0x01` subclass `0x06` prog-IF `0x01` enumeration.
  - ABAR (BAR5) MMIO mapping, port rebase, command list, and Command Table allocation.
  - Native Command Queuing (NCQ) and FIS-based data transfer.
  - 64-bit LBA reads and writes via FIS Type `0x27` (Register Host to Device).
  - Cache flush command (`0xEA` FLUSH CACHE EXT).
- **Verification:** `tests/test_ahci.c` passes 3/3 test cases.

### 3.4 NVMe PCIe Controller
- **Source Files:** `src/drivers/storage/nvme.c`, `include/nvme.h`
- **Implementation Status:** Operational and verified.
- **Capabilities:**
  - PCI class `0x01` subclass `0x08` prog-IF `0x02` enumeration.
  - Controller Registers (CAP, CC, CSTS, AQA, ASQ, ACQ) MMIO access.
  - Admin Queue and I/O Submission/Completion Queue pair creation.
  - Identify Controller and Identify Namespace commands.
  - 64-bit LBA read and write submission with completion doorbell ringing.
- **Verification:** `tests/test_nvme.c` passes 3/3 test cases.

### 3.5 Partition Device & GPT / MBR Parsing
- **Source Files:** `src/fs/gpt.c`, `src/fs/partition_dev.c`, `include/gpt.h`, `include/partition_dev.h`
- **Implementation Status:** Operational and verified.
- **Capabilities:**
  - GUID Partition Table (GPT) header validation, CRC32 checks, partition entry array traversal.
  - Protective MBR detection.
  - Sub-slice block device creation (`partition_dev_create()`) translating relative partition sectors to absolute parent disk LBAs.
  - Subtraction-based overflow protection for partition boundaries.
- **Verification:** `tests/test_storage_gpt.c` passes 8/8 test cases.

---

## 4. Filesystem Architecture

### 4.1 PurpleFS v2 (Verified, Stable, Frozen)
- **Source Files:** `src/fs/purplefs_v2.c`, `include/purplefs_v2.h`
- **Implementation Status:** Production verified (frozen).
- **Parameters:**
  - Magic: `0x50555250` ("PURP")
  - Maximum Volume Size: 128 MiB (32,768 blocks of 4096 bytes)
  - Max Inodes / Files: 128 root entries (flat directory layout)
  - Direct Pointers: 12 (48 KiB direct)
  - Single Indirect: 1 pointer to 1024 32-bit block addresses (4 MiB indirect)
  - Maximum File Size: 4,243,456 bytes (4.047 MiB)
  - Journal: 16-block ring journal with CRC32 metadata transaction commit
- **Verification:** `tests/test_purplefs_v2.c` passes 25/25 comprehensive test cases with fault injection and persistence verification.

### 4.2 PurpleFS v3 (Architecture Defined, Implementation Next)
- **Specification Document:** `docs/PURPLEFS_V3_64BIT_ADDRESSING_REVIEW.md`, `docs/PURPLE_OS_BOOTABLE_IMAGE_ARCHITECTURE_PLAN.md`
- **Target Implementation Files:** `include/fs/purplefs_v3.h`, `src/fs/purplefs_v3.c`, `tests/test_purplefs_v3.c`
- **Key Features:**
  - Magic: `0x50465333` ("PFS3")
  - Block Size: 4096 bytes (8 x 512-byte disk sectors)
  - Addressing: Full 64-bit Logical Block Addressing (`uint64_t`)
  - Maximum Theoretical Capacity: 134,480,396 blocks = 550,831,702,016 bytes (~513.002 GiB)
  - Partition 2 Usable Data Capacity: 7,722,256 blocks = 31,630,360,576 bytes (~29.458 GiB)
  - Inode Structure: 256 bytes per inode, 16 inodes per 4096-byte block
  - Hierarchical Directories: Structured 64-byte directory records (`pfs3_dirent_t`) with `.` and `..` support
  - Multi-block allocation bitmaps supporting tens of millions of blocks
  - Transactional metadata journaling for crash consistency

---

## 5. Hardware Detection & Security

### 5.1 ACPI Subsystem
- **Source Files:** `src/kernel/acpi.c`, `include/acpi.h`
- **Implementation Status:** Operational.
- **Capabilities:**
  - Searches for Root System Description Pointer (RSDP) in EBDA and BIOS read-only region (`0xE0000` to `0xFFFFF`).
  - RSDT (32-bit) and XSDT (64-bit) parsing with table checksum validation.
  - Discovers MADT (Multiple APIC Description Table) for SMP CPU enumeration and I/O APIC base addresses.
  - Discovers FADT (Fixed ACPI Description Table) for power management and reboot/shutdown ports.
- **Verification:** `tests/test_acpi.c` passes 3/3 test cases.

### 5.2 AegisDetect Subsystem
- **Source Files:** `src/security/aegis_detect.c`, `include/security/aegis_detect.h`
- **Implementation Status:** Operational.
- **Capabilities:**
  - Full PCI Configuration Space scanning (buses 0–255, devices 0–31, functions 0–7).
  - Vendor ID and Device ID matching against hardware database.
  - Memory BAR and I/O BAR parsing.
  - Security integrity checks and hardware signature classification.
- **Verification:** `tests/test_aegis_detect.c` passes 4/4 test cases.

### 5.3 PurpleDetect Subsystem
- **Source Files:** `src/drivers/purple_detect.c`, `include/purple_detect.h`
- **Implementation Status:** Operational.
- **Capabilities:**
  - Discovers system controllers (PCI, AHCI, NVMe, USB, Audio, Network).
  - Populates unified hardware inventory report for the user and shell.
  - Queries driver matching engine for unattached hardware.
- **Verification:** `tests/test_purple_detect.c` passes 3/3 test cases.

---

## 6. Native Driver Subsystems

### 6.1 APDRV v3 Loader & Kernel Symbol Table (KSYM)
- **Source Files:** `src/drivers/apdrv.c`, `src/kernel/ksym.c`, `include/apdrv.h`, `include/ksym.h`
- **Implementation Status:** Operational and verified.
- **Architecture:** Purple OS native relocatable driver format (`.apdrv`).
- **Capabilities:**
  - Package header validation (`0x41504452` "APDR").
  - Target architecture verification (x86-64).
  - Symbol resolution against kernel symbol table (`ksym_resolve()`).
  - Memory allocation, segment copying, and execution of driver `init()` and `exit()` entrypoints.
  - Safe driver unload and dependency tracking.
- **Verification:** `tests/test_apdrv.c` and `tests/test_drivers.c` pass all test cases.

### 6.2 Peripheral Hardware Drivers
- **Real-Time Clock (RTC):** `src/drivers/rtc.c` — CMOS century/year/month/day/hour/minute/second reading with BCD decoding.
- **Ethernet (Intel E1000 82540EM):** `src/drivers/e1000.c` — PCI probe, MMIO configuration, MAC address reading from EEPROM, RX/TX circular ring buffers, packet transmission.
- **USB Controller (xHCI):** `src/drivers/usb.c` — PCI discovery, operational register base mapping, USB root hub enumeration.
- **Intel High Definition Audio (HDA):** `src/drivers/audio.c` — PCI discovery, GCAP/GCTL configuration, codec discovery on link.

---

## 7. Graphical Desktop Subsystem

### 7.1 Video Framebuffer & Font Rendering
- **Source Files:** `src/drivers/video.c`, `src/drivers/font8x16.c`, `include/video.h`, `include/font8x16.h`
- **Implementation Status:** Operational and verified.
- **Capabilities:**
  - Multiboot 1 linear framebuffer initialization (typically 1024x768x32bpp or 1280x800x32bpp).
  - Pixel plotting, rectangle fill, line drawing, and blitting.
  - 8x16 bitmap font rendering (`font8x16`) with full printable ASCII glyphs.
  - Hardware/software cursor drawing.
  - Off-screen double buffer allocation and screen swap to eliminate flicker.
- **Verification:** `tests/test_video.c` passes 6/6 test cases.

### 7.2 Desktop Window Manager
- **Source Files:** `src/drivers/desktop.c`, `include/desktop.h`, `src/drivers/mouse.c`, `include/mouse.h`
- **Implementation Status:** Operational and verified.
- **Capabilities:**
  - Top system menu bar with Purple OS branding, clock display, and application launcher menu.
  - Multi-window management: title bars, close buttons, active window focus, z-order stacking.
  - Smooth window dragging via mouse click-and-drag.
  - 16x11 hardware/software arrow cursor with background save-and-restore rendering.
  - Event dispatch: mouse moves, left/right clicks, key presses routed to the focused window.
- **Verification:** `tests/test_desktop.c` passes 6/6 test cases.

---

## 8. Command-Line Interface (Purple Shell)

### 8.1 Purple Shell Core
- **Source Files:** `src/kernel/shell.c`, `include/shell.h`
- **Implementation Status:** Operational and verified.
- **Capabilities:**
  - Interactive CLI supporting command parsing, tokenization, and arguments.
  - Normalization of file paths (`welcome.txt`, `./welcome.txt`, `/welcome.txt`) resolving uniformly to root-level entries under PurpleFS v2.
  - Built-in command suite: `help`, `clear`, `version`, `uname`, `uptime`, `tasks`, `kill`, `mem`, `pci`, `disks`, `ls`, `cat`, `peek`, `write`, `rm`, `format`, `launch`, `detect`.
- **Verification:** `tests/test_shell_path.c` passes 10/10 test cases.

---

## 9. Application Subsystem (`.apapplication`)

### 9.1 Application Bundle Architecture
- **Source Files:** `src/kernel/apapp.c`, `include/apapp.h`
- **Implementation Status:** Operational and verified.
- **Architecture:** Self-contained application packages (`.apapplication`) containing metadata manifest, executable payload, and graphical window configuration.
- **Capabilities:**
  - Magic header `0x41505031` ("APP1").
  - Manifest parsing: name, version, author, requested memory, window size (width, height), entrypoint.
  - Cryptographic security verification using SHA-256 digest (`src/crypto/sha256.c`).
  - Spawns task into scheduler and registers graphical window with Desktop subsystem.
- **Verification:** `tests/test_apapp.c` passes 5/5 test cases.

---

## 10. Networking Subsystem

### 10.1 Network Device & Packet Processing
- **Source Files:** `src/drivers/netdev.c`, `src/drivers/e1000.c`, `include/netdev.h`, `include/e1000.h`
- **Implementation Status:** Foundations established.
- **Capabilities:**
  - `netdev_register()` establishes network interface with MAC address and MTU (1500 bytes).
  - Packet transmission via E1000 transmit ring buffer.
  - Packet reception via circular ring descriptors with length bounds checking.
  - Architecture ready for ARP table resolution, IPv4 framing, and ICMP echo responder.

---

## 11. Ranked Risk & Issue Matrix

| Priority | Subsystem | Issue / Opportunity | Impact | Mitigation Strategy |
| :--- | :--- | :--- | :--- | :--- |
| **High** | Filesystem | PurpleFS v2 limited to 128 MiB and flat 128 files | Cannot host modern OS hierarchy (`/System`, `/Boot`, `/Users`) or large files | Implement PurpleFS v3 with 64-bit LBAs, 256-byte inodes, structured directory records, and multi-block bitmaps. |
| **Medium** | Shell | Shell lacks directory traversal commands (`mkdir`, `cd`, `pwd`) | PurpleFS v3 hierarchical directories will require shell navigation | Expand `src/kernel/shell.c` with directory manipulation and navigation commands. |
| **Medium** | Applications | Application packaging scripts and native userland bundles need expansion | Only synthetic app bundles tested in test harness | Author `docs/APAPPLICATION_SPECIFICATION.md`, build host validation tool `scripts/validate_apapp.py`, and package native userland bundles (`terminal`, `sysmon`, `files`). |
| **Low** | Networking | ARP and ICMP ping responder need higher-level integration in shell | Network interface is functional at L2, but user cannot ping host | Implement ARP cache and ICMP echo reply in `netdev.c` / `e1000.c` with shell `ping` command. |
| **Low** | Audio | Intel HDA audio codec enumeration is functional, PCM stream playback not exposed | Sound subsystem silent | Add PCM stream DMA buffer helper and shell `beep` / `play` test command. |

---

## 12. Engineering Plan & Next Phases

1. **Phase B — PurpleFS v3 Implementation:**
   - Author `include/fs/purplefs_v3.h` defining 64-bit on-disk structures (`pfs3_superblock_t`, `pfs3_inode_t`, `pfs3_dirent_t`, `pfs3_indirect_block_t`, `pfs3_journal_header_t`).
   - Author `src/fs/purplefs_v3.c` implementing formatting, superblock mounting, block bounds checking with subtraction arithmetic, inode read/write, hierarchical path traversal (`.` and `..`), and directory manipulation.
   - Author `tests/test_purplefs_v3.c` and integrate into `Makefile` test suite.
2. **Phase C — Storage & Partition Integration:**
   - Integrate PurpleFS v3 with Unified Block Device layer (`block_device_t`) and Partition Device layer (`partition_dev_t`).
3. **Phase I — Application Subsystem (`.apapplication`):**
   - Formalize `.apapplication` specification, create host packaging and validation tool, and author sample native bundles.
4. **Phase H — Purple Shell Usability Expansion:**
   - Add directory commands (`mkdir`, `cd`, `pwd`), enhanced hardware inspection, and system metrics.
5. **Phase L — Networking Engine:**
   - Complete ARP table and ICMP ping responder.
6. **Continuous Verification:**
   - Validate every phase with host test suite and AddressSanitizer.
