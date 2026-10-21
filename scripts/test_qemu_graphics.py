#!/usr/bin/env python3
import subprocess
import time
import sys

def main():
    print("=== Launching Aegis64 QEMU Graphics & Framebuffer Test ===")

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
        "video\n",
        "video mode 800 600\n",
        "video\n",
        "echo Hello Purple OS Graphical Console!\n",
        "desktop\n",
        "desktop preview\n",
        "clear\n",
        "help\n"
    ]

    for c in commands:
        try:
            proc.stdin.write(c)
            proc.stdin.flush()
            time.sleep(0.18)
        except Exception:
            break

    time.sleep(0.5)
    proc.terminate()
    try:
        stdout, stderr = proc.communicate(timeout=2)
    except Exception:
        proc.kill()
        stdout, stderr = proc.communicate()

    print("\n=== Graphics Test Captured Serial Output ===")
    print(stdout)

    assertions = [
        ("Video Configuration Command", "Video Display Configuration"),
        ("Bochs VBE Hardware Present", "Bochs VBE:  Supported (Ports 0x1CE/0x1CF, PCI 1234:1111)"),
        ("Video Mode Switch", "Video mode switched to 800x600@32bpp"),
        ("Video Framebuffer Active", "Resolution: 800x600@32bpp"),
        ("Echo Rendering Text", "Hello Purple OS Graphical Console!"),
        ("Desktop Framebuffer Render", "Desktop: Graphical desktop frame rendered to framebuffer (800x600)"),
        ("Desktop GUI Preview Rendering", "Purple OS / Aegis64 Native Desktop Environment"),
        ("Desktop Windows Rendered", "Aegis64 System Monitor"),
        ("Help Command Rendered", "AegisShell / Purple Shell Commands")
    ]

    failed = 0
    for name, pattern in assertions:
        if pattern in stdout:
            print(f"[OK] Verified: {name} ('{pattern}')")
        else:
            print(f"[FAIL] Missing: {name} ('{pattern}')")
            failed += 1

    if failed == 0:
        print("\n>>> ALL GRAPHICS & FRAMEBUFFER INTEGRATION TESTS PASSED! <<<\n")
        return 0
    else:
        print(f"\n>>> GRAPHICS INTEGRATION TESTS FAILED ({failed} missing assertions)! <<<\n")
        return 1

if __name__ == "__main__":
    sys.exit(main())
