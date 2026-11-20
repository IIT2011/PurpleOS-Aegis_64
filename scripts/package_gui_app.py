#!/usr/bin/env python3
"""Produce signed Purple OS GUI bundles and Creator-authorized signer policies."""

import argparse
import hashlib
from pathlib import Path
import re
import struct
import subprocess
from tempfile import TemporaryDirectory
import zlib

from sign_cmddb import sign_payload_rsa

DOMAIN = b"PurpleOS/APAPPLICATION/v2\0"
POLICY_DOMAIN = b"PurpleOS/APP-TRUST/v1\0"
ED25519_SPKI = bytes.fromhex("302a300506032b6570032100")
MANIFEST_SIZE = 336
MAX_BUNDLE = 32 * 1024 * 1024
MAX_ID = 55 - len("System/Applications/") - len(".apapplication")
PAGE = 4096
USER_MIN = 1 << 39
STACK_TOP = 0x7fff_f000_0000
MAX_STACK = 1024 * 1024
HEAP_SIZE = 256 * 1024 * 1024
IMAGE_LIMIT = STACK_TOP - MAX_STACK - MAX_BUNDLE - HEAP_SIZE - 5 * PAGE


def app_id(value):
    if len(value) > MAX_ID or not re.fullmatch(r"[a-z0-9]+(?:\.[a-z0-9]+)*", value):
        raise ValueError("Application ID must fit the PurpleFS path and use lowercase alphanumeric segments")
    return value


def executable(path, stack_size):
    elf = path.read_bytes()
    if len(elf) < 64 or elf[:7] != b"\x7fELF\x02\x01\x01":
        raise ValueError("Expected a little-endian ELF64 executable")
    kind, arch = struct.unpack_from("<HH", elf, 16)
    entry, offset = struct.unpack_from("<QQ", elf, 24)
    stride, count = struct.unpack_from("<HH", elf, 54)
    if (kind, arch, stride) != (2, 62, 56) or offset < 64 or offset + stride * count > len(elf):
        raise ValueError("Expected a statically linked x86-64 ELF with a valid program table")

    loads = []
    executable_entry = False
    ranges = []
    for index in range(count):
        kind, flags, off, address, _, size, mem, _ = struct.unpack_from("<IIQQQQQQ", elf, offset + stride * index)
        if kind in (2, 3, 7):
            raise ValueError("Dynamic linking, interpreters, and ELF TLS are unavailable")
        if kind != 1 or mem == 0:
            continue
        if flags not in (4, 5, 6) or (size == 0 and flags != 6):
            raise ValueError("Unsupported load permissions")
        end = address + ((mem + PAGE - 1) // PAGE) * PAGE
        if not USER_MIN <= address < end <= IMAGE_LIMIT or address % PAGE or size > mem or off + size > len(elf):
            raise ValueError("Invalid segment bounds")
        if any(address < stop and begin < end for begin, stop in ranges):
            raise ValueError("Overlapping load pages")
        ranges.append((address, end))
        executable_entry |= flags == 5 and address <= entry < address + size
        ape_flags = 1 | (2 if flags == 6 else 4 if flags == 5 else 0)
        ape_kind = 4 if size == 0 else {4: 2, 5: 1, 6: 3}[flags]
        loads.append((ape_kind, ape_flags, address, mem, elf[off:off + size]))
    if not executable_entry or not 1 <= len(loads) <= 16:
        raise ValueError("ELF must contain up to 16 load segments and a file-backed entry point")
    if sum(end - begin for begin, end in ranges) > 64 * 1024 * 1024:
        raise ValueError("Executable exceeds the image memory budget")

    descriptors = bytearray()
    payload = bytearray()
    start = 64 + 40 * len(loads)
    for kind, flags, address, mem, data in loads:
        descriptors.extend(struct.pack("<IIQQQQ", kind, flags, start + len(payload) if data else 0,
                                       len(data), address, mem))
        payload.extend(data)
    header = struct.pack("<4sBBHIQQQHHII4I", b"\x7fAPE", 1, 1, 0x8664, 64, entry,
                         min(begin for begin, _ in ranges), start + len(payload), len(loads),
                         0, stack_size, zlib.crc32(payload), 0, 0, 0, 0)
    return header + descriptors + payload


def field(header, offset, capacity, text, required=False):
    raw = text.encode("utf-8")
    if (required and not raw) or len(raw) >= capacity or any(ord(c) < 32 or 127 <= ord(c) <= 159 for c in text):
        raise ValueError("Manifest text is empty, too long, or contains control characters")
    header[offset:offset + len(raw)] = raw


def pack(args):
    identifier = app_id(args.id)
    if args.output.name != identifier + ".apapplication":
        raise ValueError("Bundle filename must match the application ID")
    if args.stack < 0 or args.stack > 1024 * 1024 or not 80 <= args.width <= 16384 or not 1 <= args.height <= 16384:
        raise ValueError("Invalid window or stack dimensions")

    image = executable(args.elf, args.stack)
    resources = args.resources.read_bytes() if args.resources else b""
    metadata = args.metadata.read_bytes() if args.metadata else b""
    if len(image) + len(resources) + len(metadata) + MANIFEST_SIZE > MAX_BUNDLE:
        raise ValueError("Bundle exceeds the runtime file size limit")
    if args.key.resolve() == args.output.resolve():
        raise ValueError("Signing key and output must be different files")

    public = subprocess.check_output(["openssl", "pkey", "-in", str(args.key), "-pubout", "-outform", "DER"])
    if not public.startswith(ED25519_SPKI) or len(public) != len(ED25519_SPKI) + 32:
        raise ValueError("An Ed25519 private key is required")
    public = public[len(ED25519_SPKI):]
    caps = 2 | int(args.console) | (4 if args.shell else 0)
    header = bytearray(MANIFEST_SIZE)
    struct.pack_into("<8sII", header, 0, b"APAPP200", 2, MANIFEST_SIZE)
    field(header, 16, 32, identifier, True)
    field(header, 48, 32, args.name, True)
    field(header, 80, 16, args.version, True)
    field(header, 96, 32, args.author)
    field(header, 128, 64, args.description)
    struct.pack_into("<4I", header, 0xc0, caps, args.priority, args.width, args.height)
    cursor = MANIFEST_SIZE
    for offset, section in [(0xd0, image), (0xd8, resources), (0xe8, metadata)]:
        if section:
            struct.pack_into("<2I", header, offset, cursor, len(section))
            cursor += len(section)
    header[240:272] = hashlib.sha256(public).digest()
    payload = image + resources + metadata

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory(dir=args.output.parent) as temp:
        message = Path(temp) / "message"
        message.write_bytes(DOMAIN + header + payload)
        signature = subprocess.check_output(
            ["openssl", "pkeyutl", "-sign", "-rawin", "-inkey", str(args.key), "-in", str(message)]
        )
    if len(signature) != 64:
        raise ValueError("Unexpected Ed25519 signature size")
    header[272:] = signature
    args.output.write_bytes(header + payload)
    print(f"{identifier} {public.hex()} {caps:x} {args.priority}")


def sign_policy(args):
    policy = args.policy.read_bytes()
    lines = policy.decode("ascii").splitlines()
    if not lines or lines[0] != "APTRUST1" or len(policy) > 16 * 1024:
        raise ValueError("Expected a bounded APTRUST1 policy")
    entries = set()
    for line in lines[1:]:
        identifier, key, caps, priority = line.split()
        if not re.fullmatch(r"[a-f0-9]{64}", key) or caps not in ("2", "3", "6", "7") or priority not in ("1", "2"):
            raise ValueError("Invalid signer policy entry")
        entry = (app_id(identifier), key)
        if entry in entries:
            raise ValueError("Duplicate app and signing key")
        entries.add(entry)
    signature = sign_payload_rsa(POLICY_DOMAIN + policy, str(args.key))
    args.policy.with_name(args.policy.name + ".sig").write_text(signature, encoding="ascii")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(required=True)
    package = commands.add_parser("pack")
    package.set_defaults(run=pack)
    package.add_argument("elf", type=Path)
    package.add_argument("output", type=Path)
    package.add_argument("--id", required=True)
    package.add_argument("--name", required=True)
    package.add_argument("--version", required=True)
    package.add_argument("--author", default="")
    package.add_argument("--description", default="")
    package.add_argument("--key", type=Path, required=True)
    package.add_argument("--width", type=int, required=True)
    package.add_argument("--height", type=int, required=True)
    package.add_argument("--stack", type=int, default=0)
    package.add_argument("--priority", type=int, choices=(1, 2), default=2)
    package.add_argument("--console", action="store_true")
    package.add_argument("--shell", action="store_true")
    package.add_argument("--resources", type=Path)
    package.add_argument("--metadata", type=Path)
    trust = commands.add_parser("sign-policy")
    trust.set_defaults(run=sign_policy)
    trust.add_argument("policy", type=Path)
    trust.add_argument("--key", type=Path, required=True)
    args = parser.parse_args()
    try:
        args.run(args)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
