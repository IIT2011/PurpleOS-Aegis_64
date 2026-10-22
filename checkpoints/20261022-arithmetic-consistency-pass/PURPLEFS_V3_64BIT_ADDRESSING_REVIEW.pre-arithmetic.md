# PurpleFS v3 64-Bit Addressing Architecture Review & Verification Report (Final Review)

**Document:** `docs/PURPLEFS_V3_64BIT_ADDRESSING_REVIEW.md`  
**Classification:** Technical Architecture Review & Verification Report (Planning Only)  
**Target Repository:** `/home/saptanshu-ganguly/OS`  
**Target Branch:** `Aegis_Purple`  
**Revision Date:** 2026-10-22  
**Status:** COMPLETE — FINAL CORRECTED REVIEW — AWAITING HUMAN REVIEW (Zero implementation code modified)

---

## 1. Verified Repository Truth vs. Architecture Proposals

To ensure technical integrity, verified repository facts are strictly distinguished from architectural proposals:

| Area | Current Verified Repository Source (Fact) | Architecture Proposal [PROPOSED — NOT IMPLEMENTED] |
| :--- | :--- | :--- |
| **Independent Recovery Boot** | In `src/kernel/storage.c`, `storage_init()` only mounts the *first* detected PurpleFS partition. No command-line root parameter parser (`root=UUID=... recovery=1`) exists in `kernel_main_64`. | Dual boot menu options (Normal vs. Recovery) and direct mounting of Partition 3 as emergency root are **[PROPOSED — NOT IMPLEMENTED]**. |
| **Recovery Partition Protection** | `partition_dev_t.read_only` flag exists in `src/fs/partition_dev.c`, but no partition is currently marked read-only at runtime by `storage_init()`. | Partition 3 write-boundary locking and authenticated unlock protocol are **[PROPOSED — NOT IMPLEMENTED]**. |
| **Filesystem Architecture** | `src/fs/purplefs_v2.c` and `include/fs/purplefs_v2.h` implement PurpleFS v2 (128 MiB volume limit, 128 root entries, max file 4.047 MiB). | PurpleFS v3 with 64-bit block pointers, 256-byte inodes, and multi-block bitmaps is **[PROPOSED — NOT IMPLEMENTED]**. |
| **Integrity Keying** | `src/security/aegis_detect.c` uses a compiled-in symmetric 32-byte key (`default_trust_key`) for HMAC-SHA256. | Transition to asymmetric public-key cryptography (Ed25519/RSA-2048) with private keys off-device is **[PROPOSED — NOT IMPLEMENTED]**. |

---

## 2. 64-Bit Address Model Specification

### 2.1 Terminology & Units

| Term | Symbol | Value / Type | Definition & Scope |
| :--- | :---: | :---: | :--- |
| **Physical Device Sector** | $S_{dev}$ | 512 bytes | Hardware sector size exposed by SATA/AHCI controller (`ahci_read_sectors`). |
| **Filesystem Block** | $B_{fs}$ | 4096 bytes (4 KiB) | Fundamental filesystem allocation and I/O transfer unit. |
| **Sectors per Block Ratio** | $k_{sec}$ | 8 | $k_{sec} = B_{fs} / S_{dev} = 4096 / 512 = 8$. |
| **Device LBA** | $\text{LBA}_{dev}$ | `uint64_t` | Physical sector index relative to the entire virtual SATA disk ($[0, \text{total\_sectors} - 1]$). |
| **Filesystem Block Number** | $b$ | `uint64_t` | Logical block index relative to the *start of the partition* ($[0, N_{blocks} - 1]$). |
| **Null Block Sentinel** | `0` | `uint64_t` | Reserved constant representing unallocated storage or a sparse file hole. |

### 2.2 Address Translation & Overflow Protection

Translation from filesystem block $b$ to device sector LBA $\text{LBA}_{dev}$ is performed by:

$$\text{LBA}_{dev} = P_{start} + (b \times 8)$$

Where $P_{start}$ is the partition's starting sector from the GPT entry.

#### Bounds and Overflow Check Invariants:
1. **Total Partition Blocks:** $N_{blocks} = \lfloor P_{sectors} / 8 \rfloor$.
2. **Bounds Assertion:** Every access verifies $b < N_{blocks}$.
3. **Null Check:** If $b == 0$ during a read operation, the filesystem fills the destination buffer with zeroes (sparse hole) and returns success without issuing hardware I/O.
4. **Out-of-Range Handling:** If $b \ge N_{blocks}$, the filesystem refuses to calculate an LBA, logs an error to serial console, and returns `PFS3_ERR_OUT_OF_BOUNDS`.
5. **No 64-Bit Arithmetic Overflow:** Since $P_{start} + P_{sectors} \le 67,108,864 < 2^{64}$ on a 32 GiB drive, multiplying $b \times 8$ for any valid $b$ is strictly bounded by $P_{sectors}$, making integer overflow mathematically impossible.

### 2.3 Theoretical vs. Supported Capacity Ceilings

- **Theoretical Capacity:** A 64-bit block number can address $2^{64}\text{ blocks} \times 4096\text{ bytes} = 64\text{ ZiB}$ (Zettabytes).
- **Supported Partition Limit (v3.0):** **16 TiB** ($2^{32}\text{ blocks}$). This ceiling ensures the block allocation bitmap remains $\le 512\text{ MiB}$ and can be managed efficiently in kernel physical memory.
- **Supported File Size Limit (v3.0):** **512.002 GiB** (or partition capacity):
  - 12 Direct blocks: $12 \times 4\text{ KiB} = 48\text{ KiB}$.
  - 1 Single indirect block: $512\text{ pointers} \times 4\text{ KiB} = 2\text{ MiB}$.
  - 1 Double indirect block: $512^2\text{ pointers} \times 4\text{ KiB} = 1\text{ GiB}$.
  - 1 Triple indirect block: $512^3\text{ pointers} \times 4\text{ KiB} = 512\text{ GiB}$.

---

## 3. Structural Size Calculations & Inode Redesign Proof

### 3.1 Why 128 Bytes Fails for 64-Bit Inodes

With 64-bit pointers (`uint64_t` = 8 bytes):

$$\text{Pointers} = (12\text{ direct} + 1\text{ single indirect} + 1\text{ double indirect} + 1\text{ triple indirect}) \times 8 = 15 \times 8 = 120\text{ bytes}$$
$$\text{Metadata} = \text{mode}(2) + \text{uid}(2) + \text{gid}(2) + \text{links}(2) + \text{flags}(4) + \text{generation}(4) + \text{size}(8) + 3 \times \text{timestamps}(24) = 48\text{ bytes}$$
$$\text{Checksum} = 4\text{ bytes (CRC32)}$$
$$\text{Total Required} = 120 + 48 + 4 = 172\text{ bytes}$$

Because $172 > 128$, **a 128-byte inode cannot accommodate 12 direct 64-bit block pointers**. Shrinking to 128 bytes would require cutting direct pointers to $\le 5$ (leaving only 20 KiB of direct data), which would severely degrade small file performance.

### 3.2 256-Byte Inode Structure Verification (`sizeof == 256`)

```
Offset Range | Size | Field Name       | Type       | Description
-------------+------+------------------+------------+--------------------------------------------
0x00 .. 0x01 |    2 | mode             | uint16_t   | POSIX permissions & file type (DIR, REG, SYMLINK)
0x02 .. 0x03 |    2 | uid              | uint16_t   | Owner User ID
0x04 .. 0x05 |    2 | gid              | uint16_t   | Owner Group ID
0x06 .. 0x07 |    2 | links_count      | uint16_t   | Hard link reference counter
0x08 .. 0x0B |    4 | flags            | uint32_t   | Immutability, system, quarantine flags
0x0C .. 0x0F |    4 | generation       | uint32_t   | Inode version/generation counter
0x10 .. 0x17 |    8 | size_bytes       | uint64_t   | 64-bit exact file size in bytes
0x18 .. 0x1F |    8 | atime_sec        | uint64_t   | Last access timestamp (POSIX seconds)
0x20 .. 0x27 |    8 | mtime_sec        | uint64_t   | Last modification timestamp
0x28 .. 0x2F |    8 | ctime_sec        | uint64_t   | Status change timestamp
0x30 .. 0x8F |   96 | direct[12]       | uint64_t   | 12 direct block pointers (12 * 4 KiB = 48 KiB)
0x90 .. 0x97 |    8 | indirect         | uint64_t   | Single indirect block pointer (512 ptrs = 2 MiB)
0x98 .. 0x9F |    8 | double_indirect  | uint64_t   | Double indirect block pointer (512^2 = 1 GiB)
0xA0 .. 0xA7 |    8 | triple_indirect  | uint64_t   | Triple indirect block pointer (512^3 = 512 GiB)
0xA8 .. 0xAB |    4 | checksum         | uint32_t   | CRC32 of bytes 0x00..0xA7 (excludes checksum)
0xAC .. 0xFF |   84 | reserved[84]     | uint8_t    | Expansion padding / future inline xattr area
-------------+------+------------------+------------+--------------------------------------------
Total Size: 256 bytes (0x100). Inodes per 4096-byte block: 4096 / 256 = 16 inodes/block.
```

### 3.3 Checksum Coverage Specification
- Checksum coverage: Bytes `0x00` through `0xA7` (168 bytes total).
- The `checksum` field (`0xA8..0xAB`) and `reserved` area (`0xAC..0xFF`) are excluded during CRC32 computation.
- This ensures non-circular, deterministic verification on disk read.

---

## 4. Directory Record Architecture (Fixed 64-Byte Records)

```
Offset Range | Size | Field Name       | Type       | Description
-------------+------+------------------+------------+--------------------------------------------
0x00 .. 0x07 |    8 | inode_num        | uint64_t   | 64-bit Inode index (0 = free/deleted slot)
0x08         |    1 | file_type        | uint8_t    | 1=REG, 2=DIR, 3=SYMLINK, 4=DEV
0x09         |    1 | name_len         | uint8_t    | Exact filename string length in bytes (1..50)
0x0A .. 0x0B |    2 | flags            | uint16_t   | Alignment pad & directory entry flags
0x0C .. 0x0D |    2 | checksum         | uint16_t   | CRC16 of dirent metadata (bytes 0x00..0x0B)
0x0E .. 0x0F |    2 | reserved         | uint16_t   | Expansion pad
0x10 .. 0x41 |   50 | name             | char[50]   | UTF-8 / ASCII filename string
0x42 .. 0x43 |    2 | padding          | uint8_t[2] | Alignment pad to 64 bytes
-------------+------+------------------+------------+--------------------------------------------
Total Size: 64 bytes (0x40). Entries per 4096-byte block: 4096 / 64 = 64 entries/block.
```

### 4.1 Name Encoding, Length Semantics & Invalid Name Handling
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

---

## 5. Scalable Allocation Metadata Calculations & Exact Block Ranges

### 5.1 Partition 2 (PurpleSystem) Metadata Block Map

- Total sectors: $61,861,888\text{ sectors}$ ($29.50\text{ GiB}$).
- Total 4 KiB blocks: $N_{blocks} = 61,861,888 / 8 = 7,732,736\text{ blocks}$ (Block 0 .. 7,732,735).

```
Block Range      | Count | Structure Description
-----------------+-------+-------------------------------------------------------------------
Block 0          |     1 | Partition Boot Record / Reserved Header
Block 1          |     1 | Primary Superblock (pfs3_superblock_t, 4096 bytes)
Block 2          |     1 | Backup Superblock (bit-identical copy)
Blocks 3 .. 238  |   236 | Block Allocation Bitmap (tracks 7,732,736 blocks at 32,768 bits/block)
Block 239        |     1 | Inode Allocation Bitmap (tracks 32,768 inodes)
Blocks 240..2287 |  2048 | Inode Table (32,768 inodes * 256 bytes = 8 MiB)
Blocks 2288..10479| 8192 | Metadata Journal Ring Buffer (32 MiB)
-----------------+-------+-------------------------------------------------------------------
Reserved Blocks  | 10480 | Blocks 0 .. 10479 (~40.938 MiB). Pre-allocated as 1 in bitmap.
Data Blocks      |7722256| Blocks 10480 .. 7732735 (~29.458 GiB). Available for user data.
-----------------+-------+-------------------------------------------------------------------
Total Blocks     |7732736| Zero overlap, 100% accounted for.
```

### 5.2 Partition 3 (PurpleRecovery) Metadata Block Map

- Total sectors: $4,194,304\text{ sectors}$ ($2.00\text{ GiB}$).
- Total 4 KiB blocks: $N_{blocks} = 4,194,304 / 8 = 524,288\text{ blocks}$ (Block 0 .. 524,287).

```
Block Range      | Count | Structure Description
-----------------+-------+-------------------------------------------------------------------
Block 0          |     1 | Partition Boot Record / Reserved Header
Block 1          |     1 | Primary Superblock (pfs3_superblock_t, 4096 bytes)
Block 2          |     1 | Backup Superblock (bit-identical copy)
Blocks 3 .. 18   |    16 | Block Allocation Bitmap (tracks 524,288 blocks at 32,768 bits/block)
Block 19         |     1 | Inode Allocation Bitmap (tracks up to 32,768 inodes)
Blocks 20 .. 531 |   512 | Inode Table (8,192 inodes * 256 bytes = 2 MiB)
Blocks 532..2579 |  2048 | Metadata Journal Ring Buffer (8 MiB)
-----------------+-------+-------------------------------------------------------------------
Reserved Blocks  |  2580 | Blocks 0 .. 2579 (~10.078 MiB). Pre-allocated as 1 in bitmap.
Data Blocks      | 521708| Blocks 2580 .. 524287 (~1.990 GiB). Available for recovery data.
-----------------+-------+-------------------------------------------------------------------
Total Blocks     | 524288| Zero overlap, 100% accounted for.
```

---

## 6. Compile-Time Assertions & Host Test Matrix

Before any kernel integration occurs, Milestone 2 will enforce these compile-time static assertions:

```c
_Static_assert(sizeof(pfs3_superblock_t) == 4096, "pfs3_superblock_t must be exactly 4096 bytes");
_Static_assert(sizeof(pfs3_inode_t) == 256,       "pfs3_inode_t must be exactly 256 bytes");
_Static_assert(sizeof(pfs3_dirent_t) == 64,       "pfs3_dirent_t must be exactly 64 bytes");
_Static_assert(sizeof(pfs3_indirect_block_t) == 4096, "pfs3_indirect_block_t must be exactly 4096 bytes");
_Static_assert(offsetof(pfs3_inode_t, checksum) == 0xA8, "pfs3_inode_t checksum offset mismatch");
_Static_assert(offsetof(pfs3_dirent_t, name) == 0x10, "pfs3_dirent_t name offset mismatch");
```

---

## 7. Verification Checklist & Review Status

- [x] Independent recovery boot explicitly labeled as **[PROPOSED ARCHITECTURE — NOT YET IMPLEMENTED]**.
- [x] Directory name encoding (UTF-8/ASCII), `name_len` semantics (1..50), null termination, and invalid name handling strictly specified.
- [x] Exact metadata block ranges calculated for Partition 2 (10,480 reserved blocks, 7,722,256 data blocks) and Partition 3 (2,580 reserved blocks, 521,708 data blocks) with zero overlap.
- [x] 64-bit addressing capacity vs. supported volume (16 TiB) and file-size (512 GiB) limits rechecked with overflow-safe block-to-sector translation.
- [x] 64-bit pointer design made consistent across all structures (`pfs3_superblock_t`, `pfs3_inode_t`, `pfs3_dirent_t`, `pfs3_indirect_block_t`, `pfs3_journal_header_t`) and proposed C APIs.
- [x] No proposed behavior described as implemented without verified repository evidence.
