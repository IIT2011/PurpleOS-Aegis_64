// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Hardware Video and Graphics Acceleration Subsystem.
//!
//! Provides abstract interfaces mirroring VLC's official vlc_va (video acceleration)
//! and vlc_decoder_device architectures in Pure Rust.

use crate::fourcc::FourCC;

/// Type of hardware graphics / acceleration engine detected
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HwAccelType {
    None,
    IntelIgd,     // Intel Gen9/Gen11 Render Command Streamer (RCS)
    AmdRadeon,    // AMD GCN/RDNA PM4 Command Processor
    NvidiaNvgd,   // NVIDIA Nouveau FIFO Pushbuffer (Daily 3D Accel)
    Purple2DBlit, // PurpleOS Fast Direct Hardware Framebuffer / Blitter
}

impl HwAccelType {
    pub fn name(&self) -> &'static str {
        match self {
            HwAccelType::None => "Software Raster",
            HwAccelType::IntelIgd => "Intel HD/Iris Graphics (igd)",
            HwAccelType::AmdRadeon => "AMD Radeon Graphics (radeongd)",
            HwAccelType::NvidiaNvgd => "NVIDIA GeForce (nvgd)",
            HwAccelType::Purple2DBlit => "PurpleOS 2D GPU Blitter",
        }
    }

    pub fn is_hardware(&self) -> bool {
        *self != HwAccelType::None
    }
}

/// Hardware acceleration capabilities report
#[derive(Copy, Clone, Debug)]
pub struct HwCapabilities {
    pub accel_type: HwAccelType,
    pub vendor_id: u16,
    pub device_id: u16,
    pub has_hardware_blit: bool,
    pub has_hardware_scaling: bool,
    pub has_hardware_3d: bool,
    pub has_page_flip: bool,
    pub max_surface_width: u32,
    pub max_surface_height: u32,
    pub driver_name: &'static str,
}

impl Default for HwCapabilities {
    fn default() -> Self {
        Self {
            accel_type: HwAccelType::None,
            vendor_id: 0,
            device_id: 0,
            has_hardware_blit: false,
            has_hardware_scaling: false,
            has_hardware_3d: false,
            has_page_flip: false,
            max_surface_width: 1024,
            max_surface_height: 768,
            driver_name: "Software Fallback",
        }
    }
}

/// Represents a hardware-backed surface or frame buffer
#[derive(Clone, Debug)]
pub struct HwSurface {
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub fourcc: FourCC,
    pub data: Vec<u32>,
    pub hw_handle: u64,
}

impl HwSurface {
    pub fn new_rgba(width: u32, height: u32) -> Self {
        let size = (width * height) as usize;
        Self {
            width,
            height,
            pitch: width * 4,
            fourcc: crate::fourcc::VLC_CODEC_RGBA,
            data: vec![0u32; size],
            hw_handle: 0,
        }
    }

    pub fn as_slice(&self) -> &[u32] {
        &self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [u32] {
        &mut self.data
    }
}

/// Abstract hardware video accelerator device interface
pub trait HardwareAccelerator: Send + Sync {
    /// Return the acceleration type
    fn accel_type(&self) -> HwAccelType;

    /// Return detailed capabilities
    fn capabilities(&self) -> HwCapabilities;

    /// Check if hardware acceleration is active
    fn is_active(&self) -> bool {
        self.accel_type().is_hardware()
    }

    /// Perform a hardware-accelerated block image transfer (blit)
    fn blit(
        &mut self,
        src: &[u32],
        src_pitch: u32,
        dst_x: u32,
        dst_y: u32,
        width: u32,
        height: u32,
    ) -> Result<usize, String>;

    /// Perform a hardware-accelerated rectangle fill
    fn fill_rect(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        color: u32,
    ) -> Result<(), String>;

    /// Perform a hardware-accelerated screen / buffer clear
    fn clear(&mut self, color: u32) -> Result<(), String>;

    /// Submit a hardware 3D command batch
    fn submit_3d_batch(&mut self, op: u32, arg1: u64, arg2: u64) -> Result<(), String>;
}

/// Software fallback dummy accelerator
pub struct DummyHardwareAccelerator {
    caps: HwCapabilities,
}

impl Default for DummyHardwareAccelerator {
    fn default() -> Self {
        Self::new()
    }
}

impl DummyHardwareAccelerator {
    pub fn new() -> Self {
        Self {
            caps: HwCapabilities::default(),
        }
    }
}

impl HardwareAccelerator for DummyHardwareAccelerator {
    fn accel_type(&self) -> HwAccelType {
        HwAccelType::None
    }

    fn capabilities(&self) -> HwCapabilities {
        self.caps
    }

    fn blit(
        &mut self,
        _src: &[u32],
        _src_pitch: u32,
        _dst_x: u32,
        _dst_y: u32,
        width: u32,
        height: u32,
    ) -> Result<usize, String> {
        Ok((width * height) as usize)
    }

    fn fill_rect(
        &mut self,
        _x: u32,
        _y: u32,
        _width: u32,
        _height: u32,
        _color: u32,
    ) -> Result<(), String> {
        Ok(())
    }

    fn clear(&mut self, _color: u32) -> Result<(), String> {
        Ok(())
    }

    fn submit_3d_batch(&mut self, _op: u32, _arg1: u64, _arg2: u64) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hw_accel_types() {
        assert!(!HwAccelType::None.is_hardware());
        assert!(HwAccelType::IntelIgd.is_hardware());
        assert!(HwAccelType::AmdRadeon.is_hardware());
        assert!(HwAccelType::NvidiaNvgd.is_hardware());
        assert!(HwAccelType::Purple2DBlit.is_hardware());
    }

    #[test]
    fn test_hw_surface_allocation() {
        let surf = HwSurface::new_rgba(640, 480);
        assert_eq!(surf.width, 640);
        assert_eq!(surf.height, 480);
        assert_eq!(surf.data.len(), 640 * 480);
    }
}
