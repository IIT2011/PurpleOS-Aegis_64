//! Intel High Definition Audio (HDA) / AC97 Subsystem for Aegis64.
//!
//! Derived from Linux Kernel sound subsystem reference:
//!   - `sound/pci/hda/hda_intel.c`
//!   - `sound/pci/hda/hda_codec.h`
//!   - Intel High Definition Audio Specification Rev 1.0a

use crate::drivers::dma::alloc_coherent;
use crate::drivers::pci::find_device;
use crate::sync::Spinlock;

pub const HDA_VENDOR_INTEL: u16 = 0x8086;
pub const HDA_DEV_ICH6: u16 = 0x2668;
pub const HDA_DEV_ICH7: u16 = 0x27D8;
pub const HDA_DEV_ICH8: u16 = 0x284B;
pub const HDA_DEV_ICH9: u16 = 0x293E;
pub const HDA_DEV_QEMU: u16 = 0x2668;

/* Register Offsets (HDA Spec Table 1) */
pub const HDA_REG_GCAP: usize = 0x00; /* Global Capabilities (16-bit) */
pub const HDA_REG_VMIN: usize = 0x02; /* Minor Version (8-bit) */
pub const HDA_REG_VMAJ: usize = 0x03; /* Major Version (8-bit) */
pub const HDA_REG_OUTPAY: usize = 0x04; /* Output Payload (16-bit) */
pub const HDA_REG_INPAY: usize = 0x06; /* Input Payload (16-bit) */
pub const HDA_REG_GCTL: usize = 0x08; /* Global Control (32-bit) */
pub const HDA_REG_WAKEEN: usize = 0x0C; /* Wake Enable (16-bit) */
pub const HDA_REG_STATESTS: usize = 0x0E; /* State Change Status (16-bit) */
pub const HDA_REG_INTCTL: usize = 0x20; /* Interrupt Control (32-bit) */
pub const HDA_REG_INTSTS: usize = 0x24; /* Interrupt Status (32-bit) */

/* CORB (Command Outbound Ring Buffer) Registers */
pub const HDA_REG_CORBLBASE: usize = 0x40; /* Lower Base Address (32-bit) */
pub const HDA_REG_CORBUBASE: usize = 0x44; /* Upper Base Address (32-bit) */
pub const HDA_REG_CORBWP: usize = 0x48; /* Write Pointer (16-bit) */
pub const HDA_REG_CORBRP: usize = 0x4A; /* Read Pointer (16-bit) */
pub const HDA_REG_CORBCTL: usize = 0x4C; /* Control (8-bit) */
pub const HDA_REG_CORBSTS: usize = 0x4D; /* Status (8-bit) */
pub const HDA_REG_CORBSIZE: usize = 0x4E; /* Size (8-bit) */

/* RIRB (Response Inbound Ring Buffer) Registers */
pub const HDA_REG_RIRBLBASE: usize = 0x50; /* Lower Base Address (32-bit) */
pub const HDA_REG_RIRBUBASE: usize = 0x54; /* Upper Base Address (32-bit) */
pub const HDA_REG_RIRBWP: usize = 0x58; /* Write Pointer (16-bit) */
pub const HDA_REG_RINTCNT: usize = 0x5A; /* Response Interrupt Count (16-bit) */
pub const HDA_REG_RIRBCTL: usize = 0x5C; /* Control (8-bit) */
pub const HDA_REG_RIRBSTS: usize = 0x5D; /* Status (8-bit) */
pub const HDA_REG_RIRBSIZE: usize = 0x5E; /* Size (8-bit) */

/* Stream Descriptor Registers (Stream 0 Output, offset 0x80 + 0 * 0x20) */
pub const HDA_SD0_OFFSET: usize = 0x80;
pub const HDA_SD_CTL: usize = 0x00; /* Stream Control (24-bit) */
pub const HDA_SD_STS: usize = 0x03; /* Stream Status (8-bit) */
pub const HDA_SD_LPIB: usize = 0x04; /* Link Position (32-bit) */
pub const HDA_SD_CBL: usize = 0x08; /* Cyclic Buffer Length (32-bit) */
pub const HDA_SD_LVI: usize = 0x0C; /* Last Valid Index (16-bit) */
pub const HDA_SD_FIFOD: usize = 0x10; /* FIFO Size (16-bit) */
pub const HDA_SD_FMT: usize = 0x12; /* Stream Format (16-bit) */
pub const HDA_SD_BDPL: usize = 0x18; /* BDL Base Lower (32-bit) */
pub const HDA_SD_BDPU: usize = 0x1C; /* BDL Base Upper (32-bit) */

/* Global Control Register Bits */
pub const HDA_GCTL_CRST: u32 = 0x00000001; /* Controller Reset */

/* CORB / RIRB Constants */
pub const CORB_ENTRIES: usize = 256;
pub const RIRB_ENTRIES: usize = 256;
pub const HDA_DMA_BUFFER_SIZE: usize = 65536;

#[repr(C, packed)]
#[derive(Copy, Clone, Default)]
pub struct HdaBdlEntry {
    pub addr: u64,
    pub length: u32,
    pub ioc: u32,
}

pub struct HdaController {
    pub mmio_base: usize,
    pub stream_base: usize,
    pub is_ready: bool,
    pub is_streaming: bool,
    pub num_codecs: u8,
    pub volume: u8,
    pub write_pos: usize,
    corb: *mut u32,
    corb_phys: u64,
    corb_wp: u16,
    rirb: *mut u64,
    rirb_phys: u64,
    rirb_rp: u16,
    bdl: *mut HdaBdlEntry,
    bdl_phys: u64,
    audio_buf: *mut u8,
    audio_buf_phys: u64,
}

unsafe impl Send for HdaController {}
unsafe impl Sync for HdaController {}

impl HdaController {
    pub const fn new() -> Self {
        Self {
            mmio_base: 0,
            stream_base: 0,
            is_ready: false,
            is_streaming: false,
            num_codecs: 0,
            volume: 80,
            write_pos: 0,
            corb: core::ptr::null_mut(),
            corb_phys: 0,
            corb_wp: 0,
            rirb: core::ptr::null_mut(),
            rirb_phys: 0,
            rirb_rp: 0,
            bdl: core::ptr::null_mut(),
            bdl_phys: 0,
            audio_buf: core::ptr::null_mut(),
            audio_buf_phys: 0,
        }
    }

    #[inline(always)]
    unsafe fn write32(&self, reg: usize, val: u32) {
        let ptr = (self.mmio_base + reg) as *mut u32;
        core::ptr::write_volatile(ptr, val);
    }

    #[inline(always)]
    unsafe fn read32(&self, reg: usize) -> u32 {
        let ptr = (self.mmio_base + reg) as *const u32;
        core::ptr::read_volatile(ptr)
    }

    #[inline(always)]
    unsafe fn write16(&self, reg: usize, val: u16) {
        let ptr = (self.mmio_base + reg) as *mut u16;
        core::ptr::write_volatile(ptr, val);
    }

    #[inline(always)]
    unsafe fn read16(&self, reg: usize) -> u16 {
        let ptr = (self.mmio_base + reg) as *const u16;
        core::ptr::read_volatile(ptr)
    }

    #[inline(always)]
    unsafe fn write8(&self, reg: usize, val: u8) {
        let ptr = (self.mmio_base + reg) as *mut u8;
        core::ptr::write_volatile(ptr, val);
    }

    #[allow(dead_code)]
    #[inline(always)]
    unsafe fn read8(&self, reg: usize) -> u8 {
        let ptr = (self.mmio_base + reg) as *const u8;
        core::ptr::read_volatile(ptr)
    }

    pub fn init_hw(&mut self, bar0: u64) -> Result<(), &'static str> {
        self.mmio_base = bar0 as usize;

        unsafe {
            // 1. Controller Hardware Reset Sequence (CRST)
            // Clear CRST bit 0 to place controller in reset
            let mut gctl = self.read32(HDA_REG_GCTL);
            self.write32(HDA_REG_GCTL, gctl & !HDA_GCTL_CRST);

            // Poll until CRST bit reads 0
            let mut timeout = 10000;
            while (self.read32(HDA_REG_GCTL) & HDA_GCTL_CRST) != 0 && timeout > 0 {
                core::hint::spin_loop();
                timeout -= 1;
            }

            // Bring out of reset: set CRST bit to 1
            gctl = self.read32(HDA_REG_GCTL);
            self.write32(HDA_REG_GCTL, gctl | HDA_GCTL_CRST);

            // Wait until CRST reads 1
            timeout = 10000;
            while (self.read32(HDA_REG_GCTL) & HDA_GCTL_CRST) == 0 && timeout > 0 {
                core::hint::spin_loop();
                timeout -= 1;
            }

            // Small delay for codecs to initialize and signal STATESTS
            for _ in 0..50000 {
                core::hint::spin_loop();
            }

            // 2. Discover Codecs via STATESTS
            let statests = self.read16(HDA_REG_STATESTS);
            self.num_codecs = 0;
            for i in 0..15 {
                if (statests & (1 << i)) != 0 {
                    self.num_codecs += 1;
                }
            }

            // 3. Allocate DMA memory for CORB (256 * 4 bytes = 1024 bytes)
            let (corb_ptr, corb_phys) = alloc_coherent(1024).ok_or("Failed to alloc CORB")?;
            self.corb = corb_ptr as *mut u32;
            self.corb_phys = corb_phys;

            // Stop CORB before programming
            self.write8(HDA_REG_CORBCTL, 0);

            // Set CORB base address
            self.write32(HDA_REG_CORBLBASE, (corb_phys & 0xFFFFFFFF) as u32);
            self.write32(HDA_REG_CORBUBASE, (corb_phys >> 32) as u32);

            // Set CORB Size (0x02 = 256 entries)
            self.write8(HDA_REG_CORBSIZE, 0x02);

            // Reset CORB read pointer
            self.write16(HDA_REG_CORBRP, 0x8000);
            while (self.read16(HDA_REG_CORBRP) & 0x8000) == 0 {
                core::hint::spin_loop();
            }
            self.write16(HDA_REG_CORBRP, 0);

            // Reset CORB write pointer
            self.write16(HDA_REG_CORBWP, 0);
            self.corb_wp = 0;

            // Start CORB DMA engine
            self.write8(HDA_REG_CORBCTL, 0x02);

            // 4. Allocate DMA memory for RIRB (256 * 8 bytes = 2048 bytes)
            let (rirb_ptr, rirb_phys) = alloc_coherent(2048).ok_or("Failed to alloc RIRB")?;
            self.rirb = rirb_ptr as *mut u64;
            self.rirb_phys = rirb_phys;

            // Stop RIRB before programming
            self.write8(HDA_REG_RIRBCTL, 0);

            // Set RIRB base address
            self.write32(HDA_REG_RIRBLBASE, (rirb_phys & 0xFFFFFFFF) as u32);
            self.write32(HDA_REG_RIRBUBASE, (rirb_phys >> 32) as u32);

            // Set RIRB Size (0x02 = 256 entries)
            self.write8(HDA_REG_RIRBSIZE, 0x02);

            // Reset RIRB write pointer
            self.write16(HDA_REG_RIRBWP, 0x8000);

            // Reset RIRB read pointer
            self.rirb_rp = 0;

            // Set Interrupt Count to maximum (255) so QEMU does not stall CORB processing
            self.write16(HDA_REG_RINTCNT, 255);

            // Clear RIRB status
            self.write8(HDA_REG_RIRBSTS, 0x05);

            // Start RIRB DMA engine (RIRBDMAEN = bit 1)
            self.write8(HDA_REG_RIRBCTL, 0x02);

            // 5. Setup DMA Buffer Descriptor List (BDL) for Audio Output Stream 0
            let (bdl_ptr, bdl_phys) = alloc_coherent(1024).ok_or("Failed to alloc BDL")?;
            self.bdl = bdl_ptr as *mut HdaBdlEntry;
            self.bdl_phys = bdl_phys;

            let (buf_ptr, buf_phys) =
                alloc_coherent(HDA_DMA_BUFFER_SIZE).ok_or("Failed to alloc Audio DMA Buf")?;
            self.audio_buf = buf_ptr;
            self.audio_buf_phys = buf_phys;

            // Setup 2 BDL entries for double buffering (32KB each)
            let e0 = self.bdl.add(0);
            (*e0).addr = buf_phys;
            (*e0).length = 32768;
            (*e0).ioc = 1;

            let e1 = self.bdl.add(1);
            (*e1).addr = buf_phys + 32768;
            (*e1).length = 32768;
            (*e1).ioc = 1;

            // Determine output stream offset from GCAP (skip input streams)
            let gcap = self.read16(HDA_REG_GCAP);
            let num_iss = ((gcap >> 8) & 0x0F) as usize;
            let stream_base = self.mmio_base + HDA_SD0_OFFSET + (num_iss * 0x20);
            self.stream_base = stream_base;

            let ctl_ptr = (stream_base + HDA_SD_CTL) as *mut u32;
            let cbl_ptr = (stream_base + HDA_SD_CBL) as *mut u32;
            let lvi_ptr = (stream_base + HDA_SD_LVI) as *mut u16;
            let fmt_ptr = (stream_base + HDA_SD_FMT) as *mut u16;
            let bdpl_ptr = (stream_base + HDA_SD_BDPL) as *mut u32;
            let bdpu_ptr = (stream_base + HDA_SD_BDPU) as *mut u32;

            // Reset stream
            core::ptr::write_volatile(ctl_ptr, 0x01); // SRST = 1
            while (core::ptr::read_volatile(ctl_ptr) & 0x01) == 0 {
                core::hint::spin_loop();
            }
            core::ptr::write_volatile(ctl_ptr, 0x00); // Clear SRST
            while (core::ptr::read_volatile(ctl_ptr) & 0x01) != 0 {
                core::hint::spin_loop();
            }

            // Set BDL pointer
            core::ptr::write_volatile(bdpl_ptr, (bdl_phys & 0xFFFFFFFF) as u32);
            core::ptr::write_volatile(bdpu_ptr, (bdl_phys >> 32) as u32);

            // Set Cyclic Buffer Length (65536 bytes)
            core::ptr::write_volatile(cbl_ptr, 65536);

            // Set Last Valid Index (1 = 2 buffers)
            core::ptr::write_volatile(lvi_ptr, 1);

            // Set format: 44.1 kHz, 16-bit, 2 channels (Stereo)
            // Bit 14: base rate (0 = 48kHz, 1 = 44.1kHz), bits 6:4: bits (001 = 16-bit), bits 3:0: channels (1 = 2 channels)
            core::ptr::write_volatile(fmt_ptr, (1 << 14) | (1 << 4) | 1);

            // Zero out DMA buffer
            core::ptr::write_bytes(self.audio_buf, 0, HDA_DMA_BUFFER_SIZE);
        }

        self.is_ready = true;

        // Configure Codec 0 Nodes:
        let gain = (self.volume as u16 * 127) / 100;
        // 1. Node 1: Audio Function Group (AFG) - Power State D0 (active)
        self.send_verb(0, 1, 0x705, 0x00);

        // 2. Node 2: Audio Output Converter (DAC) - Power State D0
        self.send_verb(0, 2, 0x705, 0x00);
        // Set Converter Format: 44.1kHz, 16-bit, Stereo
        self.send_verb16(0, 2, 0x2, (1 << 14) | (1 << 4) | 1);
        // Set Converter Stream/Channel: Stream Tag 1, Channel 0
        self.send_verb(0, 2, 0x706, 0x10);
        // Set Amp Gain: Unmute, Max Gain (127) on Left + Right (0x3B0)
        self.send_verb(0, 2, 0x3B0, gain as u8);

        // 3. Node 3: Audio Output Pin Complex - Power State D0
        self.send_verb(0, 3, 0x705, 0x00);
        // Set Pin Widget Control: Output Enable (0x40) + Headphone Amp Enable (0x80)
        self.send_verb(0, 3, 0x707, 0xC0);
        // Set Amp Gain: Unmute, Max Gain (127) on Left + Right (0x3B0)
        self.send_verb(0, 3, 0x3B0, gain as u8);

        Ok(())
    }

    /// Send a 32-bit raw verb and wait for 64-bit response
    pub fn send_verb_raw(&mut self, cmd: u32) -> u64 {
        if self.corb.is_null() || self.rirb.is_null() {
            return 0;
        }

        unsafe {
            // Clear any pending RIRB status flags before sending
            self.write8(HDA_REG_RIRBSTS, 0x05);

            // Next write pointer
            let wp = (self.corb_wp + 1) % (CORB_ENTRIES as u16);
            *self.corb.add(wp as usize) = cmd;
            self.write16(HDA_REG_CORBWP, wp);
            self.corb_wp = wp;

            // Wait for response in RIRB
            let mut timeout = 20000;
            while timeout > 0 {
                let rirb_wp = self.read16(HDA_REG_RIRBWP) & 0x00FF;
                if rirb_wp != self.rirb_rp {
                    self.rirb_rp = (self.rirb_rp + 1) % (RIRB_ENTRIES as u16);
                    let resp = *self.rirb.add(self.rirb_rp as usize);
                    // Clear RIRB status (RINTFL / RIRBOIS) and refresh RINTCNT so hardware accepts next response
                    self.write8(HDA_REG_RIRBSTS, 0x05);
                    self.write16(HDA_REG_RINTCNT, 255);
                    return resp;
                }
                timeout -= 1;
                core::hint::spin_loop();
            }
        }
        0
    }

    /// Send a 12-bit verb with 8-bit payload
    pub fn send_verb(&mut self, codec: u8, nid: u8, verb: u16, payload: u8) -> u64 {
        let cmd =
            ((codec as u32) << 28) | ((nid as u32) << 20) | ((verb as u32) << 8) | (payload as u32);
        self.send_verb_raw(cmd)
    }

    /// Send a 4-bit verb with 16-bit payload (e.g. Set Converter Format 0x2)
    pub fn send_verb16(&mut self, codec: u8, nid: u8, verb4: u8, payload16: u16) -> u64 {
        let cmd = ((codec as u32) << 28)
            | ((nid as u32) << 20)
            | (((verb4 & 0x0F) as u32) << 16)
            | (payload16 as u32);
        self.send_verb_raw(cmd)
    }

    pub fn set_volume(&mut self, vol: u8) {
        let clamped = vol.min(100);
        self.volume = clamped;
        let gain = (clamped as u16 * 127) / 100;
        // Dispatch amplifier verbs to DAC (Node 2) and Pin (Node 3)
        self.send_verb(0, 2, 0x3B0, gain as u8);
        self.send_verb(0, 3, 0x3B0, gain as u8);
    }

    pub fn start_stream(&mut self) {
        if self.stream_base == 0 {
            return;
        }
        unsafe {
            let ctl_ptr = (self.stream_base + HDA_SD_CTL) as *mut u32;
            let cur = core::ptr::read_volatile(ctl_ptr) & 0x00FFFFFF;
            // Bits 23:20 = Stream Tag 1, Bit 2 = IOCE, Bit 1 = RUN
            core::ptr::write_volatile(ctl_ptr, (cur & !(0x0F << 20)) | (1 << 20) | 0x06);
        }
        self.is_streaming = true;
    }

    pub fn stop_stream(&mut self) {
        if self.stream_base == 0 {
            return;
        }
        unsafe {
            let ctl_ptr = (self.stream_base + HDA_SD_CTL) as *mut u32;
            let cur = core::ptr::read_volatile(ctl_ptr) & 0x00FFFFFF;
            core::ptr::write_volatile(ctl_ptr, cur & !0x02);
        }
        self.is_streaming = false;
        self.write_pos = 0;
    }

    pub fn submit_pcm(&mut self, data: &[u8]) -> usize {
        if !self.is_ready || self.audio_buf.is_null() {
            return 0;
        }
        let len = data.len();
        if len == 0 {
            return 0;
        }
        let copy_len = len.min(HDA_DMA_BUFFER_SIZE);
        let start_pos = self.write_pos;
        if start_pos + copy_len <= HDA_DMA_BUFFER_SIZE {
            unsafe {
                core::ptr::copy_nonoverlapping(data.as_ptr(), self.audio_buf.add(start_pos), copy_len);
            }
        } else {
            let first = HDA_DMA_BUFFER_SIZE - start_pos;
            let second = copy_len - first;
            unsafe {
                core::ptr::copy_nonoverlapping(data.as_ptr(), self.audio_buf.add(start_pos), first);
                core::ptr::copy_nonoverlapping(data.as_ptr().add(first), self.audio_buf, second);
            }
        }
        self.write_pos = (self.write_pos + copy_len) % HDA_DMA_BUFFER_SIZE;

        if !self.is_streaming {
            self.start_stream();
        }

        copy_len
    }
}

pub static HDA: Spinlock<HdaController> = Spinlock::new(HdaController::new());

pub fn init() {
    // Scan PCI for Intel HDA Controller (8086:2668 or Audio Multimedia Class 04:03)
    let dev = find_device(HDA_VENDOR_INTEL, HDA_DEV_ICH6).or_else(|| {
        let mut match_dev = None;
        crate::drivers::pci::for_each_device(|pci_dev| {
            if pci_dev.class_id == 0x04 && pci_dev.subclass_id == 0x03 {
                match_dev = Some(*pci_dev);
            }
        });
        match_dev
    });

    if let Some(pci_dev) = dev {
        // Enable PCI Bus Mastering and Memory Space
        crate::drivers::pci::enable_bus_mastering(pci_dev.bus, pci_dev.slot, pci_dev.func);
        let bar0 = (pci_dev.bar0 & !0xF) as u64;
        let _ = crate::mm::vmm::map_mmio_range(bar0, 16384);
        let mut hda = HDA.lock();
        if hda.init_hw(bar0).is_ok() {
            crate::drivers::serial::print(
                "Audio: Intel High Definition Audio initialized (Codecs: ",
            );
            crate::drivers::serial::print_u64(hda.num_codecs as u64);
            crate::drivers::serial::println(", CORB/RIRB DMA Rings Online, 44.1kHz 16-bit Stereo)");
        }
    } else {
        crate::drivers::serial::println(
            "Audio: Intel HDA Controller not detected on PCI bus (fallback AC97 ready)",
        );
    }
}

pub fn set_volume(vol: u8) {
    HDA.lock().set_volume(vol);
}

pub fn get_volume() -> u8 {
    HDA.lock().volume
}

pub fn is_available() -> bool {
    HDA.lock().is_ready
}

pub fn is_streaming() -> bool {
    HDA.lock().is_streaming
}

pub fn start_stream() {
    HDA.lock().start_stream();
}

pub fn stop_stream() {
    HDA.lock().stop_stream();
}

pub fn submit_pcm(data: &[u8]) -> usize {
    HDA.lock().submit_pcm(data)
}

pub fn get_stream_position() -> u32 {
    let hda = HDA.lock();
    if !hda.is_ready || hda.stream_base == 0 {
        return 0;
    }
    let lpib_ptr = (hda.stream_base + HDA_SD_LPIB) as *const u32;
    unsafe { core::ptr::read_volatile(lpib_ptr) }
}
