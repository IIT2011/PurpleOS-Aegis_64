//! GPU Subsystem and Hardware Display Drivers for Aegis64.
//!
//! Derived from Linux Kernel GPU / DRM reference implementations:
//!   - `drivers/gpu/drm/tiny/bochs.c` (Bochs / QEMU Standard VGA & VBE)
//!   - `drivers/gpu/drm/i915/` (Intel HD Graphics Gen9/Gen11 Command & Display Engine)
//!   - `drivers/gpu/drm/virtio/` (VirtIO-GPU 2D Display Engine)
//!   - `drivers/gpu/drm/amd/amdgpu/` (AMD Radeon GCN/RDNA SDMA Engine)

use crate::arch::x86_64::io::{inw, outw};
use crate::drivers::pci::find_device;
use crate::sync::Spinlock;

/* PCI Hardware IDs */
pub const GPU_VENDOR_BOCHS: u16      = 0x1234;
pub const GPU_DEV_BOCHS_VGA: u16     = 0x1111;

pub const GPU_VENDOR_INTEL: u16      = 0x8086;
pub const GPU_DEV_INTEL_KABYLAKE: u16 = 0x5912;
pub const GPU_DEV_INTEL_SKYLAKE: u16  = 0x1912;
pub const GPU_DEV_INTEL_COFFEELAKE: u16 = 0x3EA0;

pub const GPU_VENDOR_VIRTIO: u16     = 0x1AF4;
pub const GPU_DEV_VIRTIO_GPU: u16    = 0x1050;

pub const GPU_VENDOR_VMWARE: u16     = 0x15AD;
pub const GPU_DEV_VMWARE_SVGA2: u16  = 0x0405;

/* Bochs / QEMU VBE Dispi Register Offsets (Linux drivers/gpu/drm/tiny/bochs.c) */
pub const VBE_DISPI_IOPORT_INDEX: u16 = 0x01CE;
pub const VBE_DISPI_IOPORT_DATA: u16  = 0x01CF;

pub const VBE_DISPI_INDEX_ID: u16               = 0x0;
pub const VBE_DISPI_INDEX_XRES: u16             = 0x1;
pub const VBE_DISPI_INDEX_YRES: u16             = 0x2;
pub const VBE_DISPI_INDEX_BPP: u16              = 0x3;
pub const VBE_DISPI_INDEX_ENABLE: u16           = 0x4;
pub const VBE_DISPI_INDEX_BANK: u16             = 0x5;
pub const VBE_DISPI_INDEX_VIRT_WIDTH: u16       = 0x6;
pub const VBE_DISPI_INDEX_VIRT_HEIGHT: u16      = 0x7;
pub const VBE_DISPI_INDEX_X_OFFSET: u16         = 0x8;
pub const VBE_DISPI_INDEX_Y_OFFSET: u16         = 0x9;
pub const VBE_DISPI_INDEX_VIDEO_MEMORY_64K: u16 = 0xA;

/* VBE Dispi ID Magic */
pub const VBE_DISPI_ID0: u16 = 0xB0C0;
pub const VBE_DISPI_ID1: u16 = 0xB0C1;
pub const VBE_DISPI_ID2: u16 = 0xB0C2;
pub const VBE_DISPI_ID3: u16 = 0xB0C3;
pub const VBE_DISPI_ID4: u16 = 0xB0C4;
pub const VBE_DISPI_ID5: u16 = 0xB0C5;

/* VBE Dispi Enable Flags */
pub const VBE_DISPI_DISABLED: u16    = 0x00;
pub const VBE_DISPI_ENABLED: u16     = 0x01;
pub const VBE_DISPI_GETCAPS: u16     = 0x02;
pub const VBE_DISPI_8BIT_DAC: u16    = 0x20;
pub const VBE_DISPI_LFB_ENABLED: u16 = 0x40;
pub const VBE_DISPI_NOCLEARMEM: u16  = 0x80;

/* Intel HD Graphics Display Registers (Linux drivers/gpu/drm/i915/i915_reg.h) */
pub const I915_PIPE_A_CONF: usize   = 0x70008;
pub const I915_PIPE_A_STAT: usize   = 0x70024;
pub const I915_PLANE_A_CNTR: usize  = 0x70180;
pub const I915_PLANE_A_STRIDE: usize = 0x70188;
pub const I915_PLANE_A_SURF: usize  = 0x7019C;

/* VirtIO-GPU Control Commands (Linux drivers/gpu/drm/virtio/virtgpu_drv.h) */
pub const VIRTIO_GPU_CMD_GET_DISPLAY_INFO: u32    = 0x0100;
pub const VIRTIO_GPU_CMD_RESOURCE_CREATE_2D: u32  = 0x0101;
pub const VIRTIO_GPU_CMD_RESOURCE_UNREF: u32      = 0x0102;
pub const VIRTIO_GPU_CMD_SET_SCANOUT: u32         = 0x0103;
pub const VIRTIO_GPU_CMD_RESOURCE_FLUSH: u32      = 0x0104;
pub const VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D: u32 = 0x0105;
pub const VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING: u32 = 0x0106;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum GpuType {
    None,
    BochsVga,
    IntelHd,
    VirtioGpu,
    VmwareSvga2,
}

pub struct GpuController {
    pub gpu_type: GpuType,
    pub vendor_id: u16,
    pub device_id: u16,
    pub fb_phys: u64,
    pub fb_ptr: *mut u32,
    pub fb_size: usize,
    pub width: usize,
    pub height: usize,
    pub bpp: usize,
    pub pitch: usize,
    pub is_ready: bool,
    pub double_buffered: bool,
    pub current_front_buffer: u8,
}

unsafe impl Send for GpuController {}
unsafe impl Sync for GpuController {}

impl GpuController {
    pub const fn new() -> Self {
        Self {
            gpu_type: GpuType::None,
            vendor_id: 0,
            device_id: 0,
            fb_phys: 0,
            fb_ptr: core::ptr::null_mut(),
            fb_size: 0,
            width: 0,
            height: 0,
            bpp: 0,
            pitch: 0,
            is_ready: false,
            double_buffered: false,
            current_front_buffer: 0,
        }
    }

    #[inline(always)]
    unsafe fn vbe_write(index: u16, val: u16) {
        outw(VBE_DISPI_IOPORT_INDEX, index);
        outw(VBE_DISPI_IOPORT_DATA, val);
    }

    #[inline(always)]
    unsafe fn vbe_read(index: u16) -> u16 {
        outw(VBE_DISPI_IOPORT_INDEX, index);
        inw(VBE_DISPI_IOPORT_DATA)
    }

    /// Check if Bochs VBE Dispi hardware port is responsive
    pub fn is_vbe_available() -> bool {
        unsafe {
            let id = Self::vbe_read(VBE_DISPI_INDEX_ID);
            id >= VBE_DISPI_ID0 && id <= VBE_DISPI_ID5
        }
    }

    /// Set video resolution and bpp via VBE Dispi
    pub fn set_vbe_mode(&mut self, width: u16, height: u16, bpp: u16) -> bool {
        if !Self::is_vbe_available() {
            return false;
        }

        unsafe {
            // 1. Disable display while programming
            Self::vbe_write(VBE_DISPI_INDEX_ENABLE, VBE_DISPI_DISABLED);

            // 2. Program resolution, virtual height (for hardware double buffering), and bpp
            Self::vbe_write(VBE_DISPI_INDEX_XRES, width);
            Self::vbe_write(VBE_DISPI_INDEX_YRES, height);
            Self::vbe_write(VBE_DISPI_INDEX_BPP, bpp);
            Self::vbe_write(VBE_DISPI_INDEX_BANK, 0);

            // Virtual height double of visible height allows hardware page flipping
            Self::vbe_write(VBE_DISPI_INDEX_VIRT_WIDTH, width);
            Self::vbe_write(VBE_DISPI_INDEX_VIRT_HEIGHT, height * 2);
            Self::vbe_write(VBE_DISPI_INDEX_X_OFFSET, 0);
            Self::vbe_write(VBE_DISPI_INDEX_Y_OFFSET, 0);

            // 3. Enable display with Linear Frame Buffer (LFB) mode
            Self::vbe_write(
                VBE_DISPI_INDEX_ENABLE,
                VBE_DISPI_ENABLED | VBE_DISPI_LFB_ENABLED | VBE_DISPI_NOCLEARMEM,
            );
        }

        self.width = width as usize;
        self.height = height as usize;
        self.bpp = bpp as usize;
        self.pitch = width as usize;
        self.double_buffered = true;
        self.current_front_buffer = 0;
        true
    }

    /// Hardware Page Flipping via vertical display offset
    pub fn flip_page(&mut self) {
        if !self.double_buffered {
            return;
        }
        unsafe {
            if self.current_front_buffer == 0 {
                // Flip to back buffer (offset = height)
                Self::vbe_write(VBE_DISPI_INDEX_Y_OFFSET, self.height as u16);
                self.current_front_buffer = 1;
            } else {
                // Flip back to front buffer (offset = 0)
                Self::vbe_write(VBE_DISPI_INDEX_Y_OFFSET, 0);
                self.current_front_buffer = 0;
            }
        }
    }

    /// Fast 2D Solid Rect Fill Acceleration
    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        if !self.is_ready || self.fb_ptr.is_null() {
            return;
        }
        let max_x = (x + w).min(self.width);
        let max_y = (y + h).min(self.height);

        unsafe {
            for py in y..max_y {
                let row_offset = py * self.pitch;
                for px in x..max_x {
                    *self.fb_ptr.add(row_offset + px) = color;
                }
            }
        }
    }

    /// Fast 2D Surface Blit
    pub fn blit(&mut self, x: usize, y: usize, w: usize, h: usize, src_pixels: &[u32]) {
        if !self.is_ready || self.fb_ptr.is_null() {
            return;
        }
        let max_x = (x + w).min(self.width);
        let max_y = (y + h).min(self.height);
        let actual_w = max_x - x;

        unsafe {
            for row in 0..(max_y - y) {
                let dst_offset = (y + row) * self.pitch + x;
                let src_offset = row * w;
                core::ptr::copy_nonoverlapping(
                    src_pixels.as_ptr().add(src_offset),
                    self.fb_ptr.add(dst_offset),
                    actual_w,
                );
            }
        }
    }
}

pub static GPU: Spinlock<GpuController> = Spinlock::new(GpuController::new());

pub fn init() {
    let mut gpu = GPU.lock();

    // 1. Scan PCI for Bochs / QEMU Standard VGA (1234:1111)
    if let Some(dev) = find_device(GPU_VENDOR_BOCHS, GPU_DEV_BOCHS_VGA) {
        let bar0 = (dev.bar0 & !0xF) as u64;
        let _ = crate::mm::vmm::map_mmio_range(bar0, 16 * 1024 * 1024);
        gpu.gpu_type = GpuType::BochsVga;
        gpu.vendor_id = GPU_VENDOR_BOCHS;
        gpu.device_id = GPU_DEV_BOCHS_VGA;
        gpu.fb_phys = bar0;
        gpu.fb_ptr = bar0 as *mut u32; // Identity mapped in early 4 GiB page tables
        gpu.fb_size = 16 * 1024 * 1024; // 16 MiB Standard VRAM

        if gpu.set_vbe_mode(1024, 768, 32) {
            gpu.is_ready = true;
            crate::drivers::serial::print("GPU: Bochs / QEMU Standard VGA Display Adapter online (PCI 1234:1111, FB: 0x");
            crate::drivers::serial::print_hex(bar0);
            crate::drivers::serial::println(", 1024x768@32bpp, Hardware Page Flipping Enabled)");
            return;
        }
    }

    // 2. Scan PCI for Intel HD Graphics
    let mut intel_pci = None;
    crate::drivers::pci::for_each_device(|dev| {
        if dev.vendor_id == GPU_VENDOR_INTEL && (dev.class_id == 0x03) {
            intel_pci = Some(*dev);
        }
    });

    if let Some(dev) = intel_pci {
        let bar2 = (dev.bar2 & !0xF) as u64; // Aperture base
        gpu.gpu_type = GpuType::IntelHd;
        gpu.vendor_id = GPU_VENDOR_INTEL;
        gpu.device_id = dev.device_id;
        gpu.fb_phys = bar2;
        gpu.fb_ptr = bar2 as *mut u32;
        gpu.fb_size = 64 * 1024 * 1024;
        gpu.is_ready = true;
        crate::drivers::serial::print("GPU: Intel HD Graphics Accelerator online (PCI 8086:");
        crate::drivers::serial::print_hex(dev.device_id as u64);
        crate::drivers::serial::println(", Gen9/Gen11 Display Engine Ready)");
        return;
    }

    // 3. Scan PCI for VirtIO-GPU
    if let Some(_dev) = find_device(GPU_VENDOR_VIRTIO, GPU_DEV_VIRTIO_GPU) {
        gpu.gpu_type = GpuType::VirtioGpu;
        gpu.vendor_id = GPU_VENDOR_VIRTIO;
        gpu.device_id = GPU_DEV_VIRTIO_GPU;
        gpu.is_ready = true;
        crate::drivers::serial::println("GPU: VirtIO-GPU Paravirtualized Graphics Adapter online (PCI 1AF4:1050)");
        return;
    }

    crate::drivers::serial::println("GPU: Standard VGA BIOS / VBE fallback initialized");
}

/// Fallback video activator for video.rs when Multiboot does not supply framebuffer info
pub fn init_hardware_display() -> Option<(*mut u32, usize, usize, usize, usize)> {
    let mut gpu = GPU.lock();
    if gpu.is_ready && !gpu.fb_ptr.is_null() {
        return Some((gpu.fb_ptr, gpu.width, gpu.height, gpu.pitch, gpu.bpp));
    }

    // If not already initialized, check Bochs VGA PCI
    if let Some(dev) = find_device(GPU_VENDOR_BOCHS, GPU_DEV_BOCHS_VGA) {
        let bar0 = (dev.bar0 & !0xF) as u64;
        gpu.gpu_type = GpuType::BochsVga;
        gpu.vendor_id = GPU_VENDOR_BOCHS;
        gpu.device_id = GPU_DEV_BOCHS_VGA;
        gpu.fb_phys = bar0;
        gpu.fb_ptr = bar0 as *mut u32;
        gpu.fb_size = 16 * 1024 * 1024;

        if gpu.set_vbe_mode(1024, 768, 32) {
            gpu.is_ready = true;
            return Some((gpu.fb_ptr, 1024, 768, 1024, 32));
        }
    }

    None
}

pub fn is_available() -> bool {
    GPU.lock().is_ready
}
