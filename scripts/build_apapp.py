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

# Add scripts directory to path to import build_apexec
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from build_apexec import build_native_apexec, build_fault_test_apexec

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

    apps = [
        {
            "filename": "PurpleTerminal.apapplication",
            "alias": "terminal.apapplication",
            "id": "terminal.apapplication",
            "name": "Purple Terminal",
            "version": "1.0.0",
            "author": "Purple OS Team",
            "desc": "Native interactive command shell for Purple OS",
            "perms": PERM_CONSOLE | PERM_GUI | PERM_STORAGE,
            "width": 640,
            "height": 400,
            "file": "/welcome.txt"
        },
        {
            "filename": "PurpleFiles.apapplication",
            "alias": "files.apapplication",
            "id": "files.apapplication",
            "name": "Purple File Manager",
            "version": "1.0.0",
            "author": "Purple OS Team",
            "desc": "PurpleFS v3 hierarchical filesystem browser",
            "perms": PERM_GUI | PERM_STORAGE,
            "width": 560,
            "height": 380,
            "file": "/"
        },
        {
            "filename": "PurpleViewer.apapplication",
            "alias": "viewer.apapplication",
            "id": "viewer.apapplication",
            "name": "Purple View",
            "version": "1.0.0",
            "author": "Purple OS Team",
            "desc": "Document viewer for .txt, .md Markdown, and .pdf files",
            "perms": PERM_GUI | PERM_STORAGE,
            "width": 600,
            "height": 440,
            "file": "/desktop.cfg"
        },
        {
            "filename": "PurpleSettings.apapplication",
            "alias": "settings.apapplication",
            "id": "settings.apapplication",
            "name": "Settings & Preferences",
            "version": "1.1.0",
            "author": "Purple OS Team",
            "desc": "System preferences, theme, audio, and display settings",
            "perms": PERM_GUI | PERM_SYSADMIN,
            "width": 480,
            "height": 320,
            "file": "/System/Configuration/system.conf"
        },
        {
            "filename": "VLC.apapplication",
            "alias": "vlc.apapplication",
            "id": "vlc.apapplication",
            "name": "VLC Media Player",
            "version": "3.0.21-purple",
            "author": "VideoLAN / Purple OS Team",
            "desc": "Default media player and streaming media engine",
            "perms": PERM_GUI | PERM_STORAGE | PERM_NETWORK,
            "width": 620,
            "height": 420,
            "file": "/Users/aegis/Videos/sample.mp4"
        },
        {
            "filename": "sysmon.apapplication",
            "alias": None,
            "id": "sysmon.apapplication",
            "name": "Aegis System Monitor",
            "version": "1.2.0",
            "author": "Aegis Security Lab",
            "desc": "Real-time task monitor and memory usage analyzer",
            "perms": PERM_CONSOLE | PERM_GUI | PERM_SYSADMIN,
            "width": 500,
            "height": 350,
            "file": None
        },
        {
            "filename": "calculator.apapplication",
            "alias": None,
            "id": "calculator.apapplication",
            "name": "Desktop Calculator",
            "version": "1.0.1",
            "author": "Purple OS Team",
            "desc": "Standard scientific desktop calculator",
            "perms": PERM_GUI,
            "width": 280,
            "height": 340,
            "file": None
        },
        {
            "filename": "appmgr.apapplication",
            "alias": None,
            "id": "appmgr.apapplication",
            "name": "Application Manager",
            "version": "1.0.0",
            "author": "Aegis Security Lab",
            "desc": "Package manager and HMAC signature validator",
            "perms": PERM_GUI | PERM_SYSADMIN | PERM_STORAGE,
            "width": 520,
            "height": 340,
            "file": None
        },
        {
            "filename": "editor.apapplication",
            "alias": None,
            "id": "editor.apapplication",
            "name": "Aegis Text Editor",
            "version": "1.0.0",
            "author": "Purple OS Team",
            "desc": "Full-screen native text editor and document creator",
            "perms": PERM_GUI | PERM_STORAGE,
            "width": 560,
            "height": 360,
            "file": "/Users/aegis/Desktop/notes.txt"
        },
        {
            "filename": "help.apapplication",
            "alias": None,
            "id": "help.apapplication",
            "name": "Help & Documentation Center",
            "version": "1.0.0",
            "author": "Purple OS Documentation Team",
            "desc": "Interactive OS guide, shortcuts, and manual",
            "perms": PERM_GUI,
            "width": 580,
            "height": 380,
            "file": None
        },
        {
            "filename": "diskmgr.apapplication",
            "alias": None,
            "id": "diskmgr.apapplication",
            "name": "Purple Partition Manager",
            "version": "1.0.0",
            "author": "Aegis Storage Group",
            "desc": "Disk utility, partition manager, and filesystem administration",
            "perms": PERM_GUI | PERM_STORAGE | PERM_SYSADMIN,
            "width": 640,
            "height": 420,
            "file": None
        }
    ]

    for app in apps:
        apex_path = os.path.join(output_dir, f"{app['id']}.apexec")
        init_msg = f"{app['name']}: Ring 3 userspace process initialized\n"
        build_native_apexec(
            apex_path,
            app["name"],
            app["width"],
            app["height"],
            init_msg,
            file_path=app.get("file")
        )

        with open(apex_path, "rb") as f:
            payload_bytes = f.read()

        out_primary = os.path.join(output_dir, app["filename"])
        build_apapp(
            out_primary,
            app["id"],
            app["name"],
            app["version"],
            app["author"],
            app["desc"],
            app["perms"],
            app["width"],
            app["height"],
            payload_bytes
        )

        # Also copy to root build/ dir if output_dir is build/apps
        if "build/apps" in output_dir:
            root_out = os.path.join("build", app["filename"])
            build_apapp(
                root_out,
                app["id"],
                app["name"],
                app["version"],
                app["author"],
                app["desc"],
                app["perms"],
                app["width"],
                app["height"],
                payload_bytes
            )

        if app["alias"]:
            out_alias = os.path.join(output_dir, app["alias"])
            build_apapp(
                out_alias,
                app["id"],
                app["name"],
                app["version"],
                app["author"],
                app["desc"],
                app["perms"],
                app["width"],
                app["height"],
                payload_bytes
            )
            if "build/apps" in output_dir:
                root_alias = os.path.join("build", app["alias"])
                build_apapp(
                    root_alias,
                    app["id"],
                    app["name"],
                    app["version"],
                    app["author"],
                    app["desc"],
                    app["perms"],
                    app["width"],
                    app["height"],
                    payload_bytes
                )

    # Controlled Ring 3 security fault test application
    fault_apex_path = os.path.join(output_dir, "fault_test.apexec")
    build_fault_test_apexec(fault_apex_path)
    with open(fault_apex_path, "rb") as f:
        fault_payload = f.read()
    build_apapp(
        os.path.join(output_dir, "fault_test.apapplication"),
        "fault_test.apapplication",
        "Ring 3 Fault Test",
        "1.0.0",
        "Aegis Security Lab",
        "Controlled Ring 3 privilege fault containment validator",
        PERM_CONSOLE,
        300,
        200,
        fault_payload
    )

if __name__ == "__main__":
    out_dir = sys.argv[1] if len(sys.argv) > 1 else "build/apps"
    generate_default_apps(out_dir)
