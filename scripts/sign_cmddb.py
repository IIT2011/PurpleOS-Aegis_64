#!/usr/bin/env python3
"""
PurpleOS Creator Cryptographic Signing Tool for /System/CLI/cmd.dtbase.
Authenticates the command database using the Creator's Root Key and 256-bit
AegisDetect Hardware Root of Trust Anchor.

Key Resolution:
  1. Local key file: `keys/creator_key.txt` (strictly local, ignored by git)
  2. CLI argument: `--key "SecretString"`
  3. CLI key file: `--key-file "path/to/key.txt"`
"""

import hashlib
import hmac
import sys
import os

DEFAULT_KEY_PATHS = [
    "keys/creator_key.txt",
    "OS/keys/creator_key.txt",
    "../keys/creator_key.txt",
]

def load_creator_key(cli_key_str: str = None, cli_key_file: str = None) -> bytes:
    """
    Loads Creator Key and derives the 256-bit (32-byte) cryptographic trust key via SHA-256.
    """
    raw_key = None
    if cli_key_str:
        raw_key = cli_key_str.strip().encode("utf-8")
    elif cli_key_file and os.path.exists(cli_key_file):
        with open(cli_key_file, "r") as f:
            raw_key = f.read().strip().encode("utf-8")
    else:
        for kp in DEFAULT_KEY_PATHS:
            if os.path.exists(kp):
                with open(kp, "r") as f:
                    raw_key = f.read().strip().encode("utf-8")
                break

    if not raw_key:
        print("[ERROR] No local keys/creator_key.txt found and no --key specified.")
        print("        Place your secret key into keys/creator_key.txt (which is ignored by git).")
        sys.exit(1)

    if len(raw_key) == 64:
        try:
            return bytes.fromhex(raw_key.decode("ascii"))
        except ValueError:
            pass

    # Derive 32-byte cryptographic trust anchor
    derived_key = hashlib.sha256(raw_key).digest()
    return derived_key

def sign_cmddb(file_path: str, derived_key: bytes):
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

    # Compute HMAC-SHA256 signature with 256-bit Creator Trust Key
    sig = hmac.new(derived_key, payload, hashlib.sha256).hexdigest()

    # Count commands
    cmd_count = sum(1 for l in payload_lines if l.strip().startswith(b"[cmd:"))

    header = (
        f"# PURPLEOS COMMAND DATABASE — CRYPTOGRAPHICALLY SIGNED BY CREATOR\n"
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
    print(f"     Signer:       Creator (Aegis Root Developer)")
    print(f"     Key ID:       AEGIS-CREATOR-V1 (AegisDetect Root of Trust)")
    print(f"     Signature:    {sig}")
    print(f"     Key (SHA256): {derived_key.hex()[:16]}... (256-bit derived)")
    print(f"     Commands:     {cmd_count}")

if __name__ == "__main__":
    target = "build/cmd.dtbase"
    cli_key = None
    cli_key_file = None

    args = sys.argv[1:]
    i = 0
    while i < len(args):
        if args[i] == "--key" and i + 1 < len(args):
            cli_key = args[i + 1]
            i += 2
        elif args[i] == "--key-file" and i + 1 < len(args):
            cli_key_file = args[i + 1]
            i += 2
        else:
            target = args[i]
            i += 1

    derived = load_creator_key(cli_key, cli_key_file)
    sign_cmddb(target, derived)
