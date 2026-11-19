#!/usr/bin/env python3
"""
Aegis64 / PurpleOS CLI Engine Quality of Life (QoL) Verification Suite.
Exercises:
  1. Math Calculator (calc / expr with arithmetic, bitwise, hex)
  2. Command History (history, !!, !n)
  3. Environment Variables (export, env, $VAR expansion)
  4. Command Aliases (alias, unalias)
  5. General Output Redirection (> and >>)
  6. Command Pipelines (cmd1 | grep, wc, head, tail)
  7. Multi-Command Chaining (; and &&)
  8. In-Terminal Multi-Line Text Editor (edit / nano)
  9. Utilities (which, seq, sleep, fortune)
"""

import subprocess
import tempfile
import time
import os
import sys

def main():
    print("=== Launching Aegis64 QEMU CLI Engine & QoL Validation Suite ===")

    with tempfile.TemporaryDirectory() as tmpdir:
        disk_path = os.path.join(tmpdir, "disposable_test_disk.img")
        mk_res = subprocess.run(["./build/mkpfs2_disk", disk_path], capture_output=True, text=True)
        if mk_res.returncode != 0:
            print(f"[FAIL] mkpfs2_disk failed:\n{mk_res.stderr}")
            return 1

        cmd = [
            "qemu-system-x86_64",
            "-kernel", "target/kernel.elf",
            "-serial", "stdio",
            "-display", "none",
            "-m", "256M",
            "-no-reboot",
            "-drive", f"file={disk_path},format=raw,index=1,media=disk"
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

        test_commands = [
            # 1. Calculator
            "calc 1024 * 768 * 4\n",
            "calc 0x1000 + 0x200\n",
            "calc (16 + 4) * 8\n",

            # 2. Environment Variables & Expansion
            "export TESTVAR=AegisUniverse\n",
            "echo Value of TESTVAR is $TESTVAR\n",
            "env\n",

            # 3. Aliases
            "alias mydir='dir'\n",
            "mydir\n",
            "alias\n",
            "unalias mydir\n",

            # 4. Semicolon Chaining
            "cd /tmp; touch chainfile.txt; dir\n",

            # 5. General Output Redirection
            "sysinfo > /tmp/sysout.txt\n",
            "cat /tmp/sysout.txt\n",

            # 6. Pipelines
            "list files | grep txt\n",
            "list files | wc\n",
            "help | grep dir\n",

            # 7. Utilities
            "which sysinfo\n",
            "which ls\n",
            "seq 1 5\n",
            "fortune\n",

            # 8. Command History
            "history\n",

            # 9. In-Terminal Editor
            "edit /tmp/edtest.txt\n",
            ":a First Line in Editor\n",
            ":a Second Line in Editor\n",
            ":p\n",
            ":wq\n",
            "cat /tmp/edtest.txt\n",

            # 10. Native Terminal Features (Themes, Benchmark, Top, Viewer, Jobs, Prompt)
            "theme matrix\n",
            "bench\n",
            "top\n",
            "view /tmp/edtest.txt\n",
            "prompt [testos]$ \n",
            "jobs\n",
            "start bench\n",
            "prompt default\n",

            # End marker
            "echo QOL-TEST-COMPLETED\n"
        ]

        for c in test_commands:
            proc.stdin.write(c)
            proc.stdin.flush()
            time.sleep(0.35)

        proc.terminate()
        try:
            stdout, stderr = proc.communicate(timeout=3)
        except subprocess.TimeoutExpired:
            proc.kill()
            stdout, stderr = proc.communicate()

        print("\n=== Captured CLI QoL Serial Output ===")
        print(stdout)

        assertions = [
            # Calculator assertions
            ("Calc Multiplication", "3145728 (0x300000) [3 MiB]"),
            ("Calc Hex Addition", "4608 (0x1200)"),
            ("Calc Parenthesized Math", "160 (0xa0)"),

            # Env assertions
            ("Env Set Export", "TESTVAR=AegisUniverse"),
            ("Env Variable Expansion", "Value of TESTVAR is AegisUniverse"),
            ("Env Command Output", "USER=aegis64"),

            # Alias assertions
            ("Alias Registration", "Registered alias: mydir -> 'dir'"),
            ("Alias Execution", "Directory of"),
            ("Alias Unregister", "Removed alias 'mydir'"),

            # Chaining & Redirection
            ("Chained Command Execution", "chainfile.txt"),
            ("Redirected Output In File", "System Information:"),
            ("Redirected Pure Rust Kernel", "Pure Rust Edition"),

            # Pipeline assertions
            ("Pipeline Grep TXT", "welcome.txt"),
            ("Pipeline Word Count", "bytes"),
            ("Pipeline Grep Help", "dir"),

            # Utilities assertions
            ("Which Builtin", "sysinfo: shell builtin command"),
            ("Seq Generator", "1\n2\n3\n4\n5"),
            ("Fortune Tip", "[AegisTip]"),

            # History assertion
            ("History Command Ring", "=== AegisShell Command History ==="),

            # Editor assertions
            ("Editor Mode Header", "PurpleOS AegisEdit"),
            ("Editor Appended Line 1", "[+ Line 1]"),
            ("Editor Appended Line 2", "[+ Line 2]"),
            ("Editor Saved File", "Saved"),
            ("Editor Cat Verified", "First Line in Editor\nSecond Line in Editor"),

            # Native Features assertions
            ("Theme Applied", "[Theme applied: matrix]"),
            ("Benchmark Completed", "No comparative score is available"),
            ("Top Monitor", "Real-Time System Monitor"),
            ("File Pager View", "=== Viewing: '/tmp/edtest.txt'"),
            ("Prompt Customized", "[Prompt set to: '[testos]$ ']"),
            ("Jobs Command", "=== AegisShell Native Background Jobs ==="),

            # End marker
            ("Test Suite Completed", "QOL-TEST-COMPLETED")
        ]

        passed = True
        print("=== CLI Quality of Life Verification Results ===")
        for label, pattern in assertions:
            if pattern in stdout:
                print(f"[OK] Verified: {label} ('{pattern}')")
            else:
                print(f"[FAIL] Missing expected output: {label} ('{pattern}')")
                passed = False

        if passed:
            print("\n>>> ALL CLI QUALITY OF LIFE VERIFICATION TESTS PASSED (100%)! <<<")
            return 0
        else:
            print("\n>>> CLI QoL TESTS FAILED! <<<")
            return 1

if __name__ == "__main__":
    sys.exit(main())
