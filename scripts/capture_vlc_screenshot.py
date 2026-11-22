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

    os.chdir(os.path.join(os.path.dirname(__file__), ".."))
    disk_path = "build/purple_gpt.img"
    wav_path = "build/purple_audio.wav"

    print(f"Ensuring disk at {disk_path} with applications...")
    subprocess.check_call([
        sys.executable,
        "build/mkpfs2_disk",
        disk_path,
        "--applications-dir", "build/applications"
    ])

    qmp_port = 4488
    cmd = [
        "qemu-system-x86_64",
        "-kernel", "target/kernel.elf",
        "-serial", "stdio",
        "-vga", "std",
        "-m", "256M",
        "-smp", "4",
        "-drive", f"file={disk_path},format=raw,index=0,media=disk",
        "-device", "intel-hda,id=hda0",
        "-device", "hda-output,id=hda0.0,audiodev=snd0",
        "-audiodev", f"wav,id=snd0,path={wav_path}",
        "-qmp", f"tcp:127.0.0.1:{qmp_port},server,nowait",
        "-display", "none",
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
    captured_chunks = []
    def read_stdout():
        while True:
            ch = proc.stdout.read(1)
            if not ch:
                break
            captured_chunks.append(ch)
            sys.stdout.write(ch)
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

    print("QMP Connected. Waiting for kernel boot & shell prompt...")
    start_wait = time.time()
    shell_ready = False
    while time.time() - start_wait < 25:
        text = "".join(captured_chunks)
        if "aegis64@purple:~$" in text or "Type 'help' for commands" in text:
            shell_ready = True
            break
        time.sleep(0.2)

    if not shell_ready:
        print("[ERROR] AegisShell did not become ready.")
        proc.kill()
        sys.exit(1)

    temp_dir = tempfile.gettempdir()
    ppm_boot = os.path.join(temp_dir, "qemu_screen_boot.ppm")
    ppm_vlc = os.path.join(temp_dir, "qemu_screen_vlc.ppm")

    time.sleep(1.0)
    # Refresh compositor frame
    proc.stdin.write("\ncompositor render\n")
    proc.stdin.flush()
    time.sleep(0.5)

    # Capture 1: Boot Desktop
    print("Capturing 1: Native Desktop on Boot...")
    f.write(json.dumps({"execute": "screendump", "arguments": {"filename": ppm_boot}}) + "\r\n")
    f.flush()
    resp1 = f.readline()
    print("QMP Response 1:", resp1.strip())

    # Send launch vlc command
    print("Launching native VLC media player ('launch vlc')...")
    proc.stdin.write("launch vlc\n")
    proc.stdin.flush()
    time.sleep(3.0)

    # Force compositor render pass
    proc.stdin.write("compositor render\n")
    proc.stdin.flush()
    time.sleep(0.5)

    # Capture 2: Live Desktop with VLC Media Player
    print("Capturing 2: Live Desktop with VLC Media Player...")
    f.write(json.dumps({"execute": "screendump", "arguments": {"filename": ppm_vlc}}) + "\r\n")
    f.flush()
    resp2 = f.readline()
    print("QMP Response 2:", resp2.strip())

    # Shutdown QEMU cleanly
    f.write(json.dumps({"execute": "quit"}) + "\r\n")
    f.flush()
    try:
        proc.wait(timeout=4)
    except Exception:
        proc.kill()

    out_dir = Path("build/screenshots")
    out_dir.mkdir(parents=True, exist_ok=True)
    png_boot = out_dir / "purple_desktop.png"
    png_vlc = out_dir / "purple_vlc_player.png"

    if os.path.exists(ppm_boot):
        ppm_to_png(ppm_boot, str(png_boot))
        try: os.remove(ppm_boot)
        except: pass
        print(f"[OK] Saved desktop screenshot: {png_boot}")

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
