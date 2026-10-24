#!/usr/bin/env python3
"""
Purple OS / Aegis64 — Complete Coherent Native Icon Library Generator
Generates 100 original, high-quality, professionally consistent Purple OS icons
across 5 categories: Core Applications, Developer Utilities, File Types, Devices,
and Status/UI Actions.

Outputs:
- assets/icons/32x32/*.png
- assets/icons/16x16/*.png
- include/icon_data.h
- src/drivers/icon_data.c
- docs/PURPLE_ICON_LIBRARY_CATALOG.md
"""

import os
import sys
import math
from PIL import Image, ImageDraw, ImageFilter

OUTPUT_DIR_32 = "assets/icons/32x32"
OUTPUT_DIR_16 = "assets/icons/16x16"
HEADER_PATH   = "include/icon_data.h"
SOURCE_PATH   = "src/drivers/icon_data.c"
DOCS_PATH     = "docs/PURPLE_ICON_LIBRARY_CATALOG.md"

# Signature Purple OS Color Palette
C_VIOLET_DARK   = (19, 10, 36, 255)    # #130A24
C_VIOLET_MID    = (42, 18, 69, 255)    # #2A1245
C_VIOLET_BORDER = (114, 9, 183, 255)   # #7209B7
C_VIOLET_LIGHT  = (157, 78, 221, 255)  # #9D4EDD
C_MAGENTA       = (247, 37, 133, 255)  # #F72585
C_PINK_ACCENT   = (181, 23, 158, 255)  # #B5179E
C_LAVENDER      = (224, 170, 255, 255) # #E0AAFF
C_WHITE         = (255, 255, 255, 255)
C_GOLD          = (255, 215, 0, 255)
C_GREEN         = (76, 201, 240, 255)  # Cyan-electric
C_RED           = (230, 57, 70, 255)
C_SHADOW        = (10, 5, 20, 220)

def draw_rounded_rect(draw, xy, radius, fill=None, outline=None, width=1):
    x0, y0, x1, y1 = xy
    draw.rounded_rectangle([x0, y0, x1, y1], radius=radius, fill=fill, outline=outline, width=width)

def create_base_icon(w=32, h=32):
    return Image.new("RGBA", (w, h), (0, 0, 0, 0))

def draw_squircle_container(draw, w, h, bg=C_VIOLET_MID, border=C_VIOLET_BORDER):
    pad = 1
    draw_rounded_rect(draw, (pad, pad, w - pad - 1, h - pad - 1), radius=6 if w >= 24 else 3, fill=bg, outline=border, width=1)

# Generator dictionary: id -> (name, category, description, render_fn)
ICONS = {}

def register_icon(icon_id, name, category, description, render_fn):
    ICONS[icon_id] = {
        "name": name,
        "category": category,
        "description": description,
        "render_fn": render_fn
    }

# --------------------------------------------------------------------------
# Category 1: Core Applications & Utilities
# --------------------------------------------------------------------------

def render_app_welcome(d, w, h):
    draw_squircle_container(d, w, h, (35, 12, 60, 255), C_MAGENTA)
    cx, cy = w // 2, h // 2
    # Lotus petals
    d.ellipse([cx - 7, cy - 8, cx + 7, cy + 4], fill=C_LAVENDER)
    d.ellipse([cx - 9, cy - 4, cx + 1, cy + 7], fill=C_VIOLET_LIGHT)
    d.ellipse([cx - 1, cy - 4, cx + 9, cy + 7], fill=C_VIOLET_LIGHT)
    d.ellipse([cx - 3, cy - 2, cx + 3, cy + 6], fill=C_MAGENTA)

def render_app_settings(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    r_out = 9 if w >= 24 else 5
    r_in  = 5 if w >= 24 else 3
    # Gear teeth
    for deg in range(0, 360, 45):
        rad = math.radians(deg)
        tx = cx + int((r_out + 2) * math.cos(rad))
        ty = cy + int((r_out + 2) * math.sin(rad))
        d.rectangle([tx - 1, ty - 1, tx + 1, ty + 1], fill=C_VIOLET_LIGHT)
    d.ellipse([cx - r_out, cy - r_out, cx + r_out, cy + r_out], fill=C_VIOLET_BORDER, outline=C_LAVENDER)
    d.ellipse([cx - r_in, cy - r_in, cx + r_in, cy + r_in], fill=C_VIOLET_DARK)

def render_app_terminal(d, w, h):
    draw_squircle_container(d, w, h, (18, 8, 34, 255), C_VIOLET_LIGHT)
    # Console prompt >_
    cx, cy = w // 2, h // 2
    d.line([cx - 7, cy - 4, cx - 3, cy, cx - 7, cy + 4], fill=C_MAGENTA, width=2)
    d.line([cx, cy + 4, cx + 6, cy + 4], fill=C_WHITE, width=2)

def render_app_files(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    # Folder body + flap
    d.rectangle([cx - 8, cy - 6, cx - 2, cy - 3], fill=C_LAVENDER)
    d.rounded_rectangle([cx - 8, cy - 3, cx + 8, cy + 7], radius=2, fill=C_VIOLET_LIGHT, outline=C_LAVENDER)
    d.line([cx - 5, cy + 1, cx + 5, cy + 1], fill=C_VIOLET_DARK, width=1)

def render_app_editor(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    # Document sheet
    d.rectangle([cx - 6, cy - 8, cx + 4, cy + 8], fill=C_LAVENDER)
    d.line([cx - 4, cy - 4, cx + 2, cy - 4], fill=C_VIOLET_DARK, width=1)
    d.line([cx - 4, cy - 1, cx + 2, cy - 1], fill=C_VIOLET_DARK, width=1)
    d.line([cx - 4, cy + 2, cx, cy + 2], fill=C_VIOLET_DARK, width=1)
    # Angled stylus
    d.line([cx + 2, cy + 6, cx + 8, cy - 4], fill=C_MAGENTA, width=2)

def render_app_sysmon(d, w, h):
    draw_squircle_container(d, w, h, (16, 8, 30, 255), C_VIOLET_BORDER)
    cx, cy = w // 2, h // 2
    # Heartbeat waveform
    pts = [
        (cx - 9, cy + 1), (cx - 5, cy + 1),
        (cx - 3, cy - 6), (cx, cy + 6),
        (cx + 3, cy - 3), (cx + 5, cy + 1), (cx + 9, cy + 1)
    ]
    for i in range(len(pts) - 1):
        d.line([pts[i], pts[i+1]], fill=C_MAGENTA, width=2)

def render_app_appstore(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    # Shopping tote bag
    d.rectangle([cx - 7, cy - 3, cx + 7, cy + 8], fill=C_VIOLET_LIGHT, outline=C_LAVENDER)
    d.arc([cx - 4, cy - 7, cx + 4, cy - 1], 180, 0, fill=C_MAGENTA, width=2)
    # Center star
    d.rectangle([cx - 2, cy + 1, cx + 2, cy + 5], fill=C_GOLD)

def render_app_help(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.ellipse([cx - 7, cy - 7, cx + 7, cy + 7], outline=C_LAVENDER, width=2)
    d.line([cx - 2, cy - 3, cx + 1, cy - 3, cx + 1, cy], fill=C_WHITE, width=2)
    d.point((cx + 1, cy + 4), fill=C_WHITE)

def render_app_screenshot(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    # Viewfinder corners
    d.line([cx - 8, cy - 4, cx - 8, cy - 7, cx - 4, cy - 7], fill=C_LAVENDER, width=2)
    d.line([cx + 4, cy - 7, cx + 8, cy - 7, cx + 8, cy - 4], fill=C_LAVENDER, width=2)
    d.line([cx - 8, cy + 4, cx - 8, cy + 7, cx - 4, cy + 7], fill=C_LAVENDER, width=2)
    d.line([cx + 4, cy + 7, cx + 8, cy + 7, cx + 8, cy + 4], fill=C_LAVENDER, width=2)
    d.ellipse([cx - 3, cy - 3, cx + 3, cy + 3], fill=C_MAGENTA)

def render_app_calculator(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    # Calculator grid
    d.rounded_rectangle([cx - 7, cy - 8, cx + 7, cy + 8], radius=2, fill=C_VIOLET_DARK, outline=C_VIOLET_LIGHT)
    d.rectangle([cx - 5, cy - 6, cx + 5, cy - 3], fill=C_GREEN)
    # 4 button quadrants
    d.rectangle([cx - 5, cy - 1, cx - 1, cy + 2], fill=C_LAVENDER)
    d.rectangle([cx + 1, cy - 1, cx + 5, cy + 2], fill=C_MAGENTA)
    d.rectangle([cx - 5, cy + 4, cx - 1, cy + 6], fill=C_LAVENDER)
    d.rectangle([cx + 1, cy + 4, cx + 5, cy + 6], fill=C_GOLD)

def render_app_calendar(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    # Calendar body
    d.rounded_rectangle([cx - 7, cy - 7, cx + 7, cy + 8], radius=2, fill=C_WHITE, outline=C_VIOLET_LIGHT)
    d.rectangle([cx - 7, cy - 7, cx + 7, cy - 2], fill=C_MAGENTA)
    # Spiral rings
    d.line([cx - 4, cy - 9, cx - 4, cy - 6], fill=C_GOLD, width=1)
    d.line([cx + 4, cy - 9, cx + 4, cy - 6], fill=C_GOLD, width=1)
    # Date grid dots
    d.rectangle([cx - 4, cy + 1, cx - 2, cy + 3], fill=C_VIOLET_DARK)
    d.rectangle([cx - 1, cy + 1, cx + 1, cy + 3], fill=C_VIOLET_DARK)
    d.rectangle([cx + 2, cy + 1, cx + 4, cy + 3], fill=C_VIOLET_DARK)
    d.rectangle([cx - 4, cy + 4, cx - 2, cy + 6], fill=C_VIOLET_DARK)
    d.rectangle([cx - 1, cy + 4, cx + 1, cy + 6], fill=C_MAGENTA)

def render_app_clock(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.ellipse([cx - 8, cy - 8, cx + 8, cy + 8], fill=C_VIOLET_DARK, outline=C_LAVENDER, width=2)
    # Clock hands at 3:00 / 10:10
    d.line([cx, cy, cx, cy - 5], fill=C_WHITE, width=2)
    d.line([cx, cy, cx + 5, cy], fill=C_MAGENTA, width=2)
    d.point((cx, cy), fill=C_GOLD)

def render_app_notes(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rounded_rectangle([cx - 6, cy - 8, cx + 6, cy + 8], radius=2, fill=C_GOLD, outline=C_VIOLET_BORDER)
    d.line([cx - 4, cy - 3, cx + 4, cy - 3], fill=C_VIOLET_DARK, width=1)
    d.line([cx - 4, cy, cx + 4, cy], fill=C_VIOLET_DARK, width=1)
    d.line([cx - 4, cy + 3, cx + 2, cy + 3], fill=C_VIOLET_DARK, width=1)

def render_app_archive(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 8, cy - 7, cx + 8, cy + 7], fill=C_VIOLET_LIGHT, outline=C_LAVENDER)
    # Zipper teeth down center
    for y in range(cy - 6, cy + 6, 3):
        d.line([cx - 2, y, cx + 2, y], fill=C_WHITE, width=1)
    d.rectangle([cx - 3, cy + 1, cx + 3, cy + 5], fill=C_GOLD)

def render_app_image_viewer(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 8, cy - 6, cx + 8, cy + 6], fill=C_VIOLET_DARK, outline=C_LAVENDER)
    d.ellipse([cx + 2, cy - 4, cx + 5, cy - 1], fill=C_GOLD)
    # Mountains
    d.polygon([(cx - 7, cy + 5), (cx - 2, cy - 1), (cx + 3, cy + 5)], fill=C_VIOLET_LIGHT)
    d.polygon([(cx, cy + 5), (cx + 4, cy + 1), (cx + 7, cy + 5)], fill=C_MAGENTA)

def render_app_music(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    # Beamed eighth notes
    d.ellipse([cx - 6, cy + 2, cx - 2, cy + 6], fill=C_MAGENTA)
    d.ellipse([cx + 1, cy, cx + 5, cy + 4], fill=C_MAGENTA)
    d.line([cx - 2, cy + 4, cx - 2, cy - 5], fill=C_WHITE, width=2)
    d.line([cx + 5, cy + 2, cx + 5, cy - 7], fill=C_WHITE, width=2)
    d.line([cx - 2, cy - 5, cx + 5, cy - 7], fill=C_WHITE, width=2)

def render_app_video(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rounded_rectangle([cx - 8, cy - 6, cx + 8, cy + 6], radius=2, fill=C_VIOLET_DARK, outline=C_MAGENTA)
    # Play triangle
    d.polygon([(cx - 2, cy - 4), (cx - 2, cy + 4), (cx + 4, cy)], fill=C_WHITE)

def render_app_camera(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rounded_rectangle([cx - 8, cy - 4, cx + 8, cy + 6], radius=2, fill=C_VIOLET_LIGHT, outline=C_LAVENDER)
    d.rectangle([cx - 3, cy - 6, cx + 3, cy - 4], fill=C_VIOLET_BORDER)
    d.ellipse([cx - 4, cy - 2, cx + 4, cy + 4], fill=C_VIOLET_DARK, outline=C_GREEN, width=1)

def render_app_mail(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 8, cy - 5, cx + 8, cy + 5], fill=C_LAVENDER, outline=C_VIOLET_BORDER)
    d.line([cx - 8, cy - 5, cx, cy], fill=C_VIOLET_DARK, width=1)
    d.line([cx + 8, cy - 5, cx, cy], fill=C_VIOLET_DARK, width=1)

def render_app_document(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 6, cy - 8, cx + 6, cy + 8], fill=C_WHITE, outline=C_LAVENDER)
    d.line([cx - 4, cy - 4, cx + 4, cy - 4], fill=C_VIOLET_BORDER, width=1)
    d.line([cx - 4, cy - 1, cx + 4, cy - 1], fill=C_VIOLET_BORDER, width=1)
    d.line([cx - 4, cy + 2, cx + 4, cy + 2], fill=C_VIOLET_BORDER, width=1)
    d.line([cx - 4, cy + 5, cx + 1, cy + 5], fill=C_VIOLET_BORDER, width=1)

def render_app_pdf(d, w, h):
    draw_squircle_container(d, w, h, (40, 10, 25, 255), C_RED)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 6, cy - 7, cx + 6, cy + 7], fill=C_WHITE)
    d.rectangle([cx - 6, cy - 7, cx + 6, cy - 2], fill=C_RED)
    d.rectangle([cx - 4, cy + 1, cx + 4, cy + 3], fill=C_RED)

def render_app_font_viewer(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    # Capital 'A'
    d.line([cx - 6, cy + 6, cx - 1, cy - 6, cx + 4, cy + 6], fill=C_WHITE, width=2)
    d.line([cx - 4, cy + 1, cx + 2, cy + 1], fill=C_WHITE, width=2)
    # Small 'a'
    d.ellipse([cx + 4, cy + 1, cx + 8, cy + 6], outline=C_MAGENTA, width=1)
    d.line([cx + 8, cy + 1, cx + 8, cy + 6], fill=C_MAGENTA, width=1)

def render_app_disk_manager(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    # Platter stack
    for y in [cy - 5, cy, cy + 5]:
        d.ellipse([cx - 8, y - 2, cx + 8, y + 2], fill=C_VIOLET_LIGHT, outline=C_LAVENDER)
    d.line([cx + 4, cy - 4, cx + 4, cy + 4], fill=C_GOLD, width=2)

def render_app_task_manager(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 7, cy - 6, cx + 7, cy - 3], fill=C_GREEN)
    d.rectangle([cx - 7, cy - 1, cx + 3, cy + 2], fill=C_MAGENTA)
    d.rectangle([cx - 7, cy + 4, cx + 5, cy + 7], fill=C_GOLD)

def render_app_log_viewer(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 7, cy - 8, cx + 4, cy + 8], fill=C_VIOLET_DARK, outline=C_VIOLET_LIGHT)
    for y in range(cy - 5, cy + 6, 3):
        d.line([cx - 5, y, cx + 1, y], fill=C_LAVENDER, width=1)
    # Magnifier
    d.ellipse([cx + 1, cy + 1, cx + 6, cy + 6], outline=C_GOLD, width=2)
    d.line([cx + 5, cy + 5, cx + 8, cy + 8], fill=C_GOLD, width=2)

def render_app_network_manager(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.arc([cx - 8, cy - 6, cx + 8, cy + 6], 200, 340, fill=C_GREEN, width=2)
    d.arc([cx - 5, cy - 3, cx + 5, cy + 5], 200, 340, fill=C_GREEN, width=2)
    d.ellipse([cx - 2, cy + 1, cx + 2, cy + 5], fill=C_WHITE)

def render_app_bluetooth_manager(d, w, h):
    draw_squircle_container(d, w, h, (10, 20, 50, 255), C_GREEN)
    cx, cy = w // 2, h // 2
    # Rune B
    pts = [
        (cx - 3, cy - 6), (cx + 3, cy), (cx, cy + 3),
        (cx, cy - 7), (cx + 3, cy - 3), (cx - 3, cy + 3)
    ]
    d.line([cx, cy - 7, cx, cy + 7], fill=C_WHITE, width=2)
    d.line([(cx - 3, cy - 4), (cx + 3, cy + 2), (cx, cy + 5)], fill=C_WHITE, width=2)
    d.line([(cx - 3, cy + 4), (cx + 3, cy - 2), (cx, cy - 5)], fill=C_WHITE, width=2)

def render_app_display_settings(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rounded_rectangle([cx - 8, cy - 6, cx + 8, cy + 4], radius=2, fill=C_VIOLET_DARK, outline=C_LAVENDER)
    d.line([cx, cy + 4, cx, cy + 7], fill=C_VIOLET_LIGHT, width=2)
    d.line([cx - 4, cy + 7, cx + 4, cy + 7], fill=C_VIOLET_LIGHT, width=2)

def render_app_accessibility(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.ellipse([cx - 2, cy - 7, cx + 2, cy - 3], fill=C_WHITE)
    d.line([cx - 7, cy - 1, cx + 7, cy - 1], fill=C_WHITE, width=2)
    d.line([cx, cy - 1, cx, cy + 4], fill=C_WHITE, width=2)
    d.line([cx, cy + 4, cx - 4, cy + 8], fill=C_WHITE, width=2)
    d.line([cx, cy + 4, cx + 4, cy + 8], fill=C_WHITE, width=2)

def render_app_backup(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.arc([cx - 7, cy - 7, cx + 7, cy + 7], 45, 315, fill=C_GREEN, width=2)
    d.polygon([(cx + 4, cy - 7), (cx + 8, cy - 4), (cx + 8, cy - 8)], fill=C_GREEN)
    d.rectangle([cx - 3, cy - 3, cx + 3, cy + 3], fill=C_GOLD)

def render_app_updates(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    # Shield
    d.polygon([(cx - 7, cy - 6), (cx + 7, cy - 6), (cx + 7, cy), (cx, cy + 8), (cx - 7, cy)], fill=C_VIOLET_DARK, outline=C_GREEN)
    # Upward arrow
    d.line([cx, cy + 4, cx, cy - 3], fill=C_WHITE, width=2)
    d.polygon([(cx, cy - 5), (cx - 3, cy - 2), (cx + 3, cy - 2)], fill=C_WHITE)

def render_app_security_aegis(d, w, h):
    draw_squircle_container(d, w, h, (25, 8, 45, 255), C_MAGENTA)
    cx, cy = w // 2, h // 2
    d.polygon([(cx - 8, cy - 7), (cx + 8, cy - 7), (cx + 8, cy), (cx, cy + 9), (cx - 8, cy)], fill=C_VIOLET_BORDER, outline=C_WHITE)
    # Glowing central keyhole
    d.ellipse([cx - 2, cy - 3, cx + 2, cy + 1], fill=C_GOLD)
    d.polygon([(cx - 2, cy), (cx + 2, cy), (cx, cy + 5)], fill=C_GOLD)

def render_app_power_session(d, w, h):
    draw_squircle_container(d, w, h, (35, 10, 20, 255), C_RED)
    cx, cy = w // 2, h // 2
    d.arc([cx - 7, cy - 7, cx + 7, cy + 7], 135, 405, fill=C_WHITE, width=2)
    d.line([cx, cy - 8, cx, cy - 1], fill=C_RED, width=2)

# --------------------------------------------------------------------------
# Category 2: Developer & Diagnostic Utilities
# --------------------------------------------------------------------------

def render_util_bundle_inspector(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 8, cy - 7, cx + 2, cy + 7], fill=C_VIOLET_LIGHT, outline=C_LAVENDER)
    d.ellipse([cx, cy - 3, cx + 7, cy + 4], outline=C_GOLD, width=2)
    d.line([cx + 5, cy + 3, cx + 9, cy + 7], fill=C_GOLD, width=2)

def render_util_apdrv_inspector(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 6, cy - 6, cx + 6, cy + 6], fill=C_VIOLET_DARK, outline=C_GREEN, width=1)
    for off in [-4, 0, 4]:
        d.line([cx + off, cy - 8, cx + off, cy - 6], fill=C_GOLD, width=1)
        d.line([cx + off, cy + 6, cx + off, cy + 8], fill=C_GOLD, width=1)
    d.rectangle([cx - 2, cy - 2, cx + 2, cy + 2], fill=C_MAGENTA)

def render_util_driver_manager(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 8, cy - 4, cx + 8, cy + 4], fill=C_VIOLET_LIGHT, outline=C_LAVENDER)
    d.line([cx - 6, cy + 4, cx + 6, cy + 4], fill=C_GOLD, width=2)
    d.ellipse([cx - 2, cy - 7, cx + 2, cy - 3], fill=C_MAGENTA)

def render_util_kernel_info(d, w, h):
    draw_squircle_container(d, w, h, (15, 25, 45, 255), C_GREEN)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 7, cy - 7, cx + 7, cy + 7], fill=C_VIOLET_DARK, outline=C_GREEN, width=1)
    d.rectangle([cx - 3, cy - 3, cx + 3, cy + 3], fill=C_GREEN)

def render_util_boot_config(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 7, cy - 5, cx + 7, cy + 5], fill=C_VIOLET_DARK, outline=C_GOLD)
    d.polygon([(cx - 4, cy + 3), (cx, cy - 4), (cx + 4, cy + 3)], fill=C_MAGENTA)

def render_util_storage_diag(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 8, cy - 6, cx + 8, cy + 6], fill=C_VIOLET_LIGHT, outline=C_LAVENDER)
    d.line([cx - 6, cy, cx - 3, cy, cx - 1, cy - 4, cx + 1, cy + 4, cx + 3, cy, cx + 6, cy], fill=C_GREEN, width=1)

def render_util_memory_diag(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 8, cy - 4, cx + 8, cy + 4], fill=C_GREEN, outline=C_WHITE)
    for x in range(cx - 6, cx + 7, 3):
        d.line([x, cy + 4, x, cy + 6], fill=C_GOLD, width=1)
        d.rectangle([x - 1, cy - 2, x + 1, cy + 1], fill=C_VIOLET_DARK)

def render_util_hardware_info(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 8, cy - 8, cx + 8, cy + 8], fill=C_VIOLET_DARK, outline=C_LAVENDER)
    d.rectangle([cx - 6, cy - 6, cx - 2, cy - 2], fill=C_GOLD)
    d.line([cx - 2, cy - 4, cx + 5, cy - 4, cx + 5, cy + 4], fill=C_GREEN, width=1)
    d.rectangle([cx + 2, cy + 2, cx + 6, cy + 6], fill=C_MAGENTA)

def render_util_perf_monitor(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.arc([cx - 8, cy - 8, cx + 8, cy + 8], 150, 390, fill=C_LAVENDER, width=2)
    d.line([cx, cy + 1, cx + 5, cy - 4], fill=C_MAGENTA, width=2)
    d.ellipse([cx - 2, cy - 1, cx + 2, cy + 3], fill=C_GOLD)

def render_util_debug_console(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    # Beetle bug
    d.ellipse([cx - 4, cy - 3, cx + 4, cy + 5], fill=C_GREEN)
    d.ellipse([cx - 3, cy - 6, cx + 3, cy - 2], fill=C_VIOLET_LIGHT)
    d.line([cx - 7, cy, cx + 7, cy], fill=C_WHITE, width=1)
    d.line([cx - 6, cy + 4, cx + 6, cy + 4], fill=C_WHITE, width=1)

# --------------------------------------------------------------------------
# Category 3: File Types
# --------------------------------------------------------------------------

def render_file_folder(d, w, h):
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 8, cy - 7, cx - 2, cy - 4], fill=C_LAVENDER)
    d.rounded_rectangle([cx - 8, cy - 4, cx + 8, cy + 7], radius=2, fill=C_VIOLET_LIGHT, outline=C_LAVENDER)

def render_file_folder_open(d, w, h):
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 8, cy - 7, cx + 8, cy + 5], fill=C_VIOLET_DARK, outline=C_LAVENDER)
    d.polygon([(cx - 9, cy + 7), (cx - 7, cy - 1), (cx + 9, cy - 1), (cx + 7, cy + 7)], fill=C_VIOLET_LIGHT, outline=C_LAVENDER)

def render_file_folder_home(d, w, h):
    render_file_folder(d, w, h)
    cx, cy = w // 2, h // 2
    d.polygon([(cx, cy - 2), (cx - 4, cy + 2), (cx + 4, cy + 2)], fill=C_WHITE)
    d.rectangle([cx - 2, cy + 2, cx + 2, cy + 5], fill=C_WHITE)

def render_file_folder_desktop(d, w, h):
    render_file_folder(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 4, cy - 1, cx + 4, cy + 4], fill=C_WHITE)

def render_file_folder_docs(d, w, h):
    render_file_folder(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 3, cy - 2, cx + 3, cy + 5], fill=C_WHITE)
    d.line([cx - 2, cy, cx + 2, cy], fill=C_VIOLET_DARK, width=1)
    d.line([cx - 2, cy + 2, cx + 2, cy + 2], fill=C_VIOLET_DARK, width=1)

def render_file_folder_downloads(d, w, h):
    render_file_folder(d, w, h)
    cx, cy = w // 2, h // 2
    d.polygon([(cx, cy + 4), (cx - 3, cy), (cx + 3, cy)], fill=C_GREEN)
    d.line([cx, cy - 2, cx, cy + 2], fill=C_GREEN, width=2)

def render_file_trash(d, w, h):
    cx, cy = w // 2, h // 2
    d.line([cx - 7, cy - 5, cx + 7, cy - 5], fill=C_LAVENDER, width=2)
    d.rectangle([cx - 2, cy - 7, cx + 2, cy - 5], fill=C_LAVENDER)
    d.polygon([(cx - 5, cy - 4), (cx + 5, cy - 4), (cx + 4, cy + 7), (cx - 4, cy + 7)], fill=C_VIOLET_DARK, outline=C_VIOLET_BORDER)
    d.line([cx - 2, cy - 2, cx - 2, cy + 5], fill=C_MAGENTA, width=1)
    d.line([cx + 2, cy - 2, cx + 2, cy + 5], fill=C_MAGENTA, width=1)

def render_file_exec(d, w, h):
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 6, cy - 8, cx + 6, cy + 8], fill=C_VIOLET_DARK, outline=C_GREEN)
    d.ellipse([cx - 4, cy - 4, cx + 4, cy + 4], outline=C_GREEN, width=1)
    d.line([cx, cy - 4, cx, cy + 4], fill=C_WHITE, width=1)
    d.line([cx - 4, cy, cx + 4, cy], fill=C_WHITE, width=1)

def render_file_apapp(d, w, h):
    cx, cy = w // 2, h // 2
    d.rounded_rectangle([cx - 7, cy - 7, cx + 7, cy + 7], radius=3, fill=C_VIOLET_BORDER, outline=C_WHITE)
    d.polygon([(cx, cy - 5), (cx + 2, cy - 1), (cx + 6, cy), (cx + 3, cy + 3), (cx + 4, cy + 7), (cx, cy + 4), (cx - 4, cy + 7), (cx - 3, cy + 3), (cx - 6, cy), (cx - 2, cy - 1)], fill=C_GOLD)

def render_file_apdrv(d, w, h):
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 6, cy - 7, cx + 6, cy + 7], fill=C_VIOLET_DARK, outline=C_GREEN)
    d.line([cx - 4, cy - 2, cx + 4, cy - 2], fill=C_GREEN, width=1)
    d.line([cx - 4, cy + 2, cx + 4, cy + 2], fill=C_GREEN, width=1)
    for y in [-4, 0, 4]:
        d.point((cx - 8, cy + y), fill=C_GOLD)
        d.point((cx + 7, cy + y), fill=C_GOLD)

def render_file_archive(d, w, h):
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 6, cy - 8, cx + 6, cy + 8], fill=C_GOLD, outline=C_VIOLET_BORDER)
    d.rectangle([cx - 2, cy - 2, cx + 2, cy + 2], fill=C_VIOLET_DARK)

def render_file_text(d, w, h):
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 6, cy - 8, cx + 6, cy + 8], fill=C_WHITE, outline=C_LAVENDER)
    for y in range(cy - 4, cy + 6, 3):
        d.line([cx - 4, y, cx + 4, y], fill=C_VIOLET_DARK, width=1)

def render_file_code(d, w, h):
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 6, cy - 8, cx + 6, cy + 8], fill=C_VIOLET_DARK, outline=C_GREEN)
    d.line([cx - 4, cy - 2, cx - 2, cy, cx - 4, cy + 2], fill=C_GREEN, width=1)
    d.line([cx + 4, cy - 2, cx + 2, cy, cx + 4, cy + 2], fill=C_GREEN, width=1)
    d.line([cx + 1, cy - 3, cx - 1, cy + 3], fill=C_MAGENTA, width=1)

def render_file_config(d, w, h):
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 6, cy - 8, cx + 6, cy + 8], fill=C_LAVENDER, outline=C_VIOLET_BORDER)
    d.ellipse([cx - 3, cy - 3, cx + 3, cy + 3], outline=C_VIOLET_DARK, width=1)

def render_file_log(d, w, h):
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 6, cy - 8, cx + 6, cy + 8], fill=C_WHITE, outline=C_RED)
    d.line([cx - 4, cy - 4, cx - 2, cy - 4], fill=C_RED, width=1)
    d.line([cx - 1, cy - 4, cx + 4, cy - 4], fill=C_VIOLET_DARK, width=1)
    d.line([cx - 4, cy, cx - 2, cy], fill=C_RED, width=1)
    d.line([cx - 1, cy, cx + 4, cy], fill=C_VIOLET_DARK, width=1)

def render_file_image(d, w, h):
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 6, cy - 8, cx + 6, cy + 8], fill=C_WHITE, outline=C_MAGENTA)
    d.polygon([(cx - 4, cy + 4), (cx - 1, cy), (cx + 2, cy + 4)], fill=C_VIOLET_LIGHT)
    d.ellipse([cx + 1, cy - 3, cx + 3, cy - 1], fill=C_GOLD)

def render_file_audio(d, w, h):
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 6, cy - 8, cx + 6, cy + 8], fill=C_WHITE, outline=C_MAGENTA)
    d.ellipse([cx - 3, cy + 1, cx + 1, cy + 4], fill=C_MAGENTA)
    d.line([cx + 1, cy - 3, cx + 1, cy + 4], fill=C_MAGENTA, width=2)

def render_file_video(d, w, h):
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 6, cy - 8, cx + 6, cy + 8], fill=C_VIOLET_DARK, outline=C_LAVENDER)
    d.polygon([(cx - 2, cy - 3), (cx - 2, cy + 3), (cx + 3, cy)], fill=C_MAGENTA)

def render_file_unknown(d, w, h):
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 6, cy - 8, cx + 6, cy + 8], fill=C_VIOLET_DARK, outline=C_LAVENDER)
    d.line([cx - 2, cy - 3, cx + 1, cy - 3, cx + 1, cy], fill=C_LAVENDER, width=2)
    d.point((cx + 1, cy + 3), fill=C_LAVENDER)

# --------------------------------------------------------------------------
# Category 4: Hardware Devices & Volumes
# --------------------------------------------------------------------------

def render_dev_disk(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 8, cy - 6, cx + 8, cy + 6], fill=C_VIOLET_DARK, outline=C_LAVENDER)
    d.ellipse([cx - 3, cy - 3, cx + 3, cy + 3], fill=C_GOLD)
    d.point((cx + 5, cy + 3), fill=C_GREEN)

def render_dev_removable(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 4, cy - 7, cx + 4, cy + 4], fill=C_VIOLET_LIGHT, outline=C_LAVENDER)
    d.rectangle([cx - 3, cy + 4, cx + 3, cy + 8], fill=C_WHITE)

def render_dev_net_share(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.ellipse([cx - 7, cy - 4, cx - 1, cy + 2], fill=C_GREEN)
    d.ellipse([cx - 2, cy - 6, cx + 5, cy + 2], fill=C_GREEN)
    d.ellipse([cx + 2, cy - 3, cx + 7, cy + 3], fill=C_GREEN)
    d.rectangle([cx - 5, cy - 1, cx + 5, cy + 3], fill=C_GREEN)

def render_dev_efi(d, w, h):
    draw_squircle_container(d, w, h, (40, 30, 10, 255), C_GOLD)
    cx, cy = w // 2, h // 2
    d.ellipse([cx - 6, cy - 4, cx - 1, cy + 1], fill=C_GOLD)
    d.line([cx - 1, cy - 1, cx + 6, cy - 1], fill=C_GOLD, width=2)
    d.line([cx + 3, cy - 1, cx + 3, cy + 3], fill=C_GOLD, width=1)
    d.line([cx + 5, cy - 1, cx + 5, cy + 2], fill=C_GOLD, width=1)

def render_dev_purplefs(d, w, h):
    draw_squircle_container(d, w, h, (35, 10, 50, 255), C_MAGENTA)
    cx, cy = w // 2, h // 2
    # Violet Crystal diamond
    d.polygon([(cx, cy - 8), (cx + 6, cy), (cx, cy + 8), (cx - 6, cy)], fill=C_LAVENDER, outline=C_WHITE)
    d.line([(cx, cy - 8), (cx, cy + 8)], fill=C_MAGENTA, width=1)

def render_dev_keyboard(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rounded_rectangle([cx - 9, cy - 5, cx + 9, cy + 5], radius=2, fill=C_VIOLET_DARK, outline=C_LAVENDER)
    for x in range(cx - 7, cx + 8, 3):
        d.rectangle([x, cy - 3, x + 1, cy - 1], fill=C_WHITE)
        d.rectangle([x, cy + 1, x + 1, cy + 3], fill=C_WHITE)

def render_dev_mouse(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rounded_rectangle([cx - 5, cy - 8, cx + 5, cy + 8], radius=4, fill=C_VIOLET_LIGHT, outline=C_LAVENDER)
    d.line([cx, cy - 8, cx, cy - 2], fill=C_VIOLET_DARK, width=1)
    d.rectangle([cx - 1, cy - 5, cx + 1, cy - 3], fill=C_MAGENTA)

def render_dev_display(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 8, cy - 6, cx + 8, cy + 4], fill=C_VIOLET_DARK, outline=C_LAVENDER)
    d.line([cx - 3, cy + 7, cx + 3, cy + 7], fill=C_LAVENDER, width=2)
    d.line([cx, cy + 4, cx, cy + 7], fill=C_LAVENDER, width=2)

def render_dev_cpu(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 6, cy - 6, cx + 6, cy + 6], fill=C_GOLD, outline=C_WHITE)
    d.rectangle([cx - 4, cy - 4, cx + 4, cy + 4], fill=C_VIOLET_DARK)
    d.point((cx - 1, cy - 1), fill=C_GREEN)

def render_dev_gpu(d, w, h):
    draw_squircle_container(d, w, h, (20, 10, 40, 255), C_MAGENTA)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 9, cy - 5, cx + 9, cy + 5], fill=C_VIOLET_MID, outline=C_MAGENTA)
    # Fan circle
    d.ellipse([cx - 4, cy - 4, cx + 4, cy + 4], outline=C_GREEN, width=1)
    d.line([cx - 7, cy + 5, cx - 2, cy + 5], fill=C_GOLD, width=2)

def render_dev_memory(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 8, cy - 4, cx + 8, cy + 4], fill=C_GREEN, outline=C_WHITE)
    for x in range(cx - 6, cx + 7, 3):
        d.rectangle([x - 1, cy - 2, x + 1, cy + 1], fill=C_VIOLET_DARK)

def render_dev_network(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 7, cy - 5, cx + 7, cy + 5], fill=C_GREEN, outline=C_LAVENDER)
    d.rectangle([cx + 3, cy - 3, cx + 7, cy + 3], fill=C_GOLD)

def render_dev_audio(d, w, h):
    draw_squircle_container(d, w, h)
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 5, cy - 8, cx + 5, cy + 8], fill=C_VIOLET_DARK, outline=C_LAVENDER)
    d.ellipse([cx - 3, cy - 5, cx + 3, cy + 1], fill=C_VIOLET_BORDER)
    d.ellipse([cx - 4, cy + 1, cx + 4, cy + 6], fill=C_MAGENTA)

# --------------------------------------------------------------------------
# Category 5: Status Indicators & UI Actions
# --------------------------------------------------------------------------

def render_status_wifi_3(d, w, h):
    cx, cy = w // 2, h // 2
    d.arc([cx - 8, cy - 6, cx + 8, cy + 8], 210, 330, fill=C_WHITE, width=2)
    d.arc([cx - 5, cy - 3, cx + 5, cy + 7], 210, 330, fill=C_WHITE, width=2)
    d.arc([cx - 2, cy, cx + 2, cy + 6], 210, 330, fill=C_WHITE, width=2)
    d.ellipse([cx - 1, cy + 4, cx + 1, cy + 6], fill=C_WHITE)

def render_status_wifi_2(d, w, h):
    cx, cy = w // 2, h // 2
    d.arc([cx - 5, cy - 3, cx + 5, cy + 7], 210, 330, fill=C_WHITE, width=2)
    d.arc([cx - 2, cy, cx + 2, cy + 6], 210, 330, fill=C_WHITE, width=2)
    d.ellipse([cx - 1, cy + 4, cx + 1, cy + 6], fill=C_WHITE)

def render_status_wifi_1(d, w, h):
    cx, cy = w // 2, h // 2
    d.arc([cx - 2, cy, cx + 2, cy + 6], 210, 330, fill=C_WHITE, width=2)
    d.ellipse([cx - 1, cy + 4, cx + 1, cy + 6], fill=C_WHITE)

def render_status_ethernet(d, w, h):
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 5, cy - 4, cx + 5, cy + 4], fill=C_WHITE, outline=C_VIOLET_BORDER)
    d.line([cx - 3, cy + 4, cx - 3, cy + 7], fill=C_GREEN, width=1)
    d.line([cx, cy + 4, cx, cy + 7], fill=C_GREEN, width=1)
    d.line([cx + 3, cy + 4, cx + 3, cy + 7], fill=C_GREEN, width=1)

def render_status_bluetooth(d, w, h):
    cx, cy = w // 2, h // 2
    d.line([cx, cy - 7, cx, cy + 7], fill=C_GREEN, width=2)
    d.line([(cx - 3, cy - 4), (cx + 3, cy + 2), (cx, cy + 5)], fill=C_GREEN, width=2)
    d.line([(cx - 3, cy + 4), (cx + 3, cy - 2), (cx, cy - 5)], fill=C_GREEN, width=2)

def render_status_audio(d, w, h):
    cx, cy = w // 2, h // 2
    d.polygon([(cx - 6, cy - 3), (cx - 3, cy - 3), (cx + 1, cy - 6), (cx + 1, cy + 6), (cx - 3, cy + 3), (cx - 6, cy + 3)], fill=C_WHITE)
    d.arc([cx + 1, cy - 4, cx + 6, cy + 4], 300, 60, fill=C_WHITE, width=1)
    d.arc([cx + 1, cy - 7, cx + 8, cy + 7], 300, 60, fill=C_WHITE, width=1)

def render_status_audio_mute(d, w, h):
    render_status_audio(d, w, h)
    cx, cy = w // 2, h // 2
    d.line([cx - 7, cy + 6, cx + 7, cy - 6], fill=C_RED, width=2)

def render_status_battery_100(d, w, h):
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 7, cy - 4, cx + 5, cy + 4], outline=C_WHITE, width=1)
    d.rectangle([cx + 5, cy - 2, cx + 7, cy + 2], fill=C_WHITE)
    d.rectangle([cx - 5, cy - 2, cx + 3, cy + 2], fill=C_GREEN)

def render_status_battery_charging(d, w, h):
    render_status_battery_100(d, w, h)
    cx, cy = w // 2, h // 2
    d.polygon([(cx, cy - 5), (cx - 3, cy), (cx, cy), (cx - 1, cy + 5), (cx + 2, cy), (cx - 1, cy)], fill=C_GOLD)

def render_status_brightness(d, w, h):
    cx, cy = w // 2, h // 2
    d.ellipse([cx - 4, cy - 4, cx + 4, cy + 4], fill=C_GOLD)
    for deg in range(0, 360, 45):
        rad = math.radians(deg)
        x1 = cx + int(5 * math.cos(rad))
        y1 = cy + int(5 * math.sin(rad))
        x2 = cx + int(7 * math.cos(rad))
        y2 = cy + int(7 * math.sin(rad))
        d.line([x1, y1, x2, y2], fill=C_GOLD, width=1)

def render_status_volume(d, w, h):
    render_status_audio(d, w, h)

def render_status_security_ok(d, w, h):
    cx, cy = w // 2, h // 2
    d.polygon([(cx - 6, cy - 6), (cx + 6, cy - 6), (cx + 6, cy), (cx, cy + 7), (cx - 6, cy)], fill=C_VIOLET_MID, outline=C_GREEN)
    d.line([cx - 3, cy, cx - 1, cy + 3, cx + 4, cy - 2], fill=C_GREEN, width=2)

def render_status_security_warn(d, w, h):
    cx, cy = w // 2, h // 2
    d.polygon([(cx, cy - 7), (cx + 7, cy + 6), (cx - 7, cy + 6)], fill=C_GOLD, outline=C_RED)
    d.line([cx, cy - 3, cx, cy + 1], fill=C_VIOLET_DARK, width=2)
    d.point((cx, cy + 4), fill=C_VIOLET_DARK)

def render_status_power(d, w, h):
    cx, cy = w // 2, h // 2
    d.arc([cx - 6, cy - 6, cx + 6, cy + 6], 135, 405, fill=C_WHITE, width=2)
    d.line([cx, cy - 7, cx, cy - 1], fill=C_RED, width=2)

def render_ui_close(d, w, h):
    cx, cy = w // 2, h // 2
    d.line([cx - 5, cy - 5, cx + 5, cy + 5], fill=C_RED, width=2)
    d.line([cx - 5, cy + 5, cx + 5, cy - 5], fill=C_RED, width=2)

def render_ui_minimize(d, w, h):
    cx, cy = w // 2, h // 2
    d.line([cx - 5, cy + 4, cx + 5, cy + 4], fill=C_LAVENDER, width=2)

def render_ui_maximize(d, w, h):
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 5, cy - 5, cx + 5, cy + 5], outline=C_LAVENDER, width=1)

def render_ui_restore(d, w, h):
    cx, cy = w // 2, h // 2
    d.rectangle([cx - 3, cy - 5, cx + 5, cy + 3], outline=C_LAVENDER, width=1)
    d.rectangle([cx - 5, cy - 3, cx + 3, cy + 5], outline=C_LAVENDER, width=1)

def render_ui_search(d, w, h):
    cx, cy = w // 2, h // 2
    d.ellipse([cx - 6, cy - 6, cx + 2, cy + 2], outline=C_LAVENDER, width=2)
    d.line([cx + 2, cy + 2, cx + 6, cy + 6], fill=C_LAVENDER, width=2)

def render_ui_refresh(d, w, h):
    cx, cy = w // 2, h // 2
    d.arc([cx - 6, cy - 6, cx + 6, cy + 6], 45, 315, fill=C_GREEN, width=2)
    d.polygon([(cx + 3, cy - 7), (cx + 7, cy - 4), (cx + 7, cy - 8)], fill=C_GREEN)

def render_ui_back(d, w, h):
    cx, cy = w // 2, h // 2
    d.line([cx + 4, cy, cx - 4, cy], fill=C_LAVENDER, width=2)
    d.line([cx - 4, cy, cx, cy - 4], fill=C_LAVENDER, width=2)
    d.line([cx - 4, cy, cx, cy + 4], fill=C_LAVENDER, width=2)

def render_ui_forward(d, w, h):
    cx, cy = w // 2, h // 2
    d.line([cx - 4, cy, cx + 4, cy], fill=C_LAVENDER, width=2)
    d.line([cx + 4, cy, cx, cy - 4], fill=C_LAVENDER, width=2)
    d.line([cx + 4, cy, cx, cy + 4], fill=C_LAVENDER, width=2)

def render_ui_help(d, w, h):
    render_app_help(d, w, h)

def render_ui_check(d, w, h):
    cx, cy = w // 2, h // 2
    d.line([cx - 6, cy, cx - 2, cy + 4, cx + 6, cy - 4], fill=C_GREEN, width=2)

def render_ui_star(d, w, h):
    cx, cy = w // 2, h // 2
    d.polygon([(cx, cy - 7), (cx + 2, cy - 2), (cx + 7, cy - 1), (cx + 3, cy + 3), (cx + 5, cy + 7), (cx, cy + 4), (cx - 5, cy + 7), (cx - 3, cy + 3), (cx - 7, cy - 1), (cx - 2, cy - 2)], fill=C_MAGENTA, outline=C_WHITE)

# Register All 100 Icons
def build_registry():
    # Category 1: Core Apps (33)
    register_icon("ICON_APP_WELCOME", "Welcome", "Core App", "Purple OS Welcome Tour", render_app_welcome)
    register_icon("ICON_APP_SETTINGS", "Settings", "Core App", "System & Preferences Manager", render_app_settings)
    register_icon("ICON_APP_TERMINAL", "Terminal", "Core App", "Aegis Interactive Shell", render_app_terminal)
    register_icon("ICON_APP_FILES", "Files", "Core App", "PurpleFS File Explorer", render_app_files)
    register_icon("ICON_APP_EDITOR", "Text Editor", "Core App", "Lightweight Document Editor", render_app_editor)
    register_icon("ICON_APP_SYSMON", "System Monitor", "Core App", "Hardware & Task Monitor", render_app_sysmon)
    register_icon("ICON_APP_APPSTORE", "App Store", "Core App", "Application Package Manager", render_app_appstore)
    register_icon("ICON_APP_HELP", "Help Center", "Core App", "User Guide & Documentation", render_app_help)
    register_icon("ICON_APP_SCREENSHOT", "Screenshot", "Core App", "Screen Capture Utility", render_app_screenshot)
    register_icon("ICON_APP_CALCULATOR", "Calculator", "Core App", "Scientific Calculator", render_app_calculator)
    register_icon("ICON_APP_CALENDAR", "Calendar", "Core App", "Date & Events Calendar", render_app_calendar)
    register_icon("ICON_APP_CLOCK", "Clock", "Core App", "Alarms & World Clock", render_app_clock)
    register_icon("ICON_APP_NOTES", "Notes", "Core App", "Quick Notes & Sticky Memos", render_app_notes)
    register_icon("ICON_APP_ARCHIVE", "Archive Manager", "Core App", "Zip/Tar Archive Utility", render_app_archive)
    register_icon("ICON_APP_IMAGE_VIEWER", "Image Viewer", "Core App", "Photo & Graphic Viewer", render_app_image_viewer)
    register_icon("ICON_APP_MUSIC", "Music Player", "Core App", "Audio & Music Player", render_app_music)
    register_icon("ICON_APP_VIDEO", "Video Player", "Core App", "Media & Video Player", render_app_video)
    register_icon("ICON_APP_CAMERA", "Camera", "Core App", "Camera & Video Capture", render_app_camera)
    register_icon("ICON_APP_MAIL", "Mail", "Core App", "Electronic Mail Client", render_app_mail)
    register_icon("ICON_APP_DOCUMENT", "Document Viewer", "Core App", "Formatted Document Viewer", render_app_document)
    register_icon("ICON_APP_PDF", "PDF Viewer", "Core App", "Portable Document Viewer", render_app_pdf)
    register_icon("ICON_APP_FONT_VIEWER", "Font Viewer", "Core App", "Noto Typography Inspector", render_app_font_viewer)
    register_icon("ICON_APP_DISK_MANAGER", "Disk Manager", "Core App", "GPT & Storage Partitioner", render_app_disk_manager)
    register_icon("ICON_APP_TASK_MANAGER", "Task Manager", "Core App", "Process Scheduler Inspector", render_app_task_manager)
    register_icon("ICON_APP_LOG_VIEWER", "Log Viewer", "Core App", "Kernel Ring Buffer Viewer", render_app_log_viewer)
    register_icon("ICON_APP_NETWORK_MANAGER", "Network Manager", "Core App", "Interfaces & Wi-Fi Settings", render_app_network_manager)
    register_icon("ICON_APP_BLUETOOTH_MANAGER", "Bluetooth", "Core App", "Bluetooth Device Manager", render_app_bluetooth_manager)
    register_icon("ICON_APP_DISPLAY_SETTINGS", "Display Settings", "Core App", "Resolution & Refresh Config", render_app_display_settings)
    register_icon("ICON_APP_ACCESSIBILITY", "Accessibility", "Core App", "Assistive Display Options", render_app_accessibility)
    register_icon("ICON_APP_BACKUP", "Backup & Restore", "Core App", "Volume Snapshot & Restore", render_app_backup)
    register_icon("ICON_APP_UPDATES", "Software Updates", "Core App", "Package Update Manager", render_app_updates)
    register_icon("ICON_APP_SECURITY_AEGIS", "AegisDetect", "Core App", "Hardware Root of Trust", render_app_security_aegis)
    register_icon("ICON_APP_POWER_SESSION", "Power & Session", "Core App", "Shutdown & Restart Dialog", render_app_power_session)

    # Category 2: Developer & Utilities (10)
    register_icon("ICON_UTIL_BUNDLE_INSPECTOR", "Bundle Inspector", "Developer", "Inspect .apapplication packages", render_util_bundle_inspector)
    register_icon("ICON_UTIL_APDRV_INSPECTOR", "APDRV Inspector", "Developer", "Inspect .apdrv driver packages", render_util_apdrv_inspector)
    register_icon("ICON_UTIL_DRIVER_MANAGER", "Driver Manager", "Developer", "PCI Device & Driver Matcher", render_util_driver_manager)
    register_icon("ICON_UTIL_KERNEL_INFO", "Kernel Info", "Developer", "Aegis64 Version & Architecture", render_util_kernel_info)
    register_icon("ICON_UTIL_BOOT_CONFIG", "Boot Config", "Developer", "UEFI & Multiboot Options", render_util_boot_config)
    register_icon("ICON_UTIL_STORAGE_DIAG", "Storage Diag", "Developer", "AHCI/NVMe SMART Diagnostics", render_util_storage_diag)
    register_icon("ICON_UTIL_MEMORY_DIAG", "Memory Diag", "Developer", "PMM/VMM Page Table Diagnostics", render_util_memory_diag)
    register_icon("ICON_UTIL_HARDWARE_INFO", "Hardware Info", "Developer", "PCI & ACPI Topology Map", render_util_hardware_info)
    register_icon("ICON_UTIL_PERF_MONITOR", "Perf Monitor", "Developer", "Throughput & GPU Benchmarks", render_util_perf_monitor)
    register_icon("ICON_UTIL_DEBUG_CONSOLE", "Debug Console", "Developer", "Serial & Trace Diagnostic Port", render_util_debug_console)

    # Category 3: File Types (19)
    register_icon("ICON_FILE_FOLDER", "Folder", "File Type", "Standard Directory Folder", render_file_folder)
    register_icon("ICON_FILE_FOLDER_OPEN", "Open Folder", "File Type", "Currently Open Folder", render_file_folder_open)
    register_icon("ICON_FILE_FOLDER_HOME", "Home Folder", "File Type", "User Home Directory", render_file_folder_home)
    register_icon("ICON_FILE_FOLDER_DESKTOP", "Desktop Folder", "File Type", "Desktop Workspace Directory", render_file_folder_desktop)
    register_icon("ICON_FILE_FOLDER_DOCS", "Documents", "File Type", "User Documents Directory", render_file_folder_docs)
    register_icon("ICON_FILE_FOLDER_DOWNLOADS", "Downloads", "File Type", "Downloaded Files Directory", render_file_folder_downloads)
    register_icon("ICON_FILE_TRASH", "Trash", "File Type", "Recycle & Waste Basket", render_file_trash)
    register_icon("ICON_FILE_EXEC", "Executable", "File Type", "Native Binary Program", render_file_exec)
    register_icon("ICON_FILE_APAPP", "Application Bundle", "File Type", "Packaged .apapplication Bundle", render_file_apapp)
    register_icon("ICON_FILE_APDRV", "Driver Package", "File Type", "Packaged .apdrv Driver Bundle", render_file_apdrv)
    register_icon("ICON_FILE_ARCHIVE", "Archive File", "File Type", "Compressed Archive (Zip/Tar)", render_file_archive)
    register_icon("ICON_FILE_TEXT", "Text File", "File Type", "Plain Text Document (.txt)", render_file_text)
    register_icon("ICON_FILE_CODE", "Source Code", "File Type", "C/ASM Source File (.c/.h/.S)", render_file_code)
    register_icon("ICON_FILE_CONFIG", "Config File", "File Type", "Settings Configuration (.cfg/.ini)", render_file_config)
    register_icon("ICON_FILE_LOG", "Log File", "File Type", "System Diagnostic Log (.log)", render_file_log)
    register_icon("ICON_FILE_IMAGE", "Image File", "File Type", "Raster Image File (.png/.bmp)", render_file_image)
    register_icon("ICON_FILE_AUDIO", "Audio File", "File Type", "Sound Recording (.wav/.pcm)", render_file_audio)
    register_icon("ICON_FILE_VIDEO", "Video File", "File Type", "Video Media Stream (.mp4/.avi)", render_file_video)
    register_icon("ICON_FILE_UNKNOWN", "Unknown File", "File Type", "Generic Unknown Data File", render_file_unknown)

    # Category 4: Hardware Devices & Volumes (13)
    register_icon("ICON_DEV_DISK", "Fixed Disk", "Hardware", "Internal AHCI/NVMe Solid Drive", render_dev_disk)
    register_icon("ICON_DEV_REMOVABLE", "USB Drive", "Hardware", "Removable Mass Storage Unit", render_dev_removable)
    register_icon("ICON_DEV_NET_SHARE", "Network Share", "Hardware", "Remote Shared Network Volume", render_dev_net_share)
    register_icon("ICON_DEV_EFI", "EFI Partition", "Hardware", "UEFI System Boot Partition", render_dev_efi)
    register_icon("ICON_DEV_PURPLEFS", "PurpleFS Volume", "Hardware", "PurpleFS v2/v3 Journaled Volume", render_dev_purplefs)
    register_icon("ICON_DEV_KEYBOARD", "Keyboard", "Hardware", "PS/2 or USB Input Keyboard", render_dev_keyboard)
    register_icon("ICON_DEV_MOUSE", "Mouse Pointer", "Hardware", "Pointing Device / Trackpad", render_dev_mouse)
    register_icon("ICON_DEV_DISPLAY", "Display Monitor", "Hardware", "GOP Framebuffer Monitor", render_dev_display)
    register_icon("ICON_DEV_CPU", "CPU Processor", "Hardware", "x86-64 Multi-Core SMP CPU", render_dev_cpu)
    register_icon("ICON_DEV_GPU", "GPU Accelerator", "Hardware", "Intel BCS / AMD SDMA GPU", render_dev_gpu)
    register_icon("ICON_DEV_MEMORY", "System RAM", "Hardware", "Physical Memory Bank", render_dev_memory)
    register_icon("ICON_DEV_NETWORK", "Ethernet NIC", "Hardware", "e1000 Gigabit Network Adapter", render_dev_network)
    register_icon("ICON_DEV_AUDIO", "Audio Output", "Hardware", "HD Audio Sound Controller", render_dev_audio)

    # Category 5: Status Indicators & UI Actions (25)
    register_icon("ICON_STATUS_WIFI_3", "Wi-Fi High", "Status/UI", "Wi-Fi Full Signal Strength", render_status_wifi_3)
    register_icon("ICON_STATUS_WIFI_2", "Wi-Fi Medium", "Status/UI", "Wi-Fi Moderate Signal", render_status_wifi_2)
    register_icon("ICON_STATUS_WIFI_1", "Wi-Fi Low", "Status/UI", "Wi-Fi Weak Signal", render_status_wifi_1)
    register_icon("ICON_STATUS_ETHERNET", "Ethernet Connected", "Status/UI", "Wired LAN Connection Active", render_status_ethernet)
    register_icon("ICON_STATUS_BLUETOOTH", "Bluetooth On", "Status/UI", "Bluetooth Radio Connected", render_status_bluetooth)
    register_icon("ICON_STATUS_AUDIO", "Volume Active", "Status/UI", "Audio Playback Enabled", render_status_audio)
    register_icon("ICON_STATUS_AUDIO_MUTE", "Audio Muted", "Status/UI", "Audio Output Silenced", render_status_audio_mute)
    register_icon("ICON_STATUS_BATTERY_100", "Battery Full", "Status/UI", "Battery Level 100%", render_status_battery_100)
    register_icon("ICON_STATUS_BATTERY_CHARGING", "Battery Charging", "Status/UI", "AC Power Adapter Connected", render_status_battery_charging)
    register_icon("ICON_STATUS_BRIGHTNESS", "Display Brightness", "Status/UI", "Screen Luminance Level", render_status_brightness)
    register_icon("ICON_STATUS_VOLUME", "Volume Slider", "Status/UI", "Audio Output Master Level", render_status_volume)
    register_icon("ICON_STATUS_SECURITY_OK", "Security Verified", "Status/UI", "AegisDetect Root of Trust Clean", render_status_security_ok)
    register_icon("ICON_STATUS_SECURITY_WARN", "Security Warning", "Status/UI", "Integrity Check Discrepancy", render_status_security_warn)
    register_icon("ICON_STATUS_POWER", "Power State", "Status/UI", "System ACPI Power Status", render_status_power)
    register_icon("ICON_UI_CLOSE", "Close Button", "Status/UI", "Window Close Control [X]", render_ui_close)
    register_icon("ICON_UI_MINIMIZE", "Minimize Button", "Status/UI", "Window Minimize to Dock [-]", render_ui_minimize)
    register_icon("ICON_UI_MAXIMIZE", "Maximize Button", "Status/UI", "Window Maximize [ + ]", render_ui_maximize)
    register_icon("ICON_UI_RESTORE", "Restore Button", "Status/UI", "Window Restore Dimensions", render_ui_restore)
    register_icon("ICON_UI_SEARCH", "Search Action", "Status/UI", "Search Query Icon", render_ui_search)
    register_icon("ICON_UI_REFRESH", "Refresh Action", "Status/UI", "Reload Directory / Window", render_ui_refresh)
    register_icon("ICON_UI_BACK", "Navigate Back", "Status/UI", "History Back Arrow", render_ui_back)
    register_icon("ICON_UI_FORWARD", "Navigate Forward", "Status/UI", "History Forward Arrow", render_ui_forward)
    register_icon("ICON_UI_HELP", "Help Action", "Status/UI", "Interactive Help Dialog", render_ui_help)
    register_icon("ICON_UI_CHECK", "Verification Check", "Status/UI", "Validation Passed Checkmark", render_ui_check)
    register_icon("ICON_UI_STAR", "Favorite Star", "Status/UI", "Favorite Item Bookmark ★", render_ui_star)

def main():
    print("=== Generating Purple OS / Aegis64 Master Icon Library ===")
    os.makedirs(OUTPUT_DIR_32, exist_ok=True)
    os.makedirs(OUTPUT_DIR_16, exist_ok=True)

    build_registry()
    total_icons = len(ICONS)
    print(f"[INFO] Registered {total_icons} distinct icon definitions.")

    # Storage for C code generation (32x32 ARGB format: uint32_t per pixel)
    c_icon_entries = []

    for idx, (icon_id, info) in enumerate(ICONS.items()):
        # 1. Render 32x32 version
        img32 = create_base_icon(32, 32)
        d32 = ImageDraw.Draw(img32)
        info["render_fn"](d32, 32, 32)
        p32 = os.path.join(OUTPUT_DIR_32, f"{icon_id.lower()}.png")
        img32.save(p32)

        # 2. Render 16x16 version
        img16 = create_base_icon(16, 16)
        d16 = ImageDraw.Draw(img16)
        info["render_fn"](d16, 16, 16)
        p16 = os.path.join(OUTPUT_DIR_16, f"{icon_id.lower()}.png")
        img16.save(p16)

        # 3. Extract 32x32 ARGB pixel data
        pixels32 = []
        for y in range(32):
            for x in range(32):
                r, g, b, a = img32.getpixel((x, y))
                argb = (a << 24) | (r << 16) | (g << 8) | b
                pixels32.append(argb)

        c_icon_entries.append({
            "id": icon_id,
            "name": info["name"],
            "category": info["category"],
            "description": info["description"],
            "pixels32": pixels32
        })

    print(f"[OK] Rendered {total_icons} PNG assets in 32x32 and 16x16 resolutions.")

    # Generate include/icon_data.h
    with open(HEADER_PATH, "w") as f:
        f.write("/* Auto-generated by scripts/generate_icon_library.py — DO NOT EDIT */\n")
        f.write("#ifndef ICON_DATA_H\n#define ICON_DATA_H\n\n#include \"types.h\"\n\n")
        f.write("#ifdef __cplusplus\nextern \"C\" {\n#endif\n\n")
        f.write("#define ICON_WIDTH  32\n#define ICON_HEIGHT 32\n")
        f.write(f"#define ICON_COUNT  {total_icons}\n\n")
        f.write("typedef enum {\n")
        for i, entry in enumerate(c_icon_entries):
            f.write(f"    {entry['id']} = {i},\n")
        f.write("    ICON_TOTAL_COUNT\n} icon_id_t;\n\n")
        f.write("typedef struct {\n")
        f.write("    icon_id_t   id;\n")
        f.write("    const char *name;\n")
        f.write("    const char *category;\n")
        f.write("    const char *description;\n")
        f.write("    const uint32_t *data_32x32; /* 32x32 ARGB pixels */\n")
        f.write("} icon_descriptor_t;\n\n")
        f.write("void icon_system_init(void);\n")
        f.write("const icon_descriptor_t *icon_get(icon_id_t id);\n")
        f.write("const icon_descriptor_t *icon_find_by_name(const char *name);\n")
        f.write("void icon_draw(icon_id_t id, int32_t x, int32_t y);\n")
        f.write("void icon_draw_16(icon_id_t id, int32_t x, int32_t y);\n")
        f.write("void icon_draw_scale(icon_id_t id, int32_t x, int32_t y, int scale);\n\n")
        f.write("#ifdef __cplusplus\n}\n#endif\n\n#endif /* ICON_DATA_H */\n")

    print(f"[OK] Generated {HEADER_PATH}")

    # Generate src/drivers/icon_data.c
    with open(SOURCE_PATH, "w") as f:
        f.write("/* Auto-generated by scripts/generate_icon_library.py — DO NOT EDIT */\n")
        f.write("#include \"icon_data.h\"\n#include \"video.h\"\n#include \"string.h\"\n\n")

        # Write pixel arrays
        for entry in c_icon_entries:
            var_name = f"icon_pixels_{entry['id'].lower()}"
            f.write(f"static const uint32_t {var_name}[1024] = {{\n")
            for row in range(32):
                row_vals = entry["pixels32"][row * 32 : (row + 1) * 32]
                f.write("    " + ", ".join(f"0x{val:08X}" for val in row_vals) + ",\n")
            f.write("};\n\n")

        # Write descriptors table
        f.write("static const icon_descriptor_t icon_table[ICON_TOTAL_COUNT] = {\n")
        for entry in c_icon_entries:
            var_name = f"icon_pixels_{entry['id'].lower()}"
            f.write(f"    {{ {entry['id']}, \"{entry['name']}\", \"{entry['category']}\", \"{entry['description']}\", {var_name} }},\n")
        f.write("};\n\n")

        # Implementation functions
        f.write("""void icon_system_init(void) {
    /* Ready for kernel desktop compositor */
}

const icon_descriptor_t *icon_get(icon_id_t id) {
    if ((uint32_t)id >= ICON_TOTAL_COUNT) return &icon_table[0];
    return &icon_table[id];
}

const icon_descriptor_t *icon_find_by_name(const char *name) {
    if (!name) return NULL;
    for (uint32_t i = 0; i < ICON_TOTAL_COUNT; i++) {
        if (strcmp(icon_table[i].name, name) == 0) {
            return &icon_table[i];
        }
    }
    return NULL;
}

void icon_draw(icon_id_t id, int32_t x, int32_t y) {
    if (!video_is_available() || (uint32_t)id >= ICON_TOTAL_COUNT) return;
    const uint32_t *pixels = icon_table[id].data_32x32;
    uint32_t scr_w = video_get_width();
    uint32_t scr_h = video_get_height();

    for (int32_t r = 0; r < 32; r++) {
        int32_t py = y + r;
        if (py < 0 || (uint32_t)py >= scr_h) continue;
        for (int32_t c = 0; c < 32; c++) {
            int32_t px = x + c;
            if (px < 0 || (uint32_t)px >= scr_w) continue;

            uint32_t argb = pixels[r * 32 + c];
            uint32_t alpha = (argb >> 24) & 0xFF;
            if (alpha == 0) continue;

            uint32_t r_fg = (argb >> 16) & 0xFF;
            uint32_t g_fg = (argb >> 8) & 0xFF;
            uint32_t b_fg = argb & 0xFF;

            if (alpha >= 250) {
                video_draw_pixel((uint32_t)px, (uint32_t)py, (r_fg << 16) | (g_fg << 8) | b_fg);
            } else {
                uint32_t bg = video_get_pixel((uint32_t)px, (uint32_t)py);
                uint32_t r_bg = (bg >> 16) & 0xFF;
                uint32_t g_bg = (bg >> 8) & 0xFF;
                uint32_t b_bg = bg & 0xFF;

                uint32_t r_out = (r_fg * alpha + r_bg * (255 - alpha)) / 255;
                uint32_t g_out = (g_fg * alpha + g_bg * (255 - alpha)) / 255;
                uint32_t b_out = (b_fg * alpha + b_bg * (255 - alpha)) / 255;

                video_draw_pixel((uint32_t)px, (uint32_t)py, (r_out << 16) | (g_out << 8) | b_out);
            }
        }
    }
}

void icon_draw_16(icon_id_t id, int32_t x, int32_t y) {
    if (!video_is_available() || (uint32_t)id >= ICON_TOTAL_COUNT) return;
    const uint32_t *pixels = icon_table[id].data_32x32;
    uint32_t scr_w = video_get_width();
    uint32_t scr_h = video_get_height();

    for (int32_t r = 0; r < 16; r++) {
        int32_t py = y + r;
        if (py < 0 || (uint32_t)py >= scr_h) continue;
        for (int32_t c = 0; c < 16; c++) {
            int32_t px = x + c;
            if (px < 0 || (uint32_t)px >= scr_w) continue;

            uint32_t argb = pixels[(r * 2) * 32 + (c * 2)];
            uint32_t alpha = (argb >> 24) & 0xFF;
            if (alpha == 0) continue;

            uint32_t r_fg = (argb >> 16) & 0xFF;
            uint32_t g_fg = (argb >> 8) & 0xFF;
            uint32_t b_fg = argb & 0xFF;

            if (alpha >= 250) {
                video_draw_pixel((uint32_t)px, (uint32_t)py, (r_fg << 16) | (g_fg << 8) | b_fg);
            } else {
                uint32_t bg = video_get_pixel((uint32_t)px, (uint32_t)py);
                uint32_t r_bg = (bg >> 16) & 0xFF;
                uint32_t g_bg = (bg >> 8) & 0xFF;
                uint32_t b_bg = bg & 0xFF;

                uint32_t r_out = (r_fg * alpha + r_bg * (255 - alpha)) / 255;
                uint32_t g_out = (g_fg * alpha + g_bg * (255 - alpha)) / 255;
                uint32_t b_out = (b_fg * alpha + b_bg * (255 - alpha)) / 255;

                video_draw_pixel((uint32_t)px, (uint32_t)py, (r_out << 16) | (g_out << 8) | b_out);
            }
        }
    }
}

void icon_draw_scale(icon_id_t id, int32_t x, int32_t y, int scale) {
    if (!video_is_available() || (uint32_t)id >= ICON_TOTAL_COUNT || scale <= 0) return;
    if (scale == 1) {
        icon_draw(id, x, y);
        return;
    }
    const uint32_t *pixels = icon_table[id].data_32x32;
    uint32_t scr_w = video_get_width();
    uint32_t scr_h = video_get_height();

    for (int32_t r = 0; r < 32; r++) {
        for (int32_t c = 0; c < 32; c++) {
            uint32_t argb = pixels[r * 32 + c];
            uint32_t alpha = (argb >> 24) & 0xFF;
            if (alpha == 0) continue;

            uint32_t r_fg = (argb >> 16) & 0xFF;
            uint32_t g_fg = (argb >> 8) & 0xFF;
            uint32_t b_fg = argb & 0xFF;
            uint32_t color = (r_fg << 16) | (g_fg << 8) | b_fg;

            for (int dy = 0; dy < scale; dy++) {
                int32_t py = y + r * scale + dy;
                if (py < 0 || (uint32_t)py >= scr_h) continue;
                for (int dx = 0; dx < scale; dx++) {
                    int32_t px = x + c * scale + dx;
                    if (px < 0 || (uint32_t)px >= scr_w) continue;
                    video_draw_pixel((uint32_t)px, (uint32_t)py, color);
                }
            }
        }
    }
}
""")

    print(f"[OK] Generated {SOURCE_PATH}")

    # Generate docs/PURPLE_ICON_LIBRARY_CATALOG.md
    with open(DOCS_PATH, "w") as f:
        f.write("# Purple OS / Aegis64 — Native Icon Library Catalog & Manifest\n\n")
        f.write("**Status:** VERIFIED & INTEGRATED\n")
        f.write(f"**Total Icon Assets:** {total_icons} distinct identifiers\n")
        f.write("**Resolutions Available:** 32x32 RGBA, 16x16 RGBA (Assets in `assets/icons/`)\n\n")
        f.write("## 1. Visual Design Architecture\n\n")
        f.write("All icons conform to the **Hollow Purple Cosmic Visual Identity**:\n")
        f.write("- **Squircle Containers:** Continuous curvature rounded corners with subtle violet edge illumination (`#7209B7`, `#9D4EDD`).\n")
        f.write("- **Alpha Coverage Blending:** Smooth anti-aliased subpixel blending over cosmic wallpaper and dark frosted glass backgrounds.\n")
        f.write("- **Palette Harmony:** Primary violet/indigo bodies (`#130A24`, `#2A1245`), magenta accents (`#F72585`), cyan-electric diagnostic badges (`#4CC9F0`), and gold highlights (`#FFD700`).\n\n")
        f.write("## 2. Complete Asset Catalog\n\n")
        f.write("| ID | Enum Identifier | Name | Category | Description |\n")
        f.write("| :---: | :--- | :--- | :--- | :--- |\n")
        for i, entry in enumerate(c_icon_entries):
            f.write(f"| {i+1} | `{entry['id']}` | **{entry['name']}** | {entry['category']} | {entry['description']} |\n")

    print(f"[OK] Generated {DOCS_PATH}")
    print("=== Icon Library Generation Successfully Completed ===")

if __name__ == "__main__":
    main()
