// SPDX-License-Identifier: LGPL-2.1-or-later
//! VideoLAN VLC Core Engine (`vlccore`) - Pure Rust Implementation.
//!
//! Provides the core architectural building blocks:
//! - High precision VLC ticks (`tick`)
//! - FourCC codecs and containers catalog (`fourcc`)
//! - Elementary Stream descriptions (`es`)
//! - Memory Block buffer pipelines (`block`)
//! - Master Clock synchronization and jitter compensation (`clock`)
//! - Stream access abstractions (`stream`)
//! - Audio Output interface (`aout`)
//! - Video Output interface (`vout`)
//! - Authentic VLC Player state machine (`player`)

pub mod tick;
pub mod fourcc;
pub mod es;
pub mod block;
pub mod clock;
pub mod stream;
pub mod aout;
pub mod vout;
pub mod player;
pub mod hw_accel;

pub use tick::VlcTick;
pub use fourcc::FourCC;
pub use es::{AudioFormat, EsCategory, EsFormat, VideoFormat};
pub use block::{Block, BlockFifo};
pub use clock::VlcClock;
pub use stream::{MemoryStream, StreamError, VlcStream};
pub use aout::AudioOutput;
pub use vout::{VideoOutput, VideoPicture};
pub use player::{PlayerState, VlcPlayer};
pub use hw_accel::{HardwareAccelerator, HwAccelType, HwCapabilities, HwSurface};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_conversions() {
        let t = VlcTick::from_ms(1500);
        assert_eq!(t.to_ms(), 1500);
        assert_eq!(t.to_sec(), 1.5);
        assert_eq!(t.to_micros(), 1_500_000);
        assert_eq!(t.to_nanos(), 1_500_000_000);

        let t2 = t + VlcTick::from_ms(500);
        assert_eq!(t2.to_ms(), 2000);
    }

    #[test]
    fn test_fourcc() {
        let fcc = FourCC::from_str("s16l").unwrap();
        assert_eq!(fcc, fourcc::VLC_CODEC_S16L);
        assert_eq!(format!("{}", fcc), "s16l");
    }

    #[test]
    fn test_block_fifo() {
        let fifo = BlockFifo::new();
        assert!(fifo.is_empty());

        let b1 = Block::alloc(256);
        let b2 = Block::alloc(512);
        fifo.put(b1);
        fifo.put(b2);

        assert_eq!(fifo.depth(), 2);
        assert_eq!(fifo.bytes(), 768);

        let popped = fifo.get().unwrap();
        assert_eq!(popped.size(), 256);
        assert_eq!(fifo.depth(), 1);
        assert_eq!(fifo.bytes(), 512);
    }

    #[test]
    fn test_player_state_transitions() {
        let aout = Box::new(aout::DummyAudioOutput::new());
        let vout = Box::new(vout::DummyVideoOutput::new());
        let mut player = VlcPlayer::new(aout, vout);

        assert_eq!(player.state(), PlayerState::Stopped);
        player.play().unwrap();
        assert_eq!(player.state(), PlayerState::Playing);
        player.pause().unwrap();
        assert_eq!(player.state(), PlayerState::Paused);
        player.play().unwrap();
        assert_eq!(player.state(), PlayerState::Playing);
        player.stop().unwrap();
        assert_eq!(player.state(), PlayerState::Stopped);
    }
}
