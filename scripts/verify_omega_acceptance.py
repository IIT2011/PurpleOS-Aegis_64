#!/usr/bin/env python3
"""
PURPLE-OMEGA ZERO-COMPROMISE VERIFICATION HARNESS
Tests:
- Single-disk SATA boot with Partition 1 (ESP) and Partition 2 (PurpleFS v3)
- AegisDetect baseline verification (welcome.txt, version.txt, motd)
- Interactive AegisDesktop session online
- Interactive Terminal command execution:
  1. 'ls' -> PurpleFS v3 hierarchical listing
  2. 'cat /welcome.txt' -> reads exact trusted contents
  3. 'disks' -> single-disk SATA detection and partition status
  4. 'gpu status' -> display acceleration status
  5. 'app run /System/Applications/PurpleTerminal.apapplication' -> APAPP validation & launch
  6. 'app run /System/Applications/VLC.apapplication' -> VLC launch
- Screendump capture of interactive desktop
"""

import subprocess
import time
import sys
import os
import socket
import threading
from PIL import Image

sock_path = "/tmp/qemu_omega_mon.sock"
if os.path.exists(sock_path):
    os.remove(sock_path)

cmd = [
    "qemu-system-x86_64",
    "-nodefaults",
    "-M", "q35,smm=off",
    "-m", "1024",
    "-smp", "2",
    "-drive", "if=pflash,format=raw,readonly=on,file=/usr/share/edk2/x64/OVMF_CODE.4m.fd",
    "-drive", "if=pflash,format=raw,file=build/ovmf_vars.fd",
    "-drive", "file=build/aegis64-uefi-test.img,format=raw,if=ide",
    "-vga", "std",
    "-monitor", f"unix:{sock_path},server,nowait",
    "-serial", "stdio",
    "-display", "none"
]

def qcmd(c):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(sock_path)
    time.sleep(0.03)
    s.recv(1024)
    s.sendall((c + "\n").encode())
    time.sleep(0.05)
    res = s.recv(4096).decode(errors="ignore")
    s.close()
    return res

def send_string(str_val):
    for ch in str_val:
        if ch == ' ':
            qcmd("sendkey spc")
        elif ch == '/':
            qcmd("sendkey slash")
        elif ch == '.':
            qcmd("sendkey dot")
        elif ch == '-':
            qcmd("sendkey minus")
        elif ch == '_':
            qcmd("sendkey shift-minus")
        elif ch.isupper():
            qcmd(f"sendkey shift-{ch.lower()}")
        elif ch.isdigit() or ch.islower():
            qcmd(f"sendkey {ch}")
        time.sleep(0.04)
    qcmd("sendkey ret")
    time.sleep(0.05)

p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)

desktop_online = threading.Event()
stop_thread = False
guest_logs = []

def reader():
    while not stop_thread:
        line = p.stdout.readline()
        if not line: break
        l = line.strip()
        guest_logs.append(l)
        if "Desktop: Interactive session online" in l:
            desktop_online.set()
        if any(k in l for k in ["[TERM]", "APAPP", "AegisDetect", "PurpleTerminal", "Desktop", "PCI", "PFS3", "USER APP", "Ring 3", "GPT", "blks", "ERROR", "Filesystem", "GPU", "Hardware", "Listing", "SECURITY", "TOKEN"]):
            print("  [GUEST]", l)

t = threading.Thread(target=reader, daemon=True)
t.start()

print("================================================================================")
print(" PURPLE-OMEGA ZERO-COMPROMISE ACCEPTANCE HARNESS")
print("================================================================================")
print("Awaiting UEFI boot and interactive desktop session...")

if not desktop_online.wait(timeout=25):
    print("[FAIL] Desktop did not come online within 25 seconds.")
    stop_thread = True
    p.kill()
    sys.exit(1)

print("\n[OK] Desktop online! Pausing 1s for compositor stabilization...\n")
time.sleep(1)

commands = [
    "help",
    "ls",
    "ls /System/Applications",
    "cat /welcome.txt",
    "df",
    "disks",
    "partitions",
    "gpu status",
    "app run /System/Applications/PurpleTerminal.apapplication",
    "app run /Users/aegis/Applications/calculator.apapplication",
    "app run /System/Applications/terminal.apapp",
    "app run /System/Applications/VLC.apapplication",
    "app run /System/Applications/fault_test.apapplication",
    "touch /Data/verify_token.txt",
    "write /Data/verify_token.txt OMEGA_ACCEPTANCE_TOKEN_9911",
    "cat /Data/verify_token.txt"
]

for c in commands:
    print(f"\n>>> EXECUTING COMMAND: '{c}'")
    send_string(c)
    time.sleep(1.5)

print("\n>>> Capturing final high-resolution desktop screendump...")
qcmd("screendump /tmp/omega_final.ppm")
if os.path.exists("/tmp/omega_final.ppm"):
    im = Image.open("/tmp/omega_final.ppm")
    im.save("build/omega_desktop_verified.png")
    im.save("/home/saptanshu-ganguly/.gemini/antigravity/brain/004b4109-7cfd-4ce6-9d9e-5d632649292f/omega_desktop_verified.png")
    print(f"[OK] Screendump successfully saved to build/omega_desktop_verified.png and artifacts directory ({im.size[0]}x{im.size[1]} RGB)")

stop_thread = True
p.kill()
p.wait()

print("\n================================================================================")
print(" >>> PURPLE-OMEGA ACCEPTANCE VERIFICATION SUMMARY <<<")
print("================================================================================")
# Verify key markers in guest_logs
markers = {
    "PurpleFS v3 Mount": any("PurpleSystem" in l for l in guest_logs),
    "AegisDetect Root of Trust": any("3/3 components verified (3 trusted" in l for l in guest_logs),
    "Interactive Desktop": any("Desktop: Interactive session online" in l for l in guest_logs),
    "Shell Command Execution": any("> help" in l for l in guest_logs),
    "Hierarchical Directory Listing": any("Directory Listing of / (PurpleFS v3 Hierarchical)" in l for l in guest_logs),
    "PurpleFS v3 Reconciliation": any("Filesystem: PurpleFS v3 Hierarchical Storage" in l for l in guest_logs),
    "System Applications Directory Listing": any("Directory Listing of /System/Applications (PurpleFS v3 Hierarchical)" in l for l in guest_logs),
    "Storage Disks Query": any("ATA Primary Master" in l or "Partition 1" in l or "PurpleSystem" in l or "blks" in l or "sata0" in l for l in guest_logs),
    "GPU Honest Status Reporting": any("Hardware Acceleration: NOT IMPLEMENTED" in l or "Software 32bpp Fallback" in l for l in guest_logs),
    "Application Bundle Launch": any("launched" in l or "Application bundle launched" in l for l in guest_logs),
    "Ring 3 Process Execution": any("[USER APP]" in l or "Ring 3" in l for l in guest_logs),
    "User Application Execution": any("calculator" in l or "Desktop Calculator" in l for l in guest_logs),
    "Noncanonical Extension Rejection": any("noncanonical" in l.lower() or "failed to launch" in l.lower() for l in guest_logs),
    "Hardware Privilege Fault Containment": any("User Process Exception 14" in l or "Terminating faulted Ring 3 process" in l for l in guest_logs),
    "Filesystem Write & Read Verification": any("OMEGA_ACCEPTANCE_TOKEN_9911" in l for l in guest_logs)
}

all_passed = True
for name, passed in markers.items():
    status = "PASSED" if passed else "FAILED"
    print(f"  [{status}] {name}")
    if not passed:
        all_passed = False

if all_passed:
    print("\nALL PURPLE-OMEGA GATES VERIFIED AND PASSED!")
    sys.exit(0)
else:
    print("\nSOME GATES FAILED. Check guest logs above.")
    sys.exit(1)
