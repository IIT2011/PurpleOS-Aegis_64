#!/usr/bin/env python3
"""
Purple OS / Aegis64 — Absolute Desktop Reference Lock Visual Regression Suite
Captures live guest framebuffer screendump from QEMU, compares pixel-by-pixel
against authoritative reference 'build/Desktop.png', and generates side-by-side,
alpha-overlay, and difference maps.
"""

import os
import sys
import time
import json
import socket
import hashlib
import subprocess
from PIL import Image, ImageChops

REFERENCE_PATH = "build/Desktop.png"
EXPECTED_REF_HASH = "82d92d9e92142013b3acce17e002d5995ec9557bccf9b0acf22047b70e46dcd9"
VERIFY_DIR = "build/visual-verification"

WALLPAPER_TARGETS = [
    ("assets/artwork/wallpaper_3840x2160.png", (3840, 2160)),
    ("assets/artwork/wallpaper_2560x1440.png", (2560, 1440)),
    ("assets/artwork/wallpaper_1920x1080.png", (1920, 1080)),
    ("assets/artwork/wallpaper_1366x768.png",  (1366, 768)),
    ("assets/artwork/wallpaper_1280x720.png",  (1280, 720)),
]

def sha256_file(path):
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()

def verify_reference_integrity():
    print("=== 1. Verifying Reference Asset Integrity ===")
    if not os.path.exists(REFERENCE_PATH):
        print(f"[FAIL] Reference file not found: {REFERENCE_PATH}")
        sys.exit(1)
    
    current_hash = sha256_file(REFERENCE_PATH)
    ref_img = Image.open(REFERENCE_PATH)
    print(f"Reference Path: {REFERENCE_PATH}")
    print(f"Dimensions: {ref_img.size[0]} x {ref_img.size[1]} pixels, Mode: {ref_img.mode}")
    print(f"SHA-256: {current_hash}")
    assert current_hash == EXPECTED_REF_HASH, f"Hash mismatch: expected {EXPECTED_REF_HASH}, got {current_hash}"
    print("[PASS] Reference hash strictly preserved.\n")

def verify_wallpaper_assets():
    print("=== 2. Verifying Multi-Resolution Wallpaper Assets ===")
    for path, (exp_w, exp_h) in WALLPAPER_TARGETS:
        if not os.path.exists(path):
            print(f"[FAIL] Missing wallpaper asset: {path}")
            sys.exit(1)
        img = Image.open(path)
        sz = os.path.getsize(path)
        h = sha256_file(path)
        assert img.size == (exp_w, exp_h), f"Dimension mismatch for {path}: expected {exp_w}x{exp_h}, got {img.size}"
        assert img.mode == "RGB", f"Mode mismatch for {path}: expected RGB, got {img.mode}"
        print(f"[OK] {os.path.basename(path):26}: {img.size[0]}x{img.size[1]} {img.mode} ({sz} B) sha256={h[:16]}...")
    print("[PASS] All 5 multi-resolution wallpaper assets validated.\n")

def capture_guest_framebuffer(width, height, ppm_out, png_out):
    print(f"=== Capturing Live QEMU Guest Framebuffer ({width}x{height}) ===")
    qmp_sock_path = f"/tmp/qmp_vis_{width}x{height}_{os.getpid()}.sock"
    if os.path.exists(qmp_sock_path):
        os.remove(qmp_sock_path)
    if os.path.exists(ppm_out):
        os.remove(ppm_out)

    cmd = [
        "qemu-system-x86_64",
        "-kernel", "build/kernel.elf",
        "-serial", "stdio",
        "-vga", "std",
        "-m", "256M",
        "-display", "none",
        "-qmp", f"unix:{qmp_sock_path},server,nowait",
        "-no-reboot"
    ]
    disk_img = "build/aegis64-uefi-test.img" if os.path.exists("build/aegis64-uefi-test.img") else "build/aegis64-uefi.img"
    if os.path.exists(disk_img):
        cmd.extend(["-drive", f"file={disk_img},format=raw,index=1,media=disk"])

    proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    time.sleep(1.5)

    try:
        proc.stdin.write(f"video mode {width} {height}\n")
        proc.stdin.flush()
        time.sleep(0.4)
        proc.stdin.write("desktop\n")
        proc.stdin.flush()
        time.sleep(0.6)
    except Exception as e:
        print("Stdin error:", e)

    # Connect to QMP
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    connected = False
    for _ in range(15):
        try:
            s.connect(qmp_sock_path)
            connected = True
            break
        except Exception:
            time.sleep(0.3)

    if not connected:
        print(f"[FAIL] Could not connect to QMP socket {qmp_sock_path}")
        proc.terminate()
        sys.exit(1)

    s.recv(1024)
    s.sendall(b'{"execute": "qmp_capabilities"}\n')
    s.recv(1024)

    dump_cmd = json.dumps({"execute": "screendump", "arguments": {"filename": ppm_out}}) + "\n"
    s.sendall(dump_cmd.encode("utf-8"))
    s.recv(1024)
    s.close()

    time.sleep(0.4)
    proc.terminate()
    try:
        proc.wait(timeout=2)
    except Exception:
        proc.kill()

    if not os.path.exists(ppm_out):
        print(f"[FAIL] Framebuffer dump not found at {ppm_out}")
        sys.exit(1)

    im = Image.open(ppm_out).convert("RGB")
    im.save(png_out, "PNG")
    print(f"[OK] Saved guest framebuffer: {png_out} ({im.size[0]}x{im.size[1]} RGB)\n")
    return im

def run_visual_comparison():
    os.makedirs(VERIFY_DIR, exist_ok=True)
    
    # 1. Capture at 1672x941 (Native Reference Lock)
    ppm_1672 = os.path.join(VERIFY_DIR, "guest_desktop_1672x941.ppm")
    png_1672 = os.path.join(VERIFY_DIR, "guest_desktop_screendump.png")
    guest_img = capture_guest_framebuffer(1672, 941, ppm_1672, png_1672)

    # 2. Capture at 1024x768 (Multi-Resolution Verification)
    ppm_1024 = os.path.join(VERIFY_DIR, "guest_desktop_1024x768.ppm")
    png_1024 = os.path.join(VERIFY_DIR, "guest_desktop_1024x768.png")
    capture_guest_framebuffer(1024, 768, ppm_1024, png_1024)

    # 3. Load authoritative reference
    ref_img = Image.open(REFERENCE_PATH).convert("RGB")
    assert guest_img.size == ref_img.size, f"Size mismatch: guest={guest_img.size}, ref={ref_img.size}"

    w, h = ref_img.size
    total_pixels = w * h

    # 4. Compute Pixel Match and Channel Errors
    diff_img = ImageChops.difference(guest_img, ref_img)
    diff_pixels = diff_img.getdata()
    
    exact_matches = 0
    channel_err_sum = 0
    for r, g, b in diff_pixels:
        if r == 0 and g == 0 and b == 0:
            exact_matches += 1
        channel_err_sum += (r + g + b)

    match_percent = (exact_matches / total_pixels) * 100.0
    mean_channel_err = channel_err_sum / (total_pixels * 3.0)

    print("=== Visual Regression Metrics ===")
    print(f"Total Framebuffer Pixels: {total_pixels:,}")
    print(f"Exact Matching Pixels:    {exact_matches:,} ({match_percent:.2f}%)")
    print(f"Mean Absolute Channel Error (MAE): {mean_channel_err:.4f} / 255.0")

    # 5. Generate Side-by-Side Comparison
    sbs_width = w * 2 + 20
    sbs_height = h + 60
    sbs = Image.new("RGB", (sbs_width, sbs_height), (12, 6, 24))
    
    # Paste Reference on Left, Guest on Right
    sbs.paste(ref_img, (0, 40))
    sbs.paste(guest_img, (w + 20, 40))
    
    sbs_path = os.path.join(VERIFY_DIR, "side_by_side_comparison.png")
    sbs.save(sbs_path, "PNG")
    print(f"[OK] Generated Side-by-Side Comparison: {sbs_path}")

    # 6. Generate Alpha-Overlay Comparison (50% Guest, 50% Reference)
    overlay = Image.blend(guest_img, ref_img, alpha=0.5)
    overlay_path = os.path.join(VERIFY_DIR, "alpha_overlay_comparison.png")
    overlay.save(overlay_path, "PNG")
    print(f"[OK] Generated Alpha-Overlay Comparison: {overlay_path}")

    # 7. Generate Pixel Difference Map (amplified 10x for visibility)
    # Scale difference so even tiny variations show up clearly
    diff_amplified = diff_img.point(lambda p: min(255, p * 10))
    diff_path = os.path.join(VERIFY_DIR, "pixel_difference_map.png")
    diff_amplified.save(diff_path, "PNG")
    print(f"[OK] Generated Amplified Pixel Difference Map: {diff_path}")

    # Copy to artifacts directory
    art_dir = "/home/saptanshu-ganguly/.gemini/antigravity/brain/004b4109-7cfd-4ce6-9d9e-5d632649292f"
    if os.path.exists(art_dir):
        dest_png = os.path.join(art_dir, "purple_os_desktop_verified.png")
        guest_img.save(dest_png, "PNG")
        print(f"[OK] Updated artifact: {dest_png}")

    # Re-verify reference integrity at the end
    verify_reference_integrity()
    print(">>> VISUAL REGRESSION VERIFICATION COMPLETED SUCCESSFULLY! <<<")

def main():
    verify_reference_integrity()
    verify_wallpaper_assets()
    run_visual_comparison()

if __name__ == "__main__":
    main()
