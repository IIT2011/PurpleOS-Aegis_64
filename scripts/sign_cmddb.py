#!/usr/bin/env python3
"""
PurpleOS Creator Cryptographic Signing Tool for /System/CLI/cmd.dtbase.
Authenticates the command database using the Creator's Offline RSA-2048 Private Key
and AegisDetect Asymmetric Public Key Root of Trust Anchor.

Mathematical Security:
  - Algorithm: RSA-2048 PKCS#1 v1.5 with SHA-256
  - Creator Private Key: keys/creator_private_key.pem (Offline, ignored by git)
  - Public Key Modulus N: Embedded directly into src/security/aegis_detect.rs
  - Signature Length: 256 bytes (512 hexadecimal characters)
  - Unforgeable: Impossible to forge without factoring 2048-bit modulus N.
"""

import subprocess
import shutil
import sys
import os

DEFAULT_KEY_PATHS = [
    "keys/creator_private_key.pem",
    "OS/keys/creator_private_key.pem",
    "../keys/creator_private_key.pem",
    "d:/OS Development/OS/keys/creator_private_key.pem",
]

OPENSSL_CANDIDATES = [
    shutil.which("openssl"),
    r"D:\OS Development\Dev Tools\Git\usr\bin\openssl.exe",
    r"C:\Program Files\Git\usr\bin\openssl.exe",
    r"C:\Program Files\OpenSSL-Win64\bin\openssl.exe",
]

def get_openssl_bin() -> str:
    for cand in OPENSSL_CANDIDATES:
        if cand and os.path.exists(cand):
            return cand
    raise RuntimeError("OpenSSL executable not found.")

def find_creator_private_key(cli_key_file: str = None) -> str:
    if cli_key_file and os.path.exists(cli_key_file):
        return os.path.abspath(cli_key_file)

    for kp in DEFAULT_KEY_PATHS:
        if os.path.exists(kp):
            return os.path.abspath(kp)

    raise FileNotFoundError(
        "Creator RSA private key not found.\n"
        "Ensure keys/creator_private_key.pem exists (which is strictly offline and ignored by git)."
    )

def sign_payload_rsa(payload: bytes, key_path: str) -> str:
    openssl = get_openssl_bin()
    p = subprocess.Popen(
        [openssl, "dgst", "-sha256", "-sign", key_path],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE
    )
    sig_bytes, err = p.communicate(input=payload)
    if p.returncode != 0:
        raise RuntimeError(f"OpenSSL signing failed: {err.decode('utf-8', errors='replace')}")
    if len(sig_bytes) != 256:
        raise RuntimeError(f"Unexpected signature length: {len(sig_bytes)} (expected 256 bytes)")
    return sig_bytes.hex()

def sign_cmddb(file_path: str, key_path: str):
    if not os.path.exists(file_path):
        print(f"[ERROR] File not found: {file_path}")
        sys.exit(1)

    raw_data = open(file_path, "rb").read()

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

    # Compute RSA-2048 PKCS#1 v1.5 signature using Creator Private Key
    sig = sign_payload_rsa(payload, key_path)

    # Count commands
    cmd_count = sum(1 for l in payload_lines if l.strip().startswith(b"[cmd:"))

    header = (
        f"# PURPLEOS COMMAND DATABASE — CRYPTOGRAPHICALLY SIGNED BY CREATOR\n"
        f"# SIGNER: Creator (Aegis Root Developer)\n"
        f"# KEY_ID: AEGIS-CREATOR-V1\n"
        f"# ALGORITHM: RSA-2048 PKCS#1 v1.5 / SHA-256\n"
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
    print(f"     Algorithm:    RSA-2048 PKCS#1 v1.5 with SHA-256")
    print(f"     Signature:    {sig[:32]}...{sig[-32:]} (512 hex characters / 2048 bits)")
    print(f"     Private Key:  {key_path}")
    print(f"     Commands:     {cmd_count}")

if __name__ == "__main__":
    target = "build/cmd.dtbase"
    cli_key_file = None

    args = sys.argv[1:]
    i = 0
    while i < len(args):
        if args[i] == "--key-file" and i + 1 < len(args):
            cli_key_file = args[i + 1]
            i += 2
        elif args[i] == "--key" and i + 1 < len(args):
            cli_key_file = args[i + 1]
            i += 2
        else:
            target = args[i]
            i += 1

    key_path = find_creator_private_key(cli_key_file)
    sign_cmddb(target, key_path)
