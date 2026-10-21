# Purple OS / Aegis64: Reproducible Test Guide

This document defines all reproducible test suites, invocation commands, verification criteria, and test interpretation for the Aegis64 kernel and PurpleFS v2.

---

## 1. Test Matrix Summary

| Test Target | Type | Command | Test Cases | Status |
|---|---|---|---|---|
| `test_pmm` | Host Unit Test | `./build/test_pmm` | 9 | **PASSED** (9/9) |
| `test_vmm` | Host Unit Test | `./build/test_vmm` | 5 | **PASSED** (5/5) |
| `test_scheduler` | Host Unit Test | `./build/test_scheduler` | 4 | **PASSED** (4/4) |
| `test_purplefs_v2` | Host Unit Test | `./build/test_purplefs_v2` | 18 | **PASSED** (18/18) |
| `test_storage_gpt` | Host Unit Test | `./build/test_storage_gpt` | 10 | **PASSED** (10/10) |
| `test-asan` | Sanitizer Test | `make test-asan` | 28 | **PASSED** (ASan + UBSan clean) |
| `check` | Multiboot Validation | `make check` | Full host suite + Multiboot | **PASSED** (Exit 0) |
| `check-scheduler-qemu` | QEMU Integration | `make check-scheduler-qemu` | 9 assertions | **PASSED** (Interleaved preemption verified) |
| `check-storage-qemu` | QEMU Storage Integration | `make check-storage-qemu` | 14 assertions | **PASSED** (GPT discovery, PurpleFS mount, shell peek) |
| `test_qemu_shell` | QEMU Interactive | `python3 scripts/test_qemu_shell.py`| 9 commands | **PASSED** (Full output matched) |
| `run-uefi` | Full UEFI / OVMF Boot | `make run-uefi` | UEFI GOP Boot | **PASSED** (Smoke log captured) |

---

## 2. Test Execution Details

### 2.1 Host Unit Test Suite
To run all host-side unit tests:
```bash
make test
```
**Expected Outcome:**
- 9 PMM tests pass: initial state, frame alloc/free, double-free protection, reserved memory bounds (0..16 MiB), unaligned address rejection, out-of-bounds rejection, contiguous frame allocation, allocation exhaustion safety, and 64-bit address overflow handling.
- 5 VMM tests pass: canonical address validation, 4 KiB page map & lookup, unmapping, 2 MiB huge page splitting, and W^X memory protection enforcement.
- 4 Scheduler tests pass: initial task state, task creation, 4-tier priority time slice verification, task termination.
- 18 PurpleFS v2 tests pass: formatting, mounting, invalid signature detection, corrupt superblock checksum detection, volume bounds (32 to 32768 blocks), file creation, lookup, small file read/write, empty files, multi-block files (direct & indirect), file truncation, persistence on regular disk image, allocation exhaustion safety, corrupt inode CRC rejection, redo journal crash recovery after commit, corrupted journal after-image rejection, corrupted journal header CRC rejection, torn journal header write rejection, crash during journal preparation handling, journal flush failure handling, read/write/flush fault injection, and repeated mount/unmount cycles.
- 10 Storage & GPT tests pass: synthetic ATA IDENTIFY parsing, absent disk and ATAPI detection, ATA timeout and read error handling, ATA sector reads and bounds/overflow checks, valid GPT parsing and PurpleFS GUID matching, corrupt MBR/GPT signature and header/partition CRC rejection, partition boundary and overlap rejection, partition-backed block device bounds and read-only enforcement, PurpleFS mounting through the partition adapter, and rejection of unformatted partitions without automatic formatting.

### 2.2 AddressSanitizer & UndefinedBehaviorSanitizer
To run filesystem and storage tests under memory and UB sanitizers:
```bash
make test-asan
```
**Expected Outcome:**
Zero memory leaks, zero heap/stack buffer overflows, zero unaligned pointer accesses, exit code 0 across 28 test cases.

### 2.3 Emulator Boot Tests
To run automated scheduler preemption verification:
```bash
make check-scheduler-qemu
```
**Expected Outcome:**
Launches QEMU headless, spawns 3 prioritized tasks (`Task_A` HIGH, `Task_B` NORMAL, `Task_C` LOW), verifies timer-driven preemption and completion of 3 iterations each, spawns and terminates a task by ID, and confirms zombie cleanup.

To run automated QEMU storage and filesystem integration verification:
```bash
make check-storage-qemu
```
**Expected Outcome:**
Generates a disposable 32 MiB GPT disk image with a valid PurpleFS v2 partition containing sample files (`welcome.txt`, `version.txt`, `motd`), launches QEMU with the image attached as an IDE drive, verifies ATA discovery, GPT table parsing, PurpleFS partition detection, read-only kernel mount, and shell execution of `disk status`, `filesystem status`, `list files`, and `peek <filename>`. Cleanly deletes temporary files on completion.

To run automated interactive shell verification:
```bash
python3 scripts/test_qemu_shell.py
```
**Expected Outcome:**
Verifies interactive shell prompt, command tokenization, and output of `sysinfo`, `cpuinfo`, `mem`, `uptime`, `tasks`, and `echo`.
