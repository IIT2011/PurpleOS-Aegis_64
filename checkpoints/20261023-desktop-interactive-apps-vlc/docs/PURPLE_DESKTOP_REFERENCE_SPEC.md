# Purple OS Desktop Reference Specification

**Specification Document:** `docs/PURPLE_DESKTOP_REFERENCE_SPEC.md`  
**Authoritative Reference File:** `build/Desktop.png`  
**Reference SHA-256:** `82d92d9e92142013b3acce17e002d5995ec9557bccf9b0acf22047b70e46dcd9`  
**File Size:** 2,321,807 bytes  
**Native Dimensions:** 1672 × 941 pixels  
**Color Format:** RGB (24-bit per pixel, 8-bit per channel, no file alpha channel)  
**Aspect Ratio:** 1.7768 : 1 (standard widescreen ~16:9)  
**Reference Type:** Complete running desktop screenshot with full UI composition overlaid upon the native cosmic fantasy vista.

---

## 1. Global Coordinate System & Scaling Rules

- **Coordinate System:** Origin `(0, 0)` is at the top-left pixel.
  - Horizontal axis: $X \in [0, 1671]$
  - Vertical axis: $Y \in [0, 940]$
- **Scaling Architecture:**
  - The UI layout is anchored to screen geometry:
    - **Top Panel:** Full width $X \in [0, W-1]$, height $H_{\text{panel}} = 36\text{ px}$.
    - **Center Clock & Motto:** Centered horizontally at $X_c = W / 2$, positioned vertically in the upper celestial clearance ($Y \in [150, 260]$).
    - **Left Launcher Card:** Left-anchored with margin $X = 22\text{ px}$, width $W_{\text{card}} \approx 440\text{ px}$ (or $26\%$ of screen width), vertical span $Y \in [75, 845]$.
    - **Right System Status Card:** Right-anchored with margin $X = W - 462\text{ px}$, width $W_{\text{card}} \approx 440\text{ px}$, vertical span $Y \in [75, 845]$.
    - **Bottom Floating Dock:** Centered horizontally at $X_c = W / 2$, width $W_{\text{dock}} \approx 900\text{ px}$, height $H_{\text{dock}} = 60\text{ px}$, floating above the screen bottom ($Y \in [H - 76, H - 16]$).
  - On lower resolutions (e.g. 1024×768 or 1280×720), card widths, paddings, and font sizes scale proportionally so no UI element overflows or collides.

---

## 2. Visual Elements & Layer Inventory

### Layer 0: Native Cosmic Vista Wallpaper
- **Visual Subject:** Anime-inspired cosmic landscape with intense violet Hollow Purple energy.
- **Components:**
  - Celestial Vortex: Centered swirling violet cosmic spiral / singularity ($X \approx 836, Y \approx 480$), radiating glowing spiral arms.
  - Sky: Deep navy to dark indigo gradient (`#060224` to `#180C38`).
  - Star Particles & Nebulae: Glowing magenta and electric violet dust motes (`#F72585`, `#C77DFF`, `#FFFFFF`).
  - Mountain Horizon: Silhouetted mountain ranges along the middle-lower horizon.
  - Reflective Lake: Still water surface reflecting the violet cosmic vortex with gentle horizontal light diffusion.
  - Torii Gate Outcrop: Middle-distance rocky cliff with traditional Japanese torii gate silhouette overlooking the water on the middle right.

### Layer 1: Top Translucent System Panel
- **Geometry:** $X: 0 \dots 1671$, $Y: 0 \dots 35$ (Height: 36 px).
- **Background:** Translucent frosted dark glass `#060224` (effective opacity $\approx 88\%$) with subtle 1 px bottom border `#3A125E`.
- **Left Brand Section:**
  - Lotus Icon: 3-petal Purple OS Lotus glyph in luminous lavender/pink (`#D2B1FC` / `#F72585`), $X \in [20, 38], Y \in [8, 26]$ (18×18 px).
  - Brand Label: "Purple OS" in clean semi-bold font (`#D2B1FC`), $X \in [46, 130], Y \approx 18$.
- **Navigation Tabs:**
  - Tab 1 ("Applications"): $X \in [196, 278]$, active state in high-contrast white/lavender (`#F8F4FF`).
    - Active Indicator: Electric violet pill underline (`#9D4EDD`), $X \in [196, 278], Y \in [33, 35]$ (height 3 px, corner radius 1.5 px).
  - Tab 2 ("Places"): $X \in [313, 360]$, inactive muted lavender (`#A686DA`).
  - Tab 3 ("System"): $X \in [385, 435]$, inactive muted lavender (`#A686DA`).
- **Right System Tray:** $X \in [1320, 1655]$
  - Network Indicator: WiFi signal arcs (`#D2B1FC`), $X \approx 1340$.
  - Audio Indicator: Speaker icon (`#D2B1FC`), $X \approx 1380$.
  - Battery Indicator: Battery capsule with green/violet fill and "98%" label, $X \in [1415, 1465]$.
  - Tray Digital Clock: "03:24" (or local time) in clean compact font (`#F8F4FF`), $X \in [1490, 1540]$.
  - User Avatar / Power Control: Circular profile badge (`#D2B1FC`), $X \approx 1625$.

### Layer 2: Hero Centerpiece (Digital Clock, Date & Motto)
- **Digital Clock:**
  - Geometry: Centered, $X \in [720, 920], Y \in [175, 235]$.
  - Digits: `03:24` (or current 24-hr system time).
  - Typography: 64 px tall bold modern sans-serif digits in pure white `#FFFFFF` with luminous violet halo glow (`#9D4EDD`, radius 6 px).
- **Date & Motto Block:**
  - Geometry: $X \in [950, 1160], Y \in [175, 235]$.
  - Date Text: "Thursday, October 22" (14 px, color `#E0AAFF`).
  - Motto Text: `DREAM • BUILD • EXPLORE` (12 px, color `#F72585` / `#C77DFF`, tracking 2 px).

### Layer 3: Left Card (Application Launcher)
- **Geometry:** $X: 22 \dots 462$ (Width: 440 px), $Y: 75 \dots 845$ (Height: 770 px).
- **Surface:** Frosted glass `#0F053D` (opacity $\approx 85\%$) with 1 px border `#7209B780` and 12 px rounded corners.
- **Internal Elements:**
  - Search Bar: $X \in [42, 442], Y \in [95, 135]$, pill shape with `#1A0A3A` fill, search icon, and placeholder "Search applications & files...".
  - Category Pills: Horizontal scroll/stack: "All", "System", "Development", "Graphics", "Security".
  - Application Grid / List:
    - `Aegis Terminal` (`>_` icon, subtitle "Command Shell & Diagnostics")
    - `System Monitor` (`#` icon, subtitle "CPU, Memory, Tasks & SMP")
    - `PurpleFS Storage` (`Dir` icon, subtitle "File Browser & Partitions")
    - `AegisDetect Vault` (`Lock` icon, subtitle "Integrity & Root of Trust")
    - `System Settings` (`Gear` icon, subtitle "Display, Sound & Network")
  - User Profile Footer: $X \in [22, 462], Y \in [795, 845]$, user avatar circle, "saptanshu | Aegis64", power icon.

### Layer 4: Right Card (System Quick Settings & Resource Meters)
- **Geometry:** $X: 1210 \dots 1650$ (Width: 440 px), $Y: 75 \dots 845$ (Height: 770 px).
- **Surface:** Frosted glass matching left card (`#0A022D` with 1 px `#7209B780` border and 12 px rounded corners).
- **Internal Elements:**
  - Header: "Quick Settings & Monitor" (`#E0AAFF`).
  - Real Hardware Progress Meters:
    - CPU Utilization: Track `#1E0C38`, fill bar `#9D4EDD` with percentage text (12% across 4 cores).
    - Memory Usage: Track `#1E0C38`, fill bar `#9D4EDD` (185 MB / 4096 MB, 4%).
    - Storage Capacity: Track `#1E0C38`, fill bar `#9D4EDD` (16 MB / 64 MB, 25%).
    - GPU Acceleration Status: Real status report (`Software 32bpp Fallback` on QEMU standard VGA).
  - 2×2 Quick Toggle Buttons:
    - [Wi-Fi: ON], [Bluetooth: ON], [Night Light: OFF], [Dark Mode: ON].
  - Slider Controls:
    - Volume Slider: Track with pill thumb (65%).
    - Brightness Slider: Track with pill thumb (80%).

### Layer 5: Bottom Floating Dock
- **Geometry:** $X: 386 \dots 1286$ (Width: 900 px, centered), $Y: 865 \dots 925$ (Height: 60 px).
- **Surface:** Floating translucent pill capsule with 28 px corner radius, background `#0C0136D0`, border 1 px `#9D4EDD60`.
- **Application Icons (Centered evenly spaced):**
  - Terminal (`>_`), Files (`Dir`), System Monitor (`#`), Security (`Vault`), Calculator (`Calc`), Settings (`Gear`), Power (`Shutdown`).
- **Running Indicators:** 4×4 px glowing violet dot (`#F8EAFA`) placed beneath icons of running applications.

---

## 3. Colors Sampled Directly from Reference

| UI Element | RGB Sample | Hex Value | Visual Description |
| :--- | :--- | :--- | :--- |
| **Top Panel Surface** | `(6, 2, 36)` | `#060224` | Midnight navy translucent glass |
| **Top Divider Line** | `(58, 18, 94)` | `#3A125E` | Muted violet border |
| **Lotus Glyph** | `(210, 177, 252)` | `#D2B1FC` | Luminous soft lilac highlight |
| **Lotus Core Glow** | `(247, 37, 133)` | `#F72585` | Electric magenta luminescence |
| **Active Tab Pill** | `(72, 5, 182)` | `#4805B6` | Vivid violet active state |
| **Inactive Tab Text** | `(166, 132, 218)` | `#A686DA` | Muted lavender |
| **Center Clock Digits**| `(255, 255, 255)` | `#FFFFFF` | Crisp white digit face |
| **Center Clock Glow**  | `(157, 78, 221)` | `#9D4EDD` | Saturated violet typography glow |
| **Inspirational Motto**| `(247, 37, 133)` | `#F72585` | Bright magenta motto text |
| **Left Card Surface**  | `(15, 5, 61)` | `#0F053D` | Dark indigo frosted glass |
| **Left Card Border**   | `(114, 9, 183)` | `#7209B7` | Semi-transparent violet rim |
| **Search Input Bar**   | `(26, 10, 58)` | `#1A0A3A` | Recessed dark purple pill |
| **Right Card Surface** | `(10, 2, 45)` | `#0A022D` | Dark cosmic violet glass |
| **Resource Bar Fill**  | `(157, 78, 221)` | `#9D4EDD` | Electric violet progress indicator |
| **Dock Surface**       | `(12, 1, 54)` | `#0C0136` | Curved capsule frosted background |
| **Dock Active Dot**    | `(248, 234, 254)` | `#F8EAFA` | Luminous white-violet running dot |

---

## 4. Typography & Font Rules

- **Primary Font Family:** Noto Sans / Aegis TrueType Vector Font (bundled in `assets/fonts/NotoSans-Regular.ttf` and `NotoSans-Bold.ttf`).
- **Bitmap Glyphs (Kernel Console Fallback):** Proportional rendering using the kernel's font glyph tables.
- **Hierarchy:**
  1. *Hero Clock:* ~64 px bold digits.
  2. *Card Headers & Section Titles:* 16 px bold sans-serif (`COLOR_GOLD` / `#E0AAFF`).
  3. *Motto & Brand:* 12–14 px bold with letter spacing.
  4. *Body Labels & Terminal:* 8×16 px fixed-pitch glyphs inside terminal window and metrics tables.

---

## 5. Details that Cannot be Measured Directly

- **Exact Font Subpixel Antialiasing Filter:** Rasterized in the PNG with standard FreeType grayscale/RGB LCD subpixel hinting. Kernel linear framebuffers without hardware shader units perform 32bpp alpha-blended glyph rasterization.
- **Background Cosmic Nebula Volumetric Depth:** The swirling vortex contains smooth continuous mathematical light gradients and particle dispersion produced by artistic generation.
