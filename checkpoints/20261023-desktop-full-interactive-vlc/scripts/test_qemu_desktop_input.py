#!/usr/bin/env python3
import subprocess
import time
import sys

def main():
    print("=== Launching Aegis64 QEMU Desktop Input & Mouse Integration Test ===")

    cmd = [
        "qemu-system-x86_64",
        "-kernel", "build/kernel.elf",
        "-serial", "stdio",
        "-display", "none",
        "-vga", "std",
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

    time.sleep(1.2)

    commands = [
        "video mode 800 600\n",
        "desktop\n",
        "mouse\n",
        "mouse set 355 55\n",
        "mouse click left\n",
        "mouse\n",
        "desktop toggle 1\n",
        "desktop session\n",
        "q\n",
        "echo DESKTOP_INPUT_COMPLETE\n"
    ]

    for c in commands:
        try:
            proc.stdin.write(c)
            proc.stdin.flush()
            time.sleep(0.2)
        except Exception:
            break

    time.sleep(0.5)
    proc.terminate()
    try:
        stdout, stderr = proc.communicate(timeout=2)
    except Exception:
        proc.kill()
        stdout, stderr = proc.communicate()

    print("\n=== Desktop Input Test Captured Serial Output ===")
    print(stdout)

    assertions = [
        ("Mouse Subsystem Initialized", "Mouse: PS/2 Auxiliary device initialized on IRQ 12"),
        ("Desktop Framebuffer Render", "Desktop: Graphical desktop frame rendered to framebuffer (800x600)"),
        ("Mouse Position Query", "=== PS/2 Mouse Subsystem ==="),
        ("Mouse Pointer Coordinate Set", "Mouse pointer set to (355, 55)"),
        ("Simulated Mouse Click", "Simulated mouse click [LEFT] at (355, 55)"),
        ("Desktop Interactive Session", "Desktop: Interactive session online"),
        ("Desktop Clean Exit", "Desktop: Interactive session terminated"),
        ("Completion Sentinel", "DESKTOP_INPUT_COMPLETE")
    ]

    failed = 0
    for name, pattern in assertions:
        if pattern in stdout:
            print(f"[OK] Verified: {name} ('{pattern}')")
        else:
            print(f"[FAIL] Missing: {name} ('{pattern}')")
            failed += 1

    if failed == 0:
        print("\n>>> ALL DESKTOP INPUT & MOUSE INTEGRATION TESTS PASSED! <<<\n")
        return 0
    else:
        print(f"\n>>> DESKTOP INPUT INTEGRATION TESTS FAILED ({failed} missing assertions)! <<<\n")
        return 1

if __name__ == "__main__":
    sys.exit(main())
