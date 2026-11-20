// SPDX-License-Identifier: LGPL-2.1-or-later
//! Authentic VideoLAN VLC Media Player - Pure Rust Application for PurpleOS.

mod ui;

use ui::VlcUi;
use vlccore::aout::AudioOutput;
use vlccore::block::Block;
use vlccore::es::AudioFormat;
use vlccore::fourcc::VLC_CODEC_S16L;
use vlccore::player::VlcPlayer;
use vlccore::tick::VlcTick;
use vlc_codec::PcmDecoder;
use vlc_demux::RawAudDemuxer;
use vlc_platform_purple::{PurpleAudioOutput, PurpleVideoOutput, PurpleWindow};

fn generate_synthetic_audio(samples: usize, phase: &mut f32) -> (Vec<u8>, Vec<i16>) {
    let mut raw = Vec::with_capacity(samples * 4);
    let mut pcm16 = Vec::with_capacity(samples * 2);

    for _ in 0..samples {
        let val1 = (*phase * 2.0 * core::f32::consts::PI).sin();
        let val2 = (*phase * 3.0 * core::f32::consts::PI).sin() * 0.5;
        let s = ((val1 + val2) * 12000.0) as i16;

        *phase += 440.0 / 44100.0;
        if *phase >= 1.0 {
            *phase -= 1.0;
        }

        // Stereo L / R
        raw.extend_from_slice(&s.to_le_bytes());
        raw.extend_from_slice(&s.to_le_bytes());
        pcm16.push(s);
        pcm16.push(s);
    }

    (raw, pcm16)
}

fn main() {
    let width = 854u32;
    let height = 480u32;

    let window = PurpleWindow::new(width, height);
    if let Err(_e) = window.open() {
        return;
    }

    let mut aout = PurpleAudioOutput::new();
    let audio_fmt = AudioFormat::new(VLC_CODEC_S16L, 2, 44100, 16);
    let _ = aout.open(&audio_fmt);

    let mut decoder = PcmDecoder::new(audio_fmt.clone());
    let _demuxer = RawAudDemuxer::new(44100, 2);

    let vout = PurpleVideoOutput::new(window.clone());
    let ui = VlcUi::new(width, height);

    let mut player = VlcPlayer::new(Box::new(aout), Box::new(vout));
    player.set_length(VlcTick::from_sec(214)); // 3:34 length track
    let _ = player.play();

    let mut phase = 0.0f32;

    loop {
        // Handle input events
        if let Some(evt) = window.poll_event() {
            match evt.event_type {
                1 => {
                    // Close request
                    let _ = player.stop();
                    break;
                }
                2 => {
                    // Pointer click
                    let px = evt.p1;
                    let py = evt.p2;
                    let bar_y = height - 60;

                    // Play/Pause button
                    if px >= 30 && px <= 58 && py >= bar_y + 24 && py <= bar_y + 52 {
                        let _ = player.toggle_pause();
                    }

                    // Progress scrub bar
                    if py >= bar_y + 6 && py <= bar_y + 20 && px >= 16 && px <= width - 16 {
                        let rel = (px - 16) as f64 / (width - 32) as f64;
                        let _ = player.seek(rel);
                    }

                    // Volume slider
                    let vol_x = width - 150;
                    if py >= bar_y + 30 && py <= bar_y + 46 && px >= vol_x && px <= vol_x + 100 {
                        let v = ((px - vol_x) as f32 / 100.0) * 2.0;
                        let _ = player.set_volume(v);
                    }
                }
                _ => {}
            }
        }

        let mut pcm_vis = Vec::new();

        // Stream audio if playing
        if player.is_playing() {
            let (raw_bytes, pcm_samples) = generate_synthetic_audio(1024, &mut phase);
            let mut block = Block::from_slice(&raw_bytes);
            block.pts = player.time();
            if let Some(decoded) = decoder.decode(&block) {
                let _ = player.aout_mut().play(&decoded);
            }
            pcm_vis = pcm_samples;

            // Advance playback position
            let next_time = player.time() + VlcTick::from_ms(23);
            player.set_time(next_time);
            if player.time().to_nanos() >= player.length().to_nanos() {
                player.set_time(VlcTick::ZERO);
            }
        }

        // Render UI frame
        window.with_buffer_mut(|buf| {
            ui.render_frame(
                buf,
                player.is_playing(),
                player.position(),
                player.volume(),
                player.time().to_ms(),
                &pcm_vis,
            );
        });

        let _ = window.present();

        // Sleep ~16ms for 60fps
        unsafe {
            vlc_platform_purple::syscall::syscall1(
                vlc_platform_purple::syscall::SYS_PURPLE_SLEEP_NANOS,
                16_000_000,
            );
        }
    }
}
