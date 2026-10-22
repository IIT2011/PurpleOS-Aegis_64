#!/usr/bin/env python3
import subprocess
import tempfile
import time
import os
import sys

def run_qemu_session(disk_path, commands, timeout_sec=8):
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

    for c in commands:
        proc.stdin.write(c)
        proc.stdin.flush()
        time.sleep(0.4)

    proc.terminate()
    try:
        stdout, stderr = proc.communicate(timeout=timeout_sec)
    except subprocess.TimeoutExpired:
        proc.kill()
        stdout, stderr = proc.communicate()

    return stdout

def main():
    print("=== Launching Aegis64 QEMU Multi-Boot PurpleFS Persistence Test ===")

    with tempfile.TemporaryDirectory() as tmpdir:
        disk_path = os.path.join(tmpdir, "disposable_persistence_disk.img")
        print(f"Creating disposable GPT test disk: {disk_path}")

        mk_res = subprocess.run(["./build/mkpfs2_disk", disk_path], capture_output=True, text=True)
        if mk_res.returncode != 0:
            print(f"[FAIL] mkpfs2_disk failed:\n{mk_res.stderr}")
            return 1
        print("[OK] Disposable GPT disk image initialized with PurpleFS v2 partition")

        # -------------------------------------------------------------
        # Session 1: Boot, Mount, Remount RW, Write File, Sync, Remount RO
        # -------------------------------------------------------------
        print("\n--- Starting QEMU Boot Session 1 (Write & Sync) ---")
        session1_commands = [
            "filesystem status\n",
            "remount rw\n",
            "touch persist_token.txt\n",
            "write persist_token.txt PURPLE_OS_PERSISTENCE_VERIFIED_CYCLE_2026\n",
            "sync\n",
            "remount ro\n",
            "filesystem status\n"
        ]
        out1 = run_qemu_session(disk_path, session1_commands)
        print("Session 1 completed. Output preview:")
        for line in out1.splitlines()[-15:]:
            print("  ", line)

        # Assertions for Session 1
        s1_assertions = [
            ("Session 1 Remount RW", "PurpleFS volume remounted READ-WRITE."),
            ("Session 1 Touch File", "Successfully created file 'persist_token.txt'"),
            ("Session 1 Write File", "Successfully wrote"),
            ("Session 1 Sync Flush", "Filesystem transactions committed and cache flushed to disk."),
            ("Session 1 Remount RO", "PurpleFS volume remounted READ-ONLY.")
        ]
        for label, needle in s1_assertions:
            if needle not in out1:
                print(f"[FAIL] Session 1 missing assertion: {label} ('{needle}')")
                return 1
            print(f"[OK] Verified: {label}")

        # -------------------------------------------------------------
        # Session 2: Fresh QEMU Boot from Same Disk, Read & Verify Content
        # -------------------------------------------------------------
        print("\n--- Starting QEMU Boot Session 2 (Reboot, Remount & Byte Verification) ---")
        session2_commands = [
            "filesystem status\n",
            "list files\n",
            "peek persist_token.txt\n"
        ]
        out2 = run_qemu_session(disk_path, session2_commands)
        print("Session 2 completed. Output preview:")
        for line in out2.splitlines()[-18:]:
            print("  ", line)

        # Assertions for Session 2
        s2_assertions = [
            ("Session 2 Clean Boot Mount", "PurpleFS: Mounted volume 'PurpleSystem'"),
            ("Session 2 Read-Only Mode", "Mount State:   MOUNTED (Read-Only Kernel Mode)"),
            ("Session 2 File Listed", "persist_token.txt"),
            ("Session 2 File Peek Header", "=== File: persist_token.txt"),
            ("Session 2 Byte Content", "PURPLE_OS_PERSISTENCE_VERIFIED_CYCLE_2026")
        ]
        for label, needle in s2_assertions:
            if needle not in out2:
                print(f"[FAIL] Session 2 missing assertion: {label} ('{needle}')")
                return 1
            print(f"[OK] Verified: {label}")

        print("\n>>> ALL MULTI-BOOT PURPLEFS PERSISTENCE TESTS PASSED! <<<")
        return 0

if __name__ == "__main__":
    sys.exit(main())
