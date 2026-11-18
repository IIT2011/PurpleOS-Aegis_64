#!/usr/bin/env python3
"""
Test Suite: PurpleDetect Hardware-Driven APDRV Bundle Matching & Auto-Loading.
Verifies:
  Session 1 (Standard Hardware):
    - PurpleDetect scans hardware at boot.
    - Matches and loads only detected hardware drivers:
        * cpu.apdrv (Core)
        * ram.apdrv (Core)
        * filesystems.apdrv (Core)
        * storage.apdrv (Matched: ATA/IDE)
        * input.apdrv (Matched: PS/2)
        * gpu.apdrv (Matched: Bochs VGA)
        * network.apdrv (Matched: Intel E1000)
    - Skips non-present hardware:
        * audio.apdrv (Skipped)
        * usb.apdrv (Skipped)
    - Confirms lsmod contains active drivers and does NOT contain audio or usb.

  Session 2 (Attached Hardware with Intel HDA Audio & xHCI USB):
    - Launches QEMU with -device intel-hda -device hda-duplex -device qemu-xhci.
    - PurpleDetect discovers Audio and USB controllers on PCI bus.
    - Matches and loads audio.apdrv and usb.apdrv automatically!
    - Confirms lsmod contains both 'audio' and 'usb' in RUNNING state!
"""

import subprocess
import tempfile
import os
import sys
import time

def run_session(desc, extra_qemu_args, expected_checks, unexpected_checks):
    print(f"\n=== Running {desc} ===")
    with tempfile.TemporaryDirectory() as tmpdir:
        disk_path = os.path.join(tmpdir, "match_test.img")
        mkpfs_bin = os.path.join("build", "mkpfs2_disk")
        subprocess.check_call([sys.executable, mkpfs_bin, disk_path])

        qemu_cmd = [
            "qemu-system-x86_64",
            "-kernel", "target/kernel.elf",
            "-serial", "stdio",
            "-display", "none",
            "-m", "256M",
            "-smp", "4",
            "-drive", f"file={disk_path},format=raw,index=0,media=disk",
            "-no-reboot"
        ] + extra_qemu_args

        proc = subprocess.Popen(
            qemu_cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )

        stdout_data = ""
        try:
            time.sleep(2.0)
            proc.stdin.write("lsmod\n")
            proc.stdin.flush()
            time.sleep(0.4)
            proc.stdin.write("devices\n")
            proc.stdin.flush()
            time.sleep(0.4)
            proc.stdin.write("poweroff\n")
            proc.stdin.flush()

            stdout_data, _ = proc.communicate(timeout=8)
        except subprocess.TimeoutExpired:
            proc.kill()
            stdout_data, _ = proc.communicate()

        all_ok = True
        print("--- Verifying Required Patterns ---")
        for label, pattern in expected_checks:
            if pattern in stdout_data:
                print(f"[OK] {label}")
            else:
                print(f"[FAIL] {label}: Pattern '{pattern}' not found")
                all_ok = False

        print("--- Verifying Excluded Patterns ---")
        for label, pattern in unexpected_checks:
            if pattern not in stdout_data:
                print(f"[OK] {label} (correctly absent)")
            else:
                print(f"[FAIL] {label}: Unexpected pattern '{pattern}' found in output")
                all_ok = False

        return all_ok, stdout_data

def main():
    print("=================================================================")
    print("  PURPLEDETECT HARDWARE-DRIVEN APDRV DRIVER MATCHING TEST SUITE  ")
    print("=================================================================")

    # Session 1: Standard Hardware (No audio, no USB controller attached)
    s1_expected = [
        ("Core CPU Loaded", "PurpleDetect: Matched hardware for 'cpu.apdrv'"),
        ("Core RAM Loaded", "PurpleDetect: Matched hardware for 'ram.apdrv'"),
        ("Core Filesystems Loaded", "PurpleDetect: Matched hardware for 'filesystems.apdrv'"),
        ("Storage Driver Matched", "PurpleDetect: Matched hardware for 'storage.apdrv'"),
        ("Input Driver Matched", "PurpleDetect: Matched hardware for 'input.apdrv'"),
        ("GPU Driver Matched", "PurpleDetect: Matched hardware for 'gpu.apdrv'"),
        ("Network Driver Matched", "PurpleDetect: Matched hardware for 'network.apdrv'"),
        ("Audio Driver Skipped", "PurpleDetect: No matching hardware detected for 'audio.apdrv'"),
        ("USB Driver Skipped", "PurpleDetect: No matching hardware detected for 'usb.apdrv'"),
        ("Driver Load Summary", "PurpleDetect: Hardware-driven driver load complete: 7 loaded, 2 skipped"),
        ("LSMOD Has Storage", "storage                         RUNNING"),
        ("LSMOD Has Network", "network                         RUNNING"),
        ("LSMOD Has GPU", "gpu                             RUNNING"),
        ("LSMOD Has Filesystems", "filesystems                     RUNNING"),
    ]
    s1_unexpected = [
        ("Audio NOT in LSMOD", "audio                           RUNNING"),
        ("USB NOT in LSMOD", "usb                             RUNNING"),
    ]
    s1_ok, _ = run_session("Session 1: Standard Hardware (Audio/USB Absent)", [], s1_expected, s1_unexpected)

    # Session 2: Attached Audio (Intel HDA) & USB (xHCI) Controller
    s2_extra = [
        "-device", "intel-hda",
        "-device", "hda-duplex",
        "-device", "qemu-xhci"
    ]
    s2_expected = [
        ("Storage Driver Matched", "PurpleDetect: Matched hardware for 'storage.apdrv'"),
        ("Audio Driver Matched", "PurpleDetect: Matched hardware for 'audio.apdrv'"),
        ("USB Driver Matched", "PurpleDetect: Matched hardware for 'usb.apdrv'"),
        ("Driver Load Summary (9 loaded, 0 skipped)", "PurpleDetect: Hardware-driven driver load complete: 9 loaded, 0 skipped"),
        ("LSMOD Has Audio", "audio                           RUNNING"),
        ("LSMOD Has USB", "usb                             RUNNING"),
    ]
    s2_unexpected = [
        ("No Skipped Audio Message", "No matching hardware detected for 'audio.apdrv'"),
        ("No Skipped USB Message", "No matching hardware detected for 'usb.apdrv'"),
    ]
    s2_ok, _ = run_session("Session 2: Attached Hardware (Intel HDA & xHCI USB Present)", s2_extra, s2_expected, s2_unexpected)

    if s1_ok and s2_ok:
        print("\n>>> ALL PURPLEDETECT HARDWARE MATCHING TESTS PASSED (100%)! <<<")
        return 0
    else:
        print("\n>>> SOME TESTS FAILED! <<<")
        return 1

if __name__ == "__main__":
    sys.exit(main())
