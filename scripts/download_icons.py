#!/usr/bin/env python3
"""
Download modern UI vector icons (SVG) from Lucide Icons (MIT License)
for PurpleOS / Aegis64 system resources.
"""

import urllib.request
import os
import sys

BASE_URL = "https://raw.githubusercontent.com/lucide-icons/lucide/main/icons"

ICONS = [
    "wifi",
    "wifi-off",
    "battery",
    "battery-charging",
    "battery-medium",
    "volume-2",
    "volume-x",
    "bluetooth",
    "sliders",
    "sliders-horizontal",
    "search",
    "bell",
    "shield",
    "power",
    "sun",
    "moon",
    "settings",
    "cpu",
    "hard-drive",
    "terminal",
    "folder",
    "activity",
]

def main():
    dest_dir = os.path.abspath("assets/icons")
    os.makedirs(dest_dir, exist_ok=True)
    print(f"Downloading {len(ICONS)} vector icons into {dest_dir}...")

    downloaded = 0
    for name in ICONS:
        url = f"{BASE_URL}/{name}.svg"
        out_file = os.path.join(dest_dir, f"{name}.svg")
        try:
            req = urllib.request.Request(
                url,
                headers={"User-Agent": "PurpleOS-Icon-Downloader/1.0"}
            )
            with urllib.request.urlopen(req, timeout=10) as resp:
                content = resp.read()
                with open(out_file, "wb") as f:
                    f.write(content)
            downloaded += 1
            print(f"  [OK] {name}.svg ({len(content)} bytes)")
        except Exception as e:
            print(f"  [FAIL] {name}.svg: {e}")

    print(f"\nSuccessfully downloaded {downloaded}/{len(ICONS)} icons.")

if __name__ == "__main__":
    main()
