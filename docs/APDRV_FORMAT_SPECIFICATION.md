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
+-----------------------------------------------------------+ [EOF]
|   (.bss Section is allocated in memory; 0 bytes in file)  |
+-----------------------------------------------------------+
```

---

## 3. Header Specification (`ApdrvHeader`)

The header occupies exactly **240 bytes** (`0xF0`) and is packed with no internal compiler padding:

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

    /* Hardware PCI Matcher (0xFFFF = wildcard / all devices) */
    pub target_vendor: u16,        // 0xE0: Target PCI Vendor ID (e.g. 0x8086)
    pub target_device: u16,        // 0xE2: Target PCI Device ID (e.g. 0x100E)
    pub flags: u32,                // 0xE4: APDRV_FLAG_*
    pub checksum: u32,             // 0xE8: Fletcher-32 or CRC32 checksum
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
