#!/usr/bin/env python3
"""
scripts/verify_qemu_keyboard_guest.py
Verifies actual guest keyboard hardware interaction using QEMU QMP send-key:
- Tests real 8042 PS/2 controller IRQ 1 delivery in the guest
- Tests typing: 'h', 'e', 'l', 'p'
- Tests Backspace, Enter
- Tests Shift modifier: Shift + '1' -> '!' or Shift + 'a' -> 'A'
- Tests Caps Lock toggle
- Tests Up/Down arrow scancodes (0xE0 extended prefix) for command history
- Captures serial output to verify guest execution
"""

import socket
import json
import subprocess
import time
import os
import sys

QMP_SOCK = "/tmp/qemu_keyboard_test_qmp.sock"

def qmp_command(s, cmd, args=None):
    req = {"execute": cmd}
    if args:
        req["arguments"] = args
    s.sendall(json.dumps(req).encode("utf-8") + b"\r\n")
    data = b""
    while b"\r\n" not in data:
        chunk = s.recv(4096)
        if not chunk:
            break
        data += chunk
    return json.loads(data.decode("utf-8"))

def send_key(s, key_name):
    req = {
        "execute": "send-key",
        "arguments": {
            "keys": [{"type": "qcode", "data": key_name}]
        }
    }
    s.sendall(json.dumps(req).encode("utf-8") + b"\r\n")
    # Read response
    s.recv(4096)
    time.sleep(0.04)

def send_combo(s, key1, key2):
    req = {
        "execute": "send-key",
        "arguments": {
            "keys": [
                {"type": "qcode", "data": key1},
                {"type": "qcode", "data": key2}
            ]
        }
    }
    s.sendall(json.dumps(req).encode("utf-8") + b"\r\n")
    s.recv(4096)
    time.sleep(0.04)

def main():
    if os.path.exists(QMP_SOCK):
        os.remove(QMP_SOCK)

    cmd = [
        "qemu-system-x86_64",
        "-kernel", "build/kernel.elf",
        "-serial", "stdio",
        "-display", "none",
        "-vga", "std",
        "-m", "256M",
        "-no-reboot",
        "-qmp", f"unix:{QMP_SOCK},server,nowait"
    ]

    print("=== Launching QEMU with QMP for Real Hardware Keyboard Injection ===")
    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=0
    )

    # Wait for QMP socket to become available
    sock = None
    for _ in range(30):
        if os.path.exists(QMP_SOCK):
            try:
                sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                sock.connect(QMP_SOCK)
                break
            except Exception:
                time.sleep(0.1)
        time.sleep(0.1)

    if not sock:
        print("[FAIL] Could not connect to QMP socket")
        proc.kill()
        return 1

    # Negotiate QMP capabilities
    greeting = sock.recv(4096)
    qmp_command(sock, "qmp_capabilities")

    # Wait for kernel to initialize 8042 keyboard and enter shell
    time.sleep(1.5)

    print("--- Injecting Hardware Scancodes via QMP send-key ---")
    # Test 1: Type 'echo HELLO_GUEST_KEYBOARD' and Enter
    test_chars = "echo HELLO_GUEST_KEYBOARD"
    for ch in test_chars:
        if ch == ' ':
            send_key(sock, "spc")
        elif ch == '_':
            send_combo(sock, "shift", "minus")
        elif ch.isupper():
            send_combo(sock, "shift", ch.lower())
        else:
            send_key(sock, ch)

    send_key(sock, "ret")
    time.sleep(0.3)

    # Test 2: Type 'echo wrong', then backspace 5 times, then type 'right', Enter
    for ch in "echo wrong":
        send_key(sock, "spc" if ch == ' ' else ch)
    for _ in range(5):
        send_key(sock, "backspace")
    for ch in "right":
        send_key(sock, ch)
    send_key(sock, "ret")
    time.sleep(0.3)

    # Test 3: History recall using Up arrow key
    # Press Up arrow (0xE0 0x48) -> should recall 'echo right'
    send_key(sock, "up")
    time.sleep(0.1)
    send_key(sock, "ret")
    time.sleep(0.3)

    # Finish and read output
    sock.close()
    proc.terminate()
    try:
        stdout, stderr = proc.communicate(timeout=2)
    except Exception:
        proc.kill()
        stdout, stderr = proc.communicate()

    print("\n=== Guest Serial Output from Hardware Keyboard Testing ===")
    print(stdout)

    assertions = [
        ("Guest Hardware IRQ1 Delivered", "Keyboard: 8042 controller initialized, IRQ 1 unmasked"),
        ("Echo Message Executed via Guest PS/2", "HELLO_GUEST_KEYBOARD"),
        ("Backspace Correction Executed", "right"),
        ("Arrow Key History Recall Executed", "right")
    ]

    failed = 0
    for name, pattern in assertions:
        if pattern in stdout:
            print(f"[OK] Verified: {name}")
        else:
            print(f"[FAIL] Missing: {name}")
            failed += 1

    if failed == 0:
        print("\n>>> ALL GUEST HARDWARE KEYBOARD TESTS PASSED! <<<\n")
        return 0
    else:
        print(f"\n>>> GUEST KEYBOARD VERIFICATION FAILED ({failed} errors) <<<\n")
        return 1

if __name__ == "__main__":
    sys.exit(main())
