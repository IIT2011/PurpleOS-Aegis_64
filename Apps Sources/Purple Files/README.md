# Purple Files

Native freestanding Rust (`x86_64-unknown-none`) file explorer for **Purple OS** (`.apapplication` v2 / GUI ABI v1).

## Features

- **Live Aero Glass UI**: Uses `0x110C` backdrop glass with `ARGB8888` translucent obsidian-plum surfaces and anti-aliased rounded geometry.
- **Right-Side macOS-Style Window Controls**: Circular Minimize (`#FEBC2E`), Maximize/Restore (`#28C840`), and Close (`#FF5F57`) buttons aligned on the right side of the headerbar with hover glyphs.
- **Navigation & Views**:
  - Sidebar quick access (`Home`, `Root /`, `System`, `Applications`, `Documents`, `Downloads`)
  - Back / Forward history, Up to parent, Refresh, clickable breadcrumbs, and editable path bar (`Ctrl+L`)
  - **Grid View** and **List View** with sorting (Name, Size, Type) and live search filtering (`Ctrl+F` or `/`)
  - Collapsible **Inspector & Text Preview** pane
- **Standard File Operations** (via GUI ABI v1 Shell capability `0x04` / `0x110A`):
  - Open folder / Launch `.apapplication`
  - New Folder (`Ctrl+Shift+N`), New File (`Ctrl+N`)
  - Rename (`F2`), Copy (`Ctrl+C`), Cut (`Ctrl+X`), Paste (`Ctrl+V`), Delete (`Del` with confirmation)
  - Right-click context menu & keyboard shortcuts

## Building & Packaging

1. Ensure the `x86_64-unknown-none` target is installed:
   ```sh
   rustup target add x86_64-unknown-none
   ```
2. Build the static ELF using your repository's `sdk/gui/purple_gui.ld`:
   ```sh
   RUSTFLAGS="-C link-arg=-T/path/to/OS/sdk/gui/purple_gui.ld" \
     cargo build --release --target x86_64-unknown-none
   ```
3. Package into `purple.files.apapplication` (requests GUI `0x02` + Shell `0x04` = `0x06`):
   ```sh
   python3 /path/to/OS/scripts/package_gui_app.py pack \
     target/x86_64-unknown-none/release/purple-files \
     stage/purple.files.apapplication \
     --id purple.files \
     --name "Purple Files" \
     --version 1.0.0 \
     --priority 2 \
     --shell \
     --width 960 \
     --height 620 \
     --key publisher.pem
   ```
4. Add the printed `trust.policy` line (`purple.files <pubkey_hex> 6 2`) to `stage/trust.policy`, sign with `creator.pem`, and build the disk image.
