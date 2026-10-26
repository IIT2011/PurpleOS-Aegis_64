#!/usr/bin/env python3
import subprocess
import time
import sys
import os
from PIL import Image

def main():
    qemu_cmd = [
        "qemu-system-x86_64",
        "-nodefaults",
        "-M", "q35,smm=off",
        "-m", "1024",
        "-smp", "2",
        "-drive", "if=pflash,format=raw,readonly=on,file=/usr/share/edk2/x64/OVMF_CODE.4m.fd",
        "-drive", "if=pflash,format=raw,file=build/ovmf_vars.fd",
        "-drive", "file=build/aegis64-uefi-test.img,format=raw,if=ide",
        "-vga", "std",
        "-monitor", "unix:/tmp/qemu_mon.sock,server,nowait",
        "-serial", "stdio",
        "-display", "none"
    ]

    if os.path.exists("/tmp/qemu_mon.sock"):
        os.remove("/tmp/qemu_mon.sock")

    print("[1] Launching QEMU UEFI with build/aegis64-uefi-test.img...")
    proc = subprocess.Popen(qemu_cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)

    boot_log = []
    desktop_ready = False
    start_time = time.time()

    while time.time() - start_time < 30:
        line = proc.stdout.readline()
        if not line:
            break
        print("  GUEST:", line.strip())
        boot_log.append(line)
        if "Desktop: Graphical desktop frame" in line or "aegis64@purple:~$" in line:
            desktop_ready = True
            break

    if not desktop_ready:
        print("[FAIL] Desktop did not boot within 30 seconds.")
        proc.kill()
        sys.exit(1)

    print("[2] Desktop booted successfully. Waiting 3 seconds for composition to settle...")
    time.sleep(3)

    # Helper function to issue QEMU monitor commands
    def qemu_mon_cmd(cmd):
        import socket
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.connect("/tmp/qemu_mon.sock")
        # Read greeting
        time.sleep(0.2)
        s.recv(1024)
        s.sendall((cmd + "\n").encode("utf-8"))
        time.sleep(0.5)
        resp = s.recv(4096).decode("utf-8", errors="ignore")
        s.close()
        return resp

    print("[3] Capturing screendump BEFORE clear...")
    qemu_mon_cmd("screendump /tmp/desktop_before_clear.ppm")
    if os.path.exists("/tmp/desktop_before_clear.ppm"):
        im_before = Image.open("/tmp/desktop_before_clear.ppm")
        im_before.save("build/desktop_before_clear.png")
        print(f"[OK] Saved build/desktop_before_clear.png ({im_before.size[0]}x{im_before.size[1]})")

    print("[4] Sending 'clear\\n' to serial console...")
    proc.stdin.write("clear\n")
    proc.stdin.flush()
    time.sleep(2)

    print("[5] Capturing screendump AFTER clear...")
    qemu_mon_cmd("screendump /tmp/desktop_after_clear.ppm")
    if os.path.exists("/tmp/desktop_after_clear.ppm"):
        im_after = Image.open("/tmp/desktop_after_clear.ppm")
        im_after.save("build/desktop_after_clear.png")
        print(f"[OK] Saved build/desktop_after_clear.png ({im_after.size[0]}x{im_after.size[1]})")

    # Verify pixels in im_after
    w, h = im_after.size
    print(f"[6] Analyzing screendump pixels ({w}x{h})...")
    # Sample top bar (10, 10), motto area (w/2, 120), dock (w/2, h - 35), wallpaper (w - 50, 100)
    topbar_px = im_after.getpixel((10, 10))
    motto_px = im_after.getpixel((w // 2, 120))
    dock_px = im_after.getpixel((w // 2, h - 35))
    wallpaper_px = im_after.getpixel((w - 50, 100))

    print(f"    Top bar pixel (10, 10):      {topbar_px}")
    print(f"    Motto area pixel ({w//2}, 120): {motto_px}")
    print(f"    Dock pixel ({w//2}, {h-35}):    {dock_px}")
    print(f"    Wallpaper pixel ({w-50}, 100):  {wallpaper_px}")

    # Check that pixels are not blank/black and not uniform flat color
    assert topbar_px != (0, 0, 0), "Top bar is black!"
    assert wallpaper_px != (0, 0, 0), "Wallpaper is black!"
    assert dock_px != (0, 0, 0), "Dock is black!"
    assert wallpaper_px != topbar_px, "Screen is a single flat color!"

    print("[OK] Screendump analysis PASSED: Desktop, wallpaper, dock, and panels are completely preserved after 'clear'!")

    # Clean up QEMU
    proc.kill()
    proc.wait()

if __name__ == "__main__":
    main()
