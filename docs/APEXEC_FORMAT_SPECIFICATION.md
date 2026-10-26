# APEXEC — PURPLE OS NATIVE EXECUTABLE FORMAT SPECIFICATION (v1.0)

**Document ID:** SPEC-APEXEC-20261025  
**Target Architecture:** x86-64 AMD64 Long Mode  
**Operating System:** Purple OS (Aegis64 Kernel)  
**Status:** Canonical Native Application Binary Specification  

---

## 1. Overview & Objectives

The `.apexec` (Aegis Purple Executable) format is the native binary executable format for Purple OS applications. It is designed to provide:
1. **Explicit Memory Permissions & W^X Enforcement:** Strict separation between executable code and writable data. Any binary attempting to load writable, executable segments is rejected fail-closed.
2. **User Address Space Bounded Isolation:** All virtual address mappings are strictly verified to reside within the canonical userspace bounds (`0x0000000000400000` to `0x00007FFFFFFFFFFF`), preventing any collision with kernel space.
3. **Deterministic Header & Segment Layout:** A clean, 64-byte fixed file header followed by 40-byte segment descriptors.
4. **Integration with `.apapplication` Bundles:** Standalone native `.apexec` binaries are packaged inside `.apapplication` bundles alongside metadata, resources, and AegisDetect cryptographic signatures.

---

## 2. Binary Layout & Structure

A complete `.apexec` binary file consists of:
1. **File Header (64 bytes):** Magic, version, architecture, entry point virtual address, segment count, stack requirement, and integrity checksum.
2. **Segment Table (`num_segments * 40` bytes):** Array of segment descriptors describing virtual memory mapping, permissions, and file offsets.
3. **Segment Payloads:** Raw machine code, string literals, and initialized data.

```
+-------------------------------------------------------+
|  APEX Header (64 bytes)                               |
|  - Magic: 0x4550417F ('\x7F' 'A' 'P' 'E')             |
|  - Version: 1                                         |
|  - Arch: 0x8664 (x86-64)                              |
|  - Entry Point (64-bit Virtual Address)               |
|  - Segment Count (N)                                  |
|  - Stack Size Request                                 |
+-------------------------------------------------------+
|  Segment Descriptor 0 (40 bytes): .text (CODE, R-X)   |
+-------------------------------------------------------+
|  Segment Descriptor 1 (40 bytes): .rodata (RODATA, R--)|
+-------------------------------------------------------+
|  Segment Descriptor 2 (40 bytes): .data (DATA, RW-)   |
+-------------------------------------------------------+
|  Segment Descriptor 3 (40 bytes): .bss (BSS, RW-)     |
+-------------------------------------------------------+
|  Payload: Machine Code (.text)                        |
+-------------------------------------------------------+
|  Payload: Read-Only Strings & Tables (.rodata)        |
+-------------------------------------------------------+
|  Payload: Initialized Global Data (.data)             |
+-------------------------------------------------------+
```

---

## 3. Data Structures

### 3.1 File Header (`apex_header_t` - 64 bytes)

```c
typedef struct {
    uint32_t magic;             /* 0x4550417F ('\x7F', 'A', 'P', 'E') */
    uint8_t  version;           /* Format Version (1) */
    uint8_t  abi_version;       /* Purple OS ABI Version (1) */
    uint16_t arch;              /* Architecture (0x8664 for AMD64) */
    uint32_t header_size;       /* 64 bytes */
    uint64_t entry_point;       /* Virtual address of entry point in userspace */
    uint64_t base_vaddr;        /* Base virtual address of image */
    uint64_t file_size;         /* Total length of file in bytes */
    uint16_t num_segments;      /* Number of segment descriptors (1..16) */
    uint16_t reserved0;         /* Reserved (must be 0) */
    uint32_t stack_size;        /* Requested user stack size (default 64 KiB) */
    uint32_t checksum;          /* CRC32 of payload */
    uint32_t reserved1[3];      /* Padding to 64 bytes */
} __attribute__((packed)) apex_header_t;
```

### 3.2 Segment Descriptor (`apex_segment_t` - 40 bytes)

```c
typedef struct {
    uint32_t type;              /* Segment Type: CODE, RODATA, DATA, BSS */
    uint32_t flags;             /* Permission flags: READ, WRITE, EXEC */
    uint64_t file_offset;       /* Offset of payload in .apexec file */
    uint64_t file_size;         /* Size of payload in file */
    uint64_t vaddr;             /* Target virtual address (page-aligned) */
    uint64_t mem_size;          /* Memory footprint (mem_size >= file_size) */
} __attribute__((packed)) apex_segment_t;
```

### 3.3 Segment Types & Permission Flags

| Type | Value | Standard Segment | Default Permissions | Description |
|:---|:---:|:---|:---:|:---|
| `APEX_SEG_NULL` | 0 | - | - | Unused entry |
| `APEX_SEG_CODE` | 1 | `.text` | `READ \| EXEC` | Executable CPU instructions |
| `APEX_SEG_RODATA` | 2 | `.rodata` | `READ` | Read-only strings & constant tables |
| `APEX_SEG_DATA` | 3 | `.data` | `READ \| WRITE` | Initialized global/static variables |
| `APEX_SEG_BSS` | 4 | `.bss` | `READ \| WRITE` | Zero-initialized unmapped memory |

**Permission Flags:**
- `APEX_FLAG_READ` = `0x01`
- `APEX_FLAG_WRITE` = `0x02`
- `APEX_FLAG_EXEC` = `0x04`

---

## 4. Kernel Validation & Security Policy

Upon loading an `.apexec` file, the Aegis64 native loader strictly enforces:
1. **Magic & Architecture Match:** `magic == 0x4550417F`, `arch == 0x8664`, `version == 1`.
2. **W^X Enforcement:** Any segment with `(flags & APEX_FLAG_WRITE) && (flags & APEX_FLAG_EXEC)` is rejected with `APEX_ERR_SECURITY_WX`.
3. **Address Space Boundedness:** All segment virtual addresses and stack allocations must fall strictly within `0x0000000000400000` to `0x00007FFFFFFFFFFFULL`.
4. **File Bounds:** `file_offset + file_size <= file_size`.
5. **Entry Point Validity:** `entry_point` must reside within an executable code segment.
6. **Stack Isolation:** User stack is allocated at high userspace address (`0x00007FFFF0000000`) with `PAGE_PRESENT | PAGE_USER | PAGE_WRITABLE | PAGE_NO_EXECUTE`.
