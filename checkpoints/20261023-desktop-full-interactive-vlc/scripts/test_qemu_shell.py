#!/usr/bin/env python3
import subprocess
import time
import sys

def main():
    print("=== Launching QEMU with Aegis64 Kernel ===")
    cmd = [
        "qemu-system-x86_64",
        "-kernel", "build/kernel.elf",
        "-serial", "stdio",
        "-display", "none",
        "-m", "256M",
        "-no-reboot"
    ]

    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=0
    )

    # Allow kernel to initialize and print prompt
    time.sleep(1.0)

    commands = [
        "sysinfo\n",
        "cpuinfo\n",
        "mem\n",
        "uptime\n",
        "tasks\n",
        "echo Aegis64-Reconstruction-Verified\n"
    ]

    for c in commands:
        proc.stdin.write(c)
        proc.stdin.flush()
        time.sleep(0.3)

    # Wait another second to collect output
    time.sleep(1.0)
    proc.terminate()
    try:
        stdout, stderr = proc.communicate(timeout=2)
    except subprocess.TimeoutExpired:
        proc.kill()
        stdout, stderr = proc.communicate()

    print("=== Captured Kernel & Shell Output ===")
    print(stdout)

    # Verifications
    required_strings = [
        "Aegis64 Kernel (Purple OS) Initializing",
        "PMM Initialized",
        "VMM Initialized",
        "AegisSched_64 / PurpleSched",
        "aegis64@purple:~$",
        "Purple OS (Aegis64 Kernel)",
        "Memory Accounting",
        "AegisSched_64 Active Tasks",
        "Aegis64-Reconstruction-Verified"
    ]

    all_passed = True
    for s in required_strings:
        if s in stdout:
            print(f"[OK] Found expected output: '{s}'")
        else:
            print(f"[FAIL] Missing expected output: '{s}'")
            all_passed = False

    if all_passed:
        print("\n>>> ALL SHELL AND SCHEDULER INTEGRATION CHECKS PASSED! <<<")
        return 0
    else:
        print("\n>>> SOME VERIFICATION CHECKS FAILED! <<<")
        return 1

if __name__ == "__main__":
    sys.exit(main())
