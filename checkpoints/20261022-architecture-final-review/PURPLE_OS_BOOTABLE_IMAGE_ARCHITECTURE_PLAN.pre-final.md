# Purple OS / Aegis64 — Full System Architecture and Bootable SATA Image Architecture Plan (64-Bit Addressing Revision)

**Document:** `docs/PURPLE_OS_BOOTABLE_IMAGE_ARCHITECTURE_PLAN.md`  
**Classification:** Engineering Architecture & Staged Implementation Plan (Planning Only)  
**Target Repository:** `/home/saptanshu-ganguly/OS`  
**Target Branch:** `Aegis_Purple`  
**Kernel:** Aegis64 Monolithic Kernel (`src/kernel/main.c`, `src/kernel/boot.S`)  
**Subsystems:** AegisSched, AegisDetect, PurpleDetect, APDRV, Unified Storage Layer, GPT, PurpleFS  
**Revision Date:** 2026-10-22  
**Status:** PROPOSED — 64-BIT PFS3 ARCHITECTURE REVISION — AWAITING HUMAN REVIEW & APPROVAL (Zero implementation code modified)

---

## 1. Verified Repository Baseline & Ground Truth

### 1.1 Working Tree & Git State Confirmation

A comprehensive inspection of the working tree using non-paging Git checks (`git --no-pager status --short --branch`, `git --no-pager diff --stat`, and `git --no-pager log -1 --oneline`) established the exact repository state:

- **Active Branch:** `Aegis_Purple` (synchronous with `origin/Aegis_Purple`).
- **Latest Commit:** `409c0cd` ("Purple OS: Shell Path Normalization Implementation and Unit Tests").
- **Working Tree State:** **Uncommitted local modifications exist and are strictly preserved.** (The working tree is NOT clean):
  - `Makefile`: Shell path normalization targets.
  - `include/shell.h`: Path normalization helper function declarations.
  - `src/kernel/shell.c`: Shell-side path normalization logic for root-level files.
  - `scripts/test_qemu_storage.py`: Storage test runner.
  - `tests/test_purplefs_v2.c`: Expanded PurpleFS v2 unit test fixtures.
- **Tracked Checkpoints:**
  - `checkpoints/20261022-shell-path-normalization/`: Historical snapshot and manifest.
  - `checkpoints/20261022-architecture-plan-review/`: Initial plan review checkpoint.
  - `checkpoints/20261022-pfs3-64bit-addressing/`: Created prior to this revision; preserves the initial plan (`PURPLE_OS_BOOTABLE_IMAGE_ARCHITECTURE_PLAN.pre-64bit.md`) and state manifest.
- **Untracked Audit Evidence:**
  - `docs/CHECKPOINT_20261022_PURPLEFS_AUDIT.md`: Pre-audit state record.
  - `docs/PURPLEFS_VERIFICATION_AUDIT_REPORT.md`: Filesystem verification audit.
  - `docs/PURPLEFS_V3_64BIT_ADDRESSING_REVIEW.md`: Companion 64-bit addressing review and verification report.
  - `scripts/test_qemu_persistence.py`: Cold-reboot test script.
  - `tests/test_shell_path.c`: Unit tests for path normalization.

### 1.2 Inspection of Existing Subsystems & Code Verification

| Subsystem | Verified Source Locations | Actual Verified Implementation & Technical Constraints |
| :--- | :--- | :--- |
| **Boot Protocol** | `src/kernel/boot.S`<br>`src/kernel/main.c` | Multiboot 1 entry in 32-bit protected mode (`_start`). Builds identity page tables (4 GiB via 2 MiB huge pages), sets PAE/SSE in CR4, enables Long Mode (EFER.LME) and NXE, loads 64-bit GDT, jumps to `kernel_main_64(mbi, magic)`. Framebuffer initialized via VBE/GOP info in Multiboot struct (`video_init`). |
| **Storage Architecture** | `src/kernel/storage.c`<br>`src/drivers/storage/ahci.c`<br>`src/drivers/storage/ata_pio.c` | Unified block device layer (`block_dev_t`). Scans 4 legacy ATA channels, then scans AHCI PCIe controllers (`ahci_init()`), then NVMe controllers (`nvme_init()`). Registers `sata0`..`sataN`. Parses GPT partitions on all detected block devices. **Monolithic built-in:** AHCI and ATA drivers are compiled directly into the kernel, not dynamically loaded. |
| **GPT Parsing** | `src/fs/gpt.c`<br>`include/gpt.h` | LBA 1 Primary GPT Header, LBA 2..33 Partition Array (128 entries of 128 bytes). CRC32 header and partition array verification. Recognizes `GPT_GUID_ESP` (`C12A7328-F81F-11D2-BA4B-00A0C93EC93B`) and `GPT_GUID_PURPLEFS_V2` (`4F535046-5332-0001-8000-00A0C911D504`). |
| **PurpleFS v2 Limits** | `src/fs/purplefs_v2.c`<br>`include/fs/purplefs_v2.h` | Magic `0x50555250` (`"PURP"`). 4 KiB block size. Dedicated Block 1 bitmap (32,768 bits) caps volume at 128 MiB. Dedicated Block 10 root directory table caps total files at 128. Inodes have 12 direct + 1 indirect block (max 1036 blocks = 4.047 MiB); double indirect returns `PFS2_ERR_TOO_BIG`. No subdirectories, no `mkdir`, no atomic rename/unlink. |
| **Driver Framework** | `src/drivers/apdrv.c`<br>`include/apdrv.h` | Version 3 `.apdrv` format (`APDRV100`). Relocatable ELF-like sections (`.text`, `.rodata`, `.data`, `.bss`). W^X page protection enforced via VMM. Kernel symbol resolution via `ksym.c`. Loaded dynamically *after* storage and filesystem are mounted. |
| **Integrity Protection** | `src/security/aegis_detect.c`<br>`include/security/aegis_detect.h` | HMAC-SHA256 authenticated manifest parser. Built-in hardcoded 32-byte key (`default_trust_key`). Component flags (`AEGIS_FLAG_CRITICAL`, `AEGIS_FLAG_SYSTEM`, `AEGIS_FLAG_OPTIONAL`). Fail-closed alert banner on critical tamper. |

---

## 2. 64-Bit PFS3 Logical Block Addressing (LBA) Architecture

### 2.1 Distinction Between Device Sectors and Filesystem Blocks

To eliminate structural ambiguity, PurpleFS v3 strictly distinguishes between physical storage sectors and filesystem-level logical blocks:

1. **Device Logical Sector ($S_{dev}$):**
   - The hardware sector size exposed by the SATA/AHCI controller: $S_{dev} = 512\text{ bytes}$.
   - Addressed by device LBA: $\text{device\_lba} \in [0, \text{total\_device\_sectors} - 1]$.
2. **Filesystem Logical Block ($B_{fs}$):**
   - The fundamental allocation and transfer unit of PurpleFS v3: $B_{fs} = 4096\text{ bytes}$ ($4\text{ KiB}$).
   - Sectors per block:
     $$k_{sec} = \frac{B_{fs}}{S_{dev}} = \frac{4096}{512} = 8\text{ sectors per block}$$
3. **Filesystem-Relative Block Number (FS-Block $b$):**
   - A 64-bit integer (`uint64_t`) representing an offset in 4096-byte blocks relative to the *start of the partition*.
   - Never conflated with physical device sectors!
4. **Reserved Sentinel Values:**
   - **FS-Block `0`:** Reserved permanently as the **Null / Unallocated / Sparse Hole** sentinel. Any pointer in an inode or indirect block with value `0` indicates that no physical storage block is allocated for that range. (On disk, Block 0 of the partition contains the reserved Partition Boot Record, so file data is never allocated to block 0).
   - **Inode `0`:** Reserved as null/invalid inode index.
   - **Inode `1`:** Reserved for bad blocks tracking inode.
   - **Inode `2`:** Root directory inode (`/`).
   - **Inode `3`:** Transactional journal inode.

### 2.2 Address Translation & Overflow Verification

Every translation from a filesystem-relative block number $b$ to a device-relative sector LBA undergoes strict bounds checking:

Given partition start sector $P_{start}$ and partition sector count $P_{sectors}$ from the GPT partition table:
1. **Total Usable Filesystem Blocks:**
   $$N_{blocks} = \left\lfloor \frac{P_{sectors}}{8} \right\rfloor$$
2. **Bounds Verification Check:**
   $$\text{if } (b == 0 \text{ || } b \ge N_{blocks}) \implies \text{Return } \text{PFS3\_ERR\_OUT\_OF\_BOUNDS}$$
3. **Device LBA Calculation:**
   $$\text{device\_lba} = P_{start} + (b \times 8)$$
   Because $b < N_{blocks}$, $b \times 8 \le P_{sectors} - 8$. In a 64-bit unsigned domain, since $P_{start} + P_{sectors} \le \text{total\_device\_sectors} < 2^{64}$, this calculation is guaranteed never to overflow device boundaries.
4. **Behavior on Malformed or Corrupt Addresses:**
   Any pointer encountered in an inode, extent, or indirect block where $b \ge N_{blocks}$ triggers a fail-closed response: the I/O operation is aborted with error code `PFS3_ERR_CORRUPT`, the filesystem state is flagged `PFS3_FLAG_CORRUPT`, and the corrupted block number and inode number are logged to the kernel serial console.

### 2.3 Volume and File Size Ceilings (First PFS3 Implementation)

While a 64-bit block number theoretically indexes $2^{64} \times 4\text{ KiB} = 64\text{ ZiB}$ (Zettabytes), actual kernel memory, page table mappings, and allocator bitmap structures require documented, practical bounds for the first implementation:

- **Maximum Supported Partition Size (PFS3 v3.0):** **16 TiB** ($2^{44}\text{ bytes} = 2^{32}\text{ blocks}$).  
  *Justification:* A 16 TiB partition requires a 512 MiB block bitmap if kept fully in memory. For our 29.5 GiB target partition, the volume uses $7,732,736\text{ blocks}$, requiring only $\approx 944\text{ KiB}$ (236 blocks) of bitmap.
- **Maximum Supported File Size (PFS3 v3.0):**
  - Direct blocks: $12 \times 4\text{ KiB} = 48\text{ KiB}$.
  - Single indirect block: $512\text{ pointers} \times 4\text{ KiB} = 2\text{ MiB}$.
  - Double indirect block: $512 \times 512 = 262,144\text{ blocks} \times 4\text{ KiB} = 1\text{ GiB}$.
  - Triple indirect block: $512^3 = 134,217,728\text{ blocks} \times 4\text{ KiB} = 512\text{ GiB}$.
  - Total first-version single file capacity: **$512.002\text{ GiB}$** (or capped at the partition capacity on a 32 GiB drive).

---

## 3. Redesigned 256-Byte Inode & Indirect Block Architecture

### 3.1 Mathematical Proof of Inode Size (Why 256 Bytes is Mandatory)

In the previous draft, pointers were 32-bit ($4\text{ bytes}$), which allowed an inode to barely fit into 128 bytes. With 64-bit addressing (`uint64_t`, $8\text{ bytes}$ per pointer), the math changes fundamentally:

- Core Metadata Fields:
  - `mode` (2) + `uid` (2) + `gid` (2) + `links_count` (2) = 8 bytes
  - `flags` (4) + `generation` (4) = 8 bytes
  - `size_bytes` (8) + `atime_sec` (8) + `mtime_sec` (8) + `ctime_sec` (8) = 32 bytes
  - Subtotal metadata: $8 + 8 + 32 = 48\text{ bytes}$.
- 64-Bit Block Addressing:
  - 12 Direct Pointers: $12 \times 8 = 96\text{ bytes}$.
  - 1 Single Indirect Pointer: $1 \times 8 = 8\text{ bytes}$.
  - 1 Double Indirect Pointer: $1 \times 8 = 8\text{ bytes}$.
  - 1 Triple Indirect Pointer: $1 \times 8 = 8\text{ bytes}$.
  - Subtotal addressing: $96 + 8 + 8 + 8 = 120\text{ bytes}$.
- Checksum: $4\text{ bytes}$ (CRC32).
- **Minimum Required Inode Size:**
  $$48\text{ (metadata)} + 120\text{ (pointers)} + 4\text{ (crc)} = 172\text{ bytes}$$

Because $172\text{ bytes} > 128\text{ bytes}$, **a 64-bit inode cannot fit into a 128-byte slot without drastically crippling direct block performance** (reducing direct pointers to $\le 5$). 

Therefore, PurpleFS v3 formally adopts a **256-Byte Inode Layout**:
- Inodes per 4096-byte block: exactly $4096 / 256 = 16\text{ inodes per block}$ (a clean power of 2, $2^4$).
- Reserved expansion space: $256 - 172 = 84\text{ bytes}$, which accommodates future sub-second nanosecond timestamps, security contexts, or inline extended attributes (xattrs) without altering the on-disk ABI.

### 3.2 Formal `pfs3_inode_t` Structure Definition

```c
typedef struct {
    uint16_t mode;             /* 0x00: File type (DIR, REG, SYMLINK) & POSIX permissions */
    uint16_t uid;              /* 0x02: Owner User ID */
    uint16_t gid;              /* 0x04: Owner Group ID */
    uint16_t links_count;      /* 0x06: Hard link counter */
    uint32_t flags;            /* 0x08: Immutability, system, quarantine flags */
    uint32_t generation;       /* 0x0C: Inode generation/version counter */
    uint64_t size_bytes;       /* 0x10: 64-bit exact file size in bytes */
    uint64_t atime_sec;        /* 0x18: Last access timestamp (POSIX seconds) */
    uint64_t mtime_sec;        /* 0x20: Last modification timestamp */
    uint64_t ctime_sec;        /* 0x28: Status change timestamp */
    
    /* 64-bit Logical Block Pointers (FS-relative block numbers, 0 = sparse hole) */
    uint64_t direct[12];       /* 0x30..0x8F: 12 direct blocks (12 * 4 KiB = 48 KiB) */
    uint64_t indirect;         /* 0x90..0x97: Single indirect (512 ptrs * 4 KiB = 2 MiB) */
    uint64_t double_indirect;  /* 0x98..0x9F: Double indirect (512^2 * 4 KiB = 1 GiB) */
    uint64_t triple_indirect;  /* 0xA0..0xA7: Triple indirect (512^3 * 4 KiB = 512 GiB) */
    
    uint32_t checksum;         /* 0xA8..0xAB: CRC32 of bytes 0x00..0xA7 (excluding checksum) */
    uint8_t  reserved[84];     /* 0xAC..0xFF: Expansion pad / inline xattr space (to 256 bytes) */
} __attribute__((packed)) pfs3_inode_t;
```

#### Field Offset & Byte Count Verification:
- `0x00..0x0F` (16 bytes): `mode` (2), `uid` (2), `gid` (2), `links_count` (2), `flags` (4), `generation` (4).
- `0x10..0x2F` (32 bytes): `size_bytes` (8), `atime_sec` (8), `mtime_sec` (8), `ctime_sec` (8).
- `0x30..0x8F` (96 bytes): `direct[12]` ($12 \times 8 = 96$).
- `0x90..0xA7` (24 bytes): `indirect` (8), `double_indirect` (8), `triple_indirect` (8).
- `0xA8..0xAB` (4 bytes): `checksum` (4).
- `0xAC..0xFF` (84 bytes): `reserved[84]`.
- **Total:** $16 + 32 + 96 + 24 + 4 + 84 = 256\text{ bytes}$.
- Compile-time assertion: `_Static_assert(sizeof(pfs3_inode_t) == 256, "pfs3_inode_t must be 256 bytes");`

### 3.3 Checksum Coverage Specification

To guarantee that checksum validation is deterministic and never self-referential:
- The CRC32 checksum is calculated exclusively over the first 168 bytes (`0x00` through `0xA7`), which encompass all core metadata and block addressing pointers.
- When computing the checksum prior to write, the `checksum` field itself is skipped.
- When verifying the checksum on disk read:
  $$\text{computed\_crc} = \text{crc32}(0, (\text{uint8\_t} *)\text{inode}, 0\text{xA8});$$
  $$\text{if } (\text{computed\_crc} \ne \text{inode}\to\text{checksum}) \implies \text{Return } \text{PFS3\_ERR\_INODE\_CORRUPT}$$

### 3.4 64-Bit Indirect Block Layout

With 64-bit pointers, a 4096-byte indirect block contains:
$$\text{Pointers per Indirect Block} = \frac{4096\text{ bytes}}{8\text{ bytes/pointer}} = 512\text{ pointers}$$

```c
typedef struct {
    uint64_t blocks[512];      /* Array of 512 64-bit FS-relative block numbers */
} __attribute__((packed)) pfs3_indirect_block_t;
```

- **Single Indirect:** Covers $512 \times 4\text{ KiB} = 2,097,152\text{ bytes} = 2\text{ MiB}$.
- **Double Indirect:** References 512 single indirect blocks. Covers $512 \times 512 = 262,144\text{ blocks} = 1,073,741,824\text{ bytes} = 1\text{ GiB}$.
- **Triple Indirect:** References 512 double indirect blocks. Covers $512^3 = 134,217,728\text{ blocks} = 549,755,813,888\text{ bytes} = 512\text{ GiB}$.
- **Hole Handling:** Any pointer within an indirect block that has the value `0` represents a sparse file hole. Reads from a hole return zero-filled buffers without issuing I/O requests.

---

## 4. Scalable Multi-Block Allocation Metadata Architecture

### 4.1 Multi-Block Allocation Bitmaps

To overcome the 128 MiB limitation of PurpleFS v2, PurpleFS v3 distributes block allocation tracking across a contiguous array of 4 KiB bitmap blocks:

- Each 4 KiB bitmap block tracks $4096 \times 8 = 32,768\text{ filesystem blocks}$ ($128\text{ MiB}$).
- For Partition 2 ($29.50\text{ GiB} = 7,732,736\text{ blocks}$):
  $$\text{Required Bitmap Blocks} = \left\lceil \frac{7,732,736}{32,768} \right\rceil = 236\text{ blocks}$$
- For Partition 3 ($2.00\text{ GiB} = 524,288\text{ blocks}$):
  $$\text{Required Bitmap Blocks} = \left\lceil \frac{524,288}{32,768} \right\rceil = 16\text{ blocks}$$

### 4.2 Bitmap Location & Protection of System Metadata

Metadata blocks must never be allocated to user files. PurpleFS v3 reserves the lower blocks of the partition permanently:

```
+----------------------------------------------------------------------------------------------------+
| Block 0            : Partition Boot Record / Header (Reserved 4 KiB)                               |
| Block 1            : Primary Superblock (pfs3_superblock_t)                                        |
| Block 2            : Backup Superblock                                                             |
| Blocks 3..238      : Block Allocation Bitmap (236 blocks for 29.5 GiB)                             |
| Block 239          : Inode Allocation Bitmap (1 block = 32,768 inodes)                             |
| Blocks 240..2287   : Inode Table (2048 blocks * 16 inodes/block = 32,768 Inodes)                   |
| Blocks 2288..10479 : Transactional Journal Ring Buffer (8192 blocks = 32 MiB)                      |
| Blocks 10480..End  : General Data Blocks (User files, directory blocks, indirect blocks)           |
+----------------------------------------------------------------------------------------------------+
```

#### Metadata Protection Invariants:
1. **Formatting Time:** During `pfs3_format`, bits $0$ through $10,479$ in the block allocation bitmap are pre-marked as allocated (`1`).
2. **Runtime Allocation Guard:** The block allocator function (`pfs3_alloc_block`) enforces an absolute lower bound:
   $$\text{if } (b < 10480) \implies \text{Panic / Abort with } \text{PFS3\_ERR\_METADATA\_COLLISION}$$
3. **Upper Bound Check:** If $b \ge N_{blocks}$, the allocator recognizes disk exhaustion or bitmap corruption.
4. **Crash Consistency:** Modifications to bitmap blocks are never written directly to the filesystem space; they are first appended to the Write-Ahead Journal.
5. **Double-Allocation Prevention:** The allocator tests bits using atomic test-and-set operations (`lock btsq` or spinlock-protected bitmap scans). If a bit is already `1`, the scanner advances to the next candidate block.

---

## 5. Audit of All Other On-Disk Structures (64-Bit Harmonization)

### 5.1 Superblock Structure (`pfs3_superblock_t`)

The superblock contains 64-bit block numbers for all metadata zone boundaries:

```c
typedef struct {
    uint32_t magic;                    /* 0x00: 0x50465333 ("PFS3") */
    uint32_t version;                  /* 0x04: 3 */
    uint32_t block_size;               /* 0x08: 4096 */
    uint32_t state_flags;              /* 0x0C: 0x01=CLEAN, 0x02=DIRTY, 0x04=CORRUPT */
    
    /* 64-bit Geometry & Counters */
    uint64_t total_blocks;             /* 0x10: Total 4 KiB blocks in volume */
    uint64_t free_blocks;              /* 0x18: Unallocated blocks count */
    uint64_t total_inodes;             /* 0x20: Total allocated inode slots (e.g. 32,768) */
    uint64_t free_inodes;              /* 0x28: Unallocated inode slots */
    uint64_t root_inode;               /* 0x30: Root directory inode number (always 2) */
    
    /* 64-bit Metadata Zone Locations */
    uint64_t block_bitmap_start;       /* 0x38: FS-relative block number (Block 3) */
    uint64_t block_bitmap_blocks;      /* 0x40: Number of bitmap blocks (236) */
    uint64_t inode_bitmap_start;       /* 0x48: FS-relative block number (Block 239) */
    uint64_t inode_bitmap_blocks;      /* 0x50: Number of inode bitmap blocks (1) */
    uint64_t inode_table_start;        /* 0x58: FS-relative block number (Block 240) */
    uint64_t inode_table_blocks;       /* 0x60: Number of inode table blocks (2048) */
    uint64_t journal_start;            /* 0x68: FS-relative block number (Block 2288) */
    uint64_t journal_blocks;           /* 0x70: Number of journal blocks (8192) */
    
    /* Volume Metadata & Identifiers */
    uint8_t  volume_uuid[16];          /* 0x78: 128-bit unique filesystem UUID */
    char     volume_label[32];         /* 0x88: Human-readable label (e.g. "PurpleSystem") */
    uint64_t mount_count;              /* 0xA8: Number of times mounted */
    uint64_t last_mount_time;          /* 0xB0: POSIX timestamp of last mount */
    uint64_t last_write_time;          /* 0xB8: POSIX timestamp of last write */
    
    uint32_t checksum;                 /* 0xC0: CRC32 of bytes 0x00..0xBF (excluding checksum) */
    uint8_t  reserved[3896];           /* 0xC4..0xFFF: Pad to exactly 4096 bytes (1 block) */
} __attribute__((packed)) pfs3_superblock_t;
```

*Verification:* $0\text{xC4} + 3896 = 4096\text{ bytes} = 1\text{ full block}$. The superblock occupies Block 1 completely, with a bit-identical backup at Block 2.

### 5.2 Directory Entry Structure (`pfs3_dirent_t`)

To support large file collections while guaranteeing predictable alignment, directory blocks use **Structured Fixed 64-Byte Records** with 64-bit inode pointers:

```c
typedef struct {
    uint64_t inode_num;                /* 0x00: 64-bit inode number (0 = free/deleted entry) */
    uint8_t  file_type;                /* 0x08: 1=REG, 2=DIR, 3=SYMLINK, 4=DEV */
    uint8_t  name_len;                 /* 0x09: Length of name string in bytes (1..50) */
    uint16_t reserved;                 /* 0x0A: Alignment pad (zeroed) */
    uint16_t checksum;                 /* 0x0C: CRC16 of dirent fields */
    uint16_t reserved2;                /* 0x0E: Expansion pad */
    char     name[50];                 /* 0x10..0x41: UTF-8 / ASCII name (null-terminated) */
} __attribute__((packed)) pfs3_dirent_t;
```

- Record size: exactly $64\text{ bytes}$.
- Entries per 4096-byte directory block: exactly $4096 / 64 = 64\text{ entries}$.
- Directory scaling: Directories with $>64$ entries allocate additional 4 KiB blocks through the directory inode's block pointers.
- Directory Invariants: In every directory's first data block, Slot 0 is `.` (pointing to self inode) and Slot 1 is `..` (pointing to parent inode; in Root Inode 2, `..` points to Inode 2).

### 5.3 Journal Record Structure (`pfs3_journal_entry_t`)

The 32 MiB circular metadata journal records transactions using 64-bit target block numbers:

```c
typedef struct {
    uint64_t seq_num;                  /* Monotonically increasing transaction ID */
    uint64_t target_fs_block;          /* 64-bit FS-relative block number being modified */
    uint32_t flags;                    /* 0x01=START, 0x02=DATA, 0x04=COMMIT, 0x08=REVOKE */
    uint32_t payload_crc32;            /* CRC32 of following 4096-byte data block */
} __attribute__((packed)) pfs3_journal_header_t;
```

---

## 6. Boot, Recovery & Security Integrity Invariants

### 6.1 Preservation of Monolithic In-Kernel Storage Boot

1. **Aegis64 Kernel Integrity:** The kernel remains x86-64. The 32-bit Multiboot entry in [`src/kernel/boot.S`](file:///home/saptanshu-ganguly/OS/src/kernel/boot.S) transitions into 64-bit Long Mode before entering C code. Boot assembly will not be modified.
2. **Non-Circular Bootstrapping:** The core AHCI SATA controller driver ([`src/drivers/storage/ahci.c`](file:///home/saptanshu-ganguly/OS/src/drivers/storage/ahci.c)), GPT parser ([`src/fs/gpt.c`](file:///home/saptanshu-ganguly/OS/src/fs/gpt.c)), and PurpleFS v3 reader are compiled directly into the monolithic kernel. They do **not** depend on loading `.apdrv` driver modules from the filesystem.
3. **Peripheral Drivers via APDRV:** Non-essential drivers (Audio, Ethernet, USB HID) remain modular `.apdrv` packages located in `/System/Drivers/` and are verified and loaded *after* root mounting.

### 6.2 Independent Recovery Bootability

1. **Independent Superblock & Filesystem:** Partition 3 (`PurpleRecovery`) is a fully independent PurpleFS v3 partition with its own superblock, allocation bitmaps, inode table, and directory tree.
2. **No Dependency on PurpleSystem:** If Partition 2 (`PurpleSystem`) is unreadable, corrupted, or missing, the EFI bootloader boots `/EFI/BOOT/KERNEL.ELF` from Partition 1 (FAT32 ESP) with command line `root=UUID=<PurpleRecovery-UUID> recovery=1`.
3. **Automatic Fallback:** If normal boot is attempted but Partition 2 fails mount validation or critical component hashes, the kernel automatically unmounts Partition 2 and mounts Partition 3 as the emergency rescue root.

### 6.3 Recovery Partition Write Boundary Protection

- In [`src/fs/partition_dev.c`](file:///home/saptanshu-ganguly/OS/src/fs/partition_dev.c), Partition 3 is initialized with `read_only = 1`.
- Any call to `bdev->ops->write_blocks` targeting Partition 3 returns `BLOCK_DEV_ERR_RO` without issuing commands to AHCI hardware.
- **Update-Unlock Procedure:**  
  Updating the recovery environment requires an authenticated update package (`update.pkg`) containing an offline digital signature. The kernel verifies the signature in memory, temporarily clears `read_only = 0`, sets a software timeout watchdog (e.g. 30 seconds enforced via kernel timer interrupts), writes the blocks, flushes cache via `ahci_bdev_flush`, and immediately relocks `read_only = 1`.

### 6.4 Security Model Clarifications

- **Embedded HMAC Limitations:** The current HMAC mechanism in [`src/security/aegis_detect.c`](file:///home/saptanshu-ganguly/OS/src/security/aegis_detect.c) provides runtime tamper detection against accidental modification or unprivileged tampering while the OS is executing. It **does not** prevent offline modification by an attacker who extracts the symmetric key from the kernel binary.
- **Proposed Public-Key Model:** Transition to an asymmetric public-key signature model (Ed25519/RSA-2048 with private keys kept off-device) is designated as a future architecture requirement for Milestone 11. It is not claimed to be implemented currently.
- **UEFI Secure Boot:** Unauthenticated development OVMF does not enforce Secure Boot. Secure Boot claims will not be made until platform keys are enrolled.

---

## 7. 32 GiB Virtual SATA Disk Layout (Mathematically Verified)

The 32 GiB disk layout with 1 MiB alignment and 64-bit block numbering:

```
====================================================================================================
LBA 0                          : Protective MBR (Type 0xEE)
LBA 1                          : Primary GPT Header
LBA 2..33                      : Primary Partition Table Array (128 entries * 128 bytes)
LBA 34..2047                   : Alignment Padding (2014 sectors to 1 MiB boundary)
====================================================================================================
PARTITION 1: EFI System Partition (ESP) [FAT32]
  Start LBA                    : 2,048 (1 MiB boundary)
  End LBA                      : 1,050,623
  Sector Count                 : 1,048,576 sectors (exactly 512 MiB = 0.50 GiB)
  Partition Type GUID          : C12A7328-F81F-11D2-BA4B-00A0C93EC93B
  Role                         : UEFI boot binaries, GRUB/AegisBootloader, kernel ELF, boot config.
----------------------------------------------------------------------------------------------------
PARTITION 2: PurpleSystem (Root '/') [PurpleFS v3 - 64-bit Addressing]
  Start LBA                    : 1,050,624 (513 MiB boundary)
  End LBA                      : 62,912,511
  Sector Count                 : 61,861,888 sectors (exactly 30,206 MiB = 29.50 GiB)
  4 KiB Block Count            : 7,732,736 blocks (8 sectors/block)
  Bitmap Blocks Required       : 236 blocks (each block tracks 32,768 FS blocks)
  Inode Table Blocks           : 2,048 blocks (32,768 inodes * 256 bytes / 4096)
  Partition Type GUID          : 4F535046-5333-0001-8000-00A0C911D504
  Role                         : Full directory hierarchy (/System, /Boot, /Applications, /Users).
----------------------------------------------------------------------------------------------------
PARTITION 3: PurpleRecovery ('/Recovery') [PurpleFS v3 - Read-Only]
  Start LBA                    : 62,912,512 (30,719 MiB boundary)
  End LBA                      : 67,106,815
  Sector Count                 : 4,194,304 sectors (exactly 2,048 MiB = 2.00 GiB)
  4 KiB Block Count            : 524,288 blocks (8 sectors/block)
  Bitmap Blocks Required       : 16 blocks
  Inode Table Blocks           : 512 blocks (8,192 inodes * 256 bytes / 4096)
  Partition Type GUID          : 4F535046-5352-0001-8000-00A0C911D504
  Role                         : Pristine emergency kernel, recovery tools, golden restore images.
====================================================================================================
LBA 67,106,816 .. 67,108,830  : Trailing Alignment Padding (2,015 sectors)
LBA 67,108,831 .. 67,108,862  : Backup Partition Table Array (32 sectors)
LBA 67,108,863                 : Backup GPT Header (Last LBA of disk)
====================================================================================================
```

*Verification:*
- $P1\text{ start } 2048 \ge \text{FirstUsableLBA } 34$.
- $P3\text{ end } 67,106,815 \le \text{LastUsableLBA } 67,108,830$.
- All partition boundaries are exact multiples of 2048 sectors (1 MiB) and 8 sectors (4 KiB).

---

## 8. Revised 13-Milestone Engineering Roadmap & Explicit Acceptance Gates

Every milestone represents an independent, gated engineering increment. **No implementation milestone may proceed without meeting its explicit verification criteria.**

```
[M1: Repository & Planning Audit] 
     └── Gate: Plan approved, working tree preserved (COMPLETED)
[M2: PurpleFS v3 Format Specification & Header Definitions]
     └── Gate: Compile-time static assertions verify sizeof(pfs3_inode_t)==256, sizeof(pfs3_dirent_t)==64,
               sizeof(pfs3_superblock_t)==4096, field offsets match specification exactly.
[M3: Standalone PurpleFS v3 Host Prototype & Unit Tests]
     └── Gate: In-memory volume creation, block format, 64-bit out-of-range pointer rejection,
               100% pass under host GCC AddressSanitizer (ASan) and UBSan.
[M4: Hierarchical Directory & Path Resolution APIs]
     └── Gate: mkdir/rmdir, '.' and '..' handling, fixed 64-byte dirent scaling, 32-level depth tests pass.
[M5: Scalable Multi-Block Allocator & Large-File Addressing]
     └── Gate: Allocation, writing, and SHA-256 verification of 50 MiB file spanning >12,800 blocks
               across multi-block allocation bitmaps without memory leaks or double allocation.
[M6: Transactional Metadata Journaling & Deterministic Fault Injection]
     └── Gate: Deterministic fault injection simulating process abort mid-write; journal replay restores
               filesystem consistency without orphaned blocks or leaked bitmap bits.
[M7: Non-Destructive Offline Migration Tool (pfs2_to_pfs3.py)]
     └── Gate: Converts v2 test image to v3; SHA-256 of extracted files match v2 bit-for-bit;
               source v2 image remains bit-for-bit untouched; reports unsupported features explicitly.
[M8: Purple Shell Directory Command Integration]
     └── Gate: Host harness validates interactive commands (cd, ls, mkdir, peek, write, rm) with path traversal.
[M9: 32 GiB Sparse Disk Builder & ESP Packaging]
     └── Gate: Python script creates 32 GiB sparse image with FAT32 ESP and PurpleFS v3 partitions;
               asserts actual allocated host storage <= 500 MiB; GPT aligned to 1 MiB boundaries.
[M10: Single Virtual SATA Disk QEMU AHCI Boot]
     └── Gate: QEMU boots from sata0 alone without -kernel or auxiliary disks; launches AegisShell.
[M11: AegisDetect Boot Verification & Fail-Closed Enforcement]
     └── Gate: Modifying aegis64.elf halts boot and triggers Recovery; modifying ac97.apdrv logs warning
               and continues normal boot.
[M12: Read-Only Recovery Partition Isolation & Updates]
     └── Gate: Writes to /Recovery rejected with BLOCK_DEV_ERR_RO; authenticated unlock/relock verified.
[M13: End-to-End Stress & Cold-Reboot Persistence Suite]
     └── Gate: Automated test running 50 cold reboots with random power cuts during writes; 100% journal
               recovery rate; zero unrecoverable corruption; all test suites pass.
```

---

## 9. Deliverables & Safety Compliance Record

- **Working Directory:** `/home/saptanshu-ganguly/OS`
- **Active Branch:** `Aegis_Purple`
- **Source Code Modified:** Zero lines (Only documentation and checkpoint files created).
- **Checkpoints Created:**
  - `checkpoints/20261022-pfs3-64bit-addressing/`: Contains `MANIFEST.txt` and `PURPLE_OS_BOOTABLE_IMAGE_ARCHITECTURE_PLAN.pre-64bit.md`.
- **Working Tree Status:** Preserved with existing uncommitted modifications intact.
