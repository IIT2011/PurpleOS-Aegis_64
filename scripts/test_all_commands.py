#!/usr/bin/env python3
import subprocess
import time
import sys
import os

def main():
    print("=================================================================")
    print("  PURPLE OS / AEGIS64 — EXHAUSTIVE SHELL COMMAND VALIDATION TEST ")
    print("=================================================================")

    # Ensure disk.img exists
    disk_path = "build/disk.img"
    if not os.path.exists(disk_path):
        print(f"Generating PurpleFS GPT disk at {disk_path}...")
        subprocess.check_call([sys.executable, "build/mkpfs2_disk", disk_path])

    cmd = [
        "qemu-system-x86_64",
        "-kernel", "target/kernel.elf",
        "-serial", "stdio",
        "-display", "none",
        "-m", "256M",
        "-smp", "4",
        "-drive", f"file={disk_path},format=raw,index=0,media=disk",
        "-device", "intel-hda",
        "-device", "hda-duplex",
        "-netdev", "user,id=u1",
        "-device", "e1000,netdev=u1",
        "-no-reboot"
    ]

    import threading
    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=0
    )

    captured_chunks = []
    def read_stdout():
        for line in iter(proc.stdout.readline, ''):
            captured_chunks.append(line)
    t = threading.Thread(target=read_stdout, daemon=True)
    t.start()

    # Allow kernel to boot and storage to mount
    print("[INIT] Waiting 2.5s for kernel boot and hardware initialization...")
    time.sleep(2.5)

    # List of test commands with brief descriptions and expected response tokens
    test_suite = [
        # Group 1: Shell Help & Versioning
        ("help", "AegisShell / Purple Shell Commands"),
        ("?", "Directory & Navigation:"),
        ("ver", "Purple OS / Aegis64 Kernel"),
        ("version", "CLI Environment: AegisShell 2.5"),
        ("uname", "x86_64-unknown-none, Pure Rust"),
        ("whoami", "root (uid=0 gid=0 groups=0(root))"),
        ("id", "Ring 0 Kernel Supervisor"),

        # Group 2: Time & System Information
        ("date", "Current CMOS RTC Date & Time: 20"),
        ("time", "UTC"),
        ("sysinfo", "OS Name:     Purple OS (Aegis64 Kernel)"),
        ("cpuinfo", "CPU Information:"),
        ("mem", "Memory Accounting & Physical Memory Status (PMM):"),
        ("memory", "Total Managed: 4096 MiB"),
        ("free", "Free Memory:"),
        ("uptime", "Uptime:"),

        # Group 3: Directory & Navigation
        ("pwd", "/"),
        ("dir", "Volume in drive is PurpleSystem"),
        ("ls", "Directory Listing of / (PurpleFS Root):"),
        ("ls -l", "Directory Listing of / (PurpleFS Root):"),
        ("tree", "Folder PATH listing for Volume PurpleSystem"),
        ("cd /dev", ""),
        ("pwd", "/dev"),
        ("dir", "Directory of /dev"),
        ("cd /proc", ""),
        ("pwd", "/proc"),
        ("dir", "Directory of /proc"),
        ("cd /tmp", ""),
        ("pwd", "/tmp"),
        ("dir", "Directory of /tmp"),
        ("cd /", ""),
        ("pwd", "/"),
        ("mkdir testdir", "Directory 'testdir' registered in VFS namespace"),
        ("rmdir testdir", "Directory 'testdir' removed"),

        # Group 4: File Viewing & Metadata
        ("stat welcome.txt", "File:     /welcome.txt"),
        ("cat welcome.txt", "Welcome to Purple OS"),
        ("type welcome.txt", "Welcome to Purple OS"),
        ("read welcome.txt", "Welcome to Purple OS"),
        ("peek welcome.txt", "Welcome to Purple OS"),
        ("head welcome.txt 1", "Welcome to Purple OS"),
        ("tail welcome.txt 1", "Welcome to Purple OS"),
        ("grep Welcome welcome.txt", "Welcome to Purple OS"),
        ("find \"Welcome\" in welcome.txt", "Welcome to Purple OS"),
        ("wc welcome.txt", "welcome.txt"),
        ("dump welcome.txt", "Hexdump of /welcome.txt"),

        # Group 5: File Operations & Redirection
        ("remount rw", "PurpleFS volume remounted READ-WRITE"),
        ("touch newfile.txt", "Successfully created file 'newfile.txt'"),
        ("write newfile.txt TestingWriteCommand", "Successfully wrote"),
        ("cat newfile.txt", "TestingWriteCommand"),
        ("append newfile.txt MoreTestingData", "Successfully appended to 'newfile.txt'"),
        ("cat newfile.txt", "TestingWriteCommandMoreTestingData"),
        ("echo DirectEchoMessage", "DirectEchoMessage"),
        ("echo EchoToFileContent > echofile.txt", "Successfully wrote"),
        ("cat echofile.txt", "EchoToFileContent"),
        ("echo AppendedEchoContent >> echofile.txt", "Successfully appended to 'echofile.txt'"),
        ("cat echofile.txt", "EchoToFileContentAppendedEchoContent"),
        ("copy echofile.txt to copied.txt", "Successfully copied 'echofile.txt' to 'copied.txt'"),
        ("cat copied.txt", "EchoToFileContentAppendedEchoContent"),
        ("move copied.txt to moved.txt", "Successfully moved 'copied.txt' to 'moved.txt'"),
        ("cat moved.txt", "EchoToFileContentAppendedEchoContent"),
        ("delete moved.txt", "Successfully removed 'moved.txt'"),
        ("del echofile.txt", "Successfully removed 'echofile.txt'"),
        ("rm newfile.txt", "Successfully removed 'newfile.txt'"),
        ("remount ro", "PurpleFS volume remounted READ-ONLY"),

        # Group 6: Storage & Filesystem Status
        ("disks", "ATA Storage Devices & GPT Partition Status:"),
        ("disk status", "sectors"),
        ("disk list", "sectors"),
        ("df", "Filesystem Status (PurpleFS v3 / v2):"),
        ("filesystem status", "Total Blocks:"),
        ("mount", "Active VFS Mountpoints & Filesystems:"),
        ("sync", "Filesystem transactions committed"),
        ("list files", "Directory Listing of / (PurpleFS Root):"),

        # Group 7: Hardware & Devices (PurpleDetect, PCI, Display, Network)
        ("devices", "PurpleDetect Hardware & Removable Device Registry"),
        ("hwinfo", "PurpleDetect Hardware & Removable Device Registry"),
        ("pci", "PCI Bus Device Inventory:"),
        ("lspci", "PCI Bus Device Inventory:"),
        ("mouse", "PS/2 Mouse Subsystem Active"),
        ("video", "Video Display Configuration:"),
        ("video mode 1024 768", "Video mode switched to 1024x768@32bpp"),
        ("3d", "Purple OS 3D Hardware Acceleration Engine"),
        ("gpu3d", "Purple OS 3D Hardware Acceleration Engine"),
        ("ifconfig", "Network Interface Configuration:"),
        ("ping 10.0.2.2", "PING 10.0.2.2"),
        ("ping to 10.0.2.2", "PING 10.0.2.2"),

        # Group 8: Driver Modules, Symbols & Security
        ("modules", "APDRV v3 Loaded Native Modules"),
        ("lsmod", "APDRV v3 Loaded Native Modules"),
        ("insmod e1000", "APDRV: Module 'e1000'"),
        ("load module e1000", "APDRV: Module 'e1000'"),
        ("ksym", "Kernel Symbol Table"),
        ("aegisdetect", "AegisDetect Boot Integrity & System Protection Subsystem:"),
        ("verify security", "AegisDetect Boot Integrity & System Protection Subsystem:"),

        # Group 9: Multiprocessing (SMP) & Task Management
        ("smp", "Symmetric Multiprocessing (SMP) Topology:"),
        ("smptest", "Concurrent worker validation is not implemented."),
        ("tasks", "AegisSched_64 Active Tasks"),
        ("ps", "AegisSched_64 Active Tasks"),
        ("tasks spawn TestWorker", "Spawned background task 'TestWorker'"),
        ("tasks user UserWorker", "Spawned isolated user task 'UserWorker'"),
        ("tasks test", "Spawning automated scheduler test tasks"),
        ("tasks", "TestWorker"),

        # Group 10: POSIX Subsystem & Validation
        ("posix status", "Aegis64 POSIX 2024 / IEEE Std 1003.1 Subsystem:"),
        ("posix test", ">>> ALL 12 POSIX SUBSYSTEM VALIDATION TESTS PASSED"),
        ("posixtest", ">>> ALL 12 POSIX SUBSYSTEM VALIDATION TESTS PASSED"),

        # Group 11: PurpleOS Native System Call Subsystem & 68 Syscalls
        ("native status", "Aegis64 / PurpleOS Native System Call Subsystem:"),
        ("native list", "Aegis64 / PurpleOS Native System Call Catalog (68 Total):"),
        ("native test", "Native Syscall Test Summary: 10/10 Subsystem Categories PASSED"),

        # Group 11: Natural Grammar Aliases
        ("show disks", "ATA Storage Devices & GPT Partition Status:"),
        ("show filesystem", "Filesystem Status (PurpleFS v3 / v2):"),
        ("show memory", "Memory Accounting & Physical Memory Status (PMM):"),
        ("show cpu", "CPU Information:"),
        ("show system", "OS Name:     Purple OS"),
        ("show uptime", "Uptime:"),
        ("show time", "Current CMOS RTC Date & Time:"),
        ("show tasks", "AegisSched_64 Active Tasks"),
        ("show devices", "PurpleDetect Hardware & Removable Device Registry"),
        ("show pci", "PCI Bus Device Inventory:"),
        ("show network", "Network Interface Configuration:"),
        ("show modules", "APDRV v3 Loaded Native Modules"),
        ("show symbols", "Kernel Symbol Table"),
        ("show video", "Video Display Configuration:"),
        ("show 3d", "Purple OS 3D Hardware Acceleration Engine"),
        ("show mouse", "PS/2 Mouse Subsystem Active"),
        ("show files", "Directory Listing of / (PurpleFS Root):"),

        # Group 12: Console & Error Handling
        ("clear", ""),
        ("cls", ""),
        ("nonexistent_command_xyz", "Unknown command: 'nonexistent_command_xyz'")
    ]

    print(f"\n[EXEC] Executing {len(test_suite)} commands sequentially in real-time...\n")

    results = []
    for idx, (command_str, expected_tok) in enumerate(test_suite, start=1):
        # Marker to delimit command output
        marker = f"__CMD_MARKER_{idx}__"
        send_line = f"{command_str}\n"

        # Record where in the captured output we are
        start_idx = len(captured_chunks)

        # Send command characters with slight delay
        for ch in send_line:
            proc.stdin.write(ch)
            proc.stdin.flush()
            time.sleep(0.003)

        # Send echo marker
        marker_line = f"echo {marker}\n"
        for ch in marker_line:
            proc.stdin.write(ch)
            proc.stdin.flush()
            time.sleep(0.003)

        # Wait for marker to appear in output
        deadline = time.time() + 3.0
        found_marker = False
        while time.time() < deadline:
            out_so_far = "".join(captured_chunks[start_idx:])
            if marker in out_so_far:
                found_marker = True
                break
            time.sleep(0.05)

        cmd_output = "".join(captured_chunks[start_idx:])

        # Verify expected token
        passed = True
        if expected_tok:
            if expected_tok in cmd_output:
                status = "PASS"
            else:
                status = "FAIL"
                passed = False
        else:
            status = "PASS"

        results.append((idx, command_str, status, expected_tok))
        sym = "[+]" if passed else "[-]"
        print(f"  {sym} #{idx:02d} | '{command_str}' -> {status}")

    time.sleep(0.5)
    proc.terminate()
    try:
        proc.wait(timeout=2)
    except subprocess.TimeoutExpired:
        proc.kill()

    print("\n=================================================================")
    print("                    TEST EXECUTION SUMMARY                       ")
    print("=================================================================")
    total = len(results)
    passed_count = sum(1 for _, _, s, _ in results if s == "PASS")
    failed_count = total - passed_count

    print(f"Total Commands Tested: {total}")
    print(f"Passed:                {passed_count} / {total} ({passed_count/total*100:.1f}%)")
    print(f"Failed:                {failed_count} / {total}")

    if failed_count > 0:
        print("\nFailed Commands:")
        for idx, cmd_str, status, exp in results:
            if status != "PASS":
                print(f"  - #{idx:02d} '{cmd_str}' (Expected: '{exp}')")
        return 1
    else:
        print("\n>>> ALL TESTED COMMANDS PASSED WITH 100% SUCCESS! <<<")
        return 0

if __name__ == "__main__":
    sys.exit(main())
