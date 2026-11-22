#!/usr/bin/env python3
"""
Test script to cleanly verify:
1. Does global bar / dock disappear after closing VLC?
2. Does mouse freeze when opening and closing Terminal?
"""

import subprocess
import socket
import json
import time
import os
import sys
import tempfile
import threading

def ppm_to_png(ppm_path, png_path):
    import struct, zlib
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
        scanlines.append(0)
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
    print(f"[OK] Saved {png_path} ({width}x{height})")

def main():
    os.chdir(os.path.join(os.path.dirname(__file__), ".."))
    disk_path = "build/purple_gpt.img"
    wav_path = "build/purple_audio.wav"

    print("Formatting GPT disk with applications...")
    subprocess.check_call([
        sys.executable,
        "build/mkpfs2_disk",
        disk_path,
        "--signing-key", "keys/creator_private_key.pem",
        "--applications-dir", "build/applications"
    ])

    qmp_port = 4501
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

    print("Connecting to QMP socket...")
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
    f.readline()
    f.write(json.dumps({"execute": "qmp_capabilities"}) + "\r\n")
    f.flush()
    f.readline()

    def wait_for_prompt(timeout=15):
        start = time.time()
        initial_len = len("".join(captured_chunks))
        while time.time() - start < timeout:
            text = "".join(captured_chunks)
            if text.endswith("aegis64@purple:~$ "):
                return True
            time.sleep(0.1)
        return False

    print("Waiting for initial shell prompt...")
    wait_for_prompt(25)

    temp_dir = tempfile.gettempdir()
    os.makedirs("build/screenshots/tests", exist_ok=True)

    def screendump(name):
        ppm = os.path.join(temp_dir, f"{name}.ppm")
        png = f"build/screenshots/tests/{name}.png"
        f.write(json.dumps({"execute": "screendump", "arguments": {"filename": ppm}}) + "\r\n")
        f.flush()
        f.readline()
        time.sleep(0.3)
        if os.path.exists(ppm):
            ppm_to_png(ppm, png)
            os.remove(ppm)

    def send_cmd(cmd_str):
        print(f"\n>>> SENDING: {cmd_str}")
        proc.stdin.write(cmd_str + "\n")
        proc.stdin.flush()
        time.sleep(0.5)
        wait_for_prompt(5)

    # 1. Capture initial desktop
    screendump("01_clean_desktop")

    # 2. Launch VLC
    send_cmd("launch vlc")
    time.sleep(2.0)
    send_cmd("window list")
    send_cmd("compositor render")
    screendump("02_vlc_open")

    # 3. Close VLC window
    send_cmd("window close 1")
    time.sleep(1.0)
    send_cmd("window list")
    send_cmd("compositor render")
    screendump("03_vlc_closed")

    # 4. Launch Terminal
    send_cmd("launch terminal")
    time.sleep(2.0)
    send_cmd("window list")
    send_cmd("compositor render")
    screendump("04_terminal_open")

    # 5. Check mouse position, then move mouse to CloseButton (888, 112)
    send_cmd("mouse")
    # Initial mouse pos is (512, 384)
    # dx = 888 - 512 = +376, dy = 112 - 384 = -272
    print("Moving mouse to Terminal Close Button at (888, 112)...")
    f.write(json.dumps({"execute": "input-send-event", "arguments": {"events": [
        {"type": "rel", "data": {"axis": "x", "value": 376}},
        {"type": "rel", "data": {"axis": "y", "value": -272}}
    ]}}) + "\r\n")
    f.flush()
    f.readline()
    time.sleep(0.5)
    send_cmd("mouse")

    # 6. Click CloseButton to close Terminal
    print("Clicking Close Button to close Terminal...")
    f.write(json.dumps({"execute": "input-send-event", "arguments": {"events": [
        {"type": "btn", "data": {"down": True, "button": "left"}}
    ]}}) + "\r\n")
    f.flush()
    f.readline()
    time.sleep(0.2)
    f.write(json.dumps({"execute": "input-send-event", "arguments": {"events": [
        {"type": "btn", "data": {"down": False, "button": "left"}}
    ]}}) + "\r\n")
    f.flush()
    f.readline()
    time.sleep(1.0)

    send_cmd("window list")
    send_cmd("tasks")
    send_cmd("compositor render")
    screendump("05_terminal_closed_by_close_button")

    # 7. Move mouse across desktop to verify responsiveness (no freeze!)
    print("Moving mouse after closing Terminal...")
    for step in range(5):
        f.write(json.dumps({"execute": "input-send-event", "arguments": {"events": [
            {"type": "rel", "data": {"axis": "x", "value": -50}},
            {"type": "rel", "data": {"axis": "y", "value": 40}}
        ]}}) + "\r\n")
        f.flush()
        f.readline()
        time.sleep(0.1)

    send_cmd("mouse")
    send_cmd("echo MOUSE_IS_RESPONSIVE")
    screendump("06_mouse_responsive_after_close")

    # 8. Re-open Terminal and close via AegisShell window command
    send_cmd("launch terminal")
    time.sleep(2.0)
    send_cmd("window list")
    send_cmd("window close 3")
    time.sleep(1.0)
    send_cmd("window list")
    send_cmd("tasks")
    send_cmd("mouse")
    screendump("07_terminal_reopened_and_closed_via_shell")

    # Clean shutdown
    f.write(json.dumps({"execute": "quit"}) + "\r\n")
    f.flush()
    try:
        proc.wait(timeout=3)
    except Exception:
        proc.kill()

    print("\nAll tests finished!")

if __name__ == "__main__":
    main()
