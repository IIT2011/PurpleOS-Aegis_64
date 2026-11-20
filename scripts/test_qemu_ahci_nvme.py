#!/usr/bin/env python3
"""Boot, write, flush, and cold-boot read a PurpleFS disk on each controller."""

import argparse
import os
from pathlib import Path
import secrets
import select
import subprocess
import tempfile
import time


PROMPT = b"aegis64@purple:~$"


def boot(kernel, image, controller, commands):
    args = [
        "qemu-system-x86_64", "-kernel", str(kernel), "-serial", "stdio",
        "-monitor", "none", "-display", "none", "-m", "256M", "-smp", "2",
        "-no-reboot", "-drive", f"file={image},if=none,format=qcow2,id=system",
    ]
    if controller == "AHCI":
        args += ["-machine", "q35", "-device", "ide-hd,drive=system,bus=ide.0"]
    elif controller == "NVMe":
        args += ["-device", "nvme,drive=system,serial=PURPLE-TEST"]
    else:
        args += ["-device", "ide-hd,drive=system,bus=ide.0"]

    with subprocess.Popen(args, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                          stderr=subprocess.STDOUT) as guest:
        pending = b""

        def read_prompt():
            nonlocal pending
            deadline = time.monotonic() + 30
            while PROMPT not in pending:
                remaining = deadline - time.monotonic()
                if remaining <= 0 or not select.select([guest.stdout], [], [], remaining)[0]:
                    raise RuntimeError(f"{controller}: shell timed out\n{pending.decode(errors='replace')}")
                data = os.read(guest.stdout.fileno(), 8192)
                if not data:
                    raise RuntimeError(f"{controller}: QEMU exited\n{pending.decode(errors='replace')}")
                pending += data
            output, pending = pending.split(PROMPT, 1)
            return output.decode(errors="replace")

        try:
            startup = read_prompt()
            device_name = {"AHCI": "sata-", "NVMe": "nvme-", "IDE": "ata"}[controller]
            if f"Registered '{device_name}" not in startup or "PurpleFS: Mounted volume" not in startup:
                raise RuntimeError(f"{controller}: system disk failed to mount\n{startup}")
            results = []
            for command in commands:
                guest.stdin.write((command + "\n").encode())
                guest.stdin.flush()
                results.append(read_prompt())
            return results
        finally:
            if guest.poll() is None:
                guest.terminate()
                try:
                    guest.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    guest.kill()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kernel", type=Path, default=Path("target/kernel.elf"))
    parser.add_argument("--disk", type=Path, default=Path("build/purple-desktop.img"),
                        help="raw PurpleFS system disk accepted by the kernel's boot policy")
    args = parser.parse_args()
    if not args.kernel.is_file() or not args.disk.is_file():
        parser.error("both the kernel and the system disk must exist")

    with tempfile.TemporaryDirectory() as directory:
        for controller in ("AHCI", "NVMe", "IDE"):
            image = Path(directory) / f"{controller}.qcow2"
            subprocess.run(["qemu-img", "create", "-q", "-f", "qcow2", "-F", "raw",
                            "-b", str(args.disk.resolve()), str(image)], check=True)
            filename = f"rw-{secrets.token_hex(5)}.txt"
            marker = f"PurpleOS-{controller}-persistent-write-{filename}"
            raw_path = "/ssd/ssd1" if controller == "NVMe" else "/hdd/hdd1"
            outputs = boot(args.kernel, image, controller, [
                "remount volume as rw", f"echo blocked > {raw_path}",
                f"echo {marker} > {filename}",
                f"cat {filename}", "sync",
            ])
            expected = ("remounted READ-WRITE", "Raw block device nodes are read-only",
                        f"Successfully wrote {len(marker)} bytes", marker, "cache flushed to disk")
            for output, item in zip(outputs, expected):
                if item not in output:
                    raise RuntimeError(f"{controller}: expected {item!r}\n{output}")
            [contents] = boot(args.kernel, image, controller, [f"cat {filename}"])
            if marker not in contents.splitlines():
                raise RuntimeError(f"{controller}: cold-boot readback failed\n{contents}")
            print(f"{controller}: mounted, wrote, flushed, and read back after cold boot")


if __name__ == "__main__":
    main()
