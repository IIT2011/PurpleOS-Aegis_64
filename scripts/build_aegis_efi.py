#!/usr/bin/env python3
"""Embed the Aegis64 ELF in the native x86-64 EFI application."""

import argparse
import os
from pathlib import Path
import struct
import subprocess
from tempfile import TemporaryDirectory


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kernel", type=Path, default=Path("target/x86_64-unknown-none/debug/aegis64"))
    parser.add_argument("--output", type=Path, default=Path("target/Aegisx64.efi"))
    parser.add_argument("--sysroot", type=Path, help="alternate sysroot containing the Rust UEFI target")
    args = parser.parse_args()
    symbols = subprocess.check_output(["rust-nm", "--defined-only", str(args.kernel)], text=True)
    entries = [line.split()[0] for line in symbols.splitlines() if line.endswith(" T aegis_uefi_entry")]
    if len(entries) != 1:
        parser.error("the kernel must contain exactly one aegis_uefi_entry symbol")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    source = Path(__file__).resolve().parents[1] / "boot/uefi/main.rs"
    with TemporaryDirectory(dir=args.output.parent) as temp:
        entry = Path(temp, "entry.bin").resolve()
        entry.write_bytes(struct.pack("<Q", int(entries[0], 16)))
        env = dict(os.environ, AEGIS_KERNEL_ELF=str(args.kernel.resolve()), AEGIS_UEFI_ENTRY=str(entry))
        command = ["rustc", "--edition=2021", "--target", "x86_64-unknown-uefi", "-C", "panic=abort",
                   "-C", "opt-level=2", "-o", str(args.output), str(source)]
        if args.sysroot:
            command += ["--sysroot", str(args.sysroot)]
        subprocess.run(command, env=env, check=True)


if __name__ == "__main__":
    main()
