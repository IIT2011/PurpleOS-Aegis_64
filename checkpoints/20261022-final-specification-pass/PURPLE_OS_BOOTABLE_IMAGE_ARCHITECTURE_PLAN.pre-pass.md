# Purple OS / Aegis64 — Full System Architecture and Bootable SATA Image Architecture Plan (Final Arithmetic & Specification Pass)

**Document:** `docs/PURPLE_OS_BOOTABLE_IMAGE_ARCHITECTURE_PLAN.md`  
**Classification:** Engineering Architecture & Staged Implementation Plan (Planning Only)  
**Target Repository:** `/home/saptanshu-ganguly/OS`  
**Target Branch:** `Aegis_Purple`  
**Kernel:** Aegis64 Monolithic Kernel (`src/kernel/main.c`, `src/kernel/boot.S`)  
**Subsystems:** AegisSched, AegisDetect, PurpleDetect, APDRV, Unified Storage Layer, GPT, PurpleFS  
**Revision Date:** 2026-10-22  
**Status:** PROPOSED ARCHITECTURE — FINAL ARITHMETIC & SPECIFICATION PASS — AWAITING HUMAN REVIEW (Zero implementation code modified)

---

## 1. Verified Repository Baseline vs. Architecture Proposals

### 1.1 Working Tree & Git State Confirmation

A comprehensive inspection of the working tree using non-paging Git checks (`git --no-pager status --short --branch`, `git --no-pager diff --stat`, and `git --no-pager log -1 --oneline`) established the exact repository baseline:

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
  - `checkpoints/20261022-pfs3-64bit-addressing/`: Pre-64bit plan checkpoint.
  - `checkpoints/20261022-architecture-final-review/`: Pre-final review checkpoint.
  - `checkpoints/20261022-arithmetic-consistency-pass/`: Created prior to this revision; preserves pre-arithmetic drafts and Git manifest.
- **Documentation & Verification Evidence:**
  - `docs/CHECKPOINT_20261022_PURPLEFS_AUDIT.md`: Pre-audit state record.
  - `docs/PURPLEFS_VERIFICATION_AUDIT_REPORT.md`: Filesystem verification audit.
  - `docs/PURPLEFS_V3_64BIT_ADDRESSING_REVIEW.md`: Companion 64-bit addressing specification.
  - `scripts/test_qemu_persistence.py`: Cold-reboot test script.
  - `tests/test_shell_path.c`: Unit tests for path normalization.

### 1.2 Strict Separation of Verified Code Facts vs. Proposed Features

| Subsystem | Verified Repository Implementation (Fact) | Proposed Future Design [PROPOSED — NOT YET IMPLEMENTED OR VERIFIED] |
| :--- | :--- | :--- |
| **Boot Protocol** | Monolithic kernel entered via 32-bit Multiboot 1 (`_start` in `src/kernel/boot.S`), sets up early identity page tables (4 GiB via 2 MiB huge pages), loads 64-bit GDT, jumps to `kernel_main_64` in `src/kernel/main.c`. | Dual boot menu options (Normal vs. Recovery) and command-line root parameter parsing (`root=UUID=... recovery=1`) are **[PROPOSED — NOT IMPLEMENTED]**. |
| **Storage Architecture** | `storage_init()` in `src/kernel/storage.c` initializes ATA PIO, AHCI SATA (`src/drivers/storage/ahci.c`), and NVMe PCIe, parses GPT, and mounts only the **first** detected PurpleFS partition. | Dual mounting of `/` and `/Recovery`, runtime read-only boundary locking on Partition 3, and automated fallback to recovery are **[PROPOSED — NOT IMPLEMENTED]**. |
| **Filesystem (PFS2 vs PFS3)** | `src/fs/purplefs_v2.c` is hardcoded to 128 MiB (1 block bitmap), 128 root entries (1 block directory table), 4.047 MiB max file (1036 blocks), and flat lookup. | PurpleFS v3 with 64-bit block addressing, 256-byte inodes, multi-block bitmaps, and hierarchical directories is **[PROPOSED — NOT IMPLEMENTED]**. |
| **Driver Framework** | `src/drivers/apdrv.c` dynamically loads relocatable v3 `.apdrv` modules with W^X page protection and resolves symbols via `ksym.c` *after* storage is mounted. | Loading peripheral drivers from a hierarchical `/System/Drivers/` directory tree is **[PROPOSED — NOT IMPLEMENTED]**. |
| **Integrity Protection** | `src/security/aegis_detect.c` authenticates a manifest using a compiled-in symmetric 32-byte HMAC-SHA256 key (`default_trust_key`). | Asymmetric public-key signature verification (Ed25519/RSA-2048 with private key off-device) is **[PROPOSED — NOT IMPLEMENTED]**. |

---

## 2. 64-Bit PFS3 Logical Block Addressing (LBA) Architecture

### 2.1 Distinction Between Device Sectors and Filesystem Blocks

To eliminate structural ambiguity, PurpleFS v3 strictly distinguishes between physical storage sectors and filesystem-level logical blocks:

1. **Device Logical Sector ($S_{dev}$):**
   - The hardware sector size exposed by the SATA/AHCI controller: $S_{dev} = 512\text{ bytes}$.
   - Addressed by device LBA: $\text{device\_lba} \in [0, D_{sectors} - 1]$, where $D_{sectors}$ is the total device capacity in sectors.
2. **Filesystem Logical Block ($B_{fs}$):**
   - The fundamental allocation and transfer unit of PurpleFS v3: $B_{fs} = 4096\text{ bytes}$ ($4\text{ KiB}$).
   - Sectors per block ratio:
     $$k_{sec} = \frac{B_{fs}}{S_{dev}} = \frac{4096}{512} = 8\text{ sectors per block}$$
3. **Filesystem-Relative Block Number (FS-Block $b$):**
   - A 64-bit unsigned integer (`uint64_t`) representing an offset in 4096-byte blocks relative to the *start of the partition*.
   - Never conflated with physical device sectors!
4. **Reserved Sentinel Values:**
   - **FS-Block `0`:** Reserved permanently as the **Null / Unallocated / Sparse Hole** sentinel. Any pointer in an inode or indirect block with value `0` indicates that no physical storage block is allocated for that range. (On disk, Block 0 contains the reserved Partition Boot Record).
   - **Inode `0`:** Reserved as null/invalid inode index.
   - **Inode `1`:** Reserved for bad blocks tracking inode.
   - **Inode `2`:** Root directory inode (`/`).
   - **Inode `3`:** Transactional journal inode.

### 2.2 Strengthened Block-to-Device-Sector Arithmetic & Safety Invariants

Translation from filesystem block $b$ to device sector LBA $\text{device\_lba}$ is performed by:

$$\text{device\_lba} = P_{start} + (b \times 8)$$

Where $P_{start}$ is the partition's starting sector from the GPT entry.

#### Precondition 1: Partition Boundary Validation against Device Capacity
During mount (`pfs3_mount`), the partition geometry is verified against the underlying block device:
$$\text{Assert } P_{start} < D_{sectors}$$
$$\text{Assert } P_{sectors} > 0$$
$$\text{Assert } P_{start} \le D_{sectors} - P_{sectors} \quad (\text{guarantees partition end } P_{start} + P_{sectors} - 1 < D_{sectors})$$
If any assertion fails, mount aborts immediately with `PFS3_ERR_INVALID_PARTITION`.

#### Precondition 2: Usable Filesystem Blocks and Handling of Partial Trailing Sectors
Because each filesystem block requires exactly 8 physical sectors:
$$N_{blocks} = \left\lfloor \frac{P_{sectors}}{8} \right\rfloor$$
$$\text{Partial Trailing Sectors } R = P_{sectors} \pmod 8$$
- **Handling of Partial Trailing Sectors:** Any trailing sectors $R \in [1, 7]$ at the end of the partition do not form a complete 4096-byte block. The filesystem **strictly ignores and excludes** these $R$ trailing sectors from the block map ($N_{blocks} \times 8 \le P_{sectors}$). They are never addressed or allocated. (On standard 1 MiB-aligned partitions, $P_{sectors}$ is an exact multiple of 2048, so $R = 0$).

#### Precondition 3: Overflow-Safe Block Bounds Check on Access
For every block read, write, or traversal:
$$\text{Valid Block Index: } 1 \le b < N_{blocks}$$
(Block `0` is the reserved Boot Record / Null pointer sentinel).

#### Mathematical Proof of Overflow Safety:
1. **Multiplication Safety ($b \times 8$):**
   $$b \le N_{blocks} - 1 \implies b \times 8 \le (N_{blocks} - 1) \times 8 = N_{blocks} \times 8 - 8 \le P_{sectors} - 8$$
   Since $P_{sectors} \le D_{sectors} < 2^{64}$, $b \times 8$ is strictly bounded by $P_{sectors} - 8$ and cannot overflow `uint64_t`.
2. **Addition Safety ($P_{start} + b \times 8$):**
   $$P_{start} + (b \times 8) \le P_{start} + P_{sectors} - 8$$
   Since $P_{start} + P_{sectors} \le D_{sectors} \le 2^{64} - 1$, the sum $P_{start} + (b \times 8)$ is strictly bounded by $D_{sectors} - 8$.
3. **Full 8-Sector Block Containment:**
   $$\text{Final Block Sector} = \text{device\_lba} + 7 = P_{start} + (b \times 8) + 7 \le P_{start} + P_{sectors} - 1 \le D_{sectors} - 1$$
   Therefore, **all 8 sectors of the block** ($[\text{device\_lba}, \text{device\_lba} + 7]$) stay strictly within both the filesystem partition and the physical device, with zero risk of arithmetic overflow or out-of-bounds corruption.

### 2.3 Exact Maximum File-Addressing Capacity vs. Supported Limits

#### Theoretical Pointer Capacity Calculation (4096-Byte Blocks, 8-Byte Pointers):
- **12 Direct Blocks:** $12 \times 4096 = 49,152\text{ bytes} = \mathbf{48\text{ KiB}}$.
- **1 Single-Indirect Block:** $512\text{ pointers} \times 4096 = 2,097,152\text{ bytes} = \mathbf{2\text{ MiB}}$.
- **1 Double-Indirect Block:** $512^2 = 262,144\text{ blocks} \times 4096 = 1,073,741,824\text{ bytes} = \mathbf{1\text{ GiB}}$.
- **1 Triple-Indirect Block:** $512^3 = 134,217,728\text{ blocks} \times 4096 = 549,755,813,888\text{ bytes} = \mathbf{512\text{ GiB}}$.

$$\text{Total Addressable Blocks} = 12 + 512 + 262,144 + 134,217,728 = \mathbf{134,480,396\text{ blocks}}$$
$$\text{Total Addressable Bytes} = 134,480,396 \times 4096 = \mathbf{550,831,702,016\text{ bytes}}$$

In exact binary units:
$$512\text{ GiB} + 1\text{ GiB} + 2\text{ MiB} + 48\text{ KiB} = \mathbf{513\text{ GiB} + 2\text{ MiB} + 48\text{ KiB}} = \mathbf{513.00201416015625\text{ GiB}}$$

#### Supported File Size Limit vs. Partition Capacity:
The theoretical pointer capacity ($\approx 513.002\text{ GiB}$) cannot exceed the filesystem partition's actual usable capacity:
- **On Partition 2 (PurpleSystem, 29.50 GiB total, 7,722,256 usable data blocks):**  
  The maximum possible file size is bounded by partition data capacity:
  $$\text{Max Supported File (Partition 2)} = 7,722,256\text{ blocks} \times 4096 = \mathbf{31,630,360,576\text{ bytes}} \approx \mathbf{29.458\text{ GiB}}$$
- **On Partition 3 (PurpleRecovery, 2.00 GiB total, 521,708 usable data blocks):**  
  The maximum possible file size is bounded by recovery data capacity:
  $$\text{Max Supported File (Partition 3)} = 521,708\text{ blocks} \times 4096 = \mathbf{2,136,915,968\text{ bytes}} \approx \mathbf{1.990\text{ GiB}}$$
- **Double-Indirect Driver Sub-Limit (Optional Phase 1 ceiling):**  
  If triple indirection is omitted in early driver prototypes, the double-indirect ceiling is:
  $$12 + 512 + 262,144 = 262,668\text{ blocks} \times 4096 = \mathbf{1,075,888,128\text{ bytes}} \approx \mathbf{1.002\text{ GiB}}\text{ (1 GiB + 2 MiB + 48 KiB)}$$

---

## 3. 32 GiB UEFI GPT Layout & Exact Metadata Block Ranges

### 3.1 Sector Arithmetic & Partition Alignment

A 32 GiB disk with 512-byte sectors has:
$$\text{Total Capacity} = 32 \times 1024 \times 1024 \times 1024\text{ bytes} = 34,359,738,368\text{ bytes}$$
$$\text{Total Sectors} = \frac{34,359,738,368}{512} = 67,108,864\text{ sectors}$$
$$\text{LBA Range} = \text{LBA } 0 \text{ to LBA } 67,108,863$$

#### GPT Structural Regions:
- **LBA 0:** Protective MBR (1 sector).
- **LBA 1:** Primary GPT Header (`MyLBA = 1`, `AlternateLBA = 67,108,863`, `FirstUsableLBA = 34`, `LastUsableLBA = 67,108,830`).
- **LBA 2..33:** Primary Partition Array (128 entries $\times$ 128 bytes = 16,384 bytes = 32 sectors).
- **LBA 34..2047:** Alignment Padding (2014 sectors = 1007 KiB) to align Partition 1 to the standard 1 MiB boundary ($2048 \times 512 = 1,048,576\text{ bytes}$).
- **LBA 67,106,816..67,108,830:** Trailing Alignment Padding (2015 sectors = ~1007.5 KiB) ensuring Partition 3 ends on a 1 MiB boundary while remaining $\le \text{LastUsableLBA}$.
- **LBA 67,108,831..67,108,862:** Backup Partition Array (32 sectors).
- **LBA 67,108,863:** Backup GPT Header (1 sector).

### 3.2 Partition Boundaries & Exact Metadata Layout

Every partition starts and ends on an exact 1 MiB boundary (multiples of 2048 sectors) and is an exact integer multiple of 4 KiB filesystem blocks (8 sectors per block):

```
====================================================================================================
LBA 0                          : Protective MBR (Type 0xEE)
LBA 1                          : Primary GPT Header
LBA 2..33                      : Primary Partition Table Array (128 entries * 128 bytes)
LBA 34..2047                   : Alignment Padding (to 1 MiB)
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
  Partition Type GUID          : 4F535046-5333-0001-8000-00A0C911D504
  Metadata Layout              :
    - Block 0                  : Partition Boot Record / Header (1 block)
    - Block 1                  : Primary Superblock (1 block)
    - Block 2                  : Backup Superblock (1 block)
    - Blocks 3 .. 238          : Block Allocation Bitmap (236 blocks = 944 KiB, tracks 7,732,736 blocks)
    - Block 239                : Inode Allocation Bitmap (1 block = 4 KiB, tracks 32,768 inodes)
    - Blocks 240 .. 2287       : Inode Table (2048 blocks = 8 MiB, 32,768 inodes * 256 bytes)
    - Blocks 2288 .. 10479     : Metadata Journal Ring Buffer (8192 blocks = 32 MiB)
    - Total Reserved Metadata  : Exactly 10,480 blocks (Blocks 0..10479, ~40.938 MiB)
    - Blocks 10480 .. 7732735  : General Data Blocks (7,722,256 blocks ~ 29.458 GiB)
----------------------------------------------------------------------------------------------------
PARTITION 3: PurpleRecovery ('/Recovery') [PurpleFS v3 - Read-Only]
  Start LBA                    : 62,912,512 (30,719 MiB boundary)
  End LBA                      : 67,106,815
  Sector Count                 : 4,194,304 sectors (exactly 2,048 MiB = 2.00 GiB)
  4 KiB Block Count            : 524,288 blocks (8 sectors/block)
  Partition Type GUID          : 4F535046-5352-0001-8000-00A0C911D504
  Metadata Layout              :
    - Block 0                  : Partition Boot Record / Header (1 block)
    - Block 1                  : Primary Superblock (1 block)
    - Block 2                  : Backup Superblock (1 block)
    - Blocks 3 .. 18           : Block Allocation Bitmap (16 blocks, tracks 524,288 blocks)
    - Block 19                 : Inode Allocation Bitmap (1 block, tracks up to 32,768 inodes)
    - Blocks 20 .. 531         : Inode Table (512 blocks = 2 MiB, 8,192 inodes * 256 bytes)
    - Blocks 532 .. 2579       : Metadata Journal Ring Buffer (2048 blocks = 8 MiB)
    - Total Reserved Metadata  : Exactly 2,580 blocks (Blocks 0..2579, ~10.078 MiB)
    - Blocks 2580 .. 524287    : General Data Blocks (521,708 blocks ~ 1.990 GiB)
====================================================================================================
LBA 67,106,816 .. 67,108,830  : Trailing Alignment Padding (2,015 sectors)
LBA 67,108,831 .. 67,108,862  : Backup Partition Table Array (32 sectors)
LBA 67,108,863                 : Backup GPT Header (Last LBA of disk)
====================================================================================================
```

*Internal Consistency Check:*
- $P1\text{ start } 2048 \ge \text{FirstUsableLBA } 34$.
- $P3\text{ end } 67,106,815 \le \text{LastUsableLBA } 67,108,830$.
- In Partition 2: $1 + 1 + 1 + 236 + 1 + 2048 + 8192 = 10,480$ reserved blocks. Blocks $10,480 \dots 7,732,735$ are data blocks. Zero overlap.
- In Partition 3: $1 + 1 + 1 + 16 + 1 + 512 + 2048 = 2,580$ reserved blocks. Blocks $2,580 \dots 524,287$ are data blocks. Zero overlap.

---

## 4. Redesigned 256-Byte Inode & Directory Record Specification

### 4.1 Inode Structure (`pfs3_inode_t`) — Exactly 256 Bytes

With 64-bit pointers (`uint64_t` = 8 bytes), direct pointers (96 bytes), indirect pointers (24 bytes), metadata (48 bytes), and CRC32 (4 bytes) total 172 bytes, proving that a 128-byte slot cannot fit 64-bit pointers. A 256-byte inode is mandatory:

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
    
    uint32_t checksum;         /* 0xA8..0xAB: CRC32 of bytes 0x00..0xA7 (excludes checksum) */
    uint8_t  reserved[84];     /* 0xAC..0xFF: Expansion pad / inline xattr area (to 256 bytes) */
} __attribute__((packed)) pfs3_inode_t;
```

- Inodes per 4096-byte block: exactly $4096 / 256 = 16\text{ inodes per block}$.
- Checksum Coverage: CRC32 calculated strictly over bytes `0x00..0xA7` (168 bytes); the `checksum` field itself and `reserved` padding are excluded to guarantee deterministic validation.

### 4.2 Directory Record Structure (`pfs3_dirent_t`) — Exactly 64 Bytes

To support large directories while guaranteeing predictable alignment, directory blocks use **Structured Fixed 64-Byte Records** with 64-bit inode pointers:

```c
typedef struct {
    uint64_t inode_num;        /* 0x00: 64-bit inode number (0 = free/deleted entry) */
    uint8_t  file_type;        /* 0x08: 1=REG, 2=DIR, 3=SYMLINK, 4=DEV */
    uint8_t  name_len;         /* 0x09: Exact length of name string in bytes (1..50) */
    uint16_t flags;            /* 0x0A: Alignment pad & entry flags */
    uint16_t checksum;         /* 0x0C: CRC16 of dirent metadata (bytes 0x00..0x0B) */
    uint16_t reserved;         /* 0x0E: Expansion pad */
    char     name[50];         /* 0x10..0x41: UTF-8 / ASCII name string */
} __attribute__((packed)) pfs3_dirent_t;
```

#### Directory Name Semantics, Encoding & Invariants:
1. **Name Encoding:** Strictly UTF-8 / 7-bit ASCII. Null characters (`0x00`) and forward slash (`/`, `0x2F`) are strictly prohibited inside `name`.
2. **`name_len` Semantics:** Stores the exact byte length of the name string, valid range $1 \le \text{name\_len} \le 50$. Does not include trailing null or padding bytes.
3. **Null-Termination Rules:**
   - If $\text{name\_len} < 50$, `name[name_len]` must be set to `\0` on disk.
   - If $\text{name\_len} == 50$, all 50 bytes are significant data. Readers MUST NOT perform unbounded string reads (`strlen(name)`) but must use `name_len`. Kernel string helpers copy `name_len` bytes into a separate 51-byte kernel buffer and append `\0`.
4. **Invalid Name Handling:**
   - Creating/renaming a file with $\text{name\_len} == 0$, $\text{name\_len} > 50$, containing `/`, or containing embedded control characters ($0\text{x01} \dots 0\text{x1F}$) returns `PFS3_ERR_INVALID_NAME`.
   - Encountering a dirent on disk with $\text{inode\_num} \ne 0$ and invalid $\text{name\_len}$ halts traversal with `PFS3_ERR_DIR_CORRUPT`.
5. **Special Directory Invariants:**
   - Slot 0 of directory data block 0: `.` (`name_len = 1`, `name = "."`, points to self inode).
   - Slot 1 of directory data block 0: `..` (`name_len = 2`, `name = ".."`, points to parent directory inode; in Root inode 2, `..` points to Inode 2).
6. **Block Capacity:** Exactly $4096 / 64 = 64\text{ entries per block}$. Directories with $>64$ entries allocate additional 4 KiB blocks through the directory inode's block list.

---

## 5. Non-Circular Boot Chain & Recovery Architecture

### 5.1 Non-Circular Monolithic Boot Sequence

1. **Layer 0 (UEFI Firmware):** Boots from Partition 1 (FAT32 ESP). Loads `/EFI/BOOT/BOOTX64.EFI`.
2. **Layer 1 (Bootloader):** Reads `/EFI/BOOT/KERNEL.ELF` into RAM and jumps to `_start`.
3. **Layer 2 (Monolithic Kernel Core):** The SATA/AHCI driver (`src/drivers/storage/ahci.c`), GPT parser (`src/fs/gpt.c`), and PurpleFS v3 reader are compiled directly into the monolithic kernel binary. **They do not depend on loading external `.apdrv` driver modules from the filesystem.**
4. **Layer 3 (AegisDetect Verification):** Once root is mounted, the kernel reads `/System/manifest.aegis` and verifies secondary drivers before execution.
5. **Layer 4 (APDRV Peripheral Loading):** Peripheral drivers (Audio, Network, USB HID) in `/System/Drivers/` are verified and dynamically loaded.

### 5.2 Independent Recovery Bootability: Proposed vs. Repository Status

- **Repository Baseline (Fact):**  
  In the current source code (`src/kernel/storage.c`), `storage_init()` only discovers block devices and mounts the *first* detected PurpleFS partition. There is currently no command-line root selection (`root=UUID=... recovery=1`) and no dual-mount logic for `/Recovery`.
- **Proposed Architecture [PROPOSED — NOT YET IMPLEMENTED OR VERIFIED]:**
  - **Independent EFI Boot Menu Item:** A dedicated GRUB menu entry passes `root=UUID=<PurpleRecovery-UUID> recovery=1`.
  - **Independent Root Mounting:** When `recovery=1` is specified, the kernel's built-in storage engine mounts Partition 3 (`PurpleRecovery`) directly as the root filesystem, completely bypassing Partition 2. Because Partition 3 has its own independent superblock, bitmaps, inode table, and directory tree, recovery remains fully functional even if Partition 2 is corrupted or erased.
  - **Automated Fallback:** If normal boot is attempted but Partition 2 fails mount validation or critical component hashes, the kernel automatically unmounts Partition 2 and mounts Partition 3 as the emergency rescue root.
- **Recovery Write Boundary Protection:** Partition 3 is locked at the block device layer (`part_dev->read_only = 1`). Writes return `BLOCK_DEV_ERR_RO`. Authenticated updates require verifying an offline signed update package in RAM, issuing an authenticated unlock call, running a software watchdog timer (30 seconds), writing blocks, flushing cache via `ahci_bdev_flush`, and immediately relocks.

---

## 6. Consistent 64-Bit Pointer Design Across All Structures & APIs

Every on-disk structure and public C API function in the PurpleFS v3 specification uses consistent 64-bit pointers and block numbers:

### 6.1 Structures
- **Superblock (`pfs3_superblock_t`):** 4096 bytes. `total_blocks`, `free_blocks`, `total_inodes`, `free_inodes`, `root_inode`, `block_bitmap_start`, `block_bitmap_blocks`, `inode_bitmap_start`, `inode_bitmap_blocks`, `inode_table_start`, `inode_table_blocks`, `journal_start`, `journal_blocks` are all `uint64_t`.
- **Inode (`pfs3_inode_t`):** 256 bytes. `size_bytes`, `direct[12]`, `indirect`, `double_indirect`, `triple_indirect`, timestamps are all `uint64_t`.
- **Directory Entry (`pfs3_dirent_t`):** 64 bytes. `inode_num` is `uint64_t`.
- **Indirect Block (`pfs3_indirect_block_t`):** 4096 bytes. Contains `uint64_t blocks[512]`.
- **Journal Header (`pfs3_journal_header_t`):** `seq_num` and `target_fs_block` are `uint64_t`.

### 6.2 Proposed C API Function Signatures
```c
int pfs3_mount(pfs3_fs_t *fs, block_device_t *bdev, uint64_t part_start_lba, uint64_t part_sector_count);
int pfs3_unmount(pfs3_fs_t *fs);
int pfs3_read_file(pfs3_fs_t *fs, uint64_t inode_num, void *buf, uint64_t count, uint64_t offset, uint64_t *out_read);
int pfs3_write_file(pfs3_fs_t *fs, uint64_t inode_num, const void *buf, uint64_t count, uint64_t offset);
int pfs3_lookup_path(pfs3_fs_t *fs, const char *path, uint64_t *out_inode);
int pfs3_create_entry(pfs3_fs_t *fs, uint64_t parent_inode, const char *name, uint8_t file_type, uint64_t *out_inode);
int pfs3_unlink_entry(pfs3_fs_t *fs, uint64_t parent_inode, const char *name);
int pfs3_alloc_block(pfs3_fs_t *fs, uint64_t *out_fs_block);
int pfs3_free_block(pfs3_fs_t *fs, uint64_t fs_block);
int pfs3_alloc_inode(pfs3_fs_t *fs, uint16_t mode, uint64_t *out_inode);
int pfs3_free_inode(pfs3_fs_t *fs, uint64_t inode_num);
```

---

## 7. Mandatory Future Implementation Gates & Unresolved Decisions

### 7.1 Compile-Time Assertions & Host Test Requirements (Milestone 2)
Future implementation must include explicit compile-time static assertions and host-side serialization tests:

```c
_Static_assert(sizeof(pfs3_superblock_t) == 4096, "pfs3_superblock_t must be exactly 4096 bytes");
_Static_assert(sizeof(pfs3_inode_t) == 256,       "pfs3_inode_t must be exactly 256 bytes");
_Static_assert(sizeof(pfs3_dirent_t) == 64,       "pfs3_dirent_t must be exactly 64 bytes");
_Static_assert(sizeof(pfs3_indirect_block_t) == 4096, "pfs3_indirect_block_t must be exactly 4096 bytes");
_Static_assert(offsetof(pfs3_inode_t, checksum) == 0xA8, "pfs3_inode_t checksum offset mismatch");
_Static_assert(offsetof(pfs3_dirent_t, name) == 0x10, "pfs3_dirent_t name offset mismatch");
```
*Clarification:* These assertions and unit tests do **not** exist in the repository today. They represent mandatory acceptance criteria for Milestone 2.

### 7.2 Unresolved Architectural Decisions (Awaiting Human Review)
1. **Inode Reserved Space Utilization:** Whether to designate bytes `0xAC..0xFF` (84 bytes) for inline extended attributes (xattrs) immediately or leave them as zeroed reserved padding for future ABI expansion.
2. **Journal Dynamic Sizing:** Whether smaller test partitions (<1 GiB) should scale journal size dynamically (e.g. 5% of volume) or retain a fixed minimum 8 MiB journal.
3. **Triple Indirection Driver Scope in Milestone 3:** Whether the initial host prototype must implement triple indirect block traversal immediately or if double indirection (up to 1 GiB) is sufficient for initial test fixtures.

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
  - `checkpoints/20261022-arithmetic-consistency-pass/`: Contains `MANIFEST.txt` and preserved pre-arithmetic drafts.
- **Working Tree Status:** Preserved with existing uncommitted modifications intact.
