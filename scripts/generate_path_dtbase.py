#!/usr/bin/env python3
"""
Generates the PurpleOS Search Path Database (/System/CLI/path.dtbase).
This file contains the search directories for executables, applications, and scripts.
As per system specifications, this file has NO cryptographic checks so users
and system scripts can frequently configure and extend search directories.
"""

import sys
import os

DEFAULT_PATHS = [
    "/System/Applications",
    "/System/CLI/scripts",
    "/System/Drivers",
    "/System/Users/aegis/Applications",
    "/tmp",
    "/",
]

def generate_path_dtbase():
    lines = [
        "# PURPLEOS SEARCH PATH DATABASE (/System/CLI/path.dtbase)",
        "# User and system configurable search paths for applications and scripts",
        "# Security checks disabled on this file to permit dynamic user customization",
        "",
        "[paths]",
    ]
    for p in DEFAULT_PATHS:
        lines.append(f"path = {p}")
    lines.append("")
    return "\n".join(lines).encode("utf-8")

if __name__ == "__main__":
    out_path = sys.argv[1] if len(sys.argv) > 1 else "OS/build/path.dtbase"
    os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
    content = generate_path_dtbase()
    with open(out_path, "wb") as f:
        f.write(content)
    print(f"[OK] Generated {out_path} ({len(content)} bytes, {len(DEFAULT_PATHS)} search paths)")
