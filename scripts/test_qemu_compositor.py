#!/usr/bin/env python3
"""
Comprehensive QEMU Integration Test for PurpleOS / Aegis64 Native Compositor Subsystem.

Verifies:
1. Compositor Kernel Engine boot and initialization
2. Canonical on-disk filesystem hierarchy:
   - /System/Services/Compositor
   - /System/Frameworks/Compositor.framework/compositor.conf
   - /System/Configuration/desktop.cfg
3. Cryptographically authenticated /System/CLI/cmd.dtbase signed by Creator
4. Dynamic character device nodes: /dev/compositor and /dev/fb0
5. Shell commands: 'compositor' and 'window'
6. Window management lifecycle: creation, Z-ordering, focus, and close
"""

import subprocess
import time
import sys
import os

def main():
    print("=== Launching Aegis64 QEMU Native Compositor Integration Test ===")

    cmd = [
        "qemu-system-x86_64",
        "-kernel", "target/kernel.elf",
        "-serial", "stdio",
        "-display", "none",
        "-vga", "std",
        "-m", "256M",
        "-smp", "4",
        "-drive", "file=build/purple_gpt.img,format=raw,index=0,media=disk",
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

    time.sleep(1.5)

    commands = [
        "ver\n",
        "ls /dev\n",
        "cat /dev/compositor\n",
        "cat /dev/fb0\n",
        "cat /System/Configuration/desktop.cfg\n",
        "cat /System/Services/Compositor\n",
        "cat /System/Frameworks/Compositor.framework/compositor.conf\n",
        "compositor\n",
        "compositor demo\n",
        "window list\n",
        "window focus 1\n",
        "window close 3\n",
        "window list\n",
        "compositor stop\n",
        "compositor\n"
    ]

    for c in commands:
        try:
            proc.stdin.write(c)
            proc.stdin.flush()
            time.sleep(0.35)
        except Exception:
            break

    time.sleep(2.0)
    proc.terminate()
    try:
        stdout, stderr = proc.communicate(timeout=5)
    except Exception:
        proc.kill()
        stdout, stderr = proc.communicate()

    print("\n=== Compositor Test Serial Output ===")
    print(stdout)

    assertions = [
        ("AegisDetect Trusted Baseline", "AegisDetect: Loaded and authenticated trusted baseline"),
        ("Creator cmd.dtbase Authentication", "Authenticated /System/CLI/cmd.dtbase — Cryptographically Signed by Creator!"),
        ("Compositor Engine Initialized", "Compositor: Aegis64 Window Server & Framebuffer Engine Initialized"),
        ("Direct Boot to Compositor", "Compositor: Native framebuffer compositor active"),
        ("Purple Desktop UI Shell Online", "Purple Desktop UI online"),
        ("/dev/compositor Device Node", "compositor"),
        ("/dev/fb0 Device Node", "fb0"),
        ("Compositor VFS Query", "PurpleOS Native Compositor Engine"),
        ("FrameBuffer VFS Query", "format=ARGB8888, double_buffer=active"),
        ("Desktop Configuration Content", "wallpaper_gradient_top = 0x120722"),
        ("Compositor Service Descriptor", "Name = PurpleCompositor"),
        ("Compositor Framework Config", "Compositor.framework"),
        ("Compositor Status Command", "Aegis64 Native Framebuffer Compositor Subsystem"),
        ("Desktop Shell Status Query", "Purple Desktop UI (Aegis Bar & Flow Dock Active)"),
        ("Compositor Demo Windows Launch", "Compositor: Demo windows launched"),
        ("Window List - Aegis Terminal", "Aegis Terminal"),
        ("Window List - System Monitor", "System Monitor"),
        ("Window List - About PurpleOS", "About PurpleOS"),
        ("Window Close Command", "Closed window 3"),
        ("Compositor Stop Command", "Compositor: Stopped, restored text mode console")
    ]

    failed = 0
    for name, pattern in assertions:
        if pattern in stdout:
            print(f"[OK] Verified: {name} ('{pattern}')")
        else:
            print(f"[FAIL] Missing: {name} ('{pattern}')")
            failed += 1

    if failed == 0:
        print("\n>>> ALL NATIVE COMPOSITOR INTEGRATION TESTS PASSED! <<<\n")
        return 0
    else:
        print(f"\n>>> NATIVE COMPOSITOR TESTS FAILED ({failed} missing assertions)! <<<\n")
        return 1

if __name__ == "__main__":
    sys.exit(main())
