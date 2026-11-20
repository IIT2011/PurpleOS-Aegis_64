#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct MultibootMmapEntry {
    pub size: u32,
    pub addr: u64,
    pub len: u64,
    pub entry_type: u32,
}

pub const MULTIBOOT_MEMORY_AVAILABLE: u32 = 1;
pub const MULTIBOOT_MEMORY_RESERVED: u32 = 2;
pub const MULTIBOOT_MEMORY_ACPI_RECLAIMABLE: u32 = 3;
pub const MULTIBOOT_MEMORY_NVS: u32 = 4;
pub const MULTIBOOT_MEMORY_BADRAM: u32 = 5;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MultibootInfo {
    pub flags: u32,
    pub mem_lower: u32,
    pub mem_upper: u32,
    pub boot_device: u32,
    pub cmdline: u32,
    pub mods_count: u32,
    pub mods_addr: u32,
    pub syms: [u32; 4],
    pub mmap_length: u32,
    pub mmap_addr: u32,
    pub drives_length: u32,
    pub drives_addr: u32,
    pub config_table: u32,
    pub boot_loader_name: u32,
    pub apm_table: u32,
    pub vbe_control_info: u32,
    pub vbe_mode_info: u32,
    pub vbe_mode: u16,
    pub vbe_interface_seg: u16,
    pub vbe_interface_off: u16,
    pub vbe_interface_len: u16,
    pub framebuffer_addr: u64,
    pub framebuffer_pitch: u32,
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
    pub framebuffer_bpp: u8,
    pub framebuffer_type: u8,
    pub color_info: [u8; 6],
}

pub const MULTIBOOT_INFO_MEMORY: u32 = 1 << 0;
pub const MULTIBOOT_INFO_BOOTDEV: u32 = 1 << 1;
pub const MULTIBOOT_INFO_CMDLINE: u32 = 1 << 2;
pub const MULTIBOOT_INFO_MODS: u32 = 1 << 3;
pub const MULTIBOOT_INFO_MEM_MAP: u32 = 1 << 6;
pub const MULTIBOOT_INFO_FRAMEBUFFER: u32 = 1 << 12;
pub const MULTIBOOT_BOOTLOADER_MAGIC: u32 = 0x2BADB002;
pub const AEGIS_UEFI_BOOT_MAGIC: u32 = 0x55454741;
pub const AEGIS_UEFI_SECURE_BOOT: u32 = 1 << 30;
pub const AEGIS_UEFI_ACPI_RSDP: u32 = 1 << 31;
