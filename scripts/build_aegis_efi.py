#!/usr/bin/env python3
"""Embed the Aegis64 ELF in the native x86-64 EFI application."""

import argparse
import hashlib
import os
import re
from pathlib import Path
import struct
import subprocess
from tempfile import TemporaryDirectory

from sign_cmddb import sign_payload_rsa


SIGNATURE_DOMAIN = b"PurpleOS/UEFI-KERNEL/v1\0"


def valid_creator_signature(modulus, payload, signature):
    # SHA-256 DigestInfo and canonical RSA-2048 PKCS#1 v1.5 encoding.
    digest_info = bytes.fromhex("3031300d060960864801650304020105000420") + hashlib.sha256(payload).digest()
    expected = b"\x00\x01" + b"\xff" * (256 - len(digest_info) - 3) + b"\x00" + digest_info
    value = int.from_bytes(signature, "big")
    return (modulus.bit_length() == 2048 and len(signature) == 256 and value < modulus
            and pow(value, 65537, modulus).to_bytes(256, "big") == expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kernel", type=Path, default=Path("target/x86_64-unknown-none/debug/aegis64"))
    parser.add_argument("--output", type=Path, default=Path("target/Aegisx64.efi"))
    parser.add_argument("--sysroot", type=Path, help="alternate sysroot containing the Rust UEFI target")
    parser.add_argument("--development", action="store_true", help="allow Secure Boot to be off")
    signing = parser.add_mutually_exclusive_group(required=True)
    signing.add_argument("--signing-key", type=Path, help="offline Creator RSA private key")
    signing.add_argument("--kernel-signature", type=Path, help="detached 256-byte Creator signature")
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    root = Path(__file__).resolve().parents[1]
    source = root / "boot/uefi/main.rs"
    trust_anchor = (root / "src/crypto/creator.rs").read_text(encoding="ascii")
    modulus = int.from_bytes(bytes(int(word, 16) for word in re.findall(r"0x([0-9a-f]{2})\b", trust_anchor)), "big")
    with TemporaryDirectory(dir=args.output.parent) as temp:
        kernel = Path(temp, "kernel.elf").resolve()
        kernel_bytes = args.kernel.read_bytes()
        kernel.write_bytes(kernel_bytes)
        symbols = subprocess.check_output(["rust-nm", "--defined-only", str(kernel)], text=True)
        entries = [line.split()[0] for line in symbols.splitlines() if line.endswith(" T aegis_uefi_entry")]
        if len(entries) != 1:
            parser.error("the kernel must contain exactly one aegis_uefi_entry symbol")
        entry_bytes = struct.pack("<Q", int(entries[0], 16))
        entry = Path(temp, "entry.bin").resolve()
        entry.write_bytes(entry_bytes)
        payload = SIGNATURE_DOMAIN + entry_bytes + kernel_bytes
        if args.signing_key:
            signature = bytes.fromhex(sign_payload_rsa(payload, str(args.signing_key.resolve())))
        else:
            signature = args.kernel_signature.read_bytes()
        if not valid_creator_signature(modulus, payload, signature):
            parser.error("kernel signature does not match the configured Creator public key")
        signature_path = Path(temp, "kernel.sig").resolve()
        signature_path.write_bytes(signature)
        env = dict(os.environ, AEGIS_KERNEL_ELF=str(kernel), AEGIS_UEFI_ENTRY=str(entry),
                   AEGIS_KERNEL_SIGNATURE=str(signature_path))
        env.pop("AEGIS_ALLOW_INSECURE_UEFI", None)
        if args.development:
            env["AEGIS_ALLOW_INSECURE_UEFI"] = "1"
        output = Path(temp, "Aegisx64.efi")
        command = ["rustc", "--edition=2021", "--target", "x86_64-unknown-uefi", "-C", "panic=abort",
                   "-C", "opt-level=2", "-o", str(output), str(source)]
        if args.sysroot:
            command += ["--sysroot", str(args.sysroot)]
        subprocess.run(command, env=env, check=True)
        output.replace(args.output)


if __name__ == "__main__":
    main()
