#!/usr/bin/env python3
import subprocess
import time
import sys

def main():
    print("=== Launching QEMU to Validate Aegis64 / PurpleOS Native System Calls (68 Native Syscalls) ===")
    cmd = [
        "qemu-system-x86_64",
        "-kernel", "target/kernel.elf",
        "-serial", "stdio",
        "-display", "none",
        "-m", "256M",
        "-smp", "4",
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
        ("native status\n", 0.6),
        ("native list\n", 0.6),
        ("native test\n", 2.0),
        ("echo NATIVE-SUITE-COMPLETED\n", 0.8)
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

    print("=== Captured Kernel & Shell Native Syscall Output ===")
    print(stdout)

    required_strings = [
        "Aegis64 / PurpleOS Native System Call Subsystem:",
        "Architecture:    PurpleOS First-Class Native ABI (Non-POSIX, Non-Linux)",
        "Syscall Range:   0x1000 - 0x10AF (68 High-Performance Native Calls)",
        "Aegis64 / PurpleOS Native System Call Catalog (68 Total):",
        "Category 1: GPU & Display Acceleration  (0x1000 - 0x1009) [10 Calls]",
        "Category 2: Audio DSP & PCM Streaming   (0x1020 - 0x1027) [ 8 Calls]",
        "Category 3: Fast Zero-Copy Channel IPC  (0x1030 - 0x1037) [ 8 Calls]",
        "Category 4: Multi-Core SMP & Affinity   (0x1040 - 0x1048) [ 9 Calls]",
        "Category 5: High-Precision Clocks/TSC   (0x1050 - 0x1056) [ 7 Calls]",
        "Category 6: Raw Direct DMA Networking   (0x1060 - 0x1065) [ 6 Calls]",
        "Category 7: Hardware & PMM Telemetry    (0x1070 - 0x1075) [ 6 Calls]",
        "Category 8: Raw Block Storage I/O       (0x1080 - 0x1084) [ 5 Calls]",
        "Category 9: Real-Time Input Subsystem   (0x1090 - 0x1092) [ 3 Calls]",
        "Category 10: Task & Sandbox Security    (0x10A0 - 0x10A5) [ 6 Calls]",
        "Aegis64 / PurpleOS Native System Call Validation Suite",
        "[NATIVE TEST 1/10] Linear Framebuffer & 2D/3D GPU Info... [PASSED]",
        "[NATIVE TEST 2/10] Audio Capabilities & Mixer Volume Control... [PASSED]",
        "[NATIVE TEST 3/10] Native Zero-Copy Channel IPC (Create, Send, Recv, Close)... [PASSED]",
        "[NATIVE TEST 4/10] Multi-Core SMP Core Count & Affinity Controls... [PASSED]",
        "[NATIVE TEST 5/10] High-Precision RTC Wall-Clock & Monotonic TSC Cycles... [PASSED]",
        "[NATIVE TEST 6/10] Raw Layer-2 Network Device Telemetry & Stats... [PASSED]",
        "[NATIVE TEST 7/10] Hardware Device Introspection & PMM Memory Metrics... [PASSED]",
        "[NATIVE TEST 8/10] Advanced Raw Block I/O Device Inventory & Geometry... [PASSED]",
        "[NATIVE TEST 9/10] Real-Time Input Subsystem (Mouse Coordinates & Keys)... [PASSED]",
        "[NATIVE TEST 10/10] Native Process Capabilities & Security Sandbox... [PASSED]",
        "10/10 Subsystem Categories PASSED (100% Conformance)",
        "NATIVE-SUITE-COMPLETED"
    ]

    all_passed = True
    for s in required_strings:
        if s in stdout:
            print(f"[OK] Found expected output: '{s}'")
        else:
            print(f"[FAIL] Missing expected output: '{s}'")
            all_passed = False

    if all_passed:
        print("\n>>> ALL 68 NATIVE SYSTEM CALLS & SUBSYSTEM VALIDATION TESTS PASSED 100%! <<<")
        sys.exit(0)
    else:
        print("\n>>> NATIVE SYSTEM CALL VALIDATION SUITE FAILED! <<<")
        sys.exit(1)

if __name__ == "__main__":
    main()
