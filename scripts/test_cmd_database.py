#!/usr/bin/env python3
"""
Test Suite for PurpleOS Command Database Subsystem (/System/CLI/cmd.dtbase).
Verifies:
  1. Boot-time SHA-256 cryptographic authentication of /System/CLI/cmd.dtbase.
  2. Loading of all dynamic command definitions into kernel dispatcher.
  3. `cmd info` reporting path, count, and SHA-256 verification status.
  4. `cmd verify` integrity verification.
  5. `help` displaying dynamic categorized commands from /System/CLI/cmd.dtbase.
  6. `help <cmd>` displaying manual entry from database.
  7. `which <cmd>` resolving through /System/CLI/cmd.dtbase.
  8. `cmd reload` dynamic hot-reloading from storage.
"""

import subprocess
import tempfile
import time
import os
import sys

def main():
    print("=================================================================")
    print("  PURPLE OS / AEGIS64 — COMMAND DATABASE (/System/CLI/cmd.dtbase)")
    print("=================================================================")

    with tempfile.TemporaryDirectory(dir="build") as tmpdir:
        disk_path = os.path.join(tmpdir, "cmddb_test_disk.img")
        subprocess.check_call([sys.executable, "build/mkpfs2_disk", disk_path])

        cmd = [
            "qemu-system-x86_64",
            "-kernel", "target/kernel.elf",
            "-serial", "stdio",
            "-display", "none",
            "-m", "256M",
            "-smp", "4",
            "-no-reboot",
            "-drive", f"file={disk_path},format=raw,index=0,media=disk"
        ]

        proc = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=0
        )

        time.sleep(2.5)

        test_commands = [
            "cmd info\n",
            "cmd verify\n",
            "help\n",
            "help ls\n",
            "help ring3\n",
            "which cat\n",
            "which ring3\n",
            "cmd reload\n",
            "echo CMDDB-TESTS-COMPLETED\n",
        ]

        for tc in test_commands:
            proc.stdin.write(tc)
            proc.stdin.flush()
            time.sleep(0.3)

        time.sleep(1.0)
        proc.terminate()
        try:
            stdout, _ = proc.communicate(timeout=3)
        except Exception:
            proc.kill()
            stdout, _ = proc.communicate()

    print("=== Captured Output ===")
    print(stdout)

    assertions = [
        ("Boot SHA-256 Verification", "[CMDDB] Authenticated /System/CLI/cmd.dtbase"),
        ("100% Integrity Verified", "100% Integrity Verified!"),
        ("Loaded Command Count", "commands into kernel execution dispatcher"),
        ("cmd info Database Path", "Database Path:     /System/CLI/cmd.dtbase"),
        ("cmd info SHA-256 Status", "SHA-256 Verified:  YES (Cryptographically Authenticated)"),
        ("cmd verify Status", "/System/CLI/cmd.dtbase SHA-256 Integrity: 100% VERIFIED"),
        ("Dynamic Help Header", "AegisShell / PurpleOS Commands (Defined in /System/CLI/cmd.dtbase):"),
        ("Dynamic Help Navigation", "Directory & Path Navigation:"),
        ("Dynamic Help Manual LS", "PurpleOS Command Manual: ls"),
        ("Dynamic Help Manual Ring3", "PurpleOS Command Manual: ring3"),
        ("Which Cat Builtin", "cat: shell builtin command (defined in /System/CLI/cmd.dtbase)"),
        ("Which Ring3 Userland", "ring3: Ring 3 user mode application (defined in /System/CLI/cmd.dtbase)"),
        ("Dynamic Hot-Reload", "Successfully reloaded"),
        ("Suite Completion", "CMDDB-TESTS-COMPLETED"),
    ]

    all_passed = True
    print("=== Assertion Results ===")
    for label, pattern in assertions:
        if pattern in stdout:
            print(f"[OK] {label} -> PASSED")
        else:
            print(f"[FAIL] {label} -> MISSING: '{pattern}'")
            all_passed = False

    if all_passed:
        print("\n>>> ALL COMMAND DATABASE VERIFICATION TESTS PASSED (100%)! <<<")
        return 0
    else:
        print("\n>>> SOME TESTS FAILED <<<")
        return 1

if __name__ == "__main__":
    sys.exit(main())
