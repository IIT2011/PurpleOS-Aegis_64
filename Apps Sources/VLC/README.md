# Official VLC Media Player for PurpleOS / Aegis64

Native port of VideoLAN's official VLC Media Player architecture to PurpleOS / Aegis64.

## Architecture

The VLC port strictly adheres to official upstream VLC modular architecture:
- **Video Output:** `modules/video_output/purple.c` implements `vout_display_t` delivering zero-copy linear ARGB8888 presentation to the PurpleOS Compositor via `PURPLE_GUI_PRESENT` (0x1104) and `PURPLE_GUI_WINDOW_GLASS` (0x110c).
- **Audio Output:** `modules/audio_output/purple.c` implements `audio_output_t` streaming real-time 44.1 kHz 16-bit PCM stereo audio directly into the kernel HDA hardware ring via `SYS_PURPLE_AUDIO_SUBMIT_PCM` (0x1023) and `SYS_PURPLE_AUDIO_SET_VOLUME` (0x1024).
- **Window Management:** `modules/video_output/window_purple.c` implements `vlc_window_t` operations, polling compositor events via `PURPLE_GUI_POLL` (0x1105).
- **Platform Layer:** `src/purple/` (`dirs.c`, `timer.c`, `filesystem.c`, `thread.c`) provides high-resolution timing via `SYS_PURPLE_CLOCK_NANOS` (0x1052) and native filesystem access.
- **Frontend Player:** `bin/purple_vlc.c` provides the iconic VLC player interface featuring real-time audio spectrum visualization, seekable timeline scrub bar, volume slider, playback controls, and mathematical rendering of the official VLC traffic cone logo.

## Packaging & Cryptographic Authentication

- Packaged as an `.apapplication` v2 binary with static x86-64 code linked via `sdk/gui/purple_gui.ld` at `0x0000008000000000`.
- Cryptographically signed with Ed25519 publisher private key (`keys/vlc_publisher.pem`).
- Authorized in `APTRUST1` policy signed with Creator RSA-2048 offline private key (`keys/creator_private_key.pem`).
- Verified at boot by AegisDetect before execution.

## Building

Run `./build.sh` from this directory.
