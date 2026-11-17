#!/usr/bin/env python3
import subprocess
import time
import sys

def main():
    print("=== Launching QEMU to Validate Aegis64 POSIX.1-2024 Compliance & Hardening ===")
    cmd = [
        "qemu-system-x86_64",
        "-kernel", "target/kernel.elf",
        "-serial", "stdio",
        "-display", "none",
        "-m", "256M",
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

    # Allow kernel to initialize and print prompt
    time.sleep(2.5)

    commands = [
        ("posix status\n", 0.6),
        ("posix test\n", 2.0),
        ("echo POSIX-SUITE-COMPLETED\n", 0.8)
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

    print("=== Captured Kernel & Shell POSIX Output ===")
    print(stdout)

    required_strings = [
        "Aegis64 POSIX 2024 / IEEE Std 1003.1 Subsystem:",
        "IEEE Std 1003.1-2024 / Linux x86_64 ABI Compatibility",
        "0xC0000080 / EFER.SCE Active (SYSCALL/SYSRET)",
        "FD 0 (stdin), FD 1 (stdout), FD 2 (stderr)",
        "Aegis64 POSIX.1-2024 Conformance & Hardening Suite",
        "[POSIX TEST 1/12] Standard Streams (stdin=0, stdout=1, stderr=2)... [POSIX OK]",
        "[POSIX TEST 2/12] File I/O Lifecycle (open, write, lseek, read, close)... [PASSED]",
        "[POSIX TEST 3/12] Shared Open-File Descriptions (dup / dup2 / shared offset)... [PASSED]",
        "[POSIX TEST 4/12] Metadata & File Status (sys_stat / sys_fstat)... [PASSED]",
        "[POSIX TEST 5/12] Virtual Memory Management (mmap / munmap / mprotect)... [PASSED]",
        "[POSIX TEST 6/12] Process Heap Expansion (sys_brk growth & contraction)... [PASSED]",
        "[POSIX TEST 7/12] Streaming Byte IPC (pipe / EOF / EPIPE detection)... [PASSED]",
        "[POSIX TEST 8/12] Real-Time & Monotonic Clocks (sys_clock_gettime)... [PASSED",
        "[POSIX TEST 9/12] Process Context & Signal Probing (getpid / kill)... [PASSED]",
        "[POSIX TEST 10/12] Signal Management (rt_sigaction / rt_sigprocmask)... [PASSED]",
        "[POSIX TEST 11/12] Directory & Path Operations (getcwd / chdir / access)... [PASSED]",
        "[POSIX TEST 12/12] User Memory Safety (rejection of null/kernel/unmapped)... [PASSED]",
        ">>> ALL 12 POSIX SUBSYSTEM VALIDATION TESTS PASSED (100% PURE RUST)! <<<",
        "POSIX-SUITE-COMPLETED"
    ]

    all_passed = True
    for s in required_strings:
        if s in stdout:
            print(f"[OK] Found expected output: '{s}'")
        else:
            print(f"[FAIL] Missing expected output: '{s}'")
            all_passed = False

    if all_passed:
        print("\n>>> ALL POSIX.1-2024 COMPLIANCE SUITE TESTS PASSED 100%! <<<")
        sys.exit(0)
    else:
        print("\n>>> POSIX COMPLIANCE SUITE FAILED! <<<")
        sys.exit(1)

if __name__ == "__main__":
    main()
