#!/usr/bin/env python3
"""
Test Suite for PurpleOS Command & Path Databases:
  - /System/CLI/cmd.dtbase (Cryptographically Signed by Creator)
  - /System/CLI/path.dtbase (Unrestricted User Search Paths)
"""

import subprocess
import tempfile
import time
import os
import sys

def main():
    print("=================================================================")
    print("  PURPLE OS / AEGIS64 — CMD & PATH DATABASE VERIFICATION SUITE   ")
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

        import threading
        captured_chunks = []
        def read_stdout():
            for line in iter(proc.stdout.readline, ''):
                captured_chunks.append(line)
        t = threading.Thread(target=read_stdout, daemon=True)
        t.start()

        time.sleep(3.5)

        test_commands = [
            "cmd info\n",
            "cmd verify\n",
            "path\n",
            "path add /System/TestBin\n",
            "path\n",
            "path remove /System/TestBin\n",
            "path reload\n",
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

        time.sleep(1.5)
        proc.terminate()
        try:
            proc.wait(timeout=3)
        except Exception:
            proc.kill()
        t.join(timeout=2)
        stdout = "".join(captured_chunks)

    print("=== Captured Output ===")
    print(stdout)

    assertions = [
        ("Boot Creator Signature Verification", "[CMDDB] Authenticated /System/CLI/cmd.dtbase — Cryptographically Signed by Creator!"),
        ("Boot Verified Signer", "[CMDDB] Verified Signer: Creator (Aegis Root Developer)"),
        ("Boot Loaded Command Count", "commands into kernel execution dispatcher"),
        ("Boot Path Database Loaded", "search path(s) from /System/CLI/path.dtbase (Unrestricted User Configuration)"),
        ("cmd info Database Path", "Database Path:     /System/CLI/cmd.dtbase"),
        ("cmd info Creator Signed", "Creator Signed:    YES (Cryptographically Signed by Creator)"),
        ("cmd verify Status", "/System/CLI/cmd.dtbase Creator Signature: 100% VERIFIED"),
        ("path Command Execution", "PurpleOS Search Path Database (/System/CLI/path.dtbase)"),
        ("path Search Hierarchy", "Active Search Hierarchy:"),
        ("path System Applications", "/System/Applications"),
        ("path Add Directory", "[PATHDB] Added '/System/TestBin' to active search paths."),
        ("path Remove Directory", "[PATHDB] Removed '/System/TestBin' from active search paths."),
        ("path Reload Command", "Successfully reloaded"),
        ("Dynamic Help Manual LS", "PurpleOS Command Manual: ls"),
        ("Dynamic Help Manual Ring3", "PurpleOS Command Manual: ring3"),
        ("Which Cat Builtin", "cat: shell builtin command (defined in /System/CLI/cmd.dtbase)"),
        ("Which Ring3 Userland", "ring3: Ring 3 user mode application (defined in /System/CLI/cmd.dtbase)"),
        ("cmd Dynamic Hot-Reload", "Successfully reloaded"),
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
        print("\n>>> ALL CMD & PATH DATABASE VERIFICATION TESTS PASSED (100%)! <<<")
        return 0
    else:
        print("\n>>> SOME TESTS FAILED <<<")
        return 1

if __name__ == "__main__":
    sys.exit(main())
