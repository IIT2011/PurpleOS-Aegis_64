#!/usr/bin/env python3
import subprocess
import time
import os
import json
import socket
from PIL import Image

def main():
    print("=== Capturing Purple OS Live Desktop Screenshot from QEMU ===")
    os.makedirs("build", exist_ok=True)
    qmp_sock_path = "build/qmp.sock"
    if os.path.exists(qmp_sock_path):
        os.remove(qmp_sock_path)

    disk_path = "build/aegis64-uefi-test.img" if os.path.exists("build/aegis64-uefi-test.img") else "build/aegis64-uefi.img"
    cmd = [
        "qemu-system-x86_64",
        "-kernel", "build/kernel.elf",
        "-serial", "stdio",
        "-vga", "std",
        "-m", "256M",
        "-vnc", ":1",
        "-qmp", f"unix:{qmp_sock_path},server,nowait",
        "-no-reboot",
        "-drive", f"file={disk_path},format=raw,index=1,media=disk"
    ]

    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=0
    )

    # Wait for QEMU to boot and establish QMP socket
    time.sleep(1.5)

    # Switch video mode and launch desktop
    try:
        proc.stdin.write("video mode 1024 768\n")
        proc.stdin.flush()
        time.sleep(0.3)
        proc.stdin.write("desktop\n")
        proc.stdin.flush()
        time.sleep(0.5)
    except Exception as e:
        print("Stdin write error:", e)

    # Connect to QMP and issue screendump
    ppm_path = "build/desktop_screenshot.ppm"
    png_path = "build/desktop_screenshot.png"

    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    for _ in range(10):
        try:
            s.connect(qmp_sock_path)
            break
        except Exception:
            time.sleep(0.3)

    # Negotiate QMP capabilities
    greeting = s.recv(1024)
    s.sendall(b'{"execute": "qmp_capabilities"}\n')
    resp = s.recv(1024)

    # Take screendump
    dump_cmd = json.dumps({"execute": "screendump", "arguments": {"filename": ppm_path}}) + "\n"
    s.sendall(dump_cmd.encode("utf-8"))
    resp2 = s.recv(1024)
    s.close()

    time.sleep(0.5)
    proc.terminate()
    try:
        proc.wait(timeout=2)
    except Exception:
        proc.kill()

    if os.path.exists(ppm_path):
        im = Image.open(ppm_path)
        im.save(png_path, "PNG")
        print(f"[OK] Successfully captured desktop screenshot: {png_path} ({im.size[0]}x{im.size[1]})")

        # Also copy to artifacts directory if available
        art_dir = "/home/saptanshu-ganguly/.gemini/antigravity/brain/004b4109-7cfd-4ce6-9d9e-5d632649292f"
        if os.path.exists(art_dir):
            art_png = os.path.join(art_dir, "purple_os_desktop_verified.png")
            im.save(art_png, "PNG")
            print(f"[OK] Saved verified desktop artifact: {art_png}")
        return 0
    else:
        print(f"[FAIL] Screenshot PPM file was not generated at {ppm_path}")
        return 1

if __name__ == "__main__":
    import sys
    sys.exit(main())
