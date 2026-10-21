# Purple OS / Aegis64: Reproducible Test Guide

This document defines all reproducible test suites, invocation commands, verification criteria, and test interpretation for the Aegis64 kernel and PurpleFS v2.

---

## 1. Test Matrix Summary

| Test Target | Type | Command | Test Cases | Status |
|---|---|---|---|---|
| `test_pmm` | Host Unit Test | `./build/test_pmm` | 9 | **PASSED** (9/9) |
| `test_vmm` | Host Unit Test | `./build/test_vmm` | 5 | **PASSED** (5/5) |
| `test_scheduler` | Host Unit Test | `./build/test_scheduler` | 4 | **PASSED** (4/4) |
| `test_purplefs_v2` | Host Unit Test | `./build/test_purplefs_v2` | 14 | **PASSED** (14/14) |
| `test_purplefs_v2_asan` | Sanitizer Test | `make test-asan` | 14 | **PASSED** (ASan + UBSan clean) |
| `check` | Multiboot Validation | `make check` | Full host suite + Multiboot | **PASSED** (Exit 0) |
| `check-scheduler-qemu` | QEMU Integration | `make check-scheduler-qemu` | 9 assertions | **PASSED** (Interleaved preemption verified) |
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
- 14 PurpleFS v2 tests pass: formatting, mounting, invalid signature detection, corrupt superblock checksum detection, volume bounds (32 to 32768 blocks), file creation, lookup, small file read/write, empty files, multi-block files (direct & indirect), file truncation, persistence on regular disk image, allocation exhaustion safety, corrupt inode CRC rejection, redo journal crash recovery after commit, corrupted journal after-image rejection, and read/write/flush fault injection.

### 2.2 AddressSanitizer & UndefinedBehaviorSanitizer
To run PurpleFS v2 tests under memory and UB sanitizers:
```bash
make test-asan
```
**Expected Outcome:**
Zero memory leaks, zero heap/stack buffer overflows, zero unaligned pointer accesses, exit code 0.

### 2.3 Emulator Boot Tests
To run automated scheduler preemption verification:
```bash
make check-scheduler-qemu
```
**Expected Outcome:**
Launches QEMU headless, spawns 3 prioritized tasks (`Task_A` HIGH, `Task_B` NORMAL, `Task_C` LOW), verifies timer-driven preemption and completion of 3 iterations each, spawns and terminates a task by ID, and confirms zombie cleanup.

To run automated interactive shell verification:
```bash
python3 scripts/test_qemu_shell.py
```
**Expected Outcome:**
Verifies interactive shell prompt, command tokenization, and output of `sysinfo`, `cpuinfo`, `mem`, `uptime`, `tasks`, and `echo`.
