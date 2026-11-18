# Engineering Journal: PurpleDetect Hardware-Driven APDRV Bundle Matching and Auto-Loading

**Date:** 18-11-2026  
**Target:** PurpleOS / Aegis64 Pure Bare-Metal Rust Kernel (`x86_64-unknown-none`)  
**Status:** Completed, 100% Pure Rust, 100% Automated Multi-Session Tests Passing  

---

## 1. Executive Summary & Directive

In accordance with user directives, **PurpleDetect** (`src/drivers/purple_detect.rs`) now actively scans hardware on every system boot and dynamically matches discovered physical peripherals and system architecture against the native **APDRV Driver Bundle** (`src/kernel/apdrv.rs`). Unconditional static loading has been completely eliminated: only drivers corresponding to actively discovered hardware are loaded into memory with hardware $W\oplus X$ protection. Drivers for non-present hardware (e.g., sound cards or USB controllers on unequipped systems) are safely skipped.

---

## 2. Technical Architecture & Dynamic Matching Engine

### A. APDRV Driver Bundle Definition ([`src/kernel/apdrv.rs`](file:///data/data/com.termux/files/home/OS%20Development/OS/src/kernel/apdrv.rs))

Each loadable module within the APDRV package is defined with strict loading prerequisites via the `DriverCondition` enumeration:

```rust
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum DriverCondition {
    Core,                                                    // Always loaded (Architecture / Memory / VFS)
    Category(crate::drivers::purple_detect::DeviceCategory), // Requires active hardware in this category
    PciDevice(u16, u16),                                     // Requires specific PCI vendor & device ID
}
```

The unified bundle matrix maps drivers to their respective hardware requirements:

| Driver Package | Condition | Hardware Detection Criteria | Action if Present | Action if Absent |
| :--- | :--- | :--- | :--- | :--- |
| `cpu.apdrv` | `Core` | x86-64 Architecture & SMP | **LOAD** | N/A (Core) |
| `ram.apdrv` | `Core` | Physical & Virtual Memory Manager | **LOAD** | N/A (Core) |
| `filesystems.apdrv` | `Core` | VFS & PurpleFS Storage Subsystem | **LOAD** | N/A (Core) |
| `storage.apdrv` | `Category(Storage)` | Primary/Secondary IDE or PCI AHCI/NVMe | **LOAD** | Skipped |
| `input.apdrv` | `Category(Input)` | PS/2 Keyboard/Mouse or USB HID | **LOAD** | Skipped |
| `gpu.apdrv` | `Category(Display)` | PCI VGA (`1234:1111`) / Intel IGD / AMD / Nvidia | **LOAD** | Skipped |
| `network.apdrv` | `Category(Network)` | Intel 82540EM (`8086:100E`) / Realtek RTL8139 | **LOAD** | Skipped |
| `audio.apdrv` | `Category(Audio)` | Intel HD Audio (`8086:2668`) / AC97 Sound | **LOAD** | **SKIPPED** |
| `usb.apdrv` | `Category(Bus)` | xHCI (`1B36:000D`) / EHCI / UHCI Controller | **LOAD** | **SKIPPED** |

---

### B. PurpleDetect Discovery & Interrogation ([`src/drivers/purple_detect.rs`](file:///data/data/com.termux/files/home/OS%20Development/OS/src/drivers/purple_detect.rs))

1. **Hardware Telemetry Queries**:
   - `has_category(cat: DeviceCategory) -> bool`: Checks whether active devices of the given category were discovered during the boot inventory.
   - `has_pci_device(vendor_id: u16, device_id: u16) -> bool`: Queries the discovered PCI configuration space for specific hardware IDs.
   - `has_pci_class(class_id: u8) -> bool`: Matches PCI class codes (e.g. `0x01` Storage, `0x02` Network, `0x03` Display, `0x04` Audio, `0x0C` USB).

2. **Dynamic Re-scan Capability ([`src/kernel/shell.rs`](file:///data/data/com.termux/files/home/OS%20Development/OS/src/kernel/shell.rs))**:
   - `devices scan` or `hw scan` triggers an on-demand hardware re-scan and driver auto-binding cycle without requiring a system reboot.

3. **Idempotent Module Reload Protection ([`src/kernel/apdrv.rs`](file:///data/data/com.termux/files/home/OS%20Development/OS/src/kernel/apdrv.rs))**:
   - In `load_binary()`, the registry is checked prior to physical page allocation. If the module is already in `RUNNING` state, redundant memory allocation is avoided.

---

## 3. Automated Test Verification Results

### Suite: `scripts/test_qemu_hardware_driver_matching.py`

- **Session 1 (Standard Hardware — Audio & USB Absent)**:
  - `cpu.apdrv`, `ram.apdrv`, `filesystems.apdrv` -> **LOADED** (Core)
  - `storage.apdrv`, `input.apdrv`, `gpu.apdrv`, `network.apdrv` -> **LOADED** (Matched active hardware)
  - `audio.apdrv`, `usb.apdrv` -> **SKIPPED** ("No matching hardware detected — skipped.")
  - `lsmod` output verified: Exactly 7 active modules loaded; `audio` and `usb` absent.
  - Result: **PASS (100%)**

- **Session 2 (Attached Hardware — Intel HDA Audio & xHCI USB Present)**:
  - QEMU launched with `-device intel-hda -device hda-duplex -device qemu-xhci`.
  - PurpleDetect discovered Audio (`8086:2668`) and USB Bus (`1b36:000d`).
  - `audio.apdrv` and `usb.apdrv` -> **LOADED** automatically!
  - `lsmod` output verified: Exactly 9 active modules loaded; `audio` and `usb` both `RUNNING`.
  - Result: **PASS (100%)**

### Full Regression Verification

| Test Suite | Commands / Checks | Result |
| :--- | :--- | :--- |
| `scripts/test_qemu_hardware_driver_matching.py` | 2 multi-device cold boot sessions | **100% PASS** |
| `scripts/test_qemu_storage.py` | ATA PIO, GPT, PFS2 mount & telemetry | **100% PASS** |
| `scripts/test_qemu_filesystems_apdrv.py` | Filesystems bundle, PurpleDetect registry & insmod | **100% PASS** |
| `scripts/test_all_commands.py` | 124 interactive shell commands | **124 / 124 PASS (100%)** |
| `scripts/test_qemu_dir_persistence.py` | 3-boot cold reboot directory persistence | **100% PASS** |
