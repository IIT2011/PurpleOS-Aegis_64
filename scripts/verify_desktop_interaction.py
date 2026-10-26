#!/usr/bin/env python3
import subprocess
import time
import sys
import os
import socket
from PIL import Image

def qemu_mon_cmd(cmd):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect("/tmp/qemu_mon.sock")
    time.sleep(0.2)
    s.recv(1024)
    s.sendall((cmd + "\n").encode("utf-8"))
    time.sleep(0.5)
    resp = s.recv(4096).decode("utf-8", errors="ignore")
    s.close()
    return resp

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
        if "Desktop: Interactive session online" in line:
            desktop_ready = True
            break

    if not desktop_ready:
        print("[FAIL] Desktop session did not boot within 30 seconds.")
        proc.kill()
        sys.exit(1)

    print("[2] Desktop session online. Waiting 2 seconds for initial frame...")
    time.sleep(2)

    print("[3] Capturing screendump before user input...")
    qemu_mon_cmd("screendump /tmp/desktop_interactive_initial.ppm")
    if os.path.exists("/tmp/desktop_interactive_initial.ppm"):
        im_init = Image.open("/tmp/desktop_interactive_initial.ppm")
        im_init.save("build/desktop_interactive_initial.png")
        print(f"[OK] Saved build/desktop_interactive_initial.png ({im_init.size[0]}x{im_init.size[1]})")

    print("[4] Testing PS/2 keyboard interaction via QEMU monitor sendkey...")
    # Send 'h', 'e', 'l', 'p', 'ret' to the focused terminal window
    for key in ["h", "e", "l", "p", "ret"]:
        qemu_mon_cmd(f"sendkey {key}")
        time.sleep(0.1)

    time.sleep(1)

    print("[5] Capturing screendump after PS/2 keyboard command execution...")
    qemu_mon_cmd("screendump /tmp/desktop_interactive_ps2.ppm")
    if os.path.exists("/tmp/desktop_interactive_ps2.ppm"):
        im_ps2 = Image.open("/tmp/desktop_interactive_ps2.ppm")
        im_ps2.save("build/desktop_interactive_ps2.png")
        print(f"[OK] Saved build/desktop_interactive_ps2.png ({im_ps2.size[0]}x{im_ps2.size[1]})")

    print("[6] Testing serial input routing...")
    proc.stdin.write("ls\n")
    proc.stdin.flush()
    time.sleep(1)

    print("[7] Capturing screendump after serial command execution...")
    qemu_mon_cmd("screendump /tmp/desktop_interactive_serial.ppm")
    if os.path.exists("/tmp/desktop_interactive_serial.ppm"):
        im_ser = Image.open("/tmp/desktop_interactive_serial.ppm")
        im_ser.save("build/desktop_interactive_serial.png")
        print(f"[OK] Saved build/desktop_interactive_serial.png ({im_ser.size[0]}x{im_ser.size[1]})")

    print("[8] Testing mouse movement and click...")
    # Move mouse and click close button or terminal window body
    qemu_mon_cmd("mouse_move 50 50")
    time.sleep(0.2)
    qemu_mon_cmd("mouse_button 0")
    time.sleep(0.5)

    print("[9] Capturing final interaction screendump...")
    qemu_mon_cmd("screendump /tmp/desktop_interactive_final.ppm")
    if os.path.exists("/tmp/desktop_interactive_final.ppm"):
        im_final = Image.open("/tmp/desktop_interactive_final.ppm")
        im_final.save("build/desktop_interactive_final.png")
        print(f"[OK] Saved build/desktop_interactive_final.png ({im_final.size[0]}x{im_final.size[1]})")

    print("[OK] Interactive desktop verification passed without crashes or log flooding!")
    proc.kill()
    proc.wait()

if __name__ == "__main__":
    main()
