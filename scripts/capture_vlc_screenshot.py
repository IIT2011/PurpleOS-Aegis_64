#!/usr/bin/env python3
"""
Capture Native Running VLC Media Player Screenshot from Aegis64 in QEMU via QMP screendump.
"""

import subprocess
import socket
import struct
import zlib
import json
import time
import os
import sys
import tempfile
from pathlib import Path

def ppm_to_png(ppm_path, png_path):
    with open(ppm_path, "rb") as f:
        magic = f.readline().strip()
        if magic != b"P6":
            raise ValueError(f"Unsupported PPM format: {magic}")
        line = f.readline()
        while line.startswith(b"#"):
            line = f.readline()
        parts = line.split()
        if len(parts) == 2:
            width, height = int(parts[0]), int(parts[1])
        else:
            width = int(parts[0])
            height = int(f.readline().strip())
        maxval = int(f.readline().strip())
        raw_pixels = f.read()

    stride = width * 3
    scanlines = bytearray()
    for y in range(height):
        scanlines.append(0)  # Filter 0: None
        start = y * stride
        scanlines.extend(raw_pixels[start : start + stride])

    compressed = zlib.compress(bytes(scanlines), 9)

    def make_chunk(chunk_type, data):
        return (
            struct.pack(">I", len(data))
            + chunk_type
            + data
            + struct.pack(">I", zlib.crc32(chunk_type + data) & 0xFFFFFFFF)
        )

    ihdr_data = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    png_bytes = (
        b"\x89PNG\r\n\x1a\n"
        + make_chunk(b"IHDR", ihdr_data)
        + make_chunk(b"IDAT", compressed)
        + make_chunk(b"IEND", b"")
    )

    with open(png_path, "wb") as f:
        f.write(png_bytes)
    print(f"[OK] Saved {png_path} ({width}x{height}, {len(png_bytes):,} bytes)")

def main():
    print("=================================================================")
    print("  PURPLE OS / AEGIS64 — VLC MEDIA PLAYER SCREENSHOT CAPTURE      ")
    print("=================================================================")

    disk_path = "build/purple_gpt.img"
    if not os.path.exists(disk_path):
        print(f"Creating disk at {disk_path}...")
        subprocess.check_call([sys.executable, "build/mkpfs2_disk", disk_path])

    qmp_port = 4488
    cmd = [
        "qemu-system-x86_64",
        "-kernel", "target/kernel.elf",
        "-serial", "stdio",
        "-vga", "std",
        "-m", "256M",
        "-smp", "4",
        "-drive", f"file={disk_path},format=raw,index=0,media=disk",
        "-qmp", f"tcp:127.0.0.1:{qmp_port},server,nowait",
        "-no-reboot"
    ]

    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=0
    )

    import threading
    captured = []
    def read_stdout():
        for line in iter(proc.stdout.readline, ''):
            captured.append(line)
            sys.stdout.write(line)
            sys.stdout.flush()
    t = threading.Thread(target=read_stdout, daemon=True)
    t.start()

    # Connect to QMP
    print("Connecting to QEMU QMP socket...")
    s = None
    for _ in range(60):
        try:
            s = socket.create_connection(("127.0.0.1", qmp_port), timeout=2.0)
            break
        except Exception:
            time.sleep(0.2)

    if not s:
        print("[ERROR] Could not connect to QMP socket.")
        proc.kill()
        sys.exit(1)

    f = s.makefile("rw", encoding="utf-8")
    greeting = f.readline()
    f.write(json.dumps({"execute": "qmp_capabilities"}) + "\r\n")
    f.flush()
    f.readline()

    print("QMP Connected. Waiting for kernel boot & shell prompt (12s)...")
    for _ in range(30):
        if any("aegis64@purple:~$" in l for l in captured):
            break
        time.sleep(0.5)

    temp_dir = tempfile.gettempdir()
    ppm_vlc = os.path.join(temp_dir, "qemu_screen_vlc.ppm")

    # Send launch vlc command
    print("Launching native VLC media player ('launch vlc')...")
    proc.stdin.write("launch vlc\n")
    proc.stdin.flush()
    time.sleep(2.5)

    # Force compositor render pass
    proc.stdin.write("compositor render\n")
    proc.stdin.flush()
    time.sleep(0.5)

    # Capture Screen
    print("Capturing Live Desktop with VLC Media Player...")
    f.write(json.dumps({"execute": "screendump", "arguments": {"filename": ppm_vlc}}) + "\r\n")
    f.flush()
    resp = f.readline()
    print("QMP Response:", resp.strip())

    # Shutdown QEMU cleanly
    f.write(json.dumps({"execute": "quit"}) + "\r\n")
    f.flush()
    try:
        proc.wait(timeout=3)
    except Exception:
        proc.kill()

    out_dir = Path("build/screenshots")
    out_dir.mkdir(parents=True, exist_ok=True)
    png_vlc = out_dir / "purple_vlc_player.png"

    if os.path.exists(ppm_vlc):
        ppm_to_png(ppm_vlc, str(png_vlc))
        try: os.remove(ppm_vlc)
        except: pass
        print(f"\n>>> VLC SCREENSHOT CAPTURED SUCCESSFULLY: {png_vlc} <<<\n")
        return 0
    else:
        print("[ERROR] PPM screenshot file was not produced by QEMU.")
        return 1

if __name__ == "__main__":
    sys.exit(main())
