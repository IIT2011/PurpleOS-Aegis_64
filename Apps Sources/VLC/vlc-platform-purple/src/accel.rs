// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Platform Hardware Acceleration Provider for PurpleOS.
//!
//! Directly invokes PurpleOS Category 1 GPU Hardware Acceleration syscalls:
//! - SYS_PURPLE_GPU_GET_CAPS: Probes hardware GPU driver (Intel igd, AMD radeongd, NVIDIA nvgd)
//! - SYS_PURPLE_GPU_BLIT: Direct coherent MMIO/DMA block transfers
//! - SYS_PURPLE_GPU_RECT_FILL: Hardware accelerated color fill
//! - SYS_PURPLE_GPU_CLEAR: Hardware accelerated surface clear
//! - SYS_PURPLE_GPU_SUBMIT_3D: Hardware 3D primitive / packet stream execution

use vlccore::hw_accel::{HardwareAccelerator, HwAccelType, HwCapabilities};
use crate::syscall::*;

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct PurpleGpuRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub color: u32,
    pub _reserved: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct PurpleGpuBlit {
    pub src_ptr: u64,
    pub dst_x: u32,
    pub dst_y: u32,
    pub width: u32,
    pub height: u32,
    pub src_pitch: u32,
    pub _reserved: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct PurpleGpuCaps {
    pub vendor_id: u16,
    pub device_id: u16,
    pub flags: u32,
    pub max_texture_dim: u32,
    pub vram_bytes: u64,
    pub name: [u8; 32],
}

#[derive(Clone, Debug)]
pub struct PurpleHardwareAccelerator {
    caps: HwCapabilities,
    active: bool,
}

impl Default for PurpleHardwareAccelerator {
    fn default() -> Self {
        Self::new()
    }
}

impl PurpleHardwareAccelerator {
    pub fn new() -> Self {
        Self {
            caps: HwCapabilities::default(),
            active: false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Probe the kernel GPU hardware acceleration subsystem
    pub fn probe() -> Self {
        let mut raw_caps = PurpleGpuCaps::default();
        let ret = unsafe {
            syscall1(
                SYS_PURPLE_GPU_GET_CAPS,
                &mut raw_caps as *mut PurpleGpuCaps as u64,
            )
        };

        let mut caps = HwCapabilities::default();
        let active;

        if ret == 0 {
            caps.vendor_id = raw_caps.vendor_id;
            caps.device_id = raw_caps.device_id;
            caps.has_hardware_blit = (raw_caps.flags & 0x01) != 0;
            caps.has_hardware_3d = (raw_caps.flags & 0x02) != 0;
            caps.has_page_flip = true;
            caps.has_hardware_scaling = true;
            caps.max_surface_width = if raw_caps.max_texture_dim > 0 { raw_caps.max_texture_dim } else { 2048 };
            caps.max_surface_height = if raw_caps.max_texture_dim > 0 { raw_caps.max_texture_dim } else { 2048 };

            caps.accel_type = match raw_caps.vendor_id {
                0x8086 => HwAccelType::IntelIgd,
                0x1002 => HwAccelType::AmdRadeon,
                0x10DE => HwAccelType::NvidiaNvgd,
                _ => {
                    if caps.has_hardware_blit {
                        HwAccelType::Purple2DBlit
                    } else {
                        HwAccelType::None
                    }
                }
            };

            caps.driver_name = match caps.accel_type {
                HwAccelType::IntelIgd => "Intel HD/Iris Graphics (igd 3D)",
                HwAccelType::AmdRadeon => "AMD Radeon Graphics (radeongd 3D)",
                HwAccelType::NvidiaNvgd => "NVIDIA GeForce (nvgd 3D)",
                HwAccelType::Purple2DBlit => "PurpleOS GPU Blitter (2D Fast LFB)",
                HwAccelType::None => "Software Framebuffer",
            };

            active = caps.accel_type.is_hardware();
        } else {
            active = false;
        }

        Self { caps, active }
    }
}

impl HardwareAccelerator for PurpleHardwareAccelerator {
    fn accel_type(&self) -> HwAccelType {
        self.caps.accel_type
    }

    fn capabilities(&self) -> HwCapabilities {
        self.caps
    }

    fn blit(
        &mut self,
        src: &[u32],
        src_pitch: u32,
        dst_x: u32,
        dst_y: u32,
        width: u32,
        height: u32,
    ) -> Result<usize, String> {
        let blit_req = PurpleGpuBlit {
            src_ptr: src.as_ptr() as u64,
            dst_x,
            dst_y,
            width,
            height,
            src_pitch,
            _reserved: 0,
        };

        let ret = unsafe {
            syscall1(
                SYS_PURPLE_GPU_BLIT,
                &blit_req as *const PurpleGpuBlit as u64,
            )
        };

        if ret < 0 {
            Err(format!("SYS_PURPLE_GPU_BLIT failed: {}", ret))
        } else {
            Ok(ret as usize)
        }
    }

    fn fill_rect(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        color: u32,
    ) -> Result<(), String> {
        let rect_req = PurpleGpuRect {
            x,
            y,
            width,
            height,
            color,
            _reserved: 0,
        };

        let ret = unsafe {
            syscall1(
                SYS_PURPLE_GPU_RECT_FILL,
                &rect_req as *const PurpleGpuRect as u64,
            )
        };

        if ret < 0 {
            Err(format!("SYS_PURPLE_GPU_RECT_FILL failed: {}", ret))
        } else {
            Ok(())
        }
    }

    fn clear(&mut self, color: u32) -> Result<(), String> {
        let ret = unsafe { syscall1(SYS_PURPLE_GPU_CLEAR, color as u64) };
        if ret < 0 {
            Err(format!("SYS_PURPLE_GPU_CLEAR failed: {}", ret))
        } else {
            Ok(())
        }
    }

    fn submit_3d_batch(&mut self, op: u32, arg1: u64, arg2: u64) -> Result<(), String> {
        let ret = unsafe { syscall3(SYS_PURPLE_GPU_SUBMIT_3D, op as u64, arg1, arg2) };
        if ret < 0 {
            Err(format!("SYS_PURPLE_GPU_SUBMIT_3D failed: {}", ret))
        } else {
            Ok(())
        }
    }
}
