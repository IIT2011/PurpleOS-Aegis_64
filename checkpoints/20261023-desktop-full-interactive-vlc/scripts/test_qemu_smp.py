#!/usr/bin/env python3
"""
test_qemu_smp.py - Automated SMP Verification Test Suite for Aegis64

Boots Aegis64 in QEMU with 4 vCPUs (-smp 4) and verifies:
1. ACPI MADT detection of 4 logical CPUs.
2. Startup IPI (SIPI) sequence and long-mode trampoline execution for APs 1, 2, 3.
3. SMP handshake and online registration of all 4 cores.
4. AegisSched_64 per-CPU idle tasks and run queues.
5. Concurrent task execution under strict CPU core affinity.
"""

import os
import sys
import time
import subprocess

def run_smp_test():
    kernel_path = "build/kernel.elf"
    if not os.path.exists(kernel_path):
        print(f"Error: {kernel_path} not found. Run 'make all' first.")
        sys.exit(1)

    cmd = [
        "qemu-system-x86_64",
        "-kernel", kernel_path,
        "-serial", "stdio",
        "-m", "256M",
        "-smp", "4",
        "-display", "none"
    ]

    print("=== Launching Aegis64 SMP Multi-Core Test (-smp 4) ===")
    proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    time.sleep(1.0)
    proc.stdin.write("smp\n")
    proc.stdin.flush()
    time.sleep(0.5)
    proc.stdin.write("smptest\n")
    proc.stdin.flush()
    time.sleep(1.0)
    proc.stdin.write("tasks\n")
    proc.stdin.flush()
    time.sleep(0.5)

    proc.kill()
    stdout, stderr = proc.communicate()

    print("=== SMP Boot & Shell Captured Output ===")
    print(stdout)

    assertions = [
        ("ACPI MADT 4 CPUs Discovery", "ACPI MADT: Local APIC Base=0x00000000fee00000, CPUs=4"),
        ("Local APIC Initialized", "Local APIC Initialized: Base="),
        ("AP 1 Brought Online", "SMP: AP core #1 (APIC ID 1) successfully brought online"),
        ("AP 2 Brought Online", "SMP: AP core #2 (APIC ID 2) successfully brought online"),
        ("AP 3 Brought Online", "SMP: AP core #3 (APIC ID 3) successfully brought online"),
        ("All 4 Cores Operational", "SMP: Multiprocessing operational. 4/4 CPUs online."),
        ("SMP Subsystem Shell Output", "Topology:        4 discovered cores, 4 online cores"),
        ("SMP Core Affinity Test Spawned", "Spawning SMP multi-core concurrent test tasks..."),
        ("Concurrent Task Execution", "SMP Worker (bound to CPU"),
        ("Per-CPU Idle Tasks Running", "idle-1"),
    ]

    all_passed = True
    print("\n=== SMP Verification Assertions ===")
    for desc, pattern in assertions:
        if pattern in stdout:
            print(f"[OK] Verified: {desc}")
        else:
            print(f"[FAILED] Missing: {desc} (pattern: '{pattern}')")
            all_passed = False

    if all_passed:
        print("\n>>> ALL STAGE 3 SMP MULTI-CORE INTEGRATION TESTS PASSED! <<<")
        sys.exit(0)
    else:
        print("\n>>> SMP MULTI-CORE INTEGRATION VERIFICATION FAILED <<<")
        sys.exit(1)

if __name__ == "__main__":
    run_smp_test()
