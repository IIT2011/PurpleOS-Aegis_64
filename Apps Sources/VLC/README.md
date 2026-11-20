# VLC Media Player for PurpleOS / Aegis64 (`Apps Sources/VLC`)

Pure Rust native conversion and port of VideoLAN's official VLC Media Player architecture for PurpleOS / Aegis64.

---

## Architecture & Subsystems

This directory houses the complete pure Rust VLC workspace:
- **`vlccore`**: The core VLC engine:
  - `tick`: High-precision nanosecond tick definitions and conversions (`vlc_tick_t`).
  - `fourcc`: Container and codec FourCC tags (`s16l`, `f32l`, `mp3`, `I420`, etc.).
  - `es`: Elementary Stream definitions (`EsFormat`, `AudioFormat`, `VideoFormat`).
  - `block`: Sliced memory blocks (`Block`) and thread-safe queues (`BlockFifo`).
  - `clock`: Clock drift compensation, rate scaling, and PTS synchronization (`vlc_clock_t`).
  - `stream`: Abstract seekable byte streams (`MemoryStream`, `FileStream`).
  - `aout`: Audio device abstraction, format negotiation, volume control (`AudioOutput`).
  - `vout`: Video presentation pipeline and picture framebuffers (`VideoOutput`).
  - `player`: Play/Pause/Stop/Seek state machine and playback controller (`VlcPlayer`).
- **`vlc-demux`**: Pure Rust demuxers:
  - `wav`: Header and chunk parser for uncompressed PCM and IEEE float WAV.
  - `raw`: Raw PCM streaming demuxer.
  - `mp3`: MPEG-1 Layer 3 sync-word search and frame header decoding.
- **`vlc-codec`**: Pure Rust decoders & format converters:
  - `pcm`: Float32 to Signed 16-bit integer and 8-bit to 16-bit PCM conversion.
- **`vlc-platform-purple`**: Native PurpleOS platform provider:
  - `audio`: Direct Ring 3 hardware DMA PCM streaming (`0x1021`, `0x1023`).
  - `video`: Double-buffered compositor presentation (`0x1101`..`0x1105`).
  - `syscall`: Raw assembly syscall dispatch for PurpleOS.
- **`vlc-app`**: Native executable frontend player:
  - Modern acrylic glass titlebar with VLC cone logo icon.
  - Center stage mathematical rendering of the iconic VLC cone logo.
  - Real-time 64-band audio spectrum visualizer.
  - Transport controls (Play/Pause, Stop, Seek scrub bar, Volume slider).

---

## Packaging & Cryptographic Authentication

- Packaged as an `.apapplication` v2 binary with static x86-64 code linked via `sdk/gui/purple_gui.ld` at `0x0000008000000000`.
- Cryptographically signed with Ed25519 publisher private key (`keys/vlc_publisher.pem`).
- Authorized in `APTRUST1` policy signed with Creator RSA-2048 offline private key (`keys/creator_private_key.pem`).
- Verified at boot by AegisDetect before execution.

---

## Directory Structure

```
Apps Sources/VLC/
├── Cargo.toml                  # Workspace manifest
├── Cargo.lock
├── README.md                   # This documentation
├── build.sh                    # Build and packaging script
├── build/                      # Packaged .apapplication and signed trust policy
│   ├── vlc.apapplication
│   ├── trust.policy
│   └── trust.policy.sig
├── vlccore/                    # Pure Rust Core Engine
├── vlc-demux/                  # Container demuxers (WAV, Raw, MP3)
├── vlc-codec/                  # Audio/Video decoders
├── vlc-platform-purple/        # Native PurpleOS platform provider
└── vlc-app/                    # VLC executable & UI renderer
```
