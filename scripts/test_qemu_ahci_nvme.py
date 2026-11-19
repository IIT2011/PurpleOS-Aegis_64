#!/usr/bin/env python3
import subprocess
import tempfile
import time
import os
import sys

def main():
    print("=== Checking Aegis64 does not advertise unimplemented AHCI/NVMe storage ===")

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
            "-kernel", "target/kernel.elf",
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

        print("\n=== Modern Storage Safety Check Output ===")
        print(stdout)

        expected = "Storage: AHCI, NVMe, USB MSC, and SD I/O unavailable; placeholders skipped"
        forbidden = [
            "BlockDev: Registered 'sata0' [AHCI/SATA]",
            "BlockDev: Registered 'nvme0n1' [NVMe PCIe]",
            "NVMe: Namespace 1 online",
        ]
        failed = 0
        if expected in stdout:
            print(f"[OK] Verified unavailable storage is reported ('{expected}')")
        else:
            print(f"[FAIL] Missing status: '{expected}'")
            failed += 1
        for pattern in forbidden:
            if pattern in stdout:
                print(f"[FAIL] Placeholder device was registered: '{pattern}'")
                failed += 1
            else:
                print(f"[OK] No placeholder registration: '{pattern}'")

        if failed == 0:
            print("\n>>> UNIMPLEMENTED STORAGE DEVICES WERE NOT ADVERTISED <<<\n")
            return 0
        else:
            print(f"\n>>> MODERN STORAGE SAFETY CHECK FAILED ({failed} missing assertions)! <<<\n")
            return 1

if __name__ == "__main__":
    sys.exit(main())
