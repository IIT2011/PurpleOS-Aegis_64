"""Build the 8K wallpaper master and compact kernel source from the clean asset."""

from __future__ import annotations

import struct
from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parents[1]
OUTPUT_DIR = ROOT / "assets" / "wallpapers"
SOURCE = OUTPUT_DIR / "purple_nebula_source.jpg"
MASTER_SIZE = (7680, 4320)
RAW_MAGIC = b"PWRAW001"


def main() -> None:
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    with Image.open(SOURCE) as image:
        source = image.convert("RGB")
        width, height = source.size
        master = source.resize(MASTER_SIZE, Image.Resampling.LANCZOS)
        master.save(OUTPUT_DIR / "purple_nebula_8k.jpg", quality=96, subsampling=0, optimize=True)

        # Derive a compact runtime copy from the 8K master so the kernel does not
        # need a JPEG decoder or a 100 MB uncompressed 8K image embedded in its ELF.
        runtime = master.resize((width, height), Image.Resampling.LANCZOS)
        header = RAW_MAGIC + struct.pack("<II", width, height)
        with (OUTPUT_DIR / "purple_nebula.pwraw").open("wb") as raw:
            raw.write(header)
            raw.write(runtime.tobytes())


if __name__ == "__main__":
    main()
