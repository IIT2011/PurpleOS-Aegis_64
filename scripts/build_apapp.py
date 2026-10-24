#!/usr/bin/env python3
"""
.apapplication Native Application Bundle Builder for Purple OS / Aegis64.
Constructs authenticated APAPP100 application bundles with HMAC-SHA256 signatures.
"""

import sys
import os
import struct
import hmac
import hashlib
import binascii

# Manifest format string:
# 8s: magic
# I: version
# I: manifest_size
# 32s: app_id
# 32s: app_name
# 16s: version_str
# 32s: author
# 64s: description
# I: permissions
# I: default_priority
# I: window_width
# I: window_height
# 8I: exec_offset, exec_size, res_offset, res_size, lib_offset, lib_size, meta_offset, meta_size
HEADER_UPTO_SIG_FMT = "<8sII32s32s16s32s64sIIII8I"
HEADER_UPTO_SIG_SIZE = struct.calcsize(HEADER_UPTO_SIG_FMT) # 240 bytes

TRAILING_FMT = "<32sIII" # hmac[32], checksum[4], reserved[8]
TRAILING_SIZE = struct.calcsize(TRAILING_FMT) # 44 bytes
MANIFEST_SIZE = HEADER_UPTO_SIG_SIZE + TRAILING_SIZE # 284 bytes

TRUST_KEY = b"PurpleOS-Aegis64-TrustAnchor-2026"

# Permissions
PERM_CONSOLE  = (1 << 0)
PERM_GUI      = (1 << 1)
PERM_STORAGE  = (1 << 2)
PERM_NETWORK  = (1 << 3)
PERM_SYSADMIN = (1 << 4)

def build_apapp(out_path, app_id, app_name, version, author, desc, perms, width, height, payload_bytes):
    exec_offset = MANIFEST_SIZE
    exec_size = len(payload_bytes)

    # 1. Pack manifest header up to signature
    header_pre = struct.pack(
        HEADER_UPTO_SIG_FMT,
        b"APAPP100",
        1,
        MANIFEST_SIZE,
        app_id.encode('utf-8')[:31],
        app_name.encode('utf-8')[:31],
        version.encode('utf-8')[:15],
        author.encode('utf-8')[:31],
        desc.encode('utf-8')[:63],
        perms,
        2, # Normal priority
        width,
        height,
        exec_offset,
        exec_size,
        0, 0, # resources
        0, 0, # libraries
        0, 0  # metadata
    )

    # 2. Compute HMAC-SHA256 signature over manifest header + executable payload
    h = hmac.new(TRUST_KEY, header_pre + payload_bytes, hashlib.sha256)
    sig = h.digest()

    # 3. Compute CRC32 checksum over payload
    checksum = binascii.crc32(payload_bytes) & 0xFFFFFFFF

    # 4. Pack trailing fields
    trailing = struct.pack(TRAILING_FMT, sig, checksum, 0, 0)
    manifest = header_pre + trailing

    # 5. Write out bundle
    os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
    with open(out_path, "wb") as f:
        f.write(manifest)
        f.write(payload_bytes)

    print(f"[OK] Generated .apapplication bundle: {out_path} ({len(manifest) + len(payload_bytes)} bytes)")
    return True

def generate_default_apps(output_dir):
    os.makedirs(output_dir, exist_ok=True)

    # Minimal x86-64 executable payload:
    # entry(): xor eax, eax; ret (31 c0 c3)
    dummy_code = bytearray([0x31, 0xC0, 0xC3, 0x90] * 16) # 64 bytes

    apps = [
        {
            "filename": "terminal.apapplication",
            "id": "terminal.apapplications",
            "name": "Purple Terminal",
            "version": "1.0.0",
            "author": "Purple OS Team",
            "desc": "Native interactive command shell for Purple OS",
            "perms": PERM_CONSOLE | PERM_GUI | PERM_STORAGE,
            "width": 640,
            "height": 400
        },
        {
            "filename": "sysmon.apapplication",
            "id": "sysmon.apapplications",
            "name": "Aegis System Monitor",
            "version": "1.2.0",
            "author": "Aegis Security Lab",
            "desc": "Real-time task monitor and memory usage analyzer",
            "perms": PERM_CONSOLE | PERM_GUI | PERM_SYSADMIN,
            "width": 500,
            "height": 350
        },
        {
            "filename": "files.apapplication",
            "id": "files.apapplications",
            "name": "Purple File Manager",
            "version": "1.0.0",
            "author": "Purple OS Team",
            "desc": "PurpleFS v3 hierarchical filesystem browser",
            "perms": PERM_GUI | PERM_STORAGE,
            "width": 560,
            "height": 380
        },
        {
            "filename": "calculator.apapplication",
            "id": "calculator.apapplications",
            "name": "Desktop Calculator",
            "version": "1.0.1",
            "author": "Purple OS Team",
            "desc": "Standard scientific desktop calculator",
            "perms": PERM_GUI,
            "width": 280,
            "height": 340
        },
        {
            "filename": "settings.apapplication",
            "id": "settings.apapplications",
            "name": "Settings & Preferences",
            "version": "1.1.0",
            "author": "Purple OS Team",
            "desc": "System preferences, theme, audio, and display settings",
            "perms": PERM_GUI | PERM_SYSADMIN,
            "width": 480,
            "height": 320
        },
        {
            "filename": "appmgr.apapplication",
            "id": "appmgr.apapplications",
            "name": "Application Manager",
            "version": "1.0.0",
            "author": "Aegis Security Lab",
            "desc": "Package manager and HMAC signature validator",
            "perms": PERM_GUI | PERM_SYSADMIN | PERM_STORAGE,
            "width": 520,
            "height": 340
        },
        {
            "filename": "editor.apapplication",
            "id": "editor.apapplications",
            "name": "Aegis Text Editor",
            "version": "1.0.0",
            "author": "Purple OS Team",
            "desc": "Full-screen native text editor and document creator",
            "perms": PERM_GUI | PERM_STORAGE,
            "width": 560,
            "height": 360
        },
        {
            "filename": "help.apapplication",
            "id": "help.apapplications",
            "name": "Help & Documentation Center",
            "version": "1.0.0",
            "author": "Purple OS Documentation Team",
            "desc": "Interactive OS guide, shortcuts, and manual",
            "perms": PERM_GUI,
            "width": 580,
            "height": 380
        },
        {
            "filename": "vlc.apapplication",
            "id": "vlc.apapplications",
            "name": "VLC Media Player",
            "version": "3.0.21-purple",
            "author": "VideoLAN / Purple OS Team",
            "desc": "Default media player and streaming media engine",
            "perms": PERM_GUI | PERM_STORAGE | PERM_NETWORK,
            "width": 620,
            "height": 420
        },
        {
            "filename": "viewer.apapplication",
            "id": "viewer.apapplications",
            "name": "Purple View",
            "version": "1.0.0",
            "author": "Purple OS Team",
            "desc": "Document viewer for .txt, .md Markdown, and .pdf files",
            "perms": PERM_GUI | PERM_STORAGE,
            "width": 600,
            "height": 440
        },
        {
            "filename": "diskmgr.apapplication",
            "id": "diskmgr.apapplications",
            "name": "Purple Partition Manager",
            "version": "1.0.0",
            "author": "Aegis Storage Group",
            "desc": "Disk utility, partition manager, and filesystem administration",
            "perms": PERM_GUI | PERM_STORAGE | PERM_SYSADMIN,
            "width": 640,
            "height": 420
        }
    ]

    for app in apps:
        out = os.path.join(output_dir, app["filename"])
        build_apapp(
            out,
            app["id"],
            app["name"],
            app["version"],
            app["author"],
            app["desc"],
            app["perms"],
            app["width"],
            app["height"],
            dummy_code
        )

if __name__ == "__main__":
    out_dir = sys.argv[1] if len(sys.argv) > 1 else "build/apps"
    generate_default_apps(out_dir)
