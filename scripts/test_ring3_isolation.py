#!/usr/bin/env python3
import subprocess
import time
import sys
import os

def main():
    print("=================================================================")
    print("  PURPLE OS / AEGIS64 — RING 3 (USER MODE) VERIFICATION SUITE   ")
    print("=================================================================")

    disk_path = "build/disk.img"
    if not os.path.exists(disk_path):
        print(f"Generating PurpleFS GPT disk at {disk_path}...")
        subprocess.check_call([sys.executable, "build/mkpfs2_disk", disk_path])

    cmd = [
        "qemu-system-x86_64",
        "-kernel", "target/kernel.elf",
        "-serial", "stdio",
        "-display", "none",
        "-m", "256M",
        "-smp", "4",
        "-drive", f"file={disk_path},format=raw,index=0,media=disk",
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

    captured_chunks = []
    def read_stdout():
        for line in iter(proc.stdout.readline, ''):
            captured_chunks.append(line)
    t = threading.Thread(target=read_stdout, daemon=True)
    t.start()

    print("[INIT] Waiting 2.5s for kernel boot and hardware initialization...")
    time.sleep(2.5)

    commands = [
        ("ring3 info\n", 0.6),
        ("ring3 test\n", 1.0),
        ("ring3 fault div0\n", 0.8),
        ("ring3 fault pagefault\n", 0.8),
        ("tasks user TestWorker\n", 0.6),
        ("echo RING3-TESTS-COMPLETED\n", 0.8)
    ]

    for c, delay in commands:
        for ch in c:
            proc.stdin.write(ch)
            proc.stdin.flush()
            time.sleep(0.01)
        time.sleep(delay)

    time.sleep(1.0)
    proc.terminate()
    try:
        proc.wait(timeout=2)
    except subprocess.TimeoutExpired:
        proc.kill()

    stdout = "".join(captured_chunks)

    print("=== Captured Output ===")
    print(stdout)

    required_assertions = [
        ("Ring 3 Architecture Info", "Privilege Architecture: x86_64 Dual-Privilege Model (Ring 0 / Ring 3)"),
        ("Dual Privilege Model", "Hardware Enforced:      CPL=0 (Kernel Supervisor), CPL=3 (Userland)"),
        ("GDT User DS", "User DS   (DPL 3):    0x1B"),
        ("GDT User CS", "User CS   (DPL 3):    0x23"),
        ("MSR STAR", "MSR_STAR (0xC0000081): 0x0010000800000000"),
        ("Canonical User Space Min", "User Space Min:       0x10000"),
        ("Canonical User Code Base", "User Code Base:       0x400000"),
        ("Canonical User Stack Top", "User Stack Top:       0x7fffffff0000"),
        ("Ring 3 Demo Spawning", "[RING3] Spawning Ring 3 Userland Demonstration Process..."),
        ("Ring 3 CPL=3 Execution", "[RING3] Aegis64 Ring 3 User Mode Execution Confirmed (CPL=3)!"),
        ("POSIX SYS_WRITE from Ring 3", "[RING3] POSIX SYS_WRITE: User Stack & Sycall ABI Verified!"),
        ("Fault Div0 Spawning", "[RING3] Spawning Ring 3 Process to Trigger Fault: Division By Zero (#DE)..."),
        ("Fault Div0 Interception", "[SECURITY] User Process Exception: Division By Zero"),
        ("Fault Pagefault Spawning", "[RING3] Spawning Ring 3 Process to Trigger Fault: Kernel Memory Access (#PF)..."),
        ("Fault Pagefault Interception", "[SECURITY] User Process Exception: Page Fault"),
        ("Isolated User Task Spawning", "Spawned isolated user task 'TestWorker'"),
        ("Zero Kernel Panic / Shell Alive", "RING3-TESTS-COMPLETED")
    ]

    all_passed = True
    print("=== Assertion Results ===")
    for name, pattern in required_assertions:
        if pattern in stdout:
            print(f"[OK] {name} -> PASSED")
        else:
            print(f"[FAIL] {name} -> FAILED (Pattern '{pattern}' not found)")
            all_passed = False

    if all_passed:
        print("\n>>> ALL RING 3 (USER MODE) VERIFICATIONS PASSED (100%)! <<<")
        sys.exit(0)
    else:
        print("\n>>> SOME RING 3 VERIFICATIONS FAILED! <<<")
        sys.exit(1)

if __name__ == "__main__":
    main()
