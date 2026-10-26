#!/usr/bin/env python3
"""
PURPLE-OMEGA RING 3 HARDWARE PRIVILEGE FAULT CONTAINMENT TEST
Validates:
  1. Boot UEFI QEMU into AegisDesktop
  2. Launch /System/Applications/fault_test.apapplication in Ring 3 (CPL 3)
  3. The unprivileged process attempts to read kernel memory (0xFFFFFFFF80000000)
  4. CPU triggers Page Fault #14 (Protection Violation, User Mode, Read)
  5. Aegis64 IDT intercepts the fault, terminates only the offending Ring 3 task
  6. The kernel, compositor, and terminal remain fully alive and responsive
  7. Subsequent shell commands ('help', 'ls') execute normally
"""

import subprocess
import time
import sys
import os
import socket
import threading

SOCK_PATH = "/tmp/qemu_fault_mon.sock"
OVMF_CODE = "/usr/share/edk2/x64/OVMF_CODE.4m.fd"
OVMF_VARS = "build/ovmf_vars.fd"
TEST_IMG = "build/aegis64-uefi-test.img"

if os.path.exists(SOCK_PATH):
    os.remove(SOCK_PATH)

cmd = [
    "qemu-system-x86_64",
    "-nodefaults",
    "-M", "q35,smm=off",
    "-m", "1024",
    "-smp", "2",
    "-drive", f"if=pflash,format=raw,readonly=on,file={OVMF_CODE}",
    "-drive", f"if=pflash,format=raw,file={OVMF_VARS}",
    "-drive", f"file={TEST_IMG},format=raw,if=ide",
    "-vga", "std",
    "-monitor", f"unix:{SOCK_PATH},server,nowait",
    "-serial", "stdio",
    "-display", "none"
]

def qcmd(c):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(SOCK_PATH)
    time.sleep(0.04)
    s.recv(1024)
    s.sendall((c + "\n").encode())
    time.sleep(0.06)
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
        time.sleep(0.035)
    qcmd("sendkey ret")
    time.sleep(0.05)

print("================================================================================")
print(" PURPLE-OMEGA RING 3 HARDWARE PRIVILEGE FAULT CONTAINMENT TEST")
print("================================================================================")

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
        if any(k in l for k in ["[TERM]", "SECURITY", "APAPP", "fault_test", "Scheduler", "PANIC"]):
            print("  [GUEST]", l)

t = threading.Thread(target=reader, daemon=True)
t.start()

if not desktop_online.wait(timeout=25):
    print("[FAIL] Desktop failed to initialize within 25 seconds.")
    stop_thread = True
    p.kill()
    sys.exit(1)

print("\n[OK] Desktop online. Launching fault_test.apapplication...")
time.sleep(1.0)

send_string("app run /System/Applications/fault_test.apapplication")
time.sleep(2.0)

print("\n>>> Testing post-fault kernel responsiveness...")
send_string("help")
time.sleep(1.5)
send_string("ls")
time.sleep(1.5)

stop_thread = True
p.kill()
p.wait()

print("\n================================================================================")
print(" >>> FAULT CONTAINMENT VERIFICATION SUMMARY <<<")
print("================================================================================")

fault_launched = any("fault_test.apapplication" in l for l in guest_logs)
security_intercept = any("User Process Exception" in l or "Terminating faulted Ring 3 process" in l for l in guest_logs)
kernel_survived = any("> help" in l for l in guest_logs) and any("> ls" in l for l in guest_logs)
no_panic = not any("KERNEL PANIC" in l for l in guest_logs)

print(f"  [{'PASSED' if fault_launched else 'FAILED'}] Ring 3 Process Spawn")
print(f"  [{'PASSED' if security_intercept else 'FAILED'}] Hardware Page Fault Interception & Containment")
print(f"  [{'PASSED' if kernel_survived else 'FAILED'}] Post-Fault Kernel & Desktop Responsiveness")
print(f"  [{'PASSED' if no_panic else 'FAILED'}] Zero Kernel Panic (Zero System Crash)")

if fault_launched and security_intercept and kernel_survived and no_panic:
    print("\nALL PRIVILEGE FAULT CONTAINMENT GATES VERIFIED AND PASSED!")
    sys.exit(0)
else:
    print("\nFAULT CONTAINMENT VERIFICATION FAILED.")
    sys.exit(1)
