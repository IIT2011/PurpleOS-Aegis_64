#!/usr/bin/env python3
import subprocess
import tempfile
import time
import os
import sys

def main():
    print("=== Launching Aegis64 QEMU Writable Storage & PurpleFS Test ===")

    with tempfile.TemporaryDirectory() as tmpdir:
        disk_path = os.path.join(tmpdir, "disposable_rw_disk.img")
        print(f"Creating disposable GPT test disk in temporary directory: {disk_path}")

        # 1. Build test disk using host-side tool
        mk_res = subprocess.run(["./build/mkpfs2_disk", disk_path], capture_output=True, text=True)
        if mk_res.returncode != 0:
            print(f"[FAIL] mkpfs2_disk failed:\n{mk_res.stderr}")
            return 1
        print("[OK] Disposable GPT disk image prepared with PurpleFS v2 partition")

        # 2. Launch QEMU with disposable disk attached to IDE controller (index 1 = Primary Slave)
        cmd = [
            "qemu-system-x86_64",
            "-kernel", "build/kernel.elf",
            "-serial", "stdio",
            "-display", "none",
            "-m", "256M",
            "-no-reboot",
            "-drive", f"file={disk_path},format=raw,index=1,media=disk"
        ]

        proc = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=0
        )

        time.sleep(1.2)

        # 3. Exercise Shell Writable Storage Commands
        commands = [
            "filesystem status\n",
            "remount rw\n",
            "filesystem status\n",
            "touch newnote.txt\n",
            "write newnote.txt PurpleOS Stage 7 Writable Filesystem Active!\n",
            "sync\n",
            "list files\n",
            "peek newnote.txt\n",
            "remount ro\n",
            "filesystem status\n"
        ]

        for c in commands:
            proc.stdin.write(c)
            proc.stdin.flush()
            time.sleep(0.4)

        proc.terminate()
        try:
            stdout, stderr = proc.communicate(timeout=3)
        except subprocess.TimeoutExpired:
            proc.kill()
            stdout, stderr = proc.communicate()

        print("\n=== Writable Storage Integration Captured Output ===")
        print(stdout)

        # 4. Mandatory Assertions
        required_assertions = [
            ("Initial Read-Only Status", "Mount State:   MOUNTED (Read-Only Kernel Mode)"),
            ("Remount Read-Write", "PurpleFS volume remounted READ-WRITE."),
            ("Read-Write Status", "Mount State:   MOUNTED (Read-Write Kernel Mode)"),
            ("Create File", "Successfully created file 'newnote.txt'"),
            ("Write File", "Successfully wrote"),
            ("Filesystem Sync", "Filesystem transactions committed and cache flushed to disk."),
            ("List Files with New Note", "newnote.txt"),
            ("Peek New Note", "PurpleOS Stage 7 Writable Filesystem Active!"),
            ("Remount Read-Only", "PurpleFS volume remounted READ-ONLY."),
            ("Final Read-Only Status", "Mount State:   MOUNTED (Read-Only Kernel Mode)")
        ]

        passed = True
        print("=== Writable Storage Assertions ===")
        for label, pattern in required_assertions:
            if pattern in stdout:
                print(f"[OK] Verified: {label} ('{pattern}')")
            else:
                print(f"[FAIL] Missing expected output: {label} ('{pattern}')")
                passed = False

        if passed:
            print("\n>>> ALL STAGE 7 WRITABLE STORAGE & PURPLEFS INTEGRATION TESTS PASSED! <<<")
            return 0
        else:
            print("\n>>> STAGE 7 WRITABLE STORAGE INTEGRATION TESTS FAILED! <<<")
            return 1

if __name__ == "__main__":
    sys.exit(main())
