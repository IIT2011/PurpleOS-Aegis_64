#!/usr/bin/env python3
"""
.apapplication Forensic Validator for Purple OS / Aegis64.
Inspects and cryptographically verifies application bundles.
"""

import sys
import os
import struct
import hmac
import hashlib
import binascii

HEADER_UPTO_SIG_FMT = "<8sII32s32s16s32s64sIIII8I"
HEADER_UPTO_SIG_SIZE = struct.calcsize(HEADER_UPTO_SIG_FMT) # 240 bytes
TRAILING_FMT = "<32sIII"
TRAILING_SIZE = struct.calcsize(TRAILING_FMT) # 44 bytes
MANIFEST_SIZE = HEADER_UPTO_SIG_SIZE + TRAILING_SIZE # 284 bytes

TRUST_KEY = b"PurpleOS-Aegis64-TrustAnchor-2026"

PERM_NAMES = {
    (1 << 0): "CONSOLE",
    (1 << 1): "GUI",
    (1 << 2): "STORAGE",
    (1 << 3): "NETWORK",
    (1 << 4): "SYSADMIN"
}

def validate_apapp(filepath):
    if not filepath.endswith(".apapplication"):
        print(f"[FAIL] Invalid application bundle extension in '{filepath}' (expected '.apapplication')")
        return False

    if not os.path.exists(filepath):
        print(f"[FAIL] File not found: {filepath}")
        return False

    with open(filepath, "rb") as f:
        data = f.read()

    file_size = len(data)
    if file_size < MANIFEST_SIZE:
        print(f"[FAIL] Bundle too small: {file_size} bytes (minimum {MANIFEST_SIZE} bytes)")
        return False

    # 1. Unpack Manifest
    header_raw = data[:HEADER_UPTO_SIG_SIZE]
    trailing_raw = data[HEADER_UPTO_SIG_SIZE:MANIFEST_SIZE]

    (magic, version, manifest_size, app_id, app_name, version_str,
     author, desc, perms, priority, width, height,
     exec_off, exec_sz, res_off, res_sz, lib_off, lib_sz, meta_off, meta_sz) = struct.unpack(
        HEADER_UPTO_SIG_FMT, header_raw
    )

    sig, checksum, _, _ = struct.unpack(TRAILING_FMT, trailing_raw)

    magic_str = magic.decode('ascii', errors='replace')
    if magic != b"APAPP100":
        print(f"[FAIL] Bad Magic: {magic_str} (expected 'APAPP100')")
        return False

    if version != 1:
        print(f"[FAIL] Unsupported version: {version}")
        return False

    if manifest_size != MANIFEST_SIZE:
        print(f"[FAIL] Manifest size mismatch: {manifest_size} != {MANIFEST_SIZE}")
        return False

    # Clean strings
    clean = lambda b: b.split(b'\0')[0].decode('utf-8', errors='replace')
    app_id_s = clean(app_id)
    app_name_s = clean(app_name)
    ver_s = clean(version_str)
    auth_s = clean(author)
    desc_s = clean(desc)

    # Decode permissions
    perm_list = [name for bit, name in PERM_NAMES.items() if (perms & bit)]
    perms_str = ", ".join(perm_list) if perm_list else "NONE"

    # Bounds check payload
    if exec_off + exec_sz > file_size:
        print(f"[FAIL] Executable payload out of bounds: offset {exec_off} + size {exec_sz} > file size {file_size}")
        return False

    payload = data[exec_off:exec_off + exec_sz]

    # Verify HMAC-SHA256 signature
    h = hmac.new(TRUST_KEY, header_raw + payload, hashlib.sha256)
    expected_sig = h.digest()
    sig_valid = (sig == expected_sig)

    # Verify payload checksum
    calc_crc = binascii.crc32(payload) & 0xFFFFFFFF
    crc_valid = (checksum == calc_crc)

    print("=====================================================")
    print(f" AegisDetect Forensic Audit: {os.path.basename(filepath)}")
    print("=====================================================")
    print(f" Application ID   : {app_id_s}")
    print(f" Name             : {app_name_s} (v{ver_s})")
    print(f" Author           : {auth_s}")
    print(f" Description      : {desc_s}")
    print(f" Window Geometry  : {width} x {height} px")
    print(f" Task Priority    : {priority}")
    print(f" Permissions      : {perms_str} (0x{perms:04X})")
    print("-----------------------------------------------------")
    print(f" Executable Code  : {exec_sz} bytes at offset 0x{exec_off:04X}")
    print(f" Payload Checksum : 0x{checksum:08X} {'[VALID]' if crc_valid else '[CORRUPT]'}")
    print(f" HMAC Signature   : {'[AUTHENTICATED]' if sig_valid else '[INVALID/TAMPERED]'}")
    print(f" Trust Anchor Key : PurpleOS-Aegis64-TrustAnchor-2026")
    print("=====================================================")

    return sig_valid and crc_valid

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <path-to-.apapplication>")
        sys.exit(1)

    ok = validate_apapp(sys.argv[1])
    sys.exit(0 if ok else 1)
