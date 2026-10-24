#!/usr/bin/env python3
import subprocess
import time
import sys

def main():
    print("=== Launching AegisSched_64 Preemption Test ===")
    cmd = [
        "qemu-system-x86_64",
        "-kernel", "build/kernel.elf",
        "-serial", "stdio",
        "-display", "none",
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

    time.sleep(1.0)

    # Spawn test tasks
    proc.stdin.write("task test\n")
    proc.stdin.flush()
    time.sleep(0.5)

    # Print tasks table while running
    proc.stdin.write("tasks\n")
    proc.stdin.flush()
    time.sleep(0.8)

    # Spawn custom task and kill it
    proc.stdin.write("task spawn KillTarget\n")
    proc.stdin.flush()
    time.sleep(0.3)

    proc.stdin.write("tasks\n")
    proc.stdin.flush()
    time.sleep(0.3)

    proc.stdin.write("task kill 6\n") # Task 6 is KillTarget (1=kernel, 2=idle, 3=Task_A, 4=Task_B, 5=Task_C, 6=KillTarget)
    proc.stdin.flush()
    time.sleep(0.5)

    proc.stdin.write("tasks\n")
    proc.stdin.flush()
    time.sleep(0.5)

    proc.terminate()
    try:
        stdout, stderr = proc.communicate(timeout=2)
    except subprocess.TimeoutExpired:
        proc.kill()
        stdout, stderr = proc.communicate()

    print("=== Scheduler Test Captured Output ===")
    print(stdout)

    # Assertions
    required = [
        "Spawning automated scheduler test tasks",
        "TaskWorker [Task_A]: iteration 1/3",
        "TaskWorker [Task_B]: iteration 1/3",
        "TaskWorker [Task_C]: iteration 1/3",
        "TaskWorker [Task_A]: finished execution",
        "TaskWorker [Task_B]: finished execution",
        "TaskWorker [Task_C]: finished execution",
        "Spawned background task 'KillTarget'",
        "Terminated task 6"
    ]

    passed = True
    for item in required:
        if item in stdout:
            print(f"[OK] Verified: {item}")
        else:
            print(f"[FAIL] Missing expected scheduler event: '{item}'")
            passed = False

    if passed:
        print("\n>>> ALL AEGISSCHED_64 PREEMPTION & LIFECYCLE TESTS PASSED! <<<")
        return 0
    else:
        print("\n>>> SCHEDULER PREEMPTION TESTS FAILED! <<<")
        return 1

if __name__ == "__main__":
    sys.exit(main())
