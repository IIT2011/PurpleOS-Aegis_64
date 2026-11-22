#!/usr/bin/env bash
set -euo pipefail

# Build script for official VLC Media Player (.apapplication v2 / GUI ABI v1)
# Native freestanding compilation for PurpleOS / Aegis64 x86-64

PURPLE_OS_ROOT="${PURPLE_OS_ROOT:-../..}"
VLC_REF_ROOT="${VLC_REF_ROOT:-../../../References/vlc}"
LDSCRIPT="${LDSCRIPT:-$PURPLE_OS_ROOT/sdk/gui/purple_gui.ld}"
PACKER="${PACKER:-$PURPLE_OS_ROOT/scripts/package_gui_app.py}"
PUBLISHER_KEY="${PUBLISHER_KEY:-$PURPLE_OS_ROOT/keys/vlc_publisher.pem}"
OUT_DIR="${OUT_DIR:-build}"

mkdir -p "$OUT_DIR"

CLANG_RES=$(clang -print-resource-dir)

echo "==> Compiling native VLC Media Player objects for x86_64-unknown-none..."

MODULES=(
    "bin/purple_vlc.c:purple_vlc.o"
    "modules/video_output/purple.c:vout_purple.o"
    "modules/audio_output/purple.c:aout_purple.o"
    "modules/video_output/window_purple.c:window_purple.o"
    "src/purple/dirs.c:dirs.o"
    "src/purple/timer.c:timer.o"
    "src/purple/filesystem.c:filesystem.o"
    "src/purple/thread.c:thread.o"
)

OBJS=()

for entry in "${MODULES[@]}"; do
    mod="${entry%%:*}"
    obj_name="${entry##*:}"
    src="$VLC_REF_ROOT/$mod"
    obj="$OUT_DIR/$obj_name"
    echo "  [CC] $mod -> $obj"
    clang --target=x86_64-unknown-none -ffreestanding -nostdinc \
        -isystem "$CLANG_RES/include" \
        -I"$VLC_REF_ROOT/include" \
        -O2 -Wall -Wextra -Werror -mcmodel=large \
        -c "$src" -o "$obj"
    OBJS+=("$obj")
done

ELF="$OUT_DIR/vlc.elf"
echo "==> Linking static x86-64 ELF executable: $ELF"
ld.lld -T "$LDSCRIPT" "${OBJS[@]}" -o "$ELF"

if [[ -f "$PACKER" && -f "$PUBLISHER_KEY" ]]; then
    echo "==> Packaging and cryptographically signing vlc.apapplication..."
    python3 "$PACKER" pack \
        "$ELF" \
        "$OUT_DIR/vlc.apapplication" \
        --id vlc \
        --name "VLC Media Player" \
        --version 3.0.21 \
        --author "VideoLAN / PurpleOS" \
        --description "Official VLC Media Player ported natively to PurpleOS" \
        --width 854 \
        --height 480 \
        --priority 2 \
        --key "$PUBLISHER_KEY"
    cp -f "$OUT_DIR/vlc.apapplication" "$PURPLE_OS_ROOT/build/applications/vlc.apapplication"
    echo "==> Successfully built signed bundle: $OUT_DIR/vlc.apapplication"
    echo "==> Staged to $PURPLE_OS_ROOT/build/applications/vlc.apapplication"
else
    echo "Warning: Skipping .apapplication packaging (packer or key missing)." >&2
fi
