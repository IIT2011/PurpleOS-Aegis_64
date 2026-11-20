#!/usr/bin/env bash
set -euo pipefail

# Build script for Purple Files (.apapplication v2 / GUI ABI v1)
# Run from the Purple Files project directory or pass PURPLE_OS_ROOT=/path/to/OS

PURPLE_OS_ROOT="${PURPLE_OS_ROOT:-../..}"
LDSCRIPT="${LDSCRIPT:-$PURPLE_OS_ROOT/sdk/gui/purple_gui.ld}"
PACKER="${PACKER:-$PURPLE_OS_ROOT/scripts/package_gui_app.py}"
PUBLISHER_KEY="${PUBLISHER_KEY:-publisher.pem}"
OUT_DIR="${OUT_DIR:-out}"

mkdir -p "$OUT_DIR"

if [[ -f "$LDSCRIPT" ]]; then
    export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-T$LDSCRIPT"
else
    echo "Warning: linker script not found at $LDSCRIPT; set LDSCRIPT=/path/to/purple_gui.ld" >&2
fi

cargo build --release --target x86_64-unknown-none

ELF="target/x86_64-unknown-none/release/purple-files"
cp "$ELF" "$OUT_DIR/purple-files.elf"
echo "Built static ELF: $OUT_DIR/purple-files.elf"

if [[ -f "$PACKER" && -f "$PUBLISHER_KEY" ]]; then
    python3 "$PACKER" pack \
        "$OUT_DIR/purple-files.elf" \
        "$OUT_DIR/purple.files.apapplication" \
        --id purple.files \
        --name "Purple Files" \
        --version 1.0.0 \
        --author "Purple OS" \
        --description "Native file explorer with Live Aero glass" \
        --priority 2 \
        --shell \
        --width 960 \
        --height 620 \
        --key "$PUBLISHER_KEY"
else
    echo "Skipping .apapplication packaging (set PACKER and PUBLISHER_KEY to package)."
fi
