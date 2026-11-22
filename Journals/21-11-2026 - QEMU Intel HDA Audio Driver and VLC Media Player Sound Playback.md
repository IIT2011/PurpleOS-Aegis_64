# Engineering Journal: QEMU Intel HDA Audio Integration & VLC Sound Playback in PurpleOS

**Date:** 21 November 2026  
**System:** PurpleOS / Aegis64 (Pure Rust `#![no_std]` 64-bit Kernel)  
**Host Environment:** QEMU x86-64 Emulator (v11.1.3) on Android/Termux  
**Component:** Bare-Metal Intel HDA Controller Driver (`audio.rs`), Purple ABI Native Syscalls (`purple_abi.rs`), Official VLC Media Player (`vlc.apapplication`)

---

## 1. Objective
Establish full end-to-end sound playback in PurpleOS by:
1. Attaching virtual Intel High Definition Audio (HDA) hardware in QEMU via `-device intel-hda,id=hda0 -device hda-output,id=hda0.0,audiodev=snd0 -audiodev wav,id=snd0,path=build/purple_audio.wav`.
2. Enabling PurpleOS to detect the PCI Intel HDA device (`8086:2668`), initialize bus mastering, program CORB/RIRB DMA command rings, configure codec routing verbs, and start the output stream DMA engine.
3. Hooking the native Purple ABI audio system calls (`SYS_PURPLE_AUDIO_STREAM_START` 0x1021, `SYS_PURPLE_AUDIO_STREAM_STOP` 0x1022, `SYS_PURPLE_AUDIO_SUBMIT_PCM` 0x1023).
4. Launching the official VLC Media Player (`launch vlc`), which streams 44.1 kHz 16-bit stereo PCM audio samples into the kernel audio ring buffer.
5. Verifying audio generation and recording authentic non-zero PCM waveform samples in `build/purple_audio.wav`.

---

## 2. Technical Implementation & Architectural Challenges Resolved

### A. PCI Bus Mastering
For the Intel HDA controller to perform DMA transfers into host RAM for CORB/RIRB and audio streams, PCI Bus Mastering must be enabled on the controller:
- Added `crate::drivers::pci::enable_bus_mastering(pci_dev.bus, pci_dev.slot, pci_dev.func)` during driver discovery in [audio.rs](file:///data/data/com.termux/files/home/OS%20Development/OS/src/drivers/audio.rs#L440).

### B. Dynamic Output Stream Descriptor Base (ISS vs OSS)
According to the Intel High Definition Audio specification:
- Stream descriptors in MMIO begin at offset `0x80`.
- Input streams (ISS) precede output streams (OSS).
- The Global Capabilities register (`GCAP`, MMIO offset `0x00`) encodes `num_iss` in bits 11:8. In QEMU, `num_iss = 4`.
- Attempting to write output registers to offset `0x80` targeted Input Stream 0 (`IN0`), causing QEMU to ignore output playback.
- **Solution:** Dynamically compute output stream base:
  $$\text{stream\_base} = \text{mmio\_base} + 0x80 + (\text{num\_iss} \times 0x20)$$
  Targeting Output Stream 0 (`OUT4`) at offset `0x100`.

### C. CORB / RIRB Flow Control & RINTCNT Threshold
- Setting `HDA_REG_RINTCNT` (0x5A) to `1` caused QEMU to pause CORB command execution (`rirb count reached`) because `rirb_count == rirb_cnt`.
- **Solution:** Set `HDA_REG_RINTCNT` to `255` and explicitly clear `HDA_REG_RIRBSTS` (`0x05`) before issuing verbs.

### D. Codec Routing Verbs
Configured the default Intel HDA audio codec nodes:
1. **Node 1 (Audio Function Group):** Power State D0 (`0x705, 0x00`).
2. **Node 2 (Audio Output DAC):** Power State D0 (`0x705, 0x00`), 44.1 kHz 16-bit stereo format (`0x2, 0x4011`), stream tag 1 channel 0 (`0x706, 0x10`), and unmuted amplifier gain (`0x3B0, 0x7F`).
3. **Node 3 (Pin Complex):** Power State D0 (`0x705, 0x00`), Output + Headphone Amp enable (`0x707, 0xC0`), unmuted output gain (`0x3B0, 0x7F`).

### E. Circular DMA Ring Buffer & Native Syscalls
- Implemented `HdaController::submit_pcm(&mut self, data: &[u8]) -> usize` with circular wrapping across the 64 KB DMA buffer.
- Connected `SYS_PURPLE_AUDIO_STREAM_START`, `SYS_PURPLE_AUDIO_STREAM_STOP`, and `SYS_PURPLE_AUDIO_SUBMIT_PCM` in [purple_abi.rs](file:///data/data/com.termux/files/home/OS%20Development/OS/src/kernel/purple_abi.rs#L940-L985).

---

## 3. End-to-End Verification Results

Automated test executed via [test_qemu_vlc_audio.py](file:///data/data/com.termux/files/home/OS%20Development/OS/scripts/test_qemu_vlc_audio.py):

```
============================================================
  AUDIO SUBSYSTEM VERIFICATION REPORT
============================================================
1. Intel HDA Driver Initialized: [PASS]
2. Native Audio Syscall Test:    [PASS]
3. VLC Application Launched:     [PASS]
4. Output Audio File:            build/purple_audio.wav (1,004,500 bytes)
   - Channels:    2 (Stereo)
   - Sample Rate: 44100 Hz
   - Bit Depth:   16 bits
   - Total Frames:251,114
   - Duration:    5.69 seconds
   - Waveform Amplitude Range: [-3771, 3768] (Non-zero audio active)
5. Audio Stream Playback:        [PASS]
============================================================
```

- **Output Audio:** 1,004,500 bytes of verified PCM audio written to [purple_audio.wav](file:///data/data/com.termux/files/home/OS%20Development/OS/build/purple_audio.wav).
- **Format:** 44.1 kHz, 16-bit Little-Endian, 2 Channels (Stereo).
- **Waveform Activity:** Amplitude range `[-3771, 3768]` confirming real, non-silent audio data streaming directly from VLC running inside PurpleOS.
