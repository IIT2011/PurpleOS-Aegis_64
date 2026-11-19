# APDRV v3 Native Kernel Module Format Specification

**Document Version:** 3.0.0  
**Target Architecture:** x86_64 (`x86_64-unknown-none`)  
**Operating System:** Purple OS / Aegis64 Monolithic Kernel  
**Status:** Canonical Native Driver Specification  

---

## 1. Overview & Purpose

The **`.apdrv`** (Aegis Peripheral Driver) format is the native relocatable binary package format for loadable kernel modules in Purple OS / Aegis64. Designed specifically for bare-metal x86-64 execution, `.apdrv` provides:

1. **Deterministic Relocatable Loading:** Fixed section-based dynamic linking against kernel symbols (`ksym`).
2. **Hardware Auto-Probing:** Declarative PCI Vendor and Device matching (`target_vendor`, `target_device`).
3. **Hardware-Enforced $W \oplus X$ Memory Protection:** Independent page-aligned sections ensuring executable pages are never writable, and writable pages are never executable.
4. **Standardized Driver ABI:** Unified entry points for module lifecycle (`init`, `fini`), device binding (`probe`), and control dispatching (`ioctl`).

---

## 2. Binary Layout

An `.apdrv` v3 file is a contiguous binary file consisting of a fixed 240-byte header followed by tables and code/data sections:

```
+-----------------------------------------------------------+ 0x00000000
|                ApdrvHeader (240 bytes)                    |
+-----------------------------------------------------------+ import_offset
|            Symbol Import Table (apdrv_sym_t[])            |
+-----------------------------------------------------------+ reloc_offset
|            Relocation Table (apdrv_reloc_t[])             |
+-----------------------------------------------------------+ text_offset
|            .text Section (x86-64 Machine Code)            |
+-----------------------------------------------------------+ rodata_offset
|            .rodata Section (Read-Only Data & Strings)     |
+-----------------------------------------------------------+ data_offset
|            .data Section (Initialized Writable Data)      |
+-----------------------------------------------------------+
|   (.bss Section is allocated in memory; 0 bytes in file)  |
+-----------------------------------------------------------+
| 512 ASCII hex bytes: RSA-2048 PKCS#1 v1.5 SHA-256 signature|
+-----------------------------------------------------------+ [EOF]
```

---

## 3. Header Specification (`ApdrvHeader`)

The header occupies exactly **240 bytes** (`0xF0`) and is packed with no internal compiler padding. The `checksum` field contains CRC-32/ISO-HDLC of the package payload with the checksum field set to zero. A package ends with exactly 512 ASCII hexadecimal bytes containing an RSA-2048 PKCS#1 v1.5 SHA-256 signature over every preceding package byte, including the populated checksum, header, and tables. The kernel rejects packages without a valid trailer. Build signed packages with `scripts/build_apdrv.py <output-dir> --signing-key <private-key-path>`; keep the private key outside the repository. Unsigned output from the builder is for inspection only and cannot be loaded.

```rust
#[repr(C, packed)]
pub struct ApdrvHeader {
    pub magic: [u8; 8],            // 0x00: "APDRV100" (8 bytes)
    pub version: u32,              // 0x08: Format version (3)
    pub header_size: u32,          // 0x0C: Size of header (240 bytes)
    pub mod_type: [u8; 8],         // 0x10: "AEGISMOD" (8 bytes)
    pub name: [u8; 32],            // 0x18: Driver identifier name (null-terminated UTF-8)
    pub description: [u8; 64],     // 0x38: Human-readable description
    pub author: [u8; 32],          // 0x78: Author / Organization
    pub mod_version: u32,          // 0x98: e.g. 0x010000 = 1.0.0

    /* Section Offsets in File & Sizes in Bytes */
    pub text_offset: u32,          // 0x9C: Offset to .text machine code
    pub text_size: u32,            // 0xA0: Size of .text in bytes
    pub rodata_offset: u32,        // 0xA4: Offset to .rodata
    pub rodata_size: u32,          // 0xA8: Size of .rodata in bytes
    pub data_offset: u32,          // 0xAC: Offset to .data
    pub data_size: u32,            // 0xB0: Size of .data in bytes
    pub bss_size: u32,             // 0xB4: Size of uninitialized .bss in RAM

    /* Relocation Table */
    pub reloc_offset: u32,         // 0xB8: File offset to relocation table
    pub reloc_count: u32,          // 0xBC: Number of relocation entries

    /* Symbol Import/Export Tables */
    pub import_offset: u32,        // 0xC0: File offset to import symbol table
    pub import_count: u32,         // 0xC4: Number of imported symbols
    pub export_offset: u32,        // 0xC8: File offset to exported symbols
    pub export_count: u32,         // 0xCC: Number of exported symbols

    /* Entry Point Offsets (relative to .text base) */
    pub entry_init: u32,           // 0xD0: extern "C" fn() -> i32
    pub entry_fini: u32,           // 0xD4: extern "C" fn() -> i32
    pub entry_probe: u32,          // 0xD8: extern "C" fn(vendor: u32, device: u32) -> i32
    pub entry_ioctl: u32,          // 0xDC: extern "C" fn(cmd: u32, arg: *mut u8) -> i32
    // Entry offsets are relative to .text; 0 is valid. Use 0xFFFFFFFF when absent.

    /* Hardware PCI Matcher (0xFFFF = wildcard / all devices) */
    pub target_vendor: u16,        // 0xE0: Target PCI Vendor ID (e.g. 0x8086)
    pub target_device: u16,        // 0xE2: Target PCI Device ID (e.g. 0x100E)
    pub flags: u32,                // 0xE4: APDRV_FLAG_*
    pub checksum: u32,             // 0xE8: CRC-32/ISO-HDLC over signed payload with this field zeroed
    pub reserved: u32,             // 0xEC: Alignment padding to 240 bytes
}
```

### Magic Constants & Identifiers
- `magic`: `b"APDRV100"` (ASCII string identifying version 1.0 series specification)
- `version`: `3` (Format revision 3 with $W \oplus X$ alignment support)
- `mod_type`: `b"AEGISMOD"` (Kernel loadable module; distinct from user `APAPP` bundles)

---

## 4. Symbol Import Table (`ApdrvSymbol`)

Each symbol entry occupies **40 bytes**:

```rust
#[repr(C, packed)]
pub struct ApdrvSymbol {
    pub name: [u8; 32],            // Symbol name (null-terminated string, max 31 chars)
    pub address: u64,              // Resolved runtime memory address (filled by loader)
}
```

During module load, the kernel iterates through `import_count` entries and resolves each name against the Kernel Symbol Table (`ksym`). If any symbol cannot be found, the loader halts and fails closed (`-10: Unresolved Symbol`).

Standard kernel symbols available for import:
- `kinfo`: Kernel diagnostic logging
- `block_dev_register`: Registers a `BlockDevice` driver
- `netdev_register`: Registers a `NetDevice` driver
- `pci_enable_bus_mastering`: Enables DMA bus mastering for a PCI device
- `alloc_coherent` / `free_coherent`: Allocates contiguous physical DMA memory

---

## 5. Relocation Engine (`ApdrvReloc`)

Each relocation record occupies **20 bytes**:

```rust
#[repr(C, packed)]
pub struct ApdrvReloc {
    pub offset: u32,               // Byte offset in loaded module memory image
    pub reloc_type: u32,           // APDRV_RELOC_*
    pub sym_index: u32,            // Import table index OR Section Base Index
    pub addend: i64,               // 64-bit signed addend
}
```

### Relocation Types
| Constant | Value | Description | Relocation Formula |
| :--- | :--- | :--- | :--- |
| `APDRV_RELOC_NONE` | `0` | No-op relocation | None |
| `APDRV_RELOC_64` | `1` | 64-bit Absolute Relocation | `*(u64*)target = sym + addend` |
| `APDRV_RELOC_PC32` | `2` | 32-bit PC-Relative Offset | `*(i32*)target = sym + addend - target` |
| `APDRV_RELOC_PLT32` | `4` | 32-bit Procedure Linkage Offset | `*(i32*)target = sym + addend - target` |
| `APDRV_RELOC_32` | `10` | 32-bit Absolute Relocation | `*(u32*)target = (sym + addend) as u32` |

### Section Base Pseudo-Indices
When `sym_index` has a value $\ge \text{0xFFF0}$, it refers directly to an in-memory section base address rather than an import table symbol:
- `APDRV_SEC_TEXT` (`0xFFF0`): Base memory address of loaded `.text`
- `APDRV_SEC_RODATA` (`0xFFF1`): Base memory address of loaded `.rodata`
- `APDRV_SEC_DATA` (`0xFFF2`): Base memory address of loaded `.data`
- `APDRV_SEC_BSS` (`0xFFF3`): Base memory address of allocated `.bss`
- `APDRV_SEC_MODULE` (`0xFFF4`): Base memory address of the entire module image

---

## 6. Flags & Hardware Auto-Binding

```rust
pub const APDRV_FLAG_ENFORCE_WX: u32 = 0x0001; // Module requires page-aligned W^X enforcement
pub const APDRV_FLAG_AUTO_PROBE: u32 = 0x0002; // Automatically probe against PCI bus
```

### Hardware Auto-Binding Sequence:
1. When a PCI bus scan discovers a device $(V, D)$:
2. The kernel queries each running module with `APDRV_FLAG_AUTO_PROBE`.
3. If `(target_vendor == 0xFFFF || target_vendor == V) && (target_device == 0xFFFF || target_device == D)`:
4. The kernel executes `entry_probe(V, D)`.
5. If `entry_probe` returns `0`, the module claims and binds the device (`bound_devices += 1`).

---

## 7. Memory Layout & $W \oplus X$ Protection

When an `.apdrv` binary is loaded into memory, the kernel calculates separate page-aligned allocations for each section:

```
               IN-MEMORY PAGE-ALIGNED MODULE LAYOUT
Base ──► ┌──────────────────────────────────────────┐
         │              .text Section               │  Permissions: RX
         │  (Code, entry_init, entry_probe, etc.)   │  (PAGE_PRESENT | NO_EXECUTE=0 | WRITABLE=0)
4 KiB ──► ├──────────────────────────────────────────┤
         │              .rodata Section             │  Permissions: R
         │  (Strings, constants, descriptor tables) │  (PAGE_PRESENT | NO_EXECUTE=1 | WRITABLE=0)
8 KiB ──► ├──────────────────────────────────────────┤
         │              .data Section               │  Permissions: RW
         │  (Initialized global writable state)     │  (PAGE_PRESENT | NO_EXECUTE=1 | WRITABLE=1)
         ├──────────────────────────────────────────┤
         │              .bss Section                │  Permissions: RW
         │  (Zeroed uninitialized memory)           │  (PAGE_PRESENT | NO_EXECUTE=1 | WRITABLE=1)
End  ───► └──────────────────────────────────────────┘
```

Strict enforcement ensures that **no page in memory can ever be simultaneously writable and executable**, preventing buffer overflow and code injection exploits at the hardware MMU level.

---

## 8. Canonical Package Naming Hierarchy & Directory Structure

Purple OS / Aegis64 establishes a formal two-tier naming architecture for all `.apdrv` modules:

### 8.1. Subsystem Family Packages (Canonical Functional Classes)
For unified device stacks that aggregate drivers for an entire peripheral class, canonical title-case and lowercase filenames are standardized:

| Canonical Family Name | Lowercase Alias | Target Hardware Class | Target PCI Class / Subclass |
| :--- | :--- | :--- | :--- |
| **`Storage.apdrv`** | `storage.apdrv` | SATA AHCI 1.3, NVMe 1.4, Legacy ATA PIO | `0x01` (Mass Storage) |
| **`Network.apdrv`** | `network.apdrv` / `ethernet.apdrv` | PCIe Wired Ethernet (Intel, Realtek) | `0x02:0x00` (Ethernet) |
| **`WiFi.apdrv`** | `wifi.apdrv` | 802.11 Wireless Network Adapters | `0x02:0x80` (Wireless Controller) |
| **`USB.apdrv`** | `usb.apdrv` | USB Host Controllers (xHCI 3.0, EHCI 2.0) | `0x0C:0x03` (Serial Bus / USB) |
| **`Audio.apdrv`** | `audio.apdrv` | High Definition Audio & AC97 Codecs | `0x04:0x03` (Multimedia Audio) |
| **`Graphics.apdrv`** | `graphics.apdrv` | GPU Display Adapters & Accelerators | `0x03:0x00` (VGA Compatible) |
| **`Input.apdrv`** | `input.apdrv` | PS/2 8042 Keyboard, Mouse & USB HID | Wildcard (`0xFFFF:0xFFFF`) |
| **`Bluetooth.apdrv`** | `bluetooth.apdrv` | Bluetooth HCI Host Controllers (USB/UART) | `0x0D:0x11` (Wireless Bluetooth) |
| **`Camera.apdrv`** | `camera.apdrv` | USB Video Class (UVC) Webcams & Capture | `0x0E:0x02` (Video Streaming) |

### 8.2. Chipset-Specific Packages
When specialized hardware registers, proprietary DMA ring engines, or vendor microcode are required, dedicated chipset packages are named after the controller:
- **`e1000.apdrv`**: Intel 82540EM / 82545EM Gigabit Ethernet (`8086:100E`, `8086:100F`)
- **`r8139.apdrv`**: Realtek RTL8139 Fast Ethernet (`10EC:8139`)
- **`xhci.apdrv`**: Extensible Host Controller Interface USB 3.0 (`0C:03:30`)
- **`hdaudio.apdrv`**: Intel High Definition Audio (`8086:2668`)
- **`bochs_gpu.apdrv`**: Bochs / QEMU DRM VGA Video Adapter (`1234:1111`)
- **`gpu-intel.apdrv`**: Intel HD Graphics Gen9/Gen11 Accelerator (`8086:5912`)
- **`gpu-amd.apdrv`**: AMD Radeon GCN/RDNA Blit Accelerator (`1002:67DF`)

### 8.3. Directory Structure on PurpleFS
Driver modules reside in canonical paths on the filesystem:
1. **Primary Directory:** `/System/Drivers/`
   ```
   /System/Drivers/
   ├── Storage.apdrv    (or storage.apdrv)
   ├── Network.apdrv    (or network.apdrv, e1000.apdrv, r8139.apdrv)
   ├── WiFi.apdrv       (or wifi.apdrv)
   ├── USB.apdrv        (or usb.apdrv, xhci.apdrv)
   ├── Audio.apdrv      (or audio.apdrv, hdaudio.apdrv)
   ├── Graphics.apdrv   (or graphics.apdrv, bochs_gpu.apdrv)
   ├── Input.apdrv      (or input.apdrv)
   ├── Bluetooth.apdrv  (or bluetooth.apdrv)
   └── Camera.apdrv     (or camera.apdrv)
   ```
2. **Legacy / Root Fallback:** `/` (e.g. `/e1000.apdrv`, `/storage.apdrv`)
3. **Build Artifacts Directory:** `build/drivers_apdrv/` and `build/`

### 8.4. Resolution Precedence
When the kernel loads a module via `insmod <name>` or during PCI auto-probing:
1. **Case-Insensitive Normalization:** Path lookups treat `Storage.apdrv` and `storage.apdrv` as identical.
2. **Specific-First Matching:** If an exact chipset driver exists for the discovered PCI Device ID (e.g. `e1000.apdrv` for `8086:100E`), it is prioritized over generic class drivers.
3. **Family Fallback:** If no chipset-specific driver matches, the generic subsystem family driver (e.g. `Network.apdrv` or `Storage.apdrv` with wildcard `0xFFFF`) is invoked to bind the device.

