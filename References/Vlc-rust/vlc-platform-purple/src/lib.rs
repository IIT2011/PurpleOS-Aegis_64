// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Platform Provider for PurpleOS in Pure Rust.

pub mod syscall;
pub mod audio;
pub mod video;

pub use audio::PurpleAudioOutput;
pub use video::{GuiEvent, PurpleVideoOutput, PurpleWindow};
