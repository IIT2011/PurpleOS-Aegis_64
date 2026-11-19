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
import re

def main():
    print("=== Launching Aegis64 QEMU Native Compositor Integration Test ===")

    disk = os.environ.get("PURPLEOS_TEST_DISK", "build/purple_gpt.img")
    cmd = [
        "qemu-system-x86_64",
        "-kernel", "target/kernel.elf",
        "-serial", "stdio",
        "-display", "none",
        "-vga", "std",
        "-m", "256M",
        "-smp", "4",
        "-drive", f"file={disk},format=raw,index=0,media=disk",
        "-snapshot",
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
        ("AegisDetect Trusted Manifest", "AegisDetect: Verification summary: 3/3 components verified"),
        ("Creator cmd.dtbase Authentication", "Authenticated /System/CLI/cmd.dtbase — Cryptographically Signed by Creator!"),
        ("Compositor Engine Initialized", "Compositor: Aegis64 Window Server & Framebuffer Engine Initialized"),
        ("Direct Boot to Compositor", "Compositor: Native framebuffer compositor active"),
        ("Purple Desktop UI Shell Online", "Purple Desktop UI online"),
        ("/dev/compositor Device Node", "compositor"),
        ("/dev/fb0 Device Node", "fb0"),
        ("Compositor VFS Query", "PurpleOS Native Compositor Engine"),
        ("Framebuffer VFS Query", "format=32bpp"),
        ("Desktop Configuration Content", "wallpaper_gradient_top = 0x120722"),
        ("Compositor Service Descriptor", "Name = PurpleCompositor"),
        ("Compositor Framework Config", "Compositor.framework"),
        ("Compositor Status Command", "Aegis64 Native Framebuffer Compositor Subsystem"),
        ("Desktop Shell Status Query", "Purple Desktop UI (Purple Bar & Purple Dock Active)"),
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

    if not re.search(r"(?m)^fb0: [1-9]\d*x[1-9]\d*x32, pitch=[1-9]\d*, format=32bpp$", stdout):
        print("[FAIL] Missing framebuffer device geometry")
        failed += 1

    statuses = stdout.split("=== Aegis64 Native Framebuffer Compositor Subsystem ===")
    boot_statuses = statuses[1:3]
    counters = [re.search(r"Frames Rendered:\s+(\d+)", status) for status in boot_statuses]
    if (len(counters) != 2 or not all(counters)
            or int(counters[1][1]) <= int(counters[0][1])
            or not all(re.search(r"Windows Open:\s+0\b", status) for status in boot_statuses)):
        print("[FAIL] Expected idle frame updates before opening diagnostic windows")
        failed += 1
    else:
        print("[OK] Idle frames advance while the desktop has zero windows")

    if failed == 0:
        print("\n>>> ALL NATIVE COMPOSITOR INTEGRATION TESTS PASSED! <<<\n")
        return 0
    else:
        print(f"\n>>> NATIVE COMPOSITOR TESTS FAILED ({failed} missing assertions)! <<<\n")
        return 1

if __name__ == "__main__":
    sys.exit(main())
