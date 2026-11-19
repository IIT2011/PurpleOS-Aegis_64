#!/usr/bin/env python3
"""
Generate native Rust source file src/drivers/inter_font.rs
Rasterizes Inter-Regular.ttf and Inter-Bold.ttf vector outlines into:
  - FONT_INTER_REGULAR_8X16: [[u8; 16]; 256] (monochrome bitmask)
  - FONT_INTER_REGULAR_ALPHA: [[[u8; 8]; 16]; 256] (8-bit anti-aliased alpha coverage)
  - FONT_INTER_BOLD_8X16: [[u8; 16]; 256] (monochrome bitmask)
  - FONT_INTER_BOLD_ALPHA: [[[u8; 8]; 16]; 256] (8-bit anti-aliased alpha coverage)
"""

import os
import sys
import struct

def parse_ttf(font_path):
    with open(font_path, 'rb') as f:
        data = f.read()

    sfnt, num_tables = struct.unpack_from('>IH', data, 0)
    tables = {struct.unpack_from('>4s', data, 12 + i*16)[0].decode('latin1'): struct.unpack_from('>III', data, 12 + i*16 + 4)[1:] for i in range(num_tables)}
    head_off = tables['head'][0]
    loca_off = tables['loca'][0]
    glyf_off = tables['glyf'][0]
    cmap_base = tables['cmap'][0]
    units_per_em, = struct.unpack_from('>H', data, head_off + 18)
    loca_format, = struct.unpack_from('>h', data, head_off + 50)
    cmap_count, = struct.unpack_from('>H', data, cmap_base + 2)
    cmap_off = None
    for i in range(cmap_count):
        platform, encoding, offset = struct.unpack_from('>HHI', data, cmap_base + 4 + i * 8)
        candidate = cmap_base + offset
        fmt, = struct.unpack_from('>H', data, candidate)
        if fmt == 4 and (platform == 0 or (platform == 3 and encoding == 1)):
            cmap_off = candidate
            break
    if cmap_off is None:
        raise ValueError(f'{font_path} lacks a Unicode format 4 cmap')

    _, _, _, seg_count_x2 = struct.unpack_from('>HHHH', data, cmap_off)
    seg_count = seg_count_x2 // 2
    end_codes = struct.unpack_from(f'>{seg_count}H', data, cmap_off + 14)
    start_codes = struct.unpack_from(f'>{seg_count}H', data, cmap_off + 16 + seg_count * 2)
    id_deltas = struct.unpack_from(f'>{seg_count}h', data, cmap_off + 16 + seg_count * 4)
    id_range_offsets = struct.unpack_from(f'>{seg_count}H', data, cmap_off + 16 + seg_count * 6)

    def get_gid(cp):
        for i in range(seg_count):
            if cp <= end_codes[i]:
                if cp >= start_codes[i]:
                    if id_range_offsets[i] == 0:
                        return (cp + id_deltas[i]) & 0xFFFF
                    else:
                        off = cmap_off + 16 + seg_count * 6 + i * 2 + id_range_offsets[i] + (cp - start_codes[i]) * 2
                        index, = struct.unpack_from('>H', data, off)
                        return (index + id_deltas[i]) & 0xFFFF if index else 0
                else: return 0
        return 0

    def get_poly(gid):
        if loca_format == 0:
            off1, off2 = (value * 2 for value in struct.unpack_from('>HH', data, loca_off + gid * 2))
        else:
            off1, off2 = struct.unpack_from('>II', data, loca_off + gid * 4)
        if off1 == off2:
            return []
        g_data = data[glyf_off + off1 : glyf_off + off2]
        num_contours, xmin, ymin, xmax, ymax = struct.unpack_from('>hhhhh', g_data, 0)
        if num_contours <= 0: return []
        end_pts = struct.unpack_from(f'>{num_contours}H', g_data, 10)
        num_pts = end_pts[-1] + 1
        ins_len, = struct.unpack_from('>H', g_data, 10 + num_contours * 2)
        flags_off = 10 + num_contours * 2 + 2 + ins_len
        flags = []
        curr = flags_off
        while len(flags) < num_pts:
            flag = g_data[curr]; curr += 1
            flags.append(flag)
            if flag & 8:
                cnt = g_data[curr]; curr += 1
                flags.extend([flag] * cnt)
        x_coords, x = [], 0
        for flag in flags:
            if flag & 2:
                val = g_data[curr]; curr += 1
                x += val if (flag & 16) else -val
            elif not (flag & 16):
                val, = struct.unpack_from('>h', g_data, curr); curr += 2
                x += val
            x_coords.append(x)
        y_coords, y = [], 0
        for flag in flags:
            if flag & 4:
                val = g_data[curr]; curr += 1
                y += val if (flag & 32) else -val
            elif not (flag & 32):
                val, = struct.unpack_from('>h', g_data, curr); curr += 2
                y += val
            y_coords.append(y)
        pts = list(zip(x_coords, y_coords, [bool(f & 1) for f in flags]))
        contours, start_idx = [], 0
        for end_idx in end_pts:
            c_pts = pts[start_idx : end_idx + 1]
            start_idx = end_idx + 1
            if not c_pts: continue
            full_pts = []
            n = len(c_pts)
            for i in range(n):
                p0 = c_pts[i]; p1 = c_pts[(i + 1) % n]
                full_pts.append(p0)
                if not p0[2] and not p1[2]:
                    full_pts.append(((p0[0] + p1[0])/2, (p0[1] + p1[1])/2, True))
            # Start from an on-curve point so the closing quadratic is preserved.
            first = next(i for i, point in enumerate(full_pts) if point[2])
            full_pts = full_pts[first:] + full_pts[:first]
            poly = []
            m = len(full_pts); i = 0
            while i < m:
                curr_p = full_pts[i]; next_p = full_pts[(i + 1) % m]
                if curr_p[2] and next_p[2]:
                    poly.append((curr_p[0], curr_p[1])); i += 1
                elif curr_p[2] and not next_p[2]:
                    p2 = full_pts[(i + 2) % m]
                    for step in range(8):
                        t = step / 8.0
                        bx = (1-t)**2 * curr_p[0] + 2*(1-t)*t * next_p[0] + t**2 * p2[0]
                        by = (1-t)**2 * curr_p[1] + 2*(1-t)*t * next_p[1] + t**2 * p2[1]
                        poly.append((bx, by))
                    i += 2
                else:
                    poly.append((curr_p[0], curr_p[1])); i += 1
            contours.append(poly)
        return contours

    def point_in_poly(px, py, poly):
        inside = False
        n = len(poly)
        for i in range(n):
            x1, y1 = poly[i]; x2, y2 = poly[(i + 1) % n]
            if ((y1 > py) != (y2 > py)) and (px < (x2 - x1) * (py - y1) / (y2 - y1 + 1e-12) + x1):
                inside = not inside
        return inside

    glyphs_bits = []
    glyphs_alpha = []

    cell_width, cell_height = 8, 16
    outlines = {cp: get_poly(get_gid(cp)) for cp in range(32, 127)}
    y_values = [y for contours in outlines.values() for contour in contours for _, y in contour]
    bottom, top = min(y_values), max(y_values)
    scale = min(cell_height / units_per_em, (cell_height - 2) / (top - bottom))
    baseline = (cell_height - (top - bottom) * scale) / 2 + top * scale
    samples = (0.125, 0.375, 0.625, 0.875)
    sample_count = len(samples) ** 2

    for cp in range(256):
        if cp < 32 or cp >= 127:
            # Placeholder/fallback will be filled
            glyphs_bits.append([0]*16)
            glyphs_alpha.append([[0]*8 for _ in range(16)])
            continue

        contours = outlines[cp]
        if not contours:
            glyphs_bits.append([0]*16)
            glyphs_alpha.append([[0]*8 for _ in range(16)])
            continue

        x_values = [x for contour in contours for x, _ in contour]
        left, right = min(x_values), max(x_values)
        scale_x = min(scale, (cell_width - 1) / (right - left))
        origin_x = (cell_width - (right + left) * scale_x) / 2
        row_bits = []
        row_alphas = []
        for r in range(cell_height):
            bits = 0
            alphas = []
            for c in range(cell_width):
                hits = 0
                for sy in samples:
                    for sx in samples:
                        fx = (c + sx - origin_x) / scale_x
                        fy = (baseline - (r + sy)) / scale
                        in_count = sum(point_in_poly(fx, fy, poly) for poly in contours)
                        if in_count % 2 == 1:
                            hits += 1
                alpha = round(hits * 255 / sample_count)
                alphas.append(alpha)
                if hits * 2 >= sample_count:
                    bits |= (0x80 >> c)
            row_bits.append(bits)
            row_alphas.append(alphas)
        glyphs_bits.append(row_bits)
        glyphs_alpha.append(row_alphas)

    return glyphs_bits, glyphs_alpha

def load_vga_font(vga_path):
    with open(vga_path, 'r') as f:
        content = f.read()
    # Parse FONT8X16
    lines = content.split('\n')
    glyphs = []
    curr = []
    in_font = False
    for line in lines:
        if 'FONT8X16' in line:
            in_font = True
            continue
        if in_font:
            if '];' in line:
                break
            line = line.strip()
            if line.startswith('['):
                parts = line.strip('[],').split(',')
                row = [int(p.strip(), 16) if '0x' in p else int(p.strip()) for p in parts if p.strip()]
                glyphs.append(row)
    return glyphs

def main():
    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    reg_path = os.path.join(base_dir, "assets/fonts/Inter/Inter-Regular.ttf")
    bold_path = os.path.join(base_dir, "assets/fonts/Inter/Inter-Bold.ttf")
    vga_path = os.path.join(base_dir, "src/drivers/font8x16.rs")
    out_path = os.path.join(base_dir, "src/drivers/inter_font.rs")

    print(f"Loading VGA fallback from {vga_path}...")
    vga_glyphs = load_vga_font(vga_path)

    print(f"Rasterizing Inter Regular from {reg_path}...")
    reg_bits, reg_alpha = parse_ttf(reg_path)

    print(f"Rasterizing Inter Bold from {bold_path}...")
    bold_bits, bold_alpha = parse_ttf(bold_path)

    # Fill fallback for non-printable ASCII or empty glyphs
    for cp in range(256):
        if cp < len(vga_glyphs):
            vga_row = vga_glyphs[cp]
            # If Inter is empty, use VGA
            if sum(reg_bits[cp]) == 0 and sum(vga_row) > 0 and cp != 32:
                reg_bits[cp] = vga_row[:]
                for r in range(16):
                    for c in range(8):
                        if (vga_row[r] & (0x80 >> c)) != 0:
                            reg_alpha[cp][r][c] = 255
            if sum(bold_bits[cp]) == 0 and sum(vga_row) > 0 and cp != 32:
                bold_bits[cp] = vga_row[:]
                for r in range(16):
                    for c in range(8):
                        if (vga_row[r] & (0x80 >> c)) != 0:
                            bold_alpha[cp][r][c] = 255

    print(f"Generating {out_path}...")
    with open(out_path, "w") as f:
        f.write("//! Native Pre-Rasterized Inter Font Subsystem for PurpleOS / Aegis64.\n")
        f.write("//!\n")
        f.write("//! Contains high-precision anti-aliased alpha coverage and monochrome bitmasks\n")
        f.write("//! rasterized directly from Inter-Regular.ttf and Inter-Bold.ttf vector outlines.\n\n")

        # Regular bitmask
        f.write("pub const FONT_INTER_REGULAR_8X16: [[u8; 16]; 256] = [\n")
        for cp in range(256):
            f.write("    [" + ", ".join(f"0x{b:02X}" for b in reg_bits[cp]) + f"], // 0x{cp:02X} ('{chr(cp) if 32 <= cp < 127 else '.'}')\n")
        f.write("];\n\n")

        # Bold bitmask
        f.write("pub const FONT_INTER_BOLD_8X16: [[u8; 16]; 256] = [\n")
        for cp in range(256):
            f.write("    [" + ", ".join(f"0x{b:02X}" for b in bold_bits[cp]) + f"], // 0x{cp:02X} ('{chr(cp) if 32 <= cp < 127 else '.'}')\n")
        f.write("];\n\n")

        # Regular alpha
        f.write("pub const FONT_INTER_REGULAR_ALPHA: [[[u8; 8]; 16]; 256] = [\n")
        for cp in range(256):
            f.write(f"    [ // 0x{cp:02X} ('{chr(cp) if 32 <= cp < 127 else '.'}')\n")
            for r in range(16):
                f.write("        [" + ", ".join(f"0x{a:02X}" for a in reg_alpha[cp][r]) + "],\n")
            f.write("    ],\n")
        f.write("];\n\n")

        # Bold alpha
        f.write("pub const FONT_INTER_BOLD_ALPHA: [[[u8; 8]; 16]; 256] = [\n")
        for cp in range(256):
            f.write(f"    [ // 0x{cp:02X} ('{chr(cp) if 32 <= cp < 127 else '.'}')\n")
            for r in range(16):
                f.write("        [" + ", ".join(f"0x{a:02X}" for a in bold_alpha[cp][r]) + "],\n")
            f.write("    ],\n")
        f.write("];\n")

    print(f"[OK] Generated {out_path} ({os.path.getsize(out_path):,} bytes)")

if __name__ == "__main__":
    main()
