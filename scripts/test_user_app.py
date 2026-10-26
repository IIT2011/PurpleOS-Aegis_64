#!/usr/bin/env python3
import subprocess, time, os, socket, threading

sock_path = "/tmp/qemu_user_sock"
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
    time.sleep(0.02)
    s.recv(1024)
    s.sendall((c + "\n").encode())
    time.sleep(0.05)
    res = s.recv(4096).decode(errors="ignore")
    s.close()
    return res

def send_string(str_val):
    for ch in str_val:
        if ch == ' ': qcmd("sendkey spc")
        elif ch == '/': qcmd("sendkey slash")
        elif ch == '.': qcmd("sendkey dot")
        elif ch == '-': qcmd("sendkey minus")
        elif ch.isupper(): qcmd(f"sendkey shift-{ch.lower()}")
        elif ch.isdigit() or ch.islower(): qcmd(f"sendkey {ch}")
        time.sleep(0.03)
    qcmd("sendkey ret")
    time.sleep(0.05)

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
        # PRINT ALL INFO / USER APP / SCHEDULER / ERROR LINES
        if any(w in l for w in ["[INFO]", "[ERROR]", "[USER APP]", "Scheduler", "APEX"]):
            print("  >>", l)

t = threading.Thread(target=reader, daemon=True)
t.start()

print("Booting guest...")
desktop_online.wait(timeout=25)
print("[OK] Desktop session online! Waiting 1s...")
time.sleep(1)

print("\n>>> Launching PurpleTerminal.apapplication...")
send_string("app run /System/Applications/PurpleTerminal.apapplication")
time.sleep(3)

stop_thread = True
p.kill()
p.wait()
