#!/usr/bin/env python3
"""
Systemwide Font Integration Test Suite for PurpleOS / Aegis64.
Verifies:
  1. Discovery of /System/Resources/Fonts directory and font assets via VFS.
  2. Byte-accurate reading of multi-block TTF files via PurpleFS v2 indirect blocks.
  3. Parsing of TrueType SFNT metadata (Family, Style, Designer, Version, Glyphs, Metrics).
  4. Dynamic systemwide font selection ('font set inter', 'font set vga').
  5. Anti-aliased font rendering pipeline.
"""

import subprocess
import time
import sys
import os

def main():
    print("=== Launching Aegis64 Systemwide Font Integration Test ===")
    disk_path = os.environ.get("PURPLEOS_TEST_DISK", "build/purple_gpt.img")
    if not os.path.exists(disk_path):
        subprocess.check_call([sys.executable, "build/mkpfs2_disk", disk_path])

    cmd = [
        "qemu-system-x86_64",
        "-kernel", "target/kernel.elf",
        "-serial", "stdio",
        "-vga", "std",
        "-m", "256M",
        "-smp", "4",
        "-drive", f"file={disk_path},format=raw,index=0,media=disk",
        "-display", "none",
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

    import threading
    captured = []
    def read_stdout():
        for line in iter(proc.stdout.readline, ''):
            captured.append(line)
            sys.stdout.write(line)
            sys.stdout.flush()

    t = threading.Thread(target=read_stdout, daemon=True)
    t.start()

    time.sleep(3.5)

    commands = [
        "ls /System/Resources\n",
        "ls /System/Resources/Fonts\n",
        "cat /System/Resources/Fonts/Inter.conf\n",
        "font\n",
        "font list\n",
        "font info Inter-Regular.ttf\n",
        "font info Inter-Bold.ttf\n",
        "font set vga\n",
        "font set inter\n",
    ]

    for c in commands:
        proc.stdin.write(c)
        proc.stdin.flush()
        time.sleep(1.2)

    proc.terminate()
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()

    t.join(timeout=5)
    stdout = "".join(captured)
    print("\n=== Verifying Test Assertions ===")

    assertions = [
        ("Directory /System/Resources/Fonts Discovered", "Fonts"),
        ("Inter-Regular.ttf in Font Directory", "Inter-Regular.ttf"),
        ("Inter-Bold.ttf in Font Directory", "Inter-Bold.ttf"),
        ("Inter.conf Descriptor Seeded", "Family = Inter"),
        ("System Font Subsystem Online", "PurpleOS System Font Subsystem"),
        ("Active Font is Inter", "Inter 4.1"),
        ("Font Registry Enumeration", "System Fonts Registry (/System/Resources/Fonts/):"),
        ("TrueType Font Specification Header", "TrueType Font Vector Specification"),
        ("Inter-Regular Family Verified", "Family Name:     Inter"),
        ("Inter-Regular Style Verified", "Subfamily/Style: Regular"),
        ("Inter Designer Verified", "Designer:        Rasmus Andersson"),
        ("Inter Units Per EM Verified", "Units per EM:    2048"),
        ("Inter Total Glyphs Verified", "Total Glyphs:    2937"),
        ("Inter-Bold Style Verified", "Subfamily/Style: Bold"),
        ("Font Switch to VGA", "System font set to: Classic IBM VGA"),
        ("Font Switch to Inter", "System font set to: Inter (Subpixel Anti-Aliased Vector Font)"),
    ]

    failed = 0
    for name, pattern in assertions:
        if pattern in stdout:
            print(f"[OK] Verified: {name}")
        else:
            print(f"[FAIL] Missing: {name} (expected '{pattern}')")
            failed += 1

    if failed == 0:
        print("\n>>> ALL SYSTEMWIDE FONT INTEGRATION TESTS PASSED (16/16)! <<<")
        sys.exit(0)
    else:
        print(f"\n>>> FAILED {failed} TEST(S) <<<")
        sys.exit(1)

if __name__ == "__main__":
    main()
