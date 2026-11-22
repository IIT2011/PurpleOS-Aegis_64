# 21-11-2026 — macOS Global Application Bar & System Font Implementation

## Summary
Designed, implemented, and verified a full-width macOS-style Global Application Bar at the top of the PurpleOS screen (`y = 0..28`, `x = 0..width`), rendered using the native system font (**Inter Regular** and **Inter Bold**) with fractional subpixel precision and coverage alpha blending. The bar dynamically switches active application titles and menu hierarchies depending on window focus (VLC, Finder, Terminal, Files, Activity Monitor) while providing macOS-style status extras on the right side.

---

## Technical Implementations

### 1. Global Application Bar Chrome & Geometry
- **Location & Dimensions:** Flush across the very top (`y = 0..28`, `x = 0..width`).
- **Acrylic Translucency:** Frosted backdrop sampling with dual pass alpha blending:
  - Top specular highlight sheen at `y = 0` (`0x20FFFFFF`)
  - Deep frosted dark amethyst glass (`0xDC120824`)
  - Bottom subtle neon violet boundary border line at `y = 27` (`0x408C67C8`)
- **Compositor Work Area Adjustment:**
  - `desktop::BAR_HEIGHT` set to `28`.
  - `work_area()` in `compositor/mod.rs` updated to start at `y = BAR_HEIGHT` (28px), ensuring maximized or tiled windows rest immediately below the global menu bar without obscuring it.
  - Dropdown panels (`Applications`, `QuickSettings`) dynamically open below the bar at `y = 32`.

### 2. System Font Engine Integration (Inter TTF)
- Outlines rasterized from `assets/fonts/Inter/Inter-Regular.ttf` and `assets/fonts/Inter/Inter-Bold.ttf` via `ttf_parser`.
- Typography rendering utilizes `Surface::draw_text` and `Surface::draw_text_bold` with anti-aliasing.
- Baseline vertically centered in the 28px bar at `text_y = (28 - 20) / 2 = 4`.

### 3. Left-Side Dynamic Application Menus
- **Purple OS Mark:** Native 22×22 icon at `x = 10, y = 3`. Clicking toggles the system application launcher panel.
- **Dynamic Application Title:** Displayed in **Bold Inter** (`FontWeight::Bold`). Tracks focused window via `w.is_focused`:
  - When VLC is focused: **VLC**
  - When Finder / Desktop is focused: **Finder**
  - When Terminal is focused: **Terminal**
  - When Files is focused: **Files**
- **Dynamic Menu Items:** Displayed in **Regular Inter** (`FontWeight::Regular`) in `#E2E8F0`:
  - **VLC:** `Media  Playback  Audio  Video  Subtitle  Tools  View  Help`
  - **Finder:** `File  Edit  View  Go  Window  Help`
  - **Terminal:** `Shell  Edit  View  Window  Help`

### 4. Right-Side macOS Status Extras
- **Hardware 3D Acceleration Pill:** Rounded glass capsule `[ 3D ACCEL ]` with electric cyan border (`0x9038BDF8`) and cyan bold typography (`0xFF38BDF8`).
- **WiFi Status Icon:** Vector anti-aliased `ICON_WIFI_FULL` mask (electric cyan when network is ready, muted when offline).
- **Volume Indicator:** Vector anti-aliased `ICON_VOLUME_FULL` mask with speaker horn and sound wave arcs.
- **Battery Level:** 20×10 mini battery icon with terminal nipple filled with crisp emerald green (`0xE034D399`, 100% charged).
- **Control Center Icon:** Anti-aliased `ShellIcon::Chevron` triggering Quick Settings.
- **Digital Clock & Calendar:** Day of week, day of month, month abbreviation, and hardware RTC CMOS time (`Sat 21 Nov 08:03`) with animated blinking colon separator.

---

## Verification
- Built kernel: `cargo build --target x86_64-unknown-none` & `rust-objcopy` -> zero warnings.
- Ran QEMU with linear framebuffer and Intel HDA audio (`scripts/capture_vlc_screenshot.py`).
- Produced high-resolution PNG screendumps:
  - `build/screenshots/purple_desktop.png` (Boot desktop with Finder active)
  - `build/screenshots/purple_vlc_player.png` (Live desktop with VLC active)
- Copied both images to conversation artifact directory and updated `screen_capture.md`.
