# PurpleFS v2 On-Disk Specification & Architecture

**Document Version:** 2.0.0  
**Status:** Approved Specification (Stage 4A Reconstruction)  
**Kernel:** Aegis64 / Purple OS  

---

## 1. Volume Geometry & Layout

PurpleFS v2 is a 64-bit native, crash-resilient journaled filesystem engineered for Purple OS.

### 1.1 Volume Limits
- **Block Size:** 4,096 bytes (`0x1000`).
- **Standard Volume Limit:** 32,768 blocks (134,217,728 bytes = 128 MiB).
- **Minimum Volume Size:** 32 blocks (131,072 bytes = 128 KiB).
- **Historical Volume Discrepancy Note:** While certain historical storage logs mentioned larger partitions (e.g. 4,128,507 blocks), the definitive PurpleFS v2 baseline specification records a 32,768-block general volume specification. This Stage 4A implementation strictly adheres to the 32,768-block volume design where a single 4,096-byte bitmap block covers exactly 32,768 blocks (32,768 bits). Supporting partitions beyond 32,768 blocks requires multiple bitmap blocks (future v2.1 extension).

### 1.2 Block Map Allocation

```
+---------------+---------------+-------------------+---------------------+-----------------+
| Block 0       | Block 1       | Blocks 2 - 9      | Blocks 10 - (N-9)   | Blocks (N-8)-(N-1)|
| Superblock    | Block Bitmap  | Inode Table       | Data Blocks         | Redo Journal    |
| (4096 bytes)  | (4096 bytes)  | (8 blocks, 256 in)| (File & Dir Data)   | (8 blocks, 6 AI)|
+---------------+---------------+-------------------+---------------------+-----------------+
```

- **Block 0: Superblock** (Offset `0x00000000`): Contains magic signature, geometry, counters, feature flags, and volume metadata.
- **Block 1: Block Allocation Bitmap** (Offset `0x00001000`): 4,096 bytes = 32,768 bits. Bit $i = 1$ indicates Block $i$ is allocated; Bit $i = 0$ indicates free.
- **Blocks 2–9: Inode Table** (Offset `0x00002000` to `0x00009FFF`): 8 blocks $\times$ 4,096 bytes = 32,768 bytes. Each inode is 128 bytes, yielding exactly **256 inodes** (Inodes 0..255).
- **Blocks 10 through $(N-9)$: Data Blocks**: Cluster of data blocks for files and directory entries.
- **Blocks $(N-8)$ through $(N-1)$: Redo Journal**: Exactly the final 8 blocks of the volume.

---

## 2. Superblock Layout (Block 0)

All multi-byte integer fields are stored in **Little-Endian** byte order.

| Offset (Bytes) | Field Name | Width (Bytes) | Type | Description |
|---|---|---|---|---|
| `0x0000` | `magic` | 16 | `uint8_t[16]` | Magic signature: `PURPLEFS\0PFS2\0\0\0` |
| `0x0010` | `version` | 4 | `uint32_t` | Filesystem version: `2` |
| `0x0014` | `block_size` | 4 | `uint32_t` | Block size in bytes: `4096` |
| `0x0018` | `total_blocks` | 4 | `uint32_t` | Total blocks in volume (e.g. `32768`) |
| `0x001C` | `free_blocks` | 4 | `uint32_t` | Count of unallocated data blocks |
| `0x0020` | `total_inodes` | 4 | `uint32_t` | Total inodes allocated: `256` |
| `0x0024` | `free_inodes` | 4 | `uint32_t` | Count of unallocated inodes |
| `0x0028` | `bitmap_start_block` | 4 | `uint32_t` | Block index of allocation bitmap: `1` |
| `0x002C` | `bitmap_block_count` | 4 | `uint32_t` | Count of bitmap blocks: `1` |
| `0x0030` | `inode_start_block` | 4 | `uint32_t` | Block index of inode table: `2` |
| `0x0034` | `inode_block_count` | 4 | `uint32_t` | Inode table block count: `8` |
| `0x0038` | `data_start_block` | 4 | `uint32_t` | Block index where data begins: `10` |
| `0x003C` | `journal_start_block` | 4 | `uint32_t` | Start block of redo journal: `total_blocks - 8` |
| `0x0040` | `journal_block_count`| 4 | `uint32_t` | Redo journal block count: `8` |
| `0x0044` | `feature_flags` | 4 | `uint32_t` | Feature flags: Bit 0 = Redo Journal (`0x1`) |
| `0x0048` | `mount_time` | 8 | `uint64_t` | Timestamp of last mount (seconds/ticks) |
| `0x0050` | `write_time` | 8 | `uint64_t` | Timestamp of last write operation |
| `0x0058` | `mount_count` | 4 | `uint32_t` | Incremented on each mount |
| `0x005C` | `clean_unmount` | 4 | `uint32_t` | `1` if clean, `0` if active / crashed |
| `0x0060` | `volume_uuid` | 16 | `uint8_t[16]` | 128-bit unique volume identifier |
| `0x0070` | `volume_label` | 16 | `char[16]` | Null-terminated human label (e.g. `purpledata`) |
| `0x0080` | `checksum` | 4 | `uint32_t` | CRC32 of superblock (`0x0000` to `0x007F`) |
| `0x0084` | `reserved` | 3964 | `uint8_t[3964]` | Reserved for future expansion (zero-filled) |

---

## 3. Inode Layout (128 Bytes)

Each 4,096-byte inode block stores $\frac{4096}{128} = 32$ inodes.  
Inode indices range from `0` to `255`:
- **Inode 0:** Reserved (Null/Invalid).
- **Inode 1:** Root directory (`/`).
- **Inodes 2..255:** Standard files and subdirectories.

| Offset (Bytes) | Field Name | Width (Bytes) | Type | Description |
|---|---|---|---|---|
| `0x00` | `mode` | 2 | `uint16_t` | File type (`0x8000` = File, `0x4000` = Dir) & perms |
| `0x02` | `uid` | 2 | `uint16_t` | User ID of owner |
| `0x04` | `gid` | 2 | `uint16_t` | Group ID of owner |
| `0x06` | `links_count` | 2 | `uint16_t` | Reference count (links to this inode) |
| `0x08` | `size_bytes` | 8 | `uint64_t` | File length in bytes |
| `0x10` | `atime` | 8 | `uint64_t` | Last access time |
| `0x18` | `mtime` | 8 | `uint64_t` | Last modification time |
| `0x20` | `ctime` | 8 | `uint64_t` | Creation time |
| `0x28` | `flags` | 4 | `uint32_t` | Inode flags |
| `0x2C` | `direct_blocks` | 48 | `uint32_t[12]` | 12 Direct block pointers ($12 \times 4096 = 48\text{ KiB}$) |
| `0x5C` | `indirect_block` | 4 | `uint32_t` | 1 Single-indirect block pointer ($1024 \times 4\text{ KiB} = 4\text{ MiB}$) |
| `0x60` | `double_indirect` | 4 | `uint32_t` | 1 Double-indirect block pointer (covers volume limit) |
| `0x64` | `reserved` | 24 | `uint32_t[6]` | Reserved (zero-filled) |
| `0x7C` | `checksum` | 4 | `uint32_t` | CRC32 of inode fields (`0x00` to `0x7B`) |

---

## 4. Directory Entry Layout (32 Bytes)

Directories are stored in standard data blocks allocated to directory inodes. Each directory entry occupies 32 bytes ($4096 / 32 = 128$ entries per data block):

| Offset (Bytes) | Field Name | Width (Bytes) | Type | Description |
|---|---|---|---|---|
| `0x00` | `inode` | 4 | `uint32_t` | Inode number (`0` = unused / unlinked entry) |
| `0x04` | `rec_len` | 2 | `uint16_t` | Entry length (32 bytes) |
| `0x06` | `name_len` | 1 | `uint8_t` | Length of filename string |
| `0x07` | `file_type` | 1 | `uint8_t` | `1` = Regular File, `2` = Directory |
| `0x08` | `name` | 24 | `char[24]` | Filename string (null-terminated if $<24$) |

---

## 5. Redo Journal Protocol & Layout

The Redo Journal occupies the final **8 blocks** of the partition: `[total_blocks - 8, total_blocks - 1]`.

### 5.1 Journal Block Map
- **Block `total_blocks - 8`:** Journal Superblock & Transaction Header (`pfs2_journal_header_t`).
- **Blocks `total_blocks - 7` .. `total_blocks - 2`:** After-Image Data Slots 0..5 (up to 6 blocks).
- **Block `total_blocks - 1`:** Reserved / Journal Commit Tail.

### 5.2 Journal Header Structure (`pfs2_journal_header_t`)
- Location: Block `total_blocks - 8` (4,096 bytes).

| Offset (Bytes) | Field Name | Width (Bytes) | Type | Description |
|---|---|---|---|---|
| `0x000` | `magic` | 4 | `uint32_t` | Journal Magic: `0x5046534A` (`PFSJ`) |
| `0x004` | `journal_state` | 4 | `uint32_t` | State: `0`=EMPTY, `1`=COMMITTED, `2`=APPLIED |
| `0x008` | `sequence_number`| 8 | `uint64_t` | Monotonically increasing transaction sequence ID |
| `0x010` | `num_after_images`| 4 | `uint32_t` | Number of modified blocks in transaction ($1 \le N \le 6$) |
| `0x014` | `target_blocks` | 24 | `uint32_t[6]` | Destination on-disk block index for each after-image |
| `0x02C` | `after_image_crcs`| 24 | `uint32_t[6]` | CRC32 checksum for each 4096-byte after-image block |
| `0x044` | `header_crc` | 4 | `uint32_t` | CRC32 of journal header (`0x000` to `0x043`) |
| `0x048` | `reserved` | 4024 | `uint8_t[4024]` | Reserved (zero-filled) |

### 5.3 Crash Consistency & Durability Protocol
1. **Transaction Staging:** Identify all metadata blocks modified during an operation (e.g. allocation bitmap block 1, inode table block 2, directory data block 10). A transaction can contain up to 6 after-image blocks.
2. **Write After-Images:** Write the modified blocks sequentially into journal blocks `journal_start + 1 + i` ($0 \le i < N$).
3. **Flush Device:** Synchronize after-image blocks to persistent storage.
4. **Write Transaction Header:** Populate target block indices, compute CRC32 for each after-image, set `journal_state = PFS2_JOURNAL_STATE_COMMITTED`, compute `header_crc`, and write Block `journal_start`.
5. **Flush Device (Commit Boundary):** The transaction is now permanently committed. If power loss occurs after this flush, recovery will replay the transaction.
6. **In-Place Application:** Copy each after-image block from the journal to its destination block `target_blocks[i]`.
7. **Flush Device:** Synchronize in-place changes to persistent media.
8. **Mark Transaction Applied:** Set `journal_state = PFS2_JOURNAL_STATE_APPLIED` (or `PFS2_JOURNAL_STATE_EMPTY`), recompute `header_crc`, write Block `journal_start`, and flush.

### 5.4 Mount Recovery Protocol
1. Read Block `journal_start`.
2. Check `magic == 0x5046534A` and verify `header_crc`. If the header is corrupted, the journal is safely rejected without modifying the disk.
3. If `journal_state == PFS2_JOURNAL_STATE_COMMITTED`:
   - Validate $1 \le \text{num\_after\_images} \le 6$.
   - Validate each `target_blocks[i] < total_blocks` (bounds check).
   - Read each after-image block from `journal_start + 1 + i` and verify its CRC32 against `after_image_crcs[i]`.
   - If all after-images pass validation, write each after-image to `target_blocks[i]`.
   - Flush device.
   - Set `journal_state = PFS2_JOURNAL_STATE_APPLIED`, write header, and flush.
4. If `journal_state == PFS2_JOURNAL_STATE_EMPTY` or `PFS2_JOURNAL_STATE_APPLIED`: No action needed.

### 5.5 Journal Correctness & Failure Scenario Matrix

The table below documents recovery behavior across all potential crash and storage failure points:

| Scenario / Point of Failure | On-Disk State at Crash | Recovery Mechanism | Safety & Durability Guarantee |
|---|---|---|---|
| **Crash during journal staging / after-image write** | Journal data blocks partially written; header block remains in previous state (`EMPTY` or `APPLIED`). | Header inspection during mount observes `state != COMMITTED`. | **No-op.** Uncommitted after-images are ignored. Filesystem on disk remains in its prior consistent state. |
| **Crash during journal header write (Torn Write)** | Block `journal_start` partially written or torn across 512-byte physical sectors. | `header_crc` verification fails, or `magic != PFS2_JOURNAL_MAGIC`. | **Fail-Safe Abort.** Mount detects header corruption and returns `PFS2_ERR_CORRUPT`/`PFS2_ERR_JOURNAL` without issuing any writes. Requires filesystem check. |
| **Corrupted header CRC** | Journal header modified on disk or bit rot occurred in transaction header. | `j_hdr->header_crc != expected_crc`. | **Fail-Safe Abort.** Mount returns `PFS2_ERR_CORRUPT`. No in-place writes are replayed. |
| **Corrupted after-images** | Header is valid `COMMITTED`, but after-image payload block has bit rot or was torn. | `pfs2_crc32(ai_buf[i]) != after_image_crcs[i]`. | **Fail-Safe Abort.** Mount detects checksum mismatch across after-images and aborts with `PFS2_ERR_CORRUPT` before overwriting any target blocks. |
| **Crash after commit flush, before in-place writes** | Header is `COMMITTED`, after-images verified, in-place blocks not yet updated. | Replay validates CRCs and writes all after-images in-place, then transitions state to `APPLIED`. | **Full Recovery.** Transaction is guaranteed durable and applied idempotently. |
| **Crash during in-place writes** | Header is `COMMITTED`; some target blocks written, some not written. | Next mount sees `COMMITTED` state, re-verifies all after-images, and re-writes all target blocks. | **Full Recovery.** Replay is idempotent; repeated application restores exact target blocks. |
| **Crash during apply transition / cleanup write** | Target blocks fully written in-place; write to mark header `APPLIED` interrupted. | If header intact: replay reapplies blocks idempotently and marks `APPLIED`. If header torn: header CRC fails (`PFS2_ERR_CORRUPT`). | **Documented Limitation.** If torn during `APPLIED` rewrite, on-disk target blocks already have valid data, but header CRC fails. Volume must be remounted with clean state or manual journal clear. |
| **Device Flush / Sync Failures** | Underlying controller rejects `flush()` (e.g. disk write cache full or I/O failure). | Commit immediately returns `PFS2_ERR_IO` before advancing transaction. | **Immediate Error.** Transaction fails at caller; no un-synced commit is recorded. |

> [!IMPORTANT]
> **Atomicity Assumption Boundary:** A 4096-byte block write is not natively atomic on 512-byte sector storage. PurpleFS v2 guarantees atomic transactions by enforcing that a transaction is only recognized if the journal header passes complete CRC32 verification over all header fields and every after-image matches its recorded CRC32. A torn 4096-byte header write will fail CRC32 verification and safely block replay rather than applying corrupt metadata.

---

## 6. Host-Side Block Device Abstraction

The filesystem core is completely decoupled from hardware ATA controllers and host OS system calls through a block-device interface:

```c
typedef struct pfs2_block_dev {
    uint32_t block_count;
    int (*read_block)(struct pfs2_block_dev *dev, uint32_t block_num, void *buf);
    int (*write_block)(struct pfs2_block_dev *dev, uint32_t block_num, const void *buf);
    int (*flush)(struct pfs2_block_dev *dev);
    void *priv;
} pfs2_block_dev_t;
```

Two backend drivers are provided:
1. **Memory Block Device:** Emulates a zero-latency disk in host RAM for fast unit tests.
2. **File Block Device:** Backed by an image file on disk (`open`, `pread`, `pwrite`, `fsync`) for persistence and restart tests.
3. **Fault-Injecting Block Device:** Wraps any block device to simulate power cuts, I/O read errors, write errors, and flush failures at exact transaction boundaries.
