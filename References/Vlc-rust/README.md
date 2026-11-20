# Pure Rust Port of VideoLAN VLC Media Player (`Vlc-rust`)

A native, 100% pure Rust conversion of the authentic VideoLAN VLC Media Player architecture for PurpleOS / Aegis64.

---

## 1. Architecture Overview & C-to-Rust Mapping

| Original VLC C/C++ Subsystem | Original Path | Pure Rust Crate / Module | Description |
|:---|:---|:---|:---|
| `vlc_tick_t` & timing | `include/vlc_tick.h` | [`vlccore::tick`](vlccore/src/tick.rs) | High-precision nanosecond tick definitions and conversions |
| FourCC Codec Catalog | `include/vlc_fourcc.h` | [`vlccore::fourcc`](vlccore/src/fourcc.rs) | Container and codec FourCC tags (`s16l`, `f32l`, `mp3`, `I420`, etc.) |
| Elementary Streams | `include/vlc_es.h` | [`vlccore::es`](vlccore/src/es.rs) | `EsFormat`, `AudioFormat`, `VideoFormat` representations |
| Memory Blocks & FIFO | `include/vlc_block.h` | [`vlccore::block`](vlccore/src/block.rs) | Sliced memory blocks with PTS/DTS and thread-safe FIFO queues |
| Master Clock & Sync | `include/vlc_clock.h` | [`vlccore::clock`](vlccore/src/clock.rs) | Clock drift compensation, rate scaling, and PTS synchronization |
| Byte Stream Reader | `include/vlc_stream.h` | [`vlccore::stream`](vlccore/src/stream.rs) | Abstract seekable byte streams (`MemoryStream`, `FileStream`) |
| Audio Output Subsystem | `include/vlc_aout.h` | [`vlccore::aout`](vlccore/src/aout.rs) | Audio device abstraction, format negotiation, volume control |
| Video Output Subsystem | `include/vlc_vout.h` | [`vlccore::vout`](vlccore/src/vout.rs) | Video presentation pipeline and picture framebuffers |
| Media Player Controller | `include/vlc_player.h` | [`vlccore::player`](vlccore/src/player.rs) | Play/Pause/Stop/Seek state machine and playback engine |
| RIFF/WAV Demuxer | `modules/demux/wav.c` | [`vlc-demux::wav`](vlc-demux/src/wav.rs) | Header and chunk parser for uncompressed PCM and IEEE float WAV |
| Raw Audio Demuxer | `modules/demux/rawaud.c` | [`vlc-demux::raw`](vlc-demux/src/raw.rs) | Raw PCM streaming demuxer |
| MPEG Audio Sync | `modules/demux/mpeg/mp3.c` | [`vlc-demux::mp3`](vlc-demux/src/mp3.rs) | MPEG-1 Layer 3 sync-word search and frame header decoding |
| Audio Format Converter | `modules/audio_filter/converter/format.c` | [`vlc-codec::pcm`](vlc-codec/src/pcm.rs) | Float32 to Signed 16-bit integer and 8-bit to 16-bit PCM conversion |
| Purple Audio Plugin | `modules/audio_output/purple.c` | [`vlc-platform-purple::audio`](vlc-platform-purple/src/audio.rs) | Direct Ring 3 hardware DMA PCM streaming (`0x1021`, `0x1023`) |
| Purple Window & Video | `modules/video_output/purple.c` | [`vlc-platform-purple::video`](vlc-platform-purple/src/video.rs) | Double-buffered compositor presentation (`0x1101`..`0x1105`) |
| VLC Frontend Player | `bin/vlc.c` | [`vlc-app`](vlc-app/src/main.rs) | Interactive UI with cone logo, scrub bar, volume, and spectrum visualizer |

---

## 2. Directory Structure

```
OS/References/Vlc-rust/ (or References/Vlc-rust/)
├── Cargo.toml                  # Cargo Workspace Definition
├── README.md                   # Architecture & Port Documentation
├── vlccore/                    # Pure Rust Core Engine
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── tick.rs             # vlc_tick_t
│       ├── fourcc.rs           # FourCC
│       ├── es.rs               # Elementary streams
│       ├── block.rs            # block_t & block_fifo_t
│       ├── clock.rs            # vlc_clock_t
│       ├── stream.rs           # stream_t
│       ├── aout.rs             # aout_instance_t
│       ├── vout.rs             # vout_thread_t
│       └── player.rs           # vlc_player_t
├── vlc-demux/                  # Demuxer Plugins
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── wav.rs              # modules/demux/wav.c
│       ├── raw.rs              # modules/demux/rawaud.c
│       └── mp3.rs              # modules/demux/mpeg/mp3.c
├── vlc-codec/                  # Audio/Video Decoders & Converters
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       └── pcm.rs              # format converter
├── vlc-platform-purple/        # Native PurpleOS Platform Provider
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── syscall.rs          # Ring 3 Syscall Assembly
│       ├── audio.rs            # HDA Audio DMA driver
│       └── video.rs            # Compositor & Window provider
└── vlc-app/                    # VLC Application Executable
    ├── Cargo.toml
    └── src/
        ├── main.rs             # Main loop & player integration
        └── ui.rs               # Acrylic glass UI & cone renderer
```

---

## 3. Building and Testing

```bash
# Verify all crates
cargo check --workspace

# Run all unit and integration tests
cargo test --workspace
```
