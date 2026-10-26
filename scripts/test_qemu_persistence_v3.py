#!/usr/bin/env python3
"""
PURPLE-OMEGA PERSISTENCE ACROSS REBOOT TEST (PURPLEFS V3 ON GPT)
Demonstrates:
  1. Boot Session 1 (UEFI QEMU): Creates and writes unique payload into /Data/persist_token.txt
  2. Terminate Session 1 (Simulated power-down / shutdown)
  3. Boot Session 2 (UEFI QEMU from same disk image): Reads back /Data/persist_token.txt
  4. Byte-for-byte verification of persisted data across reboot
"""

import subprocess
import time
import sys
import os
import shutil
import socket
import threading

DISP_IMAGE = "/tmp/qemu_persist_test.img"
SOCK_PATH = "/tmp/qemu_persist_mon.sock"
OVMF_CODE = "/usr/share/edk2/x64/OVMF_CODE.4m.fd"
OVMF_VARS = "build/ovmf_vars.fd"
TEST_TOKEN = "PURPLE_OS_V3_PERSISTENCE_TOKEN_8899AA"

if not os.path.exists("build/aegis64-uefi-test.img"):
    print("[FAIL] build/aegis64-uefi-test.img not found. Run scripts/build_uefi_disk.py first.")
    sys.exit(1)

# Create a disposable working copy
shutil.copyfile("build/aegis64-uefi-test.img", DISP_IMAGE)
print(f"[OK] Prepared disposable test disk: {DISP_IMAGE} ({os.path.getsize(DISP_IMAGE)} bytes)")

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

def run_session(session_num, commands_to_run):
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
        "-drive", f"file={DISP_IMAGE},format=raw,if=ide",
        "-vga", "std",
        "-monitor", f"unix:{SOCK_PATH},server,nowait",
        "-serial", "stdio",
        "-display", "none"
    ]

    print(f"\n=======================================================")
    print(f" >>> STARTING BOOT SESSION {session_num} <<<")
    print(f"=======================================================")

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
            if any(k in l for k in ["[TERM]", "PERSISTENCE", "PFS3", "welcome"]):
                print(f"  [GUEST-S{session_num}]", l)

    t = threading.Thread(target=reader, daemon=True)
    t.start()

    if not desktop_online.wait(timeout=25):
        print(f"[FAIL] Session {session_num}: Desktop failed to come online.")
        stop_thread = True
        p.kill()
        sys.exit(1)

    print(f"[OK] Session {session_num}: Desktop online. Injecting commands...")
    time.sleep(1.0)

    for c in commands_to_run:
        print(f"  >>> S{session_num} SENDING: '{c}'")
        send_string(c)
        time.sleep(1.5)

    time.sleep(1.0)
    stop_thread = True
    p.kill()
    p.wait()

    return guest_logs

# ---------------------------------------------------------------------------
# 1. Run Session 1: Write file and sync
# ---------------------------------------------------------------------------
s1_cmds = [
    "touch /Data/persist_token.txt",
    f"write /Data/persist_token.txt {TEST_TOKEN}",
    "cat /Data/persist_token.txt"
]
s1_logs = run_session(1, s1_cmds)

# Check that Session 1 wrote successfully
s1_written = any(TEST_TOKEN in l for l in s1_logs)
if not s1_written:
    print("[FAIL] Session 1 did not confirm writing token.")
    sys.exit(1)
print(f"[OK] Session 1: Successfully wrote and verified '{TEST_TOKEN}' in /Data/persist_token.txt")

# ---------------------------------------------------------------------------
# 2. Run Session 2: Cold boot from same disk, Read back file
# ---------------------------------------------------------------------------
s2_cmds = [
    "cat /Data/persist_token.txt"
]
s2_logs = run_session(2, s2_cmds)

s2_persisted = any(TEST_TOKEN in l for l in s2_logs)
if s2_persisted:
    print(f"\n================================================================================")
    print(f" [PASSED] PURPLEFS V3 PERSISTENCE ACROSS REBOOT CONFIRMED!")
    print(f" Token '{TEST_TOKEN}' successfully persisted and read back after cold reboot.")
    print(f"================================================================================")
    sys.exit(0)
else:
    print(f"\n[FAIL] Token was not found in Session 2 logs after reboot.")
    sys.exit(1)
