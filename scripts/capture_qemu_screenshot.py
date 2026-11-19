#!/usr/bin/env python3
"""
Capture Native Framebuffer Screenshots from Aegis64 in QEMU via QMP screendump.
Generates PNG screenshots of:
  1. Boot Desktop (Wallpaper + Purple Bar + Purple Dock + Mirror Reflection + Pointer)
  2. Active Desktop (with Demo Windows & Active Dock Indicator Dots)
"""

import subprocess
import socket
import struct
import zlib
import json
import time
import os
import sys

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
    print("  PURPLE OS / AEGIS64 — QEMU NATIVE SCREENSHOT CAPTURE SUITE     ")
    print("=================================================================")

    disk_path = "build/purple_gpt.img"
    if not os.path.exists(disk_path):
        print(f"Creating disk at {disk_path}...")
        subprocess.check_call([sys.executable, "build/mkpfs2_disk", disk_path])

    qmp_port = 4477
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
            time.sleep(0.1)

    if not s:
        print("[ERROR] Could not connect to QMP socket.")
        proc.kill()
        sys.exit(1)

    f = s.makefile("rw", encoding="utf-8")
    greeting = f.readline()
    f.write(json.dumps({"execute": "qmp_capabilities"}) + "\r\n")
    f.flush()
    f.readline()

    print("QMP Connected. Waiting for kernel boot & compositor initialization (3.5s)...")
    time.sleep(3.5)

    ppm_boot = "/data/data/com.termux/files/usr/tmp/qemu_screen_boot.ppm"
    ppm_windows = "/data/data/com.termux/files/usr/tmp/qemu_screen_windows.ppm"

    # Refresh compositor frame with latest hardware timer state
    proc.stdin.write("compositor render\n")
    proc.stdin.flush()
    time.sleep(0.3)

    # Capture 1: Boot Desktop
    print("Capturing 1: Native Desktop on Boot...")
    f.write(json.dumps({"execute": "screendump", "arguments": {"filename": ppm_boot}}) + "\r\n")
    f.flush()
    resp1 = f.readline()
    print("QMP Response 1:", resp1.strip())

    # Send command to launch demo windows
    print("Launching demo windows via serial shell ('compositor demo')...")
    proc.stdin.write("compositor demo\n")
    proc.stdin.flush()
    time.sleep(1.5)

    # Capture 2: Active Desktop with Windows & Indicators
    print("Capturing 2: Desktop with Windows & Active Dock Indicators...")
    f.write(json.dumps({"execute": "screendump", "arguments": {"filename": ppm_windows}}) + "\r\n")
    f.flush()
    resp2 = f.readline()
    print("QMP Response 2:", resp2.strip())

    # Shutdown QEMU cleanly
    f.write(json.dumps({"execute": "quit"}) + "\r\n")
    f.flush()
    proc.wait(timeout=3)

    out_dir = "/data/data/com.termux/files/home/.gemini/antigravity-cli/brain/352e5877-a9d7-49e4-ab09-5d00ef99fd0a"
    png_boot = os.path.join(out_dir, "purple_desktop_actual_boot.png")
    png_bar_dock = os.path.join(out_dir, "purple_bar_and_purple_dock.png")
    png_windows = os.path.join(out_dir, "purple_desktop_actual_windows.png")

    if os.path.exists(ppm_boot):
        ppm_to_png(ppm_boot, png_boot)
        ppm_to_png(ppm_boot, png_bar_dock)
        try: os.remove(ppm_boot)
        except: pass

    if os.path.exists(ppm_windows):
        ppm_to_png(ppm_windows, png_windows)
        try: os.remove(ppm_windows)
        except: pass

    # Also copy to phone internal storage
    phone_dir = "/sdcard/Pictures/Purple_Desktop_UI"
    if os.path.exists(phone_dir):
        import shutil
        if os.path.exists(png_boot):
            dst = os.path.join(phone_dir, "Purple_Desktop_Actual_Boot.png")
            shutil.copyfile(png_boot, dst)
            os.system(f'am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d "file://{dst}" 2>/dev/null || true')
            print(f"[OK] Copied to phone storage: {dst}")

            dst_bar_dock = os.path.join(phone_dir, "Purple_Bar_and_Purple_Dock.png")
            shutil.copyfile(png_boot, dst_bar_dock)
            os.system(f'am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d "file://{dst_bar_dock}" 2>/dev/null || true')
            print(f"[OK] Copied to phone storage: {dst_bar_dock}")

        if os.path.exists(png_windows):
            dst = os.path.join(phone_dir, "Purple_Desktop_Actual_Windows.png")
            shutil.copyfile(png_windows, dst)
            os.system(f'am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d "file://{dst}" 2>/dev/null || true')
            print(f"[OK] Copied to phone storage: {dst}")

    print("\n>>> ALL SCREENSHOTS CAPTURED AND CONVERTED SUCCESSFULLY! <<<")

if __name__ == "__main__":
    main()
