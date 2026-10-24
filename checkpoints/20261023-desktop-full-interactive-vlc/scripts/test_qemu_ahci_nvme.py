#!/usr/bin/env python3
import subprocess
import tempfile
import time
import os
import sys

def main():
    print("=== Launching Aegis64 QEMU AHCI SATA & NVMe PCIe Modern Storage Test ===")

    with tempfile.TemporaryDirectory() as tmpdir:
        sata_path = os.path.join(tmpdir, "disposable_sata.img")
        nvme_path = os.path.join(tmpdir, "disposable_nvme.img")
        print(f"Creating disposable SATA disk: {sata_path}")
        print(f"Creating disposable NVMe disk: {nvme_path}")

        # 1. Build test disks using host-side tool
        mk_res = subprocess.run(["./build/mkpfs2_disk", sata_path], capture_output=True, text=True)
        if mk_res.returncode != 0:
            print(f"[FAIL] mkpfs2_disk failed for SATA:\n{mk_res.stderr}")
            return 1

        mk_res2 = subprocess.run(["./build/mkpfs2_disk", nvme_path], capture_output=True, text=True)
        if mk_res2.returncode != 0:
            print(f"[FAIL] mkpfs2_disk failed for NVMe:\n{mk_res2.stderr}")
            return 1

        print("[OK] Disposable SATA and NVMe GPT disk images prepared with PurpleFS v2 partitions")

        # 2. Launch QEMU with AHCI SATA and NVMe PCIe controllers
        cmd = [
            "qemu-system-x86_64",
            "-kernel", "build/kernel.elf",
            "-serial", "stdio",
            "-display", "none",
            "-m", "256M",
            "-no-reboot",
            "-device", "ahci,id=ahci0",
            "-drive", f"id=sata0,file={sata_path},format=raw,if=none",
            "-device", "ide-hd,drive=sata0,bus=ahci0.0",
            "-drive", f"id=nvme0,file={nvme_path},format=raw,if=none",
            "-device", "nvme,drive=nvme0,serial=PURPLENVME1"
        ]

        proc = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=0
        )

        time.sleep(1.5)

        commands = [
            "disks\n",
            "filesystem status\n",
            "list files\n",
            "peek welcome.txt\n",
            "peek version.txt\n",
            "pci\n",
            "exit\n"
        ]

        for c in commands:
            try:
                proc.stdin.write(c)
                proc.stdin.flush()
                time.sleep(0.15)
            except Exception:
                break

        time.sleep(0.5)
        proc.terminate()
        try:
            stdout, stderr = proc.communicate(timeout=2)
        except Exception:
            proc.kill()
            stdout, stderr = proc.communicate()

        print("\n=== Modern Storage Captured Output ===")
        print(stdout)

        assertions = [
            ("AHCI Controller Discovery", "AHCI: Found controller at PCI"),
            ("AHCI Port Link Active", "AHCI: Port 0 link active"),
            ("AHCI SATA Block Device", "BlockDev: Registered 'sata0' [AHCI/SATA]"),
            ("NVMe Controller Discovery", "NVMe: Found controller at PCI"),
            ("NVMe Namespace Online", "NVMe: Namespace 1 online"),
            ("NVMe Block Device", "BlockDev: Registered 'nvme0n1' [NVMe PCIe]"),
            ("GPT Detection", "Valid GPT detected"),
            ("PurpleFS Mounting", "PurpleFS: Mounted volume 'PurpleSystem'"),
            ("Disk Status Output", "ATA Storage Devices & GPT Partition Status"),
            ("SATA Block Device in Shell", "sata0"),
            ("NVMe Block Device in Shell", "nvme0n1"),
            ("Filesystem Mounted", "MOUNTED (Read-Only Kernel Mode)"),
            ("Welcome File Present", "welcome.txt"),
            ("Version File Present", "version.txt"),
            ("Welcome Content Verified", "Welcome to Purple OS / Aegis64 Native Filesystem!"),
            ("PCI SATA Controller", "SATA Controller (AHCI)"),
            ("PCI NVMe Controller", "NVMe Controller")
        ]

        failed = 0
        for name, pattern in assertions:
            if pattern in stdout:
                print(f"[OK] Verified: {name} ('{pattern}')")
            else:
                print(f"[FAIL] Missing: {name} ('{pattern}')")
                failed += 1

        if failed == 0:
            print("\n>>> ALL MODERN STORAGE (AHCI SATA + NVME PCIE) INTEGRATION TESTS PASSED! <<<\n")
            return 0
        else:
            print(f"\n>>> MODERN STORAGE INTEGRATION TESTS FAILED ({failed} missing assertions)! <<<\n")
            return 1

if __name__ == "__main__":
    sys.exit(main())
