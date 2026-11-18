# Purple Desktop UI Specification

**Version:** 1.0.0  
**Target:** PurpleOS / Aegis64 Pure Bare-Metal Rust Kernel (`x86_64-unknown-none`)  
**Design Baseline:** Project Looking Glass (`lg3d-core`) Modernization  
**Reference Mockup:** `assets/purple_desktop_ui_final.jpg`  

---

## 1. Executive Summary & Design Vision

**Purple Desktop UI** is the native graphical user interface and desktop environment for **PurpleOS / Aegis64**. Built directly on top of the Aegis64 native compositor engine (`src/compositor/`), it takes inspiration from Sun Microsystems' pioneering **Project Looking Glass** (`lg3d-core`), modernizing its early-2000s 3D spatial concepts into an ultra-clean, high-performance, 2026 frosted-glass aesthetic ("Liquid Glassmorphism").

### Clean-Room & Licensing Integrity
- **Zero Copied Code:** All components are written from scratch in pure Rust (`#![no_std]`, zero external dependencies).
- **Architecture:** Unlike Looking Glass's Java 3D/X11 runtime, Purple Desktop UI runs bare-metal over the kernel's linear framebuffer (`/dev/fb0`) with real-time Porter-Duff alpha composition, damage tracking, and 60 Hz hardware double-buffering.

---

## 2. Desktop Shell Components

```
+-----------------------------------------------------------------------------------+
|  [P]                                 18:40 UTC                       [WiFi][Vol][Pwr]  | <- Aegis Bar (Top)
+-----------------------------------------------------------------------------------+
|                                                                                   |
|    +-----------------------------+       +-----------------------------+          |
|    | Aegis Terminal          - x |       | System Monitor          - x |          |
|    |-----------------------------|       |-----------------------------|          |
|    | aegis64@purple:~$ neofetch  |       | CPU 0-3 Waveforms           |          |
|    | [PurpleOS Logo Glyph]       |       | RAM Arc: 64 MiB / 2.0 GiB   |          |
|    | Pure Rust 64-bit Kernel     |       | Compositor: 60.0 FPS        |          |
|    | 60 FPS Compositor           |       |                             |          |
|    +-----------------------------+       +-----------------------------+          |
|                                                                                   |
|                                                                                   |
|                    +------------------------------------+                         |
|                    |  [Term] [Files] [SysMon] [Code] [Gear] |                     | <- Flow Dock
|                    |    .       .                           |                     |
|                    +------------------------------------+                         |
|                    ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~                         | <- Mirror Reflection
+-----------------------------------------------------------------------------------+
```

### A. Aegis Bar (Floating Top Panel)
- **Geometry:** Height: 32px, Top Offset: 12px, Horizontal Padding: 20px, Corner Radius: 12px.
- **Visual Style:** Translucent frosted acrylic glass (`0xCC160B2A`) with subtle 1px upper highlight border (`0x33FFFFFF`).
- **Left Region:** Stylized glowing amethyst Purple **'P'** crest button. Activates the system launcher / application drawer.
- **Center Region:** Digital system clock formatted as `HH:MM UTC` blitted with anti-aliased typography.
- **Right Region:** Minimalist system tray status indicators:
  - Wi-Fi / Ethernet link state (`8086:100E` Intel Gigabit NIC).
  - Audio output state (AC97 / Intel HDA volume level).
  - Power / Battery / ACPI state.

### B. Purple Flow Dock (Floating Bottom Shelf)
- **Geometry:** Height: 56px, Bottom Margin: 24px, Corner Radius: 20px, Centered horizontally.
- **Looking Glass Heritage:** Features an inverted alpha-gradient mirror reflection rendered immediately beneath the dock on the floor plane.
- **Launcher Icons (40x40px):**
  1. `Terminal` (`terminal.apapplication`): Sleek terminal glyph with prompt chevron.
  2. `Files` (`files.apapplication`): Translucent purple gradient file manager folder.
  3. `System Monitor` (`sysmon.apapplication`): Real-time pulse waveform icon.
  4. `Code Studio` (`editor.apapplication`): Vector code brackets glyph (`</>`).
  5. `Settings` (`settings.apapplication`): Precision holographic gear glyph.
- **Process Indicator:** 4px glowing amethyst pill dot (`0xFFC084FC`) centered below icons of running processes.

### C. Aegis Glass Window Chrome
- **Header / Titlebar:** Height: 28px, integrated drag handle, title centered in white (`0xFFF1F5F9`).
- **Window Controls (Right):** Minimalist flat vector controls:
  - Minimize (`_`): Closes to dock icon.
  - Maximize (`□`): Toggles between floating geometry and full-screen minus shell bounds.
  - Close (`✕`): Sends terminate signal to client application.
- **Frame & Drop Shadow:** 1px neon purple perimeter border (`0x66A855F7`) with 16px soft translucent Gaussian-style drop shadow (`0x55000000`).
- **Client Surface:** Backed by 32bpp ARGB8888 shared surface memory mapped into application address space.

---

## 3. Visual Styling & Color Tokens

| Token | Hex Value | Description |
| :--- | :--- | :--- |
| `COLOR_CANVAS_OBSIDIAN` | `0xFF140824` | Solid deep space background clear color |
| `COLOR_CANVAS_VIOLET` | `0xFF1B0C30` | Ambient gradient accent color |
| `COLOR_GLASS_BASE` | `0xCC160B2A` | 80% opacity frosted glass background |
| `COLOR_GLASS_BORDER` | `0x44C084FC` | Translucent neon violet structural border |
| `COLOR_AMETHYST_ACCENT`| `0xFFA855F7` | Primary vibrant amethyst highlight |
| `COLOR_NEON_VIOLET` | `0xFFC084FC` | Active indicators and glyph glows |
| `COLOR_CYAN_ACCENT` | `0xFF38BDF8` | Auxiliary metric and telemetry highlights |
| `COLOR_TEXT_PRIMARY` | `0xFFF8FAFC` | Razor-sharp crisp foreground text |
| `COLOR_TEXT_MUTED` | `0xFF94A3B8` | Subtitle and secondary metadata text |

---

## 4. Compositor Pipeline Architecture

The rendering pipeline executes within `src/compositor/pipeline.rs`:

1. **Pass 1 — Root Canvas:** Clear screen to `COLOR_CANVAS_OBSIDIAN` or draw cosmic ambient mesh gradient.
2. **Pass 2 — Desktop Shell (Back):** Render dock mirror reflection pass.
3. **Pass 3 — Window Z-Stack:** Sort active windows by ascending Z-order; blit drop shadows, window borders, titlebars, and client surfaces using Porter-Duff `SourceOver` alpha blending.
4. **Pass 4 — Desktop Shell (Front):** Render floating **Aegis Bar** (top) and **Purple Flow Dock** (bottom) with active application indicator dots.
5. **Pass 5 — Software Pointer:** Render anti-aliased cursor with zero flicker.
6. **Pass 6 — Presentation:** Zero-copy hardware VBE page flip (`drivers::gpu::flip_page()`) or coherent linear framebuffer copy at 60 Hz.

---

## 5. Upcoming Native Application Targets

1. **`terminal.apapplication`**: Windowed terminal client encapsulating the preserved Aegis CUI engine, VT100 ANSI parser, readline, and command execution pipeline inside a GUI surface.
2. **`sysmon.apapplication`**: System monitor application blitting real-time multi-core APIC telemetry, CPU utilization graphs, memory allocation meters, and compositor frame timings.
