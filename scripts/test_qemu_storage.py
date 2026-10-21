#!/usr/bin/env python3
import subprocess
import tempfile
import time
import os
import sys

def main():
    print("=== Launching Aegis64 QEMU Storage & PurpleFS Integration Test ===")

    with tempfile.TemporaryDirectory() as tmpdir:
        disk_path = os.path.join(tmpdir, "disposable_test_disk.img")
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

        # Wait for kernel boot and storage scan
        time.sleep(1.2)

        # 3. Exercise Shell Storage Commands
        commands = [
            "disk status\n",
            "filesystem status\n",
            "list files\n",
            "peek welcome.txt\n",
            "peek version.txt\n",
            "aegisdetect\n",
            "devices\n",
            "lspci\n",
            "desktop\n",
            "mouse\n"
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

        print("\n=== Storage Integration Captured Serial Output ===")
        print(stdout)

        # 4. Mandatory Assertions
        required_assertions = [
            ("Drive Discovery", "Drive detected: Model="),
            ("GPT Table Validation", "Valid GPT detected with 1 partition(s)"),
            ("PurpleFS Partition Detection", "Found PurpleFS v2 partition at index 0"),
            ("PurpleFS Mount Success", "PurpleFS: Mounted volume 'PurpleSystem'"),
            ("Disk Status Output", "ATA Storage Devices & GPT Partition Status"),
            ("Partition List in Shell", "PurpleRootFS"),
            ("Filesystem Status Output", "Mount State:   MOUNTED (Read-Only Kernel Mode)"),
            ("Filesystem Block Size", "Block Size:    4096 bytes"),
            ("Filesystem Volume Label", "Volume Label:  PurpleSystem"),
            ("List Files Directory Content", "welcome.txt"),
            ("List Files Version File", "version.txt"),
            ("List Files MOTD", "motd"),
            ("Peek Content Welcome", "Welcome to Purple OS / Aegis64 Native Filesystem!"),
            ("Peek Content Version", "Aegis64 Kernel v0.4.0 (Stage 4B Safe Kernel Storage Integration)"),
            ("AegisDetect Boot Baseline", "AegisDetect: Loaded and authenticated trusted baseline"),
            ("AegisDetect File Verify", "AegisDetect: Verification summary: 3/3 components verified (3 trusted, 0 tampered/missing)"),
            ("AegisDetect Shell Status", "AegisDetect Boot Integrity & System Protection Subsystem"),
            ("AegisDetect Shell Trusted Count", "3 total, 3 trusted, 0 tampered/missing"),
            ("PurpleDetect Registry", "PurpleDetect Hardware & Removable Device Registry"),
            ("PurpleDetect COM1 Serial", "COM1"),
            ("PurpleDetect Keyboard", "PS2KBD"),
            ("PurpleDetect ATA Controller", "ATA_PRI"),
            ("PCI Bus Inventory", "PCI Bus Device Inventory"),
            ("PCI Discovered Host Bridge", "Host Bridge")
        ]

        passed = True
        print("=== Storage Integration Assertions ===")
        for label, pattern in required_assertions:
            if pattern in stdout:
                print(f"[OK] Verified: {label} ('{pattern}')")
            else:
                print(f"[FAIL] Missing expected output: {label} ('{pattern}')")
                passed = False

        if passed:
            print("\n>>> ALL STAGE 4B STORAGE & PURPLEFS INTEGRATION TESTS PASSED! <<<")
            return 0
        else:
            print("\n>>> STAGE 4B STORAGE INTEGRATION TESTS FAILED! <<<")
            return 1

if __name__ == "__main__":
    sys.exit(main())
