#!/usr/bin/env python3
"""
PurpleOS Creator Cryptographic Signing Tool for /System/CLI/cmd.dtbase.
Signs the command database payload with the Creator's Root Key using HMAC-SHA256.

Usage:
  python3 scripts/sign_cmddb.py [file] [--key "CustomSecretKey"]

How to Sign:
  1. Simply run: `python3 scripts/sign_cmddb.py build/cmd.dtbase`
  2. Or specify your custom key: `python3 scripts/sign_cmddb.py build/cmd.dtbase --key "MySecretKey"`
"""

import hashlib
import hmac
import sys
import os

DEFAULT_CREATOR_KEY = b"PURPLEOS_AEGIS_CREATOR_ROOT_KEY_2026"

def sign_cmddb(file_path: str, key: bytes = DEFAULT_CREATOR_KEY):
    if not os.path.exists(file_path):
        print(f"[ERROR] File not found: {file_path}")
        sys.exit(1)

    raw_data = open(file_path, "rb").read()

    # Find where payload starts (after existing header comments)
    lines = raw_data.split(b"\n")
    payload_lines = []
    in_payload = False

    for line in lines:
        stripped = line.strip()
        if stripped.startswith(b"[cmd:"):
            in_payload = True
        if in_payload:
            payload_lines.append(line)

    if not payload_lines:
        print("[ERROR] No command definitions ([cmd:...]) found in file.")
        sys.exit(1)

    payload = b"\n".join(payload_lines)

    # Compute HMAC-SHA256 signature with Creator Key
    sig = hmac.new(key, payload, hashlib.sha256).hexdigest()

    # Count commands
    cmd_count = sum(1 for l in payload_lines if l.strip().startswith(b"[cmd:"))

    header = (
        f"# PURPLEOS COMMAND DATABASE\n"
        f"# SIGNER: Creator (Aegis Root Developer)\n"
        f"# KEY_ID: AEGIS-CREATOR-V1\n"
        f"# SIGNATURE: {sig}\n"
        f"# CANONICAL PATH: /System/CLI/cmd.dtbase\n"
        f"# COMMAND COUNT: {cmd_count}\n\n"
    ).encode("utf-8")

    signed_data = header + payload

    with open(file_path, "wb") as f:
        f.write(signed_data)

    print(f"[OK] Cryptographically signed {file_path}")
    print(f"     Signer:    Creator (Aegis Root Developer)")
    print(f"     Key ID:    AEGIS-CREATOR-V1")
    print(f"     Signature: {sig}")
    print(f"     Commands:  {cmd_count}")

if __name__ == "__main__":
    target = "OS/build/cmd.dtbase"
    key = DEFAULT_CREATOR_KEY

    args = sys.argv[1:]
    i = 0
    while i < len(args):
        if args[i] == "--key" and i + 1 < len(args):
            key = args[i + 1].encode("utf-8")
            i += 2
        else:
            target = args[i]
            i += 1

    sign_cmddb(target, key)
