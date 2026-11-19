#!/usr/bin/env python3
"""
Generate high-fidelity anti-aliased icon bitmasks and brand logos for PurpleOS
using 4x4 subpixel supersampling based on Lucide Icon vector specifications.
"""

import math
import os

SCALE = 4

def dist_segment(px, py, x1, y1, x2, y2):
    dx = x2 - x1
    dy = y2 - y1
    l2 = dx*dx + dy*dy
    if l2 == 0:
        return math.hypot(px - x1, py - y1)
    t = max(0.0, min(1.0, ((px - x1) * dx + (py - y1) * dy) / l2))
    return math.hypot(px - (x1 + t * dx), py - (y1 + t * dy))

def dist_arc(px, py, cx, cy, radius, start_angle_deg, end_angle_deg):
    dx = px - cx
    dy = py - cy
    dist_center = math.hypot(dx, dy)
    dist_r = abs(dist_center - radius)
    angle = math.degrees(math.atan2(dy, dx))
    if angle < 0:
        angle += 360.0
    
    # Check if within angular sweep
    in_sweep = False
    if start_angle_deg <= end_angle_deg:
        in_sweep = (start_angle_deg <= angle <= end_angle_deg)
    else:
        in_sweep = (angle >= start_angle_deg or angle <= end_angle_deg)
    
    if in_sweep:
        return dist_r
    
    # Distance to endpoints
    a1 = math.radians(start_angle_deg)
    p1x = cx + radius * math.cos(a1)
    p1y = cy + radius * math.sin(a1)
    
    a2 = math.radians(end_angle_deg)
    p2x = cx + radius * math.cos(a2)
    p2y = cy + radius * math.sin(a2)
    
    return min(math.hypot(px - p1x, py - p1y), math.hypot(px - p2x, py - p2y))

def supersample(w, h, sdf_func, stroke_radius=0.75):
    sw = w * SCALE
    sh = h * SCALE
    out = [[0 for _ in range(w)] for _ in range(h)]
    for py in range(h):
        for px in range(w):
            inside_cnt = 0
            for sy in range(SCALE):
                for sx in range(SCALE):
                    fx = px + (sx + 0.5) / SCALE
                    fy = py + (sy + 0.5) / SCALE
                    d = sdf_func(fx, fy)
                    if d <= stroke_radius:
                        inside_cnt += 1
            alpha = (inside_cnt * 255) // (SCALE * SCALE)
            out[py][px] = alpha
    return out

# 1. Wi-Fi Arcs & Dot
def sdf_wifi_dot(x, y):
    return math.hypot(x - 8.0, y - 13.5) - 0.3

def sdf_wifi_arc1(x, y):
    return dist_arc(x, y, 8.0, 13.5, 3.6, 220.0, 320.0)

def sdf_wifi_arc2(x, y):
    return dist_arc(x, y, 8.0, 13.5, 7.0, 215.0, 325.0)

def sdf_wifi_arc3(x, y):
    return dist_arc(x, y, 8.0, 13.5, 10.4, 205.0, 335.0)

def sdf_wifi_full(x, y):
    return min(sdf_wifi_dot(x, y), sdf_wifi_arc1(x, y), sdf_wifi_arc2(x, y), sdf_wifi_arc3(x, y))

# 2. Volume Speaker Cone & Waves
def sdf_volume_cone(x, y):
    # Box: (2.0, 5.5) to (4.5, 10.5)
    # Polygon: (4.5, 5.5) -> (7.5, 3.0) -> (7.5, 13.0) -> (4.5, 10.5)
    d1 = dist_segment(x, y, 2.0, 5.5, 4.5, 5.5)
    d2 = dist_segment(x, y, 2.0, 10.5, 4.5, 10.5)
    d3 = dist_segment(x, y, 2.0, 5.5, 2.0, 10.5)
    d4 = dist_segment(x, y, 4.5, 5.5, 7.5, 3.0)
    d5 = dist_segment(x, y, 7.5, 3.0, 7.5, 13.0)
    d6 = dist_segment(x, y, 7.5, 13.0, 4.5, 10.5)
    
    # Fill inside box/cone
    in_box = (2.0 <= x <= 4.5) and (5.5 <= y <= 10.5)
    in_cone = (4.5 <= x <= 7.5) and (y >= 5.5 - (x - 4.5)*0.83) and (y <= 10.5 + (x - 4.5)*0.83)
    if in_box or in_cone:
        return 0.0
    return min(d1, d2, d3, d4, d5, d6)

def sdf_volume_wave1(x, y):
    return dist_arc(x, y, 7.5, 8.0, 3.5, 315.0, 45.0)

def sdf_volume_wave2(x, y):
    return dist_arc(x, y, 7.5, 8.0, 6.2, 315.0, 45.0)

def sdf_volume_full(x, y):
    return min(sdf_volume_cone(x, y), sdf_volume_wave1(x, y), sdf_volume_wave2(x, y))

# 3. Battery Shell (20x12)
def sdf_battery_shell(x, y):
    # Rounded rect: x in [1.5, 16.5], y in [1.5, 10.5], radius 2.0
    # Terminal pip: rect x in [17.5, 18.8], y in [4.2, 7.8]
    # SDF to rounded rect boundary
    qx = max(0.0, abs(x - 9.0) - (7.5 - 2.0))
    qy = max(0.0, abs(y - 6.0) - (4.5 - 2.0))
    d_rect = math.hypot(qx, qy) - 2.0
    d_shell = abs(d_rect) # stroke outline
    
    # Pip terminal
    d_pip = dist_segment(x, y, 17.5, 4.5, 17.5, 7.5)
    return min(d_shell, d_pip)

# 4. Search Magnifying Glass (16x16)
def sdf_search(x, y):
    # Circle center (6.5, 6.5), radius 4.5
    d_circ = abs(math.hypot(x - 6.5, y - 6.5) - 4.5)
    # Handle: (9.7, 9.7) to (14.0, 14.0)
    d_handle = dist_segment(x, y, 9.7, 9.7, 13.8, 13.8)
    return min(d_circ, d_handle)

# 5. Sliders / Control Center (16x16)
def sdf_sliders(x, y):
    # Track 1 at y = 4.0: lines [2, 8] and [11, 14], knob at (9.5, 4.0) length 3.5
    d1 = min(dist_segment(x, y, 2.0, 4.0, 8.0, 4.0), dist_segment(x, y, 11.0, 4.0, 14.0, 4.0))
    k1 = dist_segment(x, y, 9.5, 2.2, 9.5, 5.8)
    
    # Track 2 at y = 8.0: lines [2, 5] and [8, 14], knob at (6.5, 8.0)
    d2 = min(dist_segment(x, y, 2.0, 8.0, 5.0, 8.0), dist_segment(x, y, 8.0, 8.0, 14.0, 8.0))
    k2 = dist_segment(x, y, 6.5, 6.2, 6.5, 9.8)
    
    # Track 3 at y = 12.0: lines [2, 10] and [13, 14], knob at (11.5, 12.0)
    d3 = min(dist_segment(x, y, 2.0, 12.0, 10.0, 12.0), dist_segment(x, y, 13.0, 12.0, 14.0, 12.0))
    k3 = dist_segment(x, y, 11.5, 10.2, 11.5, 13.8)
    
    return min(d1, k1, d2, k2, d3, k3)

# 6. Stylish P Brand Logo (18x18 ARGB)
def render_stylish_p(w=18, h=18):
    sw = w * SCALE
    sh = h * SCALE
    grid = [[(0, 0, 0, 0) for _ in range(sw)] for _ in range(sh)]

    for y in range(sh):
        for x in range(sw):
            fx = x / SCALE
            fy = y / SCALE

            # Stem with aerodynamic angled bottom
            in_stem = (3.0 <= fx <= 7.2) and (1.8 <= fy <= (16.2 - (fx - 3.0) * 0.35))

            # Outer Loop
            dx = (fx - 9.5) / 6.0
            dy = (fy - 6.2) / 4.4
            dist_sq = dx*dx + dy*dy
            in_loop_outer = (dist_sq <= 1.0) and (fx >= 5.0) and (fy >= 1.8) and (fy <= 10.8)

            # Inner Counter
            idx = (fx - 9.2) / 3.2
            idy = (fy - 6.2) / 2.3
            idist_sq = idx*idx + idy*idy
            in_loop_inner = (idist_sq < 1.0) and (fx >= 7.2)

            if (in_stem or in_loop_outer) and not in_loop_inner:
                if fy < 3.2:
                    # Specular top crest highlight
                    t = (fy - 1.8) / 1.4
                    r = int(255 - t * 20)
                    g = int(255 - t * 40)
                    b = 255
                elif in_stem and fx < 4.2:
                    # Specular stem bevel
                    r = 243
                    g = 232
                    b = 255
                elif in_loop_outer and fx > 12.0:
                    # Neon Violet loop curve
                    r = 192
                    g = 132
                    b = 252
                elif fy > 10.0:
                    # Deep amethyst lower return
                    r = 126
                    g = 34
                    b = 206
                else:
                    # Vibrant PurpleOS base
                    r = 168
                    g = 85
                    b = 247
                grid[y][x] = (r, g, b, 255)

    out = []
    for py in range(h):
        row = []
        for px in range(w):
            acc_r = acc_g = acc_b = acc_a = 0
            for sy in range(SCALE):
                for sx in range(SCALE):
                    r, g, b, a = grid[py * SCALE + sy][px * SCALE + sx]
                    acc_r += r
                    acc_g += g
                    acc_b += b
                    acc_a += a
            n = SCALE * SCALE
            out.append((acc_r // n, acc_g // n, acc_b // n, acc_a // n))
    return out

def format_u8_table(name, table, w, h):
    lines = []
    lines.append(f"/// Anti-aliased {name} ({w}x{h} 8-bit alpha mask)")
    lines.append(f"pub const {name}: [[u8; {w}]; {h}] = [")
    for row in table:
        row_str = ", ".join(f"0x{v:02X}" for v in row)
        lines.append(f"    [{row_str}],")
    lines.append("];")
    return "\n".join(lines)

def main():
    print("Generating anti-aliased icons and stylish brand logo...")
    wifi_full = supersample(16, 16, sdf_wifi_full, 0.72)
    wifi_arc1 = supersample(16, 16, sdf_wifi_arc1, 0.72)
    wifi_arc2 = supersample(16, 16, sdf_wifi_arc2, 0.72)
    wifi_arc3 = supersample(16, 16, sdf_wifi_arc3, 0.72)
    wifi_dot  = supersample(16, 16, sdf_wifi_dot,  0.50)

    vol_full  = supersample(16, 16, sdf_volume_full, 0.72)
    vol_cone  = supersample(16, 16, sdf_volume_cone, 0.72)
    vol_wave1 = supersample(16, 16, sdf_volume_wave1, 0.72)
    vol_wave2 = supersample(16, 16, sdf_volume_wave2, 0.72)

    battery_shell = supersample(20, 12, sdf_battery_shell, 0.72)
    search_icon   = supersample(16, 16, sdf_search, 0.72)
    sliders_icon  = supersample(16, 16, sdf_sliders, 0.72)

    p_logo = render_stylish_p(18, 18)

    out = []
    out.append("//! Systemwide vector and anti-aliased UI icon resources for PurpleOS")
    out.append("//! Automatically generated from Lucide Icon standards and stylized brand geometry.")
    out.append("//! 100% pure Rust `#![no_std]` compatible with subpixel fractional coverage.")
    out.append("")
    out.append("use crate::compositor::surface::Surface;")
    out.append("")
    out.append("/// Stylish 'Amethyst Prism P' Logo (18x18 ARGB8888)")
    out.append("pub const STYLISH_P_LOGO: [[u32; 18]; 18] = [")
    for row in range(18):
        row_pixels = p_logo[row * 18 : (row + 1) * 18]
        row_str = ", ".join(f"0x{a:02X}{r:02X}{g:02X}{b:02X}" for (r, g, b, a) in row_pixels)
        out.append(f"    [{row_str}],")
    out.append("];")
    out.append("")
    out.append(format_u8_table("ICON_WIFI_FULL", wifi_full, 16, 16))
    out.append("")
    out.append(format_u8_table("ICON_WIFI_ARC1", wifi_arc1, 16, 16))
    out.append("")
    out.append(format_u8_table("ICON_WIFI_ARC2", wifi_arc2, 16, 16))
    out.append("")
    out.append(format_u8_table("ICON_WIFI_ARC3", wifi_arc3, 16, 16))
    out.append("")
    out.append(format_u8_table("ICON_WIFI_DOT", wifi_dot, 16, 16))
    out.append("")
    out.append(format_u8_table("ICON_VOLUME_FULL", vol_full, 16, 16))
    out.append("")
    out.append(format_u8_table("ICON_VOLUME_CONE", vol_cone, 16, 16))
    out.append("")
    out.append(format_u8_table("ICON_VOLUME_WAVE1", vol_wave1, 16, 16))
    out.append("")
    out.append(format_u8_table("ICON_VOLUME_WAVE2", vol_wave2, 16, 16))
    out.append("")
    out.append(format_u8_table("ICON_BATTERY_SHELL", battery_shell, 20, 12))
    out.append("")
    out.append(format_u8_table("ICON_SEARCH", search_icon, 16, 16))
    out.append("")
    out.append(format_u8_table("ICON_SLIDERS", sliders_icon, 16, 16))
    out.append("")
    out.append("""/// Blit an anti-aliased icon mask onto the target surface with color tint
pub fn render_icon_mask<const W: usize, const H: usize>(
    target: &mut Surface,
    x: i32,
    y: i32,
    mask: &[[u8; W]; H],
    tint_color: u32,
) {
    if !target.is_valid() || x + (W as i32) <= 0 || y + (H as i32) <= 0 {
        return;
    }
    let tint_a = (tint_color >> 24) & 0xFF;
    let tint_rgb = tint_color & 0x00FFFFFF;

    for row in 0..H {
        let py = y + row as i32;
        if py < 0 || py as usize >= target.height {
            continue;
        }
        for col in 0..W {
            let px = x + col as i32;
            if px < 0 || px as usize >= target.width {
                continue;
            }
            let cov = mask[row][col] as u32;
            if cov > 0 {
                let eff_alpha = (cov * tint_a) / 255;
                if eff_alpha > 0 {
                    let pixel = (eff_alpha << 24) | tint_rgb;
                    target.blend_pixel(px as usize, py as usize, pixel);
                }
            }
        }
    }
}

/// Blit a pre-shaded 32-bit ARGB logo onto the target surface
pub fn render_logo_argb<const W: usize, const H: usize>(
    target: &mut Surface,
    x: i32,
    y: i32,
    logo: &[[u32; W]; H],
    alpha_mult: u32,
) {
    if !target.is_valid() || x + (W as i32) <= 0 || y + (H as i32) <= 0 {
        return;
    }

    for row in 0..H {
        let py = y + row as i32;
        if py < 0 || py as usize >= target.height {
            continue;
        }
        for col in 0..W {
            let px = x + col as i32;
            if px < 0 || px as usize >= target.width {
                continue;
            }
            let src = logo[row][col];
            let a = (src >> 24) & 0xFF;
            if a > 0 {
                let eff_a = (a * alpha_mult) / 255;
                if eff_a > 0 {
                    let pixel = (eff_a << 24) | (src & 0x00FFFFFF);
                    target.blend_pixel(px as usize, py as usize, pixel);
                }
            }
        }
    }
}
""")

    code_str = "\n".join(out)
    with open("src/compositor/icons.rs", "w") as f:
        f.write(code_str)
    print(f"Generated src/compositor/icons.rs ({len(code_str)} bytes).")

if __name__ == "__main__":
    main()
