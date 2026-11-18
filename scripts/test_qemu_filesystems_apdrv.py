#!/usr/bin/env python3
"""
Verification suite for filesystems.apdrv unified module and PurpleDetect integration.
Verifies:
  1. Boot-time automatic loading of 'filesystems' unified category module under APDRV v3.
  2. PurpleDetect registration of all 12 filesystem driver engines under DeviceCategory::Filesystem.
  3. Execution of 'filesystems', 'filesystem drivers', and 'devices' commands.
  4. 'lsmod' module registry containing 'filesystems' in RUNNING state.
"""

import subprocess
import tempfile
import os
import sys
import time

def run_test():
    print("=== Launching Aegis64 filesystems.apdrv & PurpleDetect Integration Test ===")
    
    with tempfile.TemporaryDirectory() as tmpdir:
        disk_path = os.path.join(tmpdir, "fs_apdrv_test.img")
        mkpfs_bin = os.path.join("build", "mkpfs2_disk")
        subprocess.check_call([sys.executable, mkpfs_bin, disk_path])

        qemu_cmd = [
            "qemu-system-x86_64",
            "-kernel", "target/kernel.elf",
            "-serial", "stdio",
            "-display", "none",
            "-m", "256M",
            "-smp", "4",
            "-drive", f"file={disk_path},format=raw,index=0,media=disk",
            "-no-reboot"
        ]

        proc = subprocess.Popen(
            qemu_cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )

        commands = [
            "lsmod\n",
            "devices\n",
            "filesystem drivers\n",
            "filesystems\n",
            "show devices\n",
            "insmod fat32\n",
            "lsmod\n",
            "poweroff\n"
        ]

        stdout_data = ""
        try:
            time.sleep(2.0)
            for cmd in commands:
                proc.stdin.write(cmd)
                proc.stdin.flush()
                time.sleep(0.4)

            stdout_data, _ = proc.communicate(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()
            stdout_data, _ = proc.communicate()

        print("\n=== Captured Serial Output ===")
        print(stdout_data)

        # Assertions
        required_checks = [
            ("FS Registry Init", "FS: Filesystem Driver Registry initialized (12 core filesystem engines online)"),
            ("PurpleDetect Peripherals", "PurpleDetect: Hardware inventory completed: 23 discovered peripherals registered"),
            ("APDRV Filesystems Load", "APDRV: Module 'filesystems' successfully loaded and verified with W^X enforcement"),
            ("LSMOD Filesystems Running", "filesystems                     RUNNING"),
            ("PurpleDetect PurpleFS Entry", "purplefs_v2   FILESYSTEM"),
            ("PurpleDetect FAT32 Entry", "fat32         FILESYSTEM"),
            ("PurpleDetect Ext4 Entry", "ext4          FILESYSTEM"),
            ("PurpleDetect ISO9660 Entry", "iso9660       FILESYSTEM"),
            ("PurpleDetect RAMFS Entry", "ramfs         FILESYSTEM"),
            ("Filesystem Drivers Command", "=== Purple OS Registered Filesystem Drivers (filesystems.apdrv) ==="),
            ("FAT32 in Table", "fat32"),
            ("Ext4 in Table", "ext4"),
            ("PurpleFS in Table", "purplefs_v2"),
            ("Dynamic Insmod FAT32", "APDRV: Module 'fat32' v1.0.0 loaded successfully"),
        ]

        all_ok = True
        print("\n=== Test Assertions ===")
        for name, expected in required_checks:
            if expected in stdout_data:
                print(f"[OK] {name}")
            else:
                print(f"[FAIL] {name}: Expected substring '{expected}' not found")
                all_ok = False

        if all_ok:
            print("\n>>> ALL filesystems.apdrv & PurpleDetect TESTS PASSED (100%)! <<<")
            return 0
        else:
            print("\n>>> SOME TESTS FAILED! <<<")
            return 1

if __name__ == "__main__":
    sys.exit(run_test())
