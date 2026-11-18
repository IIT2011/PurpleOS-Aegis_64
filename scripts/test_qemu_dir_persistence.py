#!/usr/bin/env python3
"""
PurpleOS / Aegis64 — Multi-Boot Directory Persistence Verification Suite
Tests persistent creation, traversal, child file creation, reboot persistence,
and rmdir lifecycle directly on PurpleFS v2 disk partitions.
"""

import subprocess
import tempfile
import time
import os
import sys

def run_qemu_session(disk_path, commands, timeout_sec=8):
    cmd = [
        "qemu-system-x86_64",
        "-kernel", "target/kernel.elf",
        "-serial", "stdio",
        "-display", "none",
        "-m", "256M",
        "-no-reboot",
        "-drive", f"file={disk_path},format=raw,index=0,media=disk"
    ]

    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=0
    )

    time.sleep(2.5)

    for c in commands:
        proc.stdin.write(c)
        proc.stdin.flush()
        time.sleep(0.4)

    time.sleep(0.8)
    proc.terminate()
    try:
        stdout, stderr = proc.communicate(timeout=timeout_sec)
    except subprocess.TimeoutExpired:
        proc.kill()
        stdout, stderr = proc.communicate()

    return stdout

def main():
    print("=== Launching Aegis64 Multi-Boot Directory Persistence Test Suite ===")

    with tempfile.TemporaryDirectory() as tmpdir:
        disk_path = os.path.join(tmpdir, "dir_persistence_test.img")
        mk_res = subprocess.run(["./build/mkpfs2_disk", disk_path], capture_output=True, text=True)
        if mk_res.returncode != 0:
            print(f"[FAIL] mkpfs2_disk failed:\n{mk_res.stderr}")
            return 1
        print("[OK] Initialized test disk with PurpleFS v2 partition")

        # -------------------------------------------------------------
        # Session 1: Boot, Remount RW, Create Directory & Child File, Sync
        # -------------------------------------------------------------
        print("\n--- Starting QEMU Boot Session 1 (Create Directory & File) ---")
        s1_commands = [
            "remount rw\n",
            "mkdir MyPersistentDir\n",
            "cd MyPersistentDir\n",
            "pwd\n",
            "touch child.txt\n",
            "write child.txt PERSISTENT_DIR_DATA_2026\n",
            "cat child.txt\n",
            "cd /\n",
            "sync\n",
            "echo S1_FINISHED\n"
        ]
        out1 = run_qemu_session(disk_path, s1_commands)
        s1_assertions = [
            ("Session 1 Mkdir Registered", "Directory 'MyPersistentDir' registered in VFS namespace."),
            ("Session 1 Directory Traversal", "/MyPersistentDir"),
            ("Session 1 Create Child File", "Successfully created file 'child.txt'."),
            ("Session 1 Write Child File", "Successfully wrote"),
            ("Session 1 Read Child File", "PERSISTENT_DIR_DATA_2026"),
            ("Session 1 Sync Flush", "Filesystem transactions committed and cache flushed to disk."),
            ("Session 1 Completed", "S1_FINISHED")
        ]
        for label, needle in s1_assertions:
            if needle not in out1:
                print(f"[FAIL] Session 1 missing assertion: {label} ('{needle}')")
                return 1
            print(f"[OK] Verified: {label}")

        # -------------------------------------------------------------
        # Session 2: Cold Reboot from Same Disk, Verify Directory & Child File
        # -------------------------------------------------------------
        print("\n--- Starting QEMU Boot Session 2 (Cold Reboot & Verification) ---")
        s2_commands = [
            "dir\n",
            "cd MyPersistentDir\n",
            "pwd\n",
            "dir\n",
            "cat child.txt\n",
            "remount rw\n",
            "rm child.txt\n",
            "cd /\n",
            "rmdir MyPersistentDir\n",
            "sync\n",
            "dir\n",
            "echo S2_FINISHED\n"
        ]
        out2 = run_qemu_session(disk_path, s2_commands)
        s2_assertions = [
            ("Session 2 Directory Present Across Reboot", "MyPersistentDir"),
            ("Session 2 Directory Traversal", "/MyPersistentDir"),
            ("Session 2 Child File Listed", "child.txt"),
            ("Session 2 Child File Content", "PERSISTENT_DIR_DATA_2026"),
            ("Session 2 Remove Child File", "Successfully removed 'child.txt'."),
            ("Session 2 Remove Directory", "Directory 'MyPersistentDir' removed."),
            ("Session 2 Sync Flush", "Filesystem transactions committed and cache flushed to disk."),
            ("Session 2 Completed", "S2_FINISHED")
        ]
        for label, needle in s2_assertions:
            if needle not in out2:
                print(f"[FAIL] Session 2 missing assertion: {label} ('{needle}')")
                return 1
            print(f"[OK] Verified: {label}")

        # -------------------------------------------------------------
        # Session 3: Cold Reboot from Same Disk, Verify Directory Removal
        # -------------------------------------------------------------
        print("\n--- Starting QEMU Boot Session 3 (Cold Reboot After Rmdir) ---")
        s3_commands = [
            "dir\n",
            "cd MyPersistentDir\n",
            "echo S3_FINISHED\n"
        ]
        out3 = run_qemu_session(disk_path, s3_commands)
        s3_assertions = [
            ("Session 3 Directory Not In CWD", "No such file or directory"),
            ("Session 3 Completed", "S3_FINISHED")
        ]
        for label, needle in s3_assertions:
            if needle not in out3:
                print(f"[FAIL] Session 3 missing assertion: {label} ('{needle}')")
                return 1
            print(f"[OK] Verified: {label}")

    print("\n>>> ALL MULTI-BOOT DIRECTORY PERSISTENCE TESTS PASSED (100%)! <<<")
    return 0

if __name__ == "__main__":
    sys.exit(main())
