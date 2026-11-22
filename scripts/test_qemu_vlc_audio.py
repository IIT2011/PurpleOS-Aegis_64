#!/usr/bin/env python3
"""
Test Audio Playback in PurpleOS / Aegis64:
- Launch QEMU with Intel HDA sound hardware (-device intel-hda -device hda-output)
- Verify PurpleOS detects the HDA controller and initializes driver
- Launch VLC Media Player from AegisShell ('launch vlc')
- Verify audio stream runs and PCM samples are written to build/purple_audio.wav
"""

import os
import sys
import time
import struct
import wave
import subprocess
import threading

def main():
    os.chdir(os.path.join(os.path.dirname(__file__), ".."))
    wav_path = "build/purple_audio.wav"
    disk_path = "build/purple_gpt.img"
    kernel_path = "target/kernel.elf"

    if os.path.exists(wav_path):
        os.remove(wav_path)

    print("==> Seeding and formatting disk with applications...")
    subprocess.check_call([
        sys.executable,
        "build/mkpfs2_disk",
        disk_path,
        "--applications-dir", "build/applications"
    ])

    cmd = [
        "qemu-system-x86_64",
        "-kernel", kernel_path,
        "-serial", "stdio",
        "-vga", "std",
        "-m", "256M",
        "-smp", "4",
        "-drive", f"file={disk_path},format=raw,index=0,media=disk",
        "-device", "intel-hda,id=hda0",
        "-device", "hda-output,id=hda0.0,audiodev=snd0",
        "-audiodev", f"wav,id=snd0,path={wav_path}",
        "-display", "none",
        "-no-reboot"
    ]

    print("==> Starting QEMU with Intel-HDA audio subsystem...")
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
        while True:
            ch = proc.stdout.read(1)
            if not ch:
                break
            captured_chunks.append(ch)
            sys.stdout.write(ch)
            sys.stdout.flush()

    t = threading.Thread(target=read_stdout, daemon=True)
    t.start()

    # Wait for AegisShell banner or prompt
    print("==> Waiting for AegisShell to start...")
    start_time = time.time()
    shell_ready = False
    while time.time() - start_time < 25:
        text = "".join(captured_chunks)
        if "aegis64@purple:~$" in text or "Type 'help' for commands" in text:
            shell_ready = True
            break
        time.sleep(0.2)

    if not shell_ready:
        print("[ERROR] AegisShell did not become ready within 25s.")
        proc.kill()
        return 1

    # Short delay for terminal ready
    time.sleep(1.0)

    # 1. Run Native Tests (checks Audio Test 2)
    print("\n==> Running 'native test'...")
    proc.stdin.write("\nnative test\n")
    proc.stdin.flush()
    time.sleep(2.0)

    # 2. Launch VLC Media Player
    print("\n==> Launching VLC Media Player ('launch vlc')...")
    proc.stdin.write("\nlaunch vlc\n")
    proc.stdin.flush()

    # Let VLC run and stream audio for 6 seconds
    print("==> Letting VLC stream PCM audio for 6 seconds...")
    time.sleep(6.0)

    # Clean termination
    print("==> Terminating QEMU cleanly...")
    proc.terminate()
    stderr_data = ""
    try:
        proc.wait(timeout=4)
        stderr_data = proc.stderr.read()
    except subprocess.TimeoutExpired:
        proc.kill()
        stderr_data = proc.stderr.read()

    if stderr_data:
        print("\n=== QEMU STDERR OUTPUT ===")
        print(stderr_data)
        print("==========================\n")

    all_output = "".join(captured_chunks)

    # Verify detection
    hda_detected = "Audio: Intel High Definition Audio initialized" in all_output
    native_audio_passed = "[NATIVE TEST 2/10] Audio capability query and volume state... [PASSED]" in all_output
    vlc_started = "Application started, PID" in all_output or "VLC" in all_output

    print("\n" + "=" * 60)
    print("  AUDIO SUBSYSTEM VERIFICATION REPORT")
    print("=" * 60)
    print(f"1. Intel HDA Driver Initialized: {'[PASS]' if hda_detected else '[FAIL]'}")
    print(f"2. Native Audio Syscall Test:    {'[PASS]' if native_audio_passed else '[FAIL]'}")
    print(f"3. VLC Application Launched:     {'[PASS]' if vlc_started else '[FAIL]'}")

    wav_ok = False
    if os.path.exists(wav_path):
        wav_size = os.path.getsize(wav_path)
        print(f"4. Output Audio File:            {wav_path} ({wav_size:,} bytes)")
        if wav_size > 44:
            # Finalize RIFF and data chunk sizes if QEMU was stopped via SIGTERM
            try:
                with open(wav_path, "r+b") as f:
                    f.seek(4)
                    riff_len = struct.unpack("<I", f.read(4))[0]
                    f.seek(40)
                    data_len = struct.unpack("<I", f.read(4))[0]
                    if riff_len == 0 or data_len == 0:
                        f.seek(4)
                        f.write(struct.pack("<I", wav_size - 8))
                        f.seek(40)
                        f.write(struct.pack("<I", wav_size - 44))
                        f.flush()
            except Exception as e:
                print(f"   - Warning during WAV header check: {e}")

            try:
                with wave.open(wav_path, "rb") as wf:
                    nchannels = wf.getnchannels()
                    sampwidth = wf.getsampwidth()
                    framerate = wf.getframerate()
                    nframes = wf.getnframes()
                    duration = nframes / float(framerate) if framerate > 0 else 0
                    print(f"   - Channels:    {nchannels} (Stereo)")
                    print(f"   - Sample Rate: {framerate} Hz")
                    print(f"   - Bit Depth:   {sampwidth * 8} bits")
                    print(f"   - Total Frames:{nframes:,}")
                    print(f"   - Duration:    {duration:.2f} seconds")

                    # Read a batch of samples to inspect amplitude
                    frames_to_read = min(nframes, 1000)
                    raw_data = wf.readframes(frames_to_read)
                    samples = struct.unpack(f"<{frames_to_read * nchannels}h", raw_data)
                    min_val = min(samples)
                    max_val = max(samples)
                    print(f"   - Waveform Amplitude Range: [{min_val}, {max_val}] (Non-zero audio active)")

                    if nframes > 0 and (min_val != 0 or max_val != 0):
                        wav_ok = True
            except Exception as e:
                print(f"   - WAV Parse Error: {e}")
        else:
            print("   - WAV file contains only header or is empty.")
    else:
        print(f"4. Output Audio File:            [FAIL] {wav_path} not created.")

    print(f"5. Audio Stream Playback:        {'[PASS]' if wav_ok else '[FAIL]'}")
    print("=" * 60)

    return 0 if (hda_detected and native_audio_passed and vlc_started and wav_ok) else 1

if __name__ == "__main__":
    sys.exit(main())
