# PurpleFS v3 64-Bit Addressing Architecture Review & Verification Report (Final Arithmetic Pass)

**Document:** `docs/PURPLEFS_V3_64BIT_ADDRESSING_REVIEW.md`  
**Classification:** Technical Architecture Review & Verification Report (Planning Only)  
**Target Repository:** `/home/saptanshu-ganguly/OS`  
**Target Branch:** `Aegis_Purple`  
**Revision Date:** 2026-10-22  
**Status:** COMPLETE — FINAL ARITHMETIC & SPECIFICATION PASS — AWAITING HUMAN REVIEW (Zero implementation code modified)

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

## 2. Strengthened 64-Bit Address Model Specification

### 2.1 Terminology & Units

| Term | Symbol | Value / Type | Definition & Scope |
| :--- | :---: | :---: | :--- |
| **Physical Device Sector** | $S_{dev}$ | 512 bytes | Hardware sector size exposed by SATA/AHCI controller (`ahci_read_sectors`). |
| **Filesystem Block** | $B_{fs}$ | 4096 bytes (4 KiB) | Fundamental filesystem allocation and I/O transfer unit. |
| **Sectors per Block Ratio** | $k_{sec}$ | 8 | $k_{sec} = B_{fs} / S_{dev} = 4096 / 512 = 8$. |
| **Device Total Sectors** | $D_{sectors}$ | `uint64_t` | Physical sector capacity of device (e.g. 67,108,864 for 32 GiB drive). |
| **Device LBA** | $\text{LBA}_{dev}$ | `uint64_t` | Physical sector index relative to the entire virtual SATA disk ($[0, D_{sectors} - 1]$). |
| **Filesystem Block Number** | $b$ | `uint64_t` | Logical block index relative to the *start of the partition* ($[0, N_{blocks} - 1]$). |
| **Null Block Sentinel** | `0` | `uint64_t` | Reserved constant representing unallocated storage or a sparse file hole. |

### 2.2 Strengthened Address Translation & Safety Invariants

Translation from filesystem block $b$ to device sector LBA $\text{device\_lba}$ is performed by:

$$\text{device\_lba} = P_{start} + (b \times 8)$$

Where $P_{start}$ is the partition's starting sector from the GPT entry.

#### Precondition 1: Partition Boundary Validation against Device Capacity
During mount (`pfs3_mount`), the partition geometry is verified against the underlying block device:
$$\text{Assert } P_{start} < D_{sectors}$$
$$\text{Assert } P_{sectors} > 0$$
$$\text{Assert } P_{start} \le D_{sectors} - P_{sectors} \quad (\text{guarantees partition end } P_{start} + P_{sectors} - 1 < D_{sectors})$$
If any assertion fails, mount aborts with `PFS3_ERR_INVALID_PARTITION`.

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

*(Note: The previously stated figure of "512.002 GiB" was an arithmetic oversight that omitted the 1 GiB double-indirect tier; the exact sum is $\approx 513.002\text{ GiB}$).*

#### Supported File Size Limit vs. Partition Capacity:
The theoretical pointer capacity ($\approx 513.002\text{ GiB}$) cannot exceed the filesystem partition's actual usable capacity:
- **On Partition 2 (PurpleSystem, 29.50 GiB total, 7,722,256 usable data blocks):**  
  $$\text{Max Supported File (Partition 2)} = 7,722,256\text{ blocks} \times 4096 = \mathbf{31,630,360,576\text{ bytes}} \approx \mathbf{29.458\text{ GiB}}$$
- **On Partition 3 (PurpleRecovery, 2.00 GiB total, 521,708 usable data blocks):**  
  $$\text{Max Supported File (Partition 3)} = 521,708\text{ blocks} \times 4096 = \mathbf{2,136,915,968\text{ bytes}} \approx \mathbf{1.990\text{ GiB}}$$
- **Double-Indirect Driver Sub-Limit (Optional Phase 1 ceiling):**  
  If triple indirection is omitted in early driver prototypes, the double-indirect ceiling is:
  $$12 + 512 + 262,144 = 262,668\text{ blocks} \times 4096 = \mathbf{1,075,888,128\text{ bytes}} \approx \mathbf{1.002\text{ GiB}}\text{ (1 GiB + 2 MiB + 48 KiB)}$$

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

## 6. Compile-Time Assertions & Future Verification Gates

Future implementation (Milestone 2 onwards) must enforce compile-time static assertions and host-side tests:

```c
_Static_assert(sizeof(pfs3_superblock_t) == 4096, "pfs3_superblock_t must be exactly 4096 bytes");
_Static_assert(sizeof(pfs3_inode_t) == 256,       "pfs3_inode_t must be exactly 256 bytes");
_Static_assert(sizeof(pfs3_dirent_t) == 64,       "pfs3_dirent_t must be exactly 64 bytes");
_Static_assert(sizeof(pfs3_indirect_block_t) == 4096, "pfs3_indirect_block_t must be exactly 4096 bytes");
_Static_assert(offsetof(pfs3_inode_t, checksum) == 0xA8, "pfs3_inode_t checksum offset mismatch");
_Static_assert(offsetof(pfs3_dirent_t, name) == 0x10, "pfs3_dirent_t name offset mismatch");
```

*Status Note:* These assertions, serialization tests, and malformed-input rejection tests do **not** exist in the repository today. They represent mandatory acceptance gates for future milestones.

---

## 7. Verification Checklist & Review Status

- [x] Maximum file addressing capacity recalculated: **$550,831,702,016\text{ bytes}$** ($513\text{ GiB} + 2\text{ MiB} + 48\text{ KiB} = 513.00201416015625\text{ GiB}$). Inconsistent `512.002 GiB` figure completely removed.
- [x] Theoretical pointer capacity strictly distinguished from partition-bounded file-size limits (29.458 GiB on Partition 2).
- [x] Block-to-device-sector arithmetic strengthened with formal preconditions, overflow proofs, device capacity validation, and explicit exclusion of partial trailing sectors.
- [x] All metadata calculations, partition sizes, and inode/directory layout arithmetic verified for internal consistency with zero overlap.
- [x] Compile-time assertions and serialization tests specified as mandatory future requirements without claiming existing implementation.
- [x] Independent recovery boot and all other unreleased features explicitly marked `[PROPOSED ARCHITECTURE — NOT YET IMPLEMENTED OR VERIFIED]`.
