#!/usr/bin/env python3
import subprocess
import time
import sys
import os

def main():
    print("=================================================================")
    print("  PURPLE OS / AEGIS64 — NORMAL USER EXPERIENCE & BUG DISCOVERY   ")
    print("=================================================================")

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

    time.sleep(2.5)

    user_scenarios = [
        # Scenario A: Initial Orientation
        ("pwd", "Check initial working directory"),
        ("ls", "List root directory files using Linux ls"),
        ("dir", "List root directory files using MS-DOS dir"),
        
        # Scenario B: Relative Navigation into Virtual Mounts
        ("cd dev", "Try relative navigation 'cd dev' from /"),
        ("pwd", "Check pwd after 'cd dev'"),
        ("ls", "Try 'ls' inside /dev"),
        ("dir", "Try 'dir' inside /dev"),
        ("cd ..", "Navigate back with 'cd ..'"),
        ("pwd", "Check pwd after 'cd ..'"),

        # Scenario C: The dot navigation bug ('cd .')
        ("cd .", "Try 'cd .' (should stay in current directory)"),
        ("pwd", "Check pwd after 'cd .'"),
        ("cd .", "Try second 'cd .'"),
        ("pwd", "Check pwd after second 'cd .'"),
        ("cd /", "Reset to root with 'cd /'"),
        ("pwd", "Verify reset to /"),

        # Scenario D: Trailing Slash Navigation
        ("cd /dev/", "Navigate with trailing slash 'cd /dev/'"),
        ("pwd", "Check pwd after 'cd /dev/'"),
        ("cd ..", "Try 'cd ..' from '/dev/'"),
        ("pwd", "Check pwd (did it return to / or stay in /dev?)"),
        ("cd /", "Reset to root"),

        # Scenario E: In-Memory /proc Inspection
        ("cd /proc", "Change directory to /proc"),
        ("pwd", "Check pwd in /proc"),
        ("ls", "List files in /proc"),
        ("cat version", "Read /proc/version"),
        ("cat cpuinfo", "Read /proc/cpuinfo"),
        ("cat meminfo", "Read /proc/meminfo"),
        ("cat uptime", "Read /proc/uptime"),
        ("cat tasks", "Read /proc/tasks"),
        ("cd /", "Reset to root"),

        # Scenario F: Tmpfs RAM Disk Behavior & Relative Path Resolution
        ("cd /tmp", "Change directory to /tmp"),
        ("pwd", "Check pwd in /tmp"),
        ("echo \"test in tmp\" > note_abs.txt", "Write file in /tmp with relative name"),
        ("cat note_abs.txt", "Attempt to read back relative file in /tmp"),
        ("cat /tmp/note_abs.txt", "Attempt to read back absolute path in /tmp"),
        ("cd /", "Return to root"),
        ("cat note_abs.txt", "Check if note_abs.txt was actually placed in / instead of /tmp!"),

        # Scenario G: Creating and navigating user directories
        ("remount rw", "Remount filesystem read-write"),
        ("mkdir myproject", "User tries to make a directory 'mkdir myproject'"),
        ("cd myproject", "User attempts to enter the created directory 'cd myproject'"),
        ("pwd", "Check if 'cd myproject' worked"),
        ("cd /", "Return to root"),

        # Scenario H: Edge Cases & Error Handling
        ("cd /nonexistent_folder_abc", "Try to cd into a non-existent directory"),
        ("pwd", "Did cd allow entering a non-existent directory?"),
        ("cd /", "Return to root"),
        ("cd welcome.txt", "Try to cd into a regular file 'cd welcome.txt'"),
        ("pwd", "Did cd allow entering a regular file?"),
        ("cd /", "Return to root"),
        ("cat /dev", "Try to cat a directory 'cat /dev'"),
        ("cat", "Try 'cat' with no arguments"),
        ("stat", "Try 'stat' with no arguments"),
        ("copy", "Try 'copy' with no arguments"),
        ("delete", "Try 'delete' with no arguments"),
        ("mount", "Check mount command output vs actual hardware devices")
    ]

    print(f"\n[USER SIMULATION] Running {len(user_scenarios)} realistic user interactions...\n")

    logs = []
    for idx, (cmd_str, desc) in enumerate(user_scenarios, start=1):
        marker = f"__SCENARIO_MARKER_{idx}__"
        start_idx = len(captured_chunks)

        for ch in f"{cmd_str}\n":
            proc.stdin.write(ch)
            proc.stdin.flush()
            time.sleep(0.003)

        for ch in f"echo {marker}\n":
            proc.stdin.write(ch)
            proc.stdin.flush()
            time.sleep(0.003)

        deadline = time.time() + 3.0
        while time.time() < deadline:
            out_so_far = "".join(captured_chunks[start_idx:])
            if marker in out_so_far:
                break
            time.sleep(0.05)

        raw_output = "".join(captured_chunks[start_idx:])
        # Strip the echoed command and marker
        lines = [l.strip() for l in raw_output.splitlines() if l.strip()]
        filtered = [l for l in lines if not l.startswith(cmd_str) and not marker in l and not "aegis64@purple" in l]
        result_text = "\n".join(filtered)

        print(f"[{idx:02d}] $ {cmd_str}")
        print(f"     Purpose: {desc}")
        print(f"     Output:  {result_text.replace(chr(10), ' | ') if result_text else '(empty)'}")
        print("-" * 65)
        logs.append((idx, cmd_str, desc, result_text))

    time.sleep(0.5)
    proc.terminate()
    try:
        proc.wait(timeout=2)
    except subprocess.TimeoutExpired:
        proc.kill()

    print("\n[USER SIMULATION COMPLETED]")

if __name__ == "__main__":
    main()
