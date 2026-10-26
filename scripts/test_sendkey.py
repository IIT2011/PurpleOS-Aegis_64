#!/usr/bin/env python3
import subprocess, time, os, socket, threading
from PIL import Image

sock_path = "/tmp/qemu_test_sock"
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
    time.sleep(0.05)
    s.recv(1024)
    s.sendall((c + "\n").encode())
    time.sleep(0.1)
    res = s.recv(4096).decode(errors="ignore")
    s.close()
    return res

p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)

desktop_online = threading.Event()
stop_thread = False

def reader():
    while not stop_thread:
        line = p.stdout.readline()
        if not line: break
        l = line.strip()
        if "Desktop: Interactive session online" in l:
            desktop_online.set()
        if any(k in l for k in ["[TERM]", "APAPP", "AegisDetect", "PurpleTerminal", "Desktop"]):
            print("  >>", l)

t = threading.Thread(target=reader, daemon=True)
t.start()

print("Waiting for desktop boot...")
desktop_online.wait(timeout=25)
print("[OK] Desktop online! Waiting 1s...")
time.sleep(1)

print("Sending 'help' via sendkey...")
for k in ["h", "e", "l", "p", "ret"]:
    qcmd(f"sendkey {k}")
    time.sleep(0.05)

time.sleep(1)

print("Sending 'ls' via sendkey...")
for k in ["l", "s", "ret"]:
    qcmd(f"sendkey {k}")
    time.sleep(0.05)

time.sleep(1)

stop_thread = True
p.kill()
p.wait()
