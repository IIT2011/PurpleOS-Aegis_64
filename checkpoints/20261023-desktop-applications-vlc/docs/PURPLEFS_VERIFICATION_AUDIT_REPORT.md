# Purple OS / Aegis64 — PurpleFS v2 Verification-First Audit Report

**Document ID:** `AUDIT-PFS2-20261022`  
**Date:** 2026-10-22  
**Operating System:** Purple OS  
**Kernel:** Aegis64 (64-bit native)  
**Active Branch:** `Aegis_Purple`  
**Git Baseline Commit:** `409c0cd00a7a074d50f2b8e0838aff466145e017`  
**Status:** Verification Audit Completed — 100% Passing  

---

## 1. Executive Summary

This forensic verification audit evaluated the stability, correctness, boundary limits, failure modes, and persistence of the **PurpleFS v2** native crash-resilient journaled filesystem.

In accordance with strict safety mandates:
- No filesystem core rewrites or speculative features were introduced.
- No physical host disks (`/dev/sd*`, `/dev/nvme*`) were touched.
- All testing utilized in-memory block devices and disposable disk images in `/tmp`.
- Every reported test was executed against current source and verified live.

### Key Audit Metrics
- **Host Test Suites:** 16/16 suites passed (100%).
- **PurpleFS v2 Host Cases:** 25/25 test cases passed (expanded from 18 to 25 focused tests).
- **Sanitizer Verification:** 16/16 test suites passed under GCC AddressSanitizer and UndefinedBehaviorSanitizer (`-fsanitize=address,undefined`) with 0 errors, 0 leaks, and 0 undefined behaviors.
- **QEMU Integration Tests:** 8/8 virtual machine check targets passed (100%), including single-core scheduling, SMP multi-core, read-only storage, writable storage, multi-boot persistence, AHCI/NVMe modern storage, graphics rendering, and interactive desktop input.

---

## 2. Step 1 — Architectural Inventory & Specifications

### 2.1 Filesystem Implementation & Public APIs
- **Implementation File:** [src/fs/purplefs_v2.c](file:///home/saptanshu-ganguly/OS/src/fs/purplefs_v2.c)
- **Header Files:** [include/purplefs_v2.h](file:///home/saptanshu-ganguly/OS/include/purplefs_v2.h), [include/host_purplefs_v2.h](file:///home/saptanshu-ganguly/OS/include/host_purplefs_v2.h)
- **Kernel Adapter:** [src/kernel/storage.c](file:///home/saptanshu-ganguly/OS/src/kernel/storage.c)

#### Public API Catalog
| Function | Signature | Purpose & Error Contracts |
|---|---|---|
| `pfs2_crc32` | `uint32_t pfs2_crc32(const void *data, size_t length)` | Computes standard CRC32 (polynomial `0xEDB88320`) for integrity verification. |
| `pfs2_format` | `int pfs2_format(pfs2_block_dev_t *dev, uint32_t total_blocks, const char *label)` | Formats block device with superblock, bitmap, 256 inodes, root directory (`.` and `..`), and 8-block redo journal. Returns `PFS2_OK` or `PFS2_ERR_*`. |
| `pfs2_mount` | `int pfs2_mount(pfs2_fs_t *fs, pfs2_block_dev_t *dev)` | Mounts filesystem, validates magic `"PURPLEFS\0PFS2\0\0\0"`, validates geometry & CRC, and replays unapplied redo journal transactions if an interrupted commit is detected. |
| `pfs2_unmount` | `int pfs2_unmount(pfs2_fs_t *fs)` | Flushes dirty metadata, marks `clean_unmount = 1`, commits superblock, and resets mount state. |
| `pfs2_lookup` | `int pfs2_lookup(pfs2_fs_t *fs, const char *name, uint32_t *out_inode)` | Searches root directory (Inode 1) for filename `name`. Returns `PFS2_OK` on match or `PFS2_ERR_NOT_FOUND`. |
| `pfs2_create_file` | `int pfs2_create_file(pfs2_fs_t *fs, const char *name, uint16_t mode, uint32_t *out_inode)` | Allocates free inode, adds 32-byte directory entry to root directory, and commits transaction via journal. Rejects duplicates (`PFS2_ERR_EXISTS`) and names $\ge 24$ chars (`PFS2_ERR_INVALID`). |
| `pfs2_read_file` | `int pfs2_read_file(pfs2_fs_t *fs, uint32_t inode_num, void *buf, size_t count, uint64_t offset, size_t *out_bytes_read)` | Reads file data through direct blocks (0..11) and single indirect block. Rejects directory inodes (`PFS2_ERR_INVALID`). Returns zeroes for unallocated sparse block holes. Returns 0 bytes read at EOF. |
| `pfs2_write_file` | `int pfs2_write_file(pfs2_fs_t *fs, uint32_t inode_num, const void *buf, size_t count, uint64_t offset, size_t *out_bytes_written)` | Allocates blocks, writes data blocks to media, updates inode size/timestamps, and commits metadata changes (inode, bitmap, indirect table, superblock) through redo journal. |
| `pfs2_truncate_file`| `int pfs2_truncate_file(pfs2_fs_t *fs, uint32_t inode_num, uint64_t new_size)` | Shrinks file, deallocates unneeded direct/indirect blocks back to bitmap, updates inode size, and commits via journal. |
| `pfs2_read_inode` | `int pfs2_read_inode(pfs2_fs_t *fs, uint32_t inode_num, pfs2_inode_t *out_inode)` | Reads 128-byte inode record from disk and validates inode CRC32 (`0x7C`). |
| `pfs2_sync` | `int pfs2_sync(pfs2_fs_t *fs)` | Flushes block device write cache to persistent storage. |
| `pfs2_journal_replay_if_needed` | `int pfs2_journal_replay_if_needed(pfs2_fs_t *fs)` | Mount recovery routine: replays committed after-images to destination blocks if crash occurred during journal commit. |

---

### 2.2 Internal State Representation
- **`pfs2_block_dev_t`:** Abstract block device interface providing:
  - `block_count`: Device capacity in 4,096-byte blocks.
  - `read_block(dev, lba, buf)`: Synchronous 4,096-byte block read.
  - `write_block(dev, lba, buf)`: Synchronous 4,096-byte block write (NULL for read-only).
  - `flush(dev)`: Synchronizes device write caches to non-volatile media.
- **`pfs2_fs_t`:** Active filesystem instance tracking:
  - `dev`: Pointer to backing `pfs2_block_dev_t`.
  - `sb`: Cached 4,096-byte superblock (`pfs2_superblock_t`).
  - `bitmap[4096]`: In-memory block allocation bitmap covering up to 32,768 volume blocks.
  - `next_journal_seq`: Monotonic journal sequence counter.
  - `is_mounted`: Boolean flag (1 = mounted, 0 = unmounted).
  - `read_only`: Boolean flag (1 = read-only mode, 0 = read-write mode).

---

### 2.3 Quantitative Limits & Boundaries

| Parameter | Current Limit | Enforcement & Architectural Reason |
|---|---|---|
| **Block Size** | 4,096 bytes (`PFS2_BLOCK_SIZE`) | Fixed on-disk geometry. |
| **Max Filename Length** | **23 characters** (plus null terminator) | `pfs2_dir_entry_t.name` is `char[24]`. `pfs2_create_file()` validates `name_len < 24`. |
| **Max File Size** | **4,243,456 bytes (~4.047 MiB)** | 12 direct blocks ($12 \times 4\text{ KiB} = 48\text{ KiB}$) + 1024 indirect blocks ($1024 \times 4\text{ KiB} = 4\text{ MiB}$). Offsets beyond this return `PFS2_ERR_TOO_BIG`. |
| **Max Root Directory Entries** | **128 entries** (126 user files + `.` + `..`) | Root directory resides in single data block (Block 10). Each entry is 32 bytes ($4096 / 32 = 128$). Additional directory blocks are `NOT IMPLEMENTED`. |
| **Total Inodes** | **256 inodes** (Inodes 0..255) | 8 inode blocks $\times$ 32 inodes/block = 256 inodes. Inode 0 is reserved; Inode 1 is root directory; Inodes 2..255 are files. |
| **Max Volume Size** | **32,768 blocks (128 MiB)** | Bitmap occupies 1 block (Block 1): 4,096 bytes $\times$ 8 bits = 32,768 bits. Multi-block bitmaps are `NOT IMPLEMENTED`. |
| **Max Open Files** | **NOT IMPLEMENTED** | The filesystem layer is completely stateless (`pfs2_read_file` / `pfs2_write_file` operate by inode number and byte offset). There is currently no kernel open-file descriptor table (`open`, `close`, `fd`). |

---

### 2.4 Unsupported & Unimplemented Behaviors

The following features were inspected in source code and explicitly cataloged as **`NOT IMPLEMENTED`**:

1. **Subdirectories & Hierarchical Paths:**
   - Subdirectory creation (`pfs2_mkdir()`): **`NOT IMPLEMENTED`**
   - Subdirectory removal (`pfs2_rmdir()`): **`NOT IMPLEMENTED`**
   - Path resolution / hierarchical traversal (`/folder/file.txt`): **`NOT IMPLEMENTED`** (`pfs2_lookup()` only searches root directory Block 10 for single flat names).
2. **File Deletion & Unlink:**
   - File removal / unlinking (`pfs2_unlink()` / `pfs2_remove()`): **`NOT IMPLEMENTED`** (no API exists to clear directory entries and free associated data blocks/inodes).
3. **File Renaming:**
   - File renaming / moving (`pfs2_rename()`): **`NOT IMPLEMENTED`**.
4. **Double-Indirect Block Addressing:**
   - The inode structure reserves `double_indirect`, but both `pfs2_read_file()` and `pfs2_write_file()` return `PFS2_ERR_TOO_BIG` when `block_idx >= 12 + 1024`. Double-indirect expansion: **`NOT IMPLEMENTED`**.
5. **Multi-Block Directory Growth:**
   - Expanding directories beyond a single 4,096-byte block (128 entries): **`NOT IMPLEMENTED`**.
6. **Multi-Block Allocation Bitmaps:**
   - Volumes exceeding 32,768 blocks (128 MiB): **`NOT IMPLEMENTED`**.
7. **POSIX File Descriptors & State:**
   - POSIX file table (`open()`, `close()`, `read()`, `write()`, `lseek()`, `dup()`): **`NOT IMPLEMENTED`**.
8. **Discretionary Access Control & Permissions:**
   - Mode permissions (`0755`, `0644`) are persisted on disk, but runtime permission enforcement against UID/GID: **`NOT IMPLEMENTED`** (the system executes as single-user kernel mode).

---

## 3. Step 2 & Step 3 — Verification Test Matrix

The test suite in [tests/test_purplefs_v2.c](file:///home/saptanshu-ganguly/OS/tests/test_purplefs_v2.c) was expanded with 7 rigorous, focused test cases addressing boundary conditions, EOF handling, invalid paths, and remount persistence.

### Complete PurpleFS v2 Host Test Results (25/25 Passed)

| Test ID | Test Name | Focus Area | Status |
|---|---|---|---|
| `TEST 01` | `test_format_and_mount` | Baseline format, superblock geometry, and mount/unmount cycle | **PASSED** |
| `TEST 02` | `test_invalid_signature_and_corrupt_sb` | Magic byte validation and superblock CRC failure rejection | **PASSED** |
| `TEST 03` | `test_volume_size_limits` | Volume boundaries (< 32 blocks rejected, > 32768 blocks rejected) | **PASSED** |
| `TEST 04` | `test_file_create_and_lookup` | Inode allocation, duplicate rejection (`PFS2_ERR_EXISTS`), lookup | **PASSED** |
| `TEST 05` | `test_file_read_write_small` | Basic read/write and string content verification | **PASSED** |
| `TEST 06` | `test_empty_and_multiblock_files` | Multi-block direct (10 blks) and indirect (16 blks) addressing | **PASSED** |
| `TEST 07` | `test_truncate_file` | File truncation from 16 blocks to 5 blocks, block freeing to bitmap | **PASSED** |
| `TEST 08` | `test_regular_file_persistence` | Persistence across file descriptor destruction and remount | **PASSED** |
| `TEST 09` | `test_allocation_exhaustion` | Out-of-space safety (`PFS2_ERR_NO_SPACE`) on 32-block volume | **PASSED** |
| `TEST 10` | `test_corrupt_inode_rejection` | Inode CRC bit-flip corruption detection (`PFS2_ERR_CORRUPT`) | **PASSED** |
| `TEST 11` | `test_journal_crash_recovery` | Mount-time replay of COMMITTED transaction after simulated crash | **PASSED** |
| `TEST 12` | `test_journal_corrupt_after_image_rejection` | Rejection of replay if after-image CRC checksum fails | **PASSED** |
| `TEST 13` | `test_journal_corrupt_header_crc` | Rejection of replay if journal header CRC is corrupted | **PASSED** |
| `TEST 14` | `test_journal_torn_header_write` | Safety when journal header magic is torn/corrupted | **PASSED** |
| `TEST 15` | `test_journal_crash_during_prep` | Safety when crash occurs in EMPTY state before commit header write | **PASSED** |
| `TEST 16` | `test_journal_flush_failure` | Safe rollback when device flush fails during transaction | **PASSED** |
| `TEST 17` | `test_fault_injection_failures` | Error resilience on mock device read/write failure injection | **PASSED** |
| `TEST 18` | `test_repeated_mount_unmount` | 25 consecutive mount/unmount cycles; monotonic mount counter | **PASSED** |
| `TEST 19` | `test_read_write_block_boundary_spans` | Unaligned reads/writes across 1-block and 3-block boundaries; sparse hole verification | **PASSED** |
| `TEST 20` | `test_empty_file_comprehensive` | 0-byte reads at offset 0/100, 0-byte writes, 0-byte truncation | **PASSED** |
| `TEST 21` | `test_missing_files_and_invalid_paths` | Non-existent files, NULL pointers, unmounted state, dot/dot-dot, nested paths | **PASSED** |
| `TEST 22` | `test_eof_and_out_of_bounds_rw` | Partial EOF reads, exact EOF reads (0 bytes), past EOF, directory read rejection, double indirect bound check (`PFS2_ERR_TOO_BIG`) | **PASSED** |
| `TEST 23` | `test_boundary_filename_and_dir_limits` | Exact 23-char name allowed; 24-char name rejected; 126 user files created until directory full (`PFS2_ERR_NO_SPACE`); free inodes counter verified | **PASSED** |
| `TEST 24` | `test_directory_creation_and_traversal_unsupported` | Verified `pfs2_mkdir` and nested traversal are unsupported; confirmed directory inodes cannot be read/written as regular files | **PASSED** |
| `TEST 25` | `test_persistence_multiblock_remount` | Host-side multi-file (5 files: 0 B, 45 B, 4 KiB, 8 KiB, 10 KiB), multi-block byte-for-byte remount verification on disposable disk | **PASSED** |

---

## 4. Step 4 — Multi-Boot Persistence Verification

Persistence was proven across both host-side image reload and physical QEMU reboot cycles.

### 4.1 Host-Side Remount Verification (`test_persistence_multiblock_remount`)
1. Created disposable regular-file disk image at `/tmp/test_persist_audit.img` (1,024 blocks = 4 MiB).
2. Formatted with PurpleFS v2.
3. Created 5 files of diverse sizes and structures:
   - `small.txt` (45 bytes): sub-block small file.
   - `exact1blk.bin` (4,096 bytes): exactly 1 full block.
   - `exact2blk.bin` (8,192 bytes): exactly 2 full blocks.
   - `span3blk.bin` (10,000 bytes): spans across 3 direct data blocks.
   - `empty.dat` (0 bytes): empty file.
4. Flushed device caches and cleanly unmounted filesystem.
5. Destroyed block device memory object, completely closing the host file descriptor.
6. Re-opened image file with fresh device and filesystem structures.
7. Remounted: verified `mount_count == 2` and `clean_unmount == 0`.
8. Looked up all 5 files: all inodes matched.
9. Read back all payloads: byte-for-byte fidelity confirmed against original memory vectors.
10. Unlinked temporary image file.

### 4.2 QEMU Multi-Boot Persistence Verification (`make check-persistence-qemu`)
Script: [scripts/test_qemu_persistence.py](file:///home/saptanshu-ganguly/OS/scripts/test_qemu_persistence.py)
- **Session 1 (Write & Commit):**
  - Formatted disposable GPT disk image in `/tmp`.
  - Booted QEMU with disk attached as Primary Slave IDE (`ata1`).
  - Executed `remount rw`.
  - Executed `touch persist_token.txt` (created Inode 7).
  - Executed `write persist_token.txt PURPLE_OS_PERSISTENCE_VERIFIED_CYCLE_2026`.
  - Executed `sync` (journal committed, disk cache flushed).
  - Executed `remount ro`.
  - Terminated QEMU Session 1.
- **Session 2 (Cold Reboot & Verification):**
  - Started a completely separate QEMU process booting `build/kernel.elf` from the same disk image.
  - Kernel booted, discovered `ata1`, validated GPT, and mounted PurpleFS partition cleanly in read-only mode.
  - Executed `list files`: verified `persist_token.txt` (Inode 7, 42 bytes) was present in directory listing.
  - Executed `peek persist_token.txt`: verified output was exactly `PURPLE_OS_PERSISTENCE_VERIFIED_CYCLE_2026`.
  - Verified `mount_count == 2` and read-only protection active.

---

## 5. Step 5 — Safety & System Regression Verification

Running `make check`, `make test`, `make test-asan`, and all QEMU check targets verified zero regressions across the entire operating system:

1. **Host Unit Test Suites (16/16 Passed):**
   - `test_pmm`: 9/9 passed
   - `test_vmm`: 5/5 passed
   - `test_scheduler`: 6/6 passed
   - `test_purplefs_v2`: 25/25 passed
   - `test_storage_gpt`: 13/13 passed
   - `test_acpi`: 8/8 passed
   - `test_aegis_detect`: 4/4 passed
   - `test_purple_detect`: 5/5 passed
   - `test_desktop`: 8/8 passed
   - `test_apdrv`: 10/10 passed
   - `test_apapp`: 5/5 passed
   - `test_video`: 6/6 passed
   - `test_block_dev`: 5/5 passed
   - `test_ahci`: 3/3 passed
   - `test_nvme`: 3/3 passed
   - `test_drivers`: 5/5 passed
2. **GCC Sanitizer Verification (`make test-asan`):**
   - All 16 suites compiled with `-fsanitize=address,undefined`.
   - Executed with 0 memory errors, 0 memory leaks, 0 out-of-bounds accesses.
3. **QEMU Virtual Machine Integration Targets:**
   - `check-scheduler-qemu`: PASSED (priority preemption, task spawning, termination)
   - `check-smp-qemu`: PASSED (4 cores online, APIC timer, per-core affinity, concurrent workers)
   - `check-storage-qemu`: PASSED (GPT discovery, read-only PurpleFS mount, file listing, AegisDetect baseline verification)
   - `check-writable-storage-qemu`: PASSED (remount rw, touch, write, sync, remount ro)
   - `check-persistence-qemu`: PASSED (multi-boot cold reboot data persistence)
   - `check-modern-storage-qemu`: PASSED (AHCI SATA controller and NVMe PCIe controller discovery and GPT/PurpleFS mounting)
   - `check-graphics-qemu`: PASSED (Bochs VBE mode switch to 800x600@32bpp linear framebuffer)
   - `check-desktop-qemu`: PASSED (PS/2 mouse tracking, interactive window click events, window toggling)

---

## 6. Step 6 — Bugs, Nuances & Smallest Recommended Next Change

### 6.1 Subtleties Uncovered
1. **Directory Entry Asymmetry with Inode Pool:**
   - Inode table provides 256 inodes (254 user files).
   - Root directory table is strictly 1 data block (128 entries total = 126 user files).
   - Once 126 files are created, `pfs2_create_file` fails with `PFS2_ERR_NO_SPACE`, leaving 128 inodes unused. Expanding directory blocks in future will bridge this asymmetry.
2. **Lack of File Unlink/Deletion:**
   - Because `pfs2_unlink()` is not yet implemented, deleted files cannot be reclaimed or removed in Purple Shell.
3. **Strict Flat Filenames vs Shell Path Formatting:**
   - `pfs2_lookup` performs an exact string comparison against directory entries. Looking up `"/welcome.txt"` or `"./welcome.txt"` fails with `PFS2_ERR_NOT_FOUND` because the root directory entry name is literally `"welcome.txt"`.

### 6.2 Smallest Recommended Next Change
To prepare PurpleFS for broader, more natural use in Purple Shell without adding risky filesystem complexity:
1. **Shell Path Normalizer:** Add a path-sanitizing helper in [src/kernel/shell.c](file:///home/saptanshu-ganguly/OS/src/kernel/shell.c) (e.g. `strip_leading_slashes()`) so that user commands like `peek /welcome.txt` or `write /note.txt ...` automatically strip leading `/` or `./` before calling `pfs2_lookup()`.
2. **Atomic File Unlink (`pfs2_unlink`):** Implement `pfs2_unlink(fs, name)` in `src/fs/purplefs_v2.c`:
   - Looks up file in root directory.
   - Clears `de->inode = 0`.
   - Frees all associated data blocks (direct and indirect) in `fs->bitmap`.
   - Clears inode record (`mode = 0`) and increments `fs->sb.free_inodes`.
   - Commits changes atomically via redo journal transaction.
   - Connects to a shell `rm <filename>` command.
