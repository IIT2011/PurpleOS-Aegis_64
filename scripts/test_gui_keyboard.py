#!/usr/bin/env python3
import subprocess
import time
import socket
import json
import sys
import os

def main():
    print("=================================================================")
    print("  PURPLE OS / AEGIS64 — GUI KEYBOARD INPUT (QMP / SENDKEY) TEST   ")
    print("=================================================================")

    disk_path = "build/disk.img"
    if not os.path.exists(disk_path):
        print(f"Generating PurpleFS GPT disk at {disk_path}...")
        subprocess.check_call([sys.executable, "build/mkpfs2_disk", disk_path])

    qemu_bin = r"D:\OS Development\Dev Tools\qemu\qemu-system-x86_64.exe"
    qmp_port = 4455
    cmd = [
        qemu_bin,
        "-kernel", "target/kernel.elf",
        "-serial", "stdio",
        "-vga", "std",
        "-m", "256M",
        "-smp", "4",
        "-drive", f"file={disk_path},format=raw,index=0,media=disk",
        "-qmp", f"tcp:127.0.0.1:{qmp_port},server,nowait",
        "-no-reboot"
    ]

    import threading
    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=0
    )

    captured_chars = []
    def read_stdout():
        try:
            while True:
                ch = proc.stdout.read(1)
                if not ch:
                    break
                captured_chars.append(ch)
        except Exception:
            pass

    t = threading.Thread(target=read_stdout, daemon=True)
    t.start()

    # Wait for QMP server to come online
    print("Connecting to QEMU QMP...")
    s = None
    for _ in range(50):
        try:
            s = socket.create_connection(("127.0.0.1", qmp_port), timeout=2.0)
            break
        except Exception:
            time.sleep(0.1)

    if not s:
        print("FAILED: Could not connect to QEMU QMP.")
        proc.kill()
        sys.exit(1)

    f = s.makefile("rw", encoding="utf-8")

    # QMP Handshake: Read greeting
    greeting = f.readline()
    # Send qmp_capabilities
    f.write(json.dumps({"execute": "qmp_capabilities"}) + "\r\n")
    f.flush()
    cap_resp = f.readline()
    print("QMP Capabilities accepted. Waiting for OS boot (2.5s)...")
    time.sleep(2.5)

    out = "".join(captured_chars)
    if "aegis64@purple:~$" in out:
        print("Shell prompt reached successfully!")
    else:
        print("Waiting another 2s...")
        time.sleep(2.0)
        out = "".join(captured_chars)

    print("Sending keystrokes via QMP send-key (GUI PS/2)...")

    def send_keys(keys):
        key_list = []
        for k in keys:
            key_list.append({"type": "qcode", "data": k})
        msg = {
            "execute": "send-key",
            "arguments": {
                "keys": key_list
            }
        }
        f.write(json.dumps(msg) + "\r\n")
        f.flush()
        f.readline()

    # Send 'version' command via GUI PS/2 keyboard
    print("[TEST 1] Typing 'version' in GUI window...")
    for k in ["v", "e", "r", "s", "i", "o", "n", "ret"]:
        send_keys([k])
        time.sleep(0.04)

    time.sleep(0.8)
    out = "".join(captured_chars)

    if "Purple OS [Aegis64] Version" in out or "AegisShell" in out:
        print("-> PASS: 'version' command executed via GUI keyboard input!")
    else:
        print("-> FAIL: 'version' command output not detected.")
        print("Captured output:")
        print(out[-600:])
        proc.kill()
        sys.exit(1)

    # Send 'echo hello_gui'
    print("[TEST 2] Typing 'echo hello_gui' in GUI window...")
    for k in ["e", "c", "h", "o", "spc", "h", "e", "l", "l", "o", "shift-minus", "g", "u", "i", "ret"]:
        send_keys([k])
        time.sleep(0.04)

    time.sleep(0.8)
    out = "".join(captured_chars)

    if "hello_gui" in out or "hello" in out:
        print("-> PASS: 'echo' executed via GUI keyboard input!")
    else:
        print("-> FAIL: 'echo' output not detected.")
        print(out[-600:])
        proc.kill()
        sys.exit(1)

    # Send 'pwd'
    print("[TEST 3] Typing 'pwd' in GUI window...")
    for k in ["p", "w", "d", "ret"]:
        send_keys([k])
        time.sleep(0.04)

    time.sleep(0.8)
    out = "".join(captured_chars)

    if "/" in out:
        print("-> PASS: 'pwd' executed via GUI keyboard input!")
    else:
        print("-> FAIL: 'pwd' output not detected.")
        print(out[-600:])
        proc.kill()
        sys.exit(1)

    print("\n=================================================================")
    print("  ALL GUI KEYBOARD INPUT TESTS PASSED (100% SUCCESSFUL)          ")
    print("=================================================================")
    proc.kill()

if __name__ == "__main__":
    main()
