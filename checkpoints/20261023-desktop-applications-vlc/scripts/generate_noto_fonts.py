#!/usr/bin/env python3
"""
Purple OS / Aegis64 — Noto Font Asset Generator
Rasterizes official Google Noto Sans and Noto Sans Mono TTF fonts into
compact, freestanding C data structures with subpixel / 8-bit grayscale
anti-aliasing (alpha coverage), metrics, UTF-8 mapping, and fast ASCII indexing.
"""

import os
import sys
import hashlib
from PIL import ImageFont

FONTS_DIR = "assets/fonts"
OUTPUT_C = "src/drivers/font_data.c"
OUTPUT_H = "include/font_data.h"

FONT_CONFIGS = [
    {
        "id": "FONT_UI_REGULAR",
        "var_prefix": "font_ui_reg",
        "ttf": "NotoSans-Regular.ttf",
        "size": 14,
        "is_mono": 0,
        "family": "Noto Sans",
        "style": "Regular",
        "all_ascii": True,
        "include_extras": True,
    },
    {
        "id": "FONT_UI_BOLD",
        "var_prefix": "font_ui_bold",
        "ttf": "NotoSans-Bold.ttf",
        "size": 14,
        "is_mono": 0,
        "family": "Noto Sans",
        "style": "Bold",
        "all_ascii": True,
        "include_extras": True,
    },
    {
        "id": "FONT_UI_TITLE",
        "var_prefix": "font_ui_title",
        "ttf": "NotoSans-Bold.ttf",
        "size": 16,
        "is_mono": 0,
        "family": "Noto Sans",
        "style": "Bold",
        "all_ascii": True,
        "include_extras": True,
    },
    {
        "id": "FONT_MONO_REG",
        "var_prefix": "font_mono_reg",
        "ttf": "NotoSansMono-Regular.ttf",
        "size": 15,
        "is_mono": 1,
        "family": "Noto Sans Mono",
        "style": "Regular",
        "all_ascii": True,
        "include_extras": True,
    },
    {
        "id": "FONT_MONO_BOLD",
        "var_prefix": "font_mono_bold",
        "ttf": "NotoSansMono-Bold.ttf",
        "size": 15,
        "is_mono": 1,
        "family": "Noto Sans Mono",
        "style": "Bold",
        "all_ascii": True,
        "include_extras": True,
    },
    {
        "id": "FONT_CLOCK_HERO",
        "var_prefix": "font_clock_hero",
        "ttf": "NotoSans-Bold.ttf",
        "size": 48,
        "is_mono": 0,
        "family": "Noto Sans",
        "style": "Bold",
        "all_ascii": False,
        "custom_chars": "0123456789:AMPSetThuFrioNaylOctvmbgDREBLIX •-+,.",
        "include_extras": False,
    },
]

EXTRA_UNICODE_CHARS = [
    '•',  # U+2022 Bullet
    '★',  # U+2605 Star
    '—',  # U+2014 Em Dash
    '–',  # U+2013 En Dash
    '→',  # U+2192 Arrow Right
    '←',  # U+2190 Arrow Left
    '↑',  # U+2191 Arrow Up
    '↓',  # U+2193 Arrow Down
    '✓',  # U+2713 Checkmark
    '✗',  # U+2717 Cross
    '□',  # U+25A1 White Square (Fallback)
    'ä', 'ö', 'ü', 'é', 'è', 'ñ', 'ç', 'á', 'í', 'ó', 'ú',
    'Ä', 'Ö', 'Ü', 'É', 'È', 'Ñ',
]

def sha256_file(path):
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()

def generate():
    print("=== Generating Noto Anti-Aliased Font Assets for Aegis64 Kernel ===")

    all_fonts_data = []

    for cfg in FONT_CONFIGS:
        ttf_path = os.path.join(FONTS_DIR, cfg["ttf"])
        if not os.path.exists(ttf_path):
            print(f"[FAIL] Missing font file: {ttf_path}")
            sys.exit(1)

        f_hash = sha256_file(ttf_path)
        font = ImageFont.truetype(ttf_path, cfg["size"])
        metrics = font.getmetrics()
        ascent, descent = metrics[0], metrics[1]

        sym_font_path = os.path.join(FONTS_DIR, "NotoSansSymbols2-Regular.ttf")
        sym_font = ImageFont.truetype(sym_font_path, cfg["size"]) if os.path.exists(sym_font_path) else None
        box_mask_bytes = bytes(font.getmask("□"))

        # Determine character set
        if cfg.get("all_ascii"):
            char_set = [chr(c) for c in range(32, 127)]
            if cfg.get("include_extras"):
                char_set += EXTRA_UNICODE_CHARS
        else:
            char_set = list(set(cfg.get("custom_chars", "")))

        # Ensure space and fallback box are included
        if ' ' not in char_set:
            char_set.append(' ')
        if '?' not in char_set:
            char_set.append('?')
        if '□' not in char_set:
            char_set.append('□')

        # Remove duplicates and sort by codepoint
        unique_codepoints = sorted(set(ord(c) for c in char_set))

        glyphs = []
        bitmap_bytes = bytearray()

        for cp in unique_codepoints:
            ch = chr(cp)
            mask = font.getmask(ch)
            bbox = font.getbbox(ch)
            adv = font.getlength(ch)

            # Fallback to NotoSansSymbols2 for missing symbols
            if sym_font and ch != "□" and bytes(mask) == box_mask_bytes:
                s_mask = sym_font.getmask(ch)
                if bytes(s_mask) != bytes(sym_font.getmask("□")):
                    mask = s_mask
                    bbox = sym_font.getbbox(ch)
                    adv = sym_font.getlength(ch)

            mw, mh = mask.size
            if mw > 0 and mh > 0:
                mbits = bytes(mask)
                offset = len(bitmap_bytes)
                bitmap_bytes.extend(mbits)
                bearing_x = bbox[0]
                bearing_y = bbox[1]
            else:
                # Blank / whitespace glyph
                offset = 0
                bearing_x = 0
                bearing_y = 0

            # If font is monospace, enforce uniform advance
            if cfg["is_mono"]:
                adv_x = round(font.getlength("M"))
            else:
                adv_x = round(adv)

            glyphs.append({
                "codepoint": cp,
                "width": mw,
                "height": mh,
                "bearing_x": bearing_x,
                "bearing_y": bearing_y,
                "advance_x": adv_x,
                "offset": offset,
            })

        # Find fallback glyph index (prefer '□', fallback to '?', then 0)
        default_idx = 0
        for i, g in enumerate(glyphs):
            if g["codepoint"] == ord('□'):
                default_idx = i
                break
            elif g["codepoint"] == ord('?'):
                default_idx = i

        # Build fast ASCII map (32..126)
        ascii_map = [0xFFFF] * 96
        for i, g in enumerate(glyphs):
            if 32 <= g["codepoint"] <= 126:
                ascii_map[g["codepoint"] - 32] = i

        all_fonts_data.append({
            "cfg": cfg,
            "hash": f_hash,
            "ascent": ascent,
            "descent": descent,
            "glyphs": glyphs,
            "bitmap_bytes": bitmap_bytes,
            "ascii_map": ascii_map,
            "default_idx": default_idx,
        })

        print(f"[OK] {cfg['id']:16}: {len(glyphs):3d} glyphs, {len(bitmap_bytes):6,d} bitmap B (ascent={ascent}, descent={descent})")

    # Generate Header
    with open(OUTPUT_H, "w") as hf:
        hf.write("/* Auto-generated by scripts/generate_noto_fonts.py - DO NOT EDIT DIRECTLY */\n")
        hf.write("#ifndef FONT_DATA_H\n#define FONT_DATA_H\n\n")
        hf.write('#include "font.h"\n\n')
        for fd in all_fonts_data:
            hf.write(f"extern const font_t {fd['cfg']['var_prefix']}_font;\n")
        hf.write("\n#endif /* FONT_DATA_H */\n")

    # Generate C Source
    with open(OUTPUT_C, "w") as cf:
        cf.write("/* Auto-generated by scripts/generate_noto_fonts.py - DO NOT EDIT DIRECTLY */\n")
        cf.write('#include "font_data.h"\n\n')

        for fd in all_fonts_data:
            pfx = fd['cfg']['var_prefix']
            bbytes = fd['bitmap_bytes']
            glyphs = fd['glyphs']
            amap = fd['ascii_map']

            cf.write(f"/* --------------------------------------------------------------------------\n")
            cf.write(f"   {fd['cfg']['id']} ({fd['cfg']['family']} {fd['cfg']['style']} {fd['cfg']['size']}px)\n")
            cf.write(f"   SHA-256: {fd['hash']}\n")
            cf.write(f"   -------------------------------------------------------------------------- */\n\n")

            # Bitmaps array
            cf.write(f"static const uint8_t {pfx}_bitmaps[{max(1, len(bbytes))}] = {{\n")
            for i in range(0, len(bbytes), 16):
                chunk = bbytes[i:i+16]
                cf.write("    " + ", ".join(f"0x{b:02X}" for b in chunk) + ",\n")
            if len(bbytes) == 0:
                cf.write("    0x00\n")
            cf.write("};\n\n")

            # Glyphs array
            cf.write(f"static const font_glyph_t {pfx}_glyphs[{len(glyphs)}] = {{\n")
            for g in glyphs:
                bmp_ptr = f"{pfx}_bitmaps + {g['offset']}" if (g['width'] > 0 and g['height'] > 0) else "NULL"
                cf.write(f"    {{ 0x{g['codepoint']:04X}, {g['width']}, {g['height']}, {g['bearing_x']}, {g['bearing_y']}, {g['advance_x']}, {bmp_ptr} }},\n")
            cf.write("};\n\n")

            # Fast ASCII map
            cf.write(f"static const uint16_t {pfx}_ascii_map[96] = {{\n")
            for i in range(0, 96, 12):
                chunk = amap[i:i+12]
                cf.write("    " + ", ".join(f"0x{idx:04X}" for idx in chunk) + ",\n")
            cf.write("};\n\n")

            # Font structure definition
            cf.write(f"const font_t {pfx}_font = {{\n")
            cf.write(f'    .name = "{fd["cfg"]["id"]}",\n')
            cf.write(f'    .family = "{fd["cfg"]["family"]}",\n')
            cf.write(f'    .style = "{fd["cfg"]["style"]}",\n')
            cf.write(f'    .size_pt = {fd["cfg"]["size"]},\n')
            cf.write(f'    .ascent = {fd["ascent"]},\n')
            cf.write(f'    .descent = {fd["descent"]},\n')
            cf.write(f'    .line_gap = 2,\n')
            cf.write(f'    .is_monospace = {fd["cfg"]["is_mono"]},\n')
            cf.write(f'    .glyph_count = {len(glyphs)},\n')
            cf.write(f'    .glyphs = {pfx}_glyphs,\n')
            cf.write(f'    .ascii_map = {pfx}_ascii_map,\n')
            cf.write(f'    .default_glyph_idx = {fd["default_idx"]}\n')
            cf.write("};\n\n")

    print(f"\n[OK] Generated header: {OUTPUT_H}")
    print(f"[OK] Generated source: {OUTPUT_C}")
    print("=== Noto Font Asset Generation Complete ===\n")

if __name__ == "__main__":
    generate()
