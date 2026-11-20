use crate::mm::pmm::{alloc_frames, PAGE_SIZE};
use crate::sync::Spinlock;

pub const APDRV_MAGIC: &[u8; 8] = b"APDRV100";
pub const APDRV_VERSION: u32 = 3;
pub const APDRV_TYPE_AEGISMOD: &[u8; 8] = b"AEGISMOD";

pub const APDRV_MAX_NAME: usize = 32;
pub const APDRV_MAX_DESC: usize = 64;
pub const APDRV_MAX_AUTHOR: usize = 32;
pub const APDRV_MAX_MODULES: usize = 32;

fn image_region_is_valid(offset: u32, size: usize, payload_len: usize) -> bool {
    if size == 0 {
        return offset == 0 || (offset as usize) <= payload_len;
    }
    let start = offset as usize;
    start >= core::mem::size_of::<ApdrvHeader>()
        && start
            .checked_add(size)
            .map_or(false, |end| end <= payload_len)
}

/* Relocation Types */
pub const APDRV_RELOC_NONE: u32 = 0;
pub const APDRV_RELOC_64: u32 = 1; // *(uint64_t*)target = sym + addend
pub const APDRV_RELOC_PC32: u32 = 2; // *(int32_t*)target = sym + addend - target
pub const APDRV_RELOC_PLT32: u32 = 4; // *(int32_t*)target = sym + addend - target
pub const APDRV_RELOC_32: u32 = 10; // *(uint32_t*)target = (uint32_t)(sym + addend)

/* Relocation Section Base Indices */
pub const APDRV_SEC_TEXT: u32 = 0xFFF0;
pub const APDRV_SEC_RODATA: u32 = 0xFFF1;
pub const APDRV_SEC_DATA: u32 = 0xFFF2;
pub const APDRV_SEC_BSS: u32 = 0xFFF3;
pub const APDRV_SEC_MODULE: u32 = 0xFFF4;

/* Flags */
pub const APDRV_FLAG_ENFORCE_WX: u32 = 0x0001;
pub const APDRV_FLAG_AUTO_PROBE: u32 = 0x0002;
pub const APDRV_ENTRY_ABSENT: u32 = u32::MAX;
pub const APDRV_CHECKSUM_OFFSET: usize = 232;

fn apdrv_crc32(image: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for (index, &byte) in image.iter().enumerate() {
        let value = if (APDRV_CHECKSUM_OFFSET..APDRV_CHECKSUM_OFFSET + 4).contains(&index) {
            0
        } else {
            byte
        };
        crc ^= value as u32;
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct ApdrvHeader {
    pub magic: [u8; 8],
    pub version: u32,
    pub header_size: u32,
    pub mod_type: [u8; 8],
    pub name: [u8; APDRV_MAX_NAME],
    pub description: [u8; APDRV_MAX_DESC],
    pub author: [u8; APDRV_MAX_AUTHOR],
    pub mod_version: u32,

    pub text_offset: u32,
    pub text_size: u32,
    pub rodata_offset: u32,
    pub rodata_size: u32,
    pub data_offset: u32,
    pub data_size: u32,
    pub bss_size: u32,

    pub reloc_offset: u32,
    pub reloc_count: u32,

    pub import_offset: u32,
    pub import_count: u32,
    pub export_offset: u32,
    pub export_count: u32,

    pub entry_init: u32,
    pub entry_fini: u32,
    pub entry_probe: u32,
    pub entry_ioctl: u32,

    pub target_vendor: u16,
    pub target_device: u16,
    pub flags: u32,
    pub checksum: u32,
    pub reserved: u32,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct ApdrvReloc {
    pub offset: u32,
    pub reloc_type: u32,
    pub sym_index: u32,
    pub addend: i64,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct ApdrvSymbol {
    pub name: [u8; APDRV_MAX_NAME],
    pub address: u64,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ModuleState {
    Unloaded,
    Loaded,
    Running,
    Error,
}

pub struct LoadedModule {
    pub name: [u8; APDRV_MAX_NAME],
    pub state: ModuleState,
    pub base_addr: usize,
    pub total_pages: usize,
    pub text_addr: usize,
    pub rodata_addr: usize,
    pub data_addr: usize,
    pub bss_addr: usize,
    pub fn_init: Option<extern "C" fn() -> i32>,
    pub fn_fini: Option<extern "C" fn() -> i32>,
    pub fn_probe: Option<extern "C" fn(u32, u32) -> i32>,
    pub fn_ioctl: Option<extern "C" fn(u32, *mut u8) -> i32>,
    pub target_vendor: u16,
    pub target_device: u16,
    pub bound_devices: u32,
    pub wx_enforced: bool,
}

impl LoadedModule {
    pub const fn empty() -> Self {
        Self {
            name: [0; APDRV_MAX_NAME],
            state: ModuleState::Unloaded,
            base_addr: 0,
            total_pages: 0,
            text_addr: 0,
            rodata_addr: 0,
            data_addr: 0,
            bss_addr: 0,
            fn_init: None,
            fn_fini: None,
            fn_probe: None,
            fn_ioctl: None,
            target_vendor: 0,
            target_device: 0,
            bound_devices: 0,
            wx_enforced: false,
        }
    }
}

static MODULE_REGISTRY: Spinlock<[LoadedModule; APDRV_MAX_MODULES]> = Spinlock::new([
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
    LoadedModule::empty(),
]);

#[cfg(not(feature = "signed-components"))]
static CPU_APDRV: &[u8] = include_bytes!("../../build/drivers_apdrv/cpu.apdrv");
#[cfg(not(feature = "signed-components"))]
static RAM_APDRV: &[u8] = include_bytes!("../../build/drivers_apdrv/ram.apdrv");
#[cfg(not(feature = "signed-components"))]
static GPU_APDRV: &[u8] = include_bytes!("../../build/drivers_apdrv/gpu.apdrv");
#[cfg(not(feature = "signed-components"))]
static STORAGE_APDRV: &[u8] = include_bytes!("../../build/drivers_apdrv/storage.apdrv");
#[cfg(not(feature = "signed-components"))]
static NETWORK_APDRV: &[u8] = include_bytes!("../../build/drivers_apdrv/network.apdrv");
#[cfg(not(feature = "signed-components"))]
static AUDIO_APDRV: &[u8] = include_bytes!("../../build/drivers_apdrv/audio.apdrv");
#[cfg(not(feature = "signed-components"))]
static USB_APDRV: &[u8] = include_bytes!("../../build/drivers_apdrv/usb.apdrv");
#[cfg(not(feature = "signed-components"))]
static INPUT_APDRV: &[u8] = include_bytes!("../../build/drivers_apdrv/input.apdrv");
#[cfg(not(feature = "signed-components"))]
static FILESYSTEMS_APDRV: &[u8] = include_bytes!("../../build/drivers_apdrv/filesystems.apdrv");

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum DriverCondition {
    Core,
    Category(crate::drivers::purple_detect::DeviceCategory),
    PciDevice(u16, u16),
}

pub struct ApdrvBundleDriver {
    pub name: &'static str,
    pub filename: &'static str,
    #[cfg(not(feature = "signed-components"))]
    pub binary: &'static [u8],
    pub condition: DriverCondition,
    pub description: &'static str,
}

pub static APDRV_BUNDLE: &[ApdrvBundleDriver] = &[
    ApdrvBundleDriver {
        name: "cpu",
        filename: "cpu.apdrv",
        #[cfg(not(feature = "signed-components"))]
        binary: CPU_APDRV,
        condition: DriverCondition::Core,
        description: "Unified CPU, SMP Multi-Core & APIC Subsystem",
    },
    ApdrvBundleDriver {
        name: "ram",
        filename: "ram.apdrv",
        #[cfg(not(feature = "signed-components"))]
        binary: RAM_APDRV,
        condition: DriverCondition::Core,
        description: "Unified Memory Management (PMM, VMM, Dynamic Heap)",
    },
    ApdrvBundleDriver {
        name: "filesystems",
        filename: "filesystems.apdrv",
        #[cfg(not(feature = "signed-components"))]
        binary: FILESYSTEMS_APDRV,
        condition: DriverCondition::Core,
        description: "Unified Filesystem Subsystem (PurpleFS, FAT, Ext4, ISO9660, RAMFS)",
    },
    ApdrvBundleDriver {
        name: "storage",
        filename: "storage.apdrv",
        #[cfg(not(feature = "signed-components"))]
        binary: STORAGE_APDRV,
        condition: DriverCondition::Category(
            crate::drivers::purple_detect::DeviceCategory::Storage,
        ),
        description: "Unified Block Storage Driver (AHCI SATA, NVMe, ATA, SDXC)",
    },
    ApdrvBundleDriver {
        name: "input",
        filename: "input.apdrv",
        #[cfg(not(feature = "signed-components"))]
        binary: INPUT_APDRV,
        condition: DriverCondition::Category(crate::drivers::purple_detect::DeviceCategory::Input),
        description: "Unified Human Interface Device Driver (PS/2 & USB HID)",
    },
    ApdrvBundleDriver {
        name: "gpu",
        filename: "gpu.apdrv",
        #[cfg(not(feature = "signed-components"))]
        binary: GPU_APDRV,
        condition: DriverCondition::Category(
            crate::drivers::purple_detect::DeviceCategory::Display,
        ),
        description: "Unified GPU Driver Stack (Bochs VBE 2D, igd, radeongd, nvgd 3D)",
    },
    ApdrvBundleDriver {
        name: "network",
        filename: "network.apdrv",
        #[cfg(not(feature = "signed-components"))]
        binary: NETWORK_APDRV,
        condition: DriverCondition::Category(
            crate::drivers::purple_detect::DeviceCategory::Network,
        ),
        description: "Unified Network Driver (Intel E1000, RTL8139, TCP/IP L2)",
    },
    ApdrvBundleDriver {
        name: "audio",
        filename: "audio.apdrv",
        #[cfg(not(feature = "signed-components"))]
        binary: AUDIO_APDRV,
        condition: DriverCondition::Category(crate::drivers::purple_detect::DeviceCategory::Audio),
        description: "Unified HD Audio Driver and PCM Streaming Pipeline",
    },
    ApdrvBundleDriver {
        name: "usb",
        filename: "usb.apdrv",
        #[cfg(not(feature = "signed-components"))]
        binary: USB_APDRV,
        condition: DriverCondition::Category(crate::drivers::purple_detect::DeviceCategory::Bus),
        description: "Unified USB 3.0 / 2.0 Host Controller Subsystem",
    },
];

impl ApdrvBundleDriver {
    fn load(&self) -> Result<usize, &'static str> {
        #[cfg(not(feature = "signed-components"))]
        return load_binary(self.binary);

        #[cfg(feature = "signed-components")]
        {
            const MAX_BOOT_MODULE_SIZE: usize = 8192;
            let path = alloc::format!("/System/Drivers/{}", self.filename);
            let size = crate::fs::vfs::file_size(&path)?;
            if size == 0 || size > MAX_BOOT_MODULE_SIZE {
                return Err("Invalid boot module size");
            }
            let mut image = [0u8; MAX_BOOT_MODULE_SIZE];
            if crate::fs::vfs::read(&path, &mut image)? != size {
                return Err("Could not read the complete boot module");
            }
            load_binary(&image[..size])
        }
    }
}

pub fn load_matching_drivers() -> (usize, usize) {
    let mut loaded = 0usize;
    let mut skipped = 0usize;

    crate::drivers::serial::println(
        "PurpleDetect: Scanning hardware inventory and matching APDRV driver bundle...",
    );

    for drv in APDRV_BUNDLE.iter() {
        let is_matched = match drv.condition {
            DriverCondition::Core => true,
            DriverCondition::Category(cat) => crate::drivers::purple_detect::has_category(cat),
            DriverCondition::PciDevice(v, d) => crate::drivers::purple_detect::has_pci_device(v, d),
        };

        if is_matched {
            crate::drivers::serial::print("PurpleDetect: Matched hardware for '");
            crate::drivers::serial::print(drv.filename);
            crate::drivers::serial::println("' — loading driver module");
            match drv.load() {
                Ok(_) => loaded += 1,
                Err(reason) => {
                    skipped += 1;
                    crate::drivers::serial::print("PurpleDetect: Driver '");
                    crate::drivers::serial::print(drv.filename);
                    crate::drivers::serial::print("' rejected: ");
                    crate::drivers::serial::println(reason);
                }
            }
        } else {
            skipped += 1;
            crate::drivers::serial::print("PurpleDetect: No matching hardware detected for '");
            crate::drivers::serial::print(drv.filename);
            crate::drivers::serial::print("' (");
            crate::drivers::serial::print(drv.description);
            crate::drivers::serial::println(") — skipped.");
        }
    }

    // Hardware Auto-Binding: probe discovered PCI devices against loaded modules
    crate::drivers::pci::for_each_device(|dev| {
        probe_pci_device(dev.vendor_id, dev.device_id);
    });

    crate::drivers::serial::print("PurpleDetect: Hardware-driven driver load complete: ");
    crate::drivers::serial::print_u64(loaded as u64);
    crate::drivers::serial::print(" loaded, ");
    crate::drivers::serial::print_u64(skipped as u64);
    crate::drivers::serial::println(" skipped.");

    (loaded, skipped)
}

pub fn init() {
    crate::drivers::serial::print("APDRV: Subsystem v3 initialized (Max Modules: ");
    crate::drivers::serial::print_u64(APDRV_MAX_MODULES as u64);
    crate::drivers::serial::println(", W^X Enforced)");
    load_matching_drivers();
}

pub fn load_binary(image: &[u8]) -> Result<usize, &'static str> {
    if image.len() < core::mem::size_of::<ApdrvHeader>() {
        return Err("Image smaller than APDRV header");
    }

    let hdr = unsafe { core::ptr::read_unaligned(image.as_ptr() as *const ApdrvHeader) };

    if &hdr.magic != APDRV_MAGIC {
        return Err("Invalid APDRV magic");
    }
    if hdr.version != APDRV_VERSION {
        return Err("Unsupported APDRV version");
    }
    if &hdr.mod_type != APDRV_TYPE_AEGISMOD {
        return Err("Invalid module type");
    }

    if hdr.header_size as usize != core::mem::size_of::<ApdrvHeader>() {
        return Err("Invalid APDRV header size");
    }

    let mod_name = core::str::from_utf8(&hdr.name)
        .unwrap_or("module")
        .trim_matches('\0');
    if !crate::security::aegis_detect::verify_driver(mod_name, image) {
        return Err("APDRV signature verification failed");
    }
    let payload_len = image.len() - crate::security::aegis_detect::APDRV_SIGNATURE_HEX_SIZE;
    if apdrv_crc32(&image[..payload_len]) != hdr.checksum {
        return Err("APDRV CRC-32 checksum mismatch");
    }

    if hdr.text_size == 0
        || !image_region_is_valid(hdr.text_offset, hdr.text_size as usize, payload_len)
        || !image_region_is_valid(hdr.rodata_offset, hdr.rodata_size as usize, payload_len)
        || !image_region_is_valid(hdr.data_offset, hdr.data_size as usize, payload_len)
    {
        return Err("APDRV section lies outside signed payload");
    }

    let reloc_size = (hdr.reloc_count as usize)
        .checked_mul(core::mem::size_of::<ApdrvReloc>())
        .ok_or("Relocation table size calculation overflow")?;
    let import_size = (hdr.import_count as usize)
        .checked_mul(core::mem::size_of::<ApdrvSymbol>())
        .ok_or("Import table size calculation overflow")?;
    let export_size = (hdr.export_count as usize)
        .checked_mul(core::mem::size_of::<ApdrvSymbol>())
        .ok_or("Export table size calculation overflow")?;
    if !image_region_is_valid(hdr.reloc_offset, reloc_size, payload_len)
        || !image_region_is_valid(hdr.import_offset, import_size, payload_len)
        || !image_region_is_valid(hdr.export_offset, export_size, payload_len)
    {
        return Err("APDRV metadata table lies outside signed payload");
    }
    let regions = [
        (hdr.text_offset as usize, hdr.text_size as usize),
        (hdr.rodata_offset as usize, hdr.rodata_size as usize),
        (hdr.data_offset as usize, hdr.data_size as usize),
        (hdr.reloc_offset as usize, reloc_size),
        (hdr.import_offset as usize, import_size),
        (hdr.export_offset as usize, export_size),
    ];
    for i in 0..regions.len() {
        let (start, size) = regions[i];
        if size == 0 {
            continue;
        }
        let end = start + size;
        for &(other_start, other_size) in &regions[i + 1..] {
            if other_size > 0 && start < other_start + other_size && other_start < end {
                return Err("APDRV sections or metadata tables overlap");
            }
        }
    }
    for entry in [
        hdr.entry_init,
        hdr.entry_fini,
        hdr.entry_probe,
        hdr.entry_ioctl,
    ] {
        if entry != APDRV_ENTRY_ABSENT && entry >= hdr.text_size {
            return Err("APDRV entry point lies outside text section");
        }
    }

    if (hdr.flags & APDRV_FLAG_ENFORCE_WX) == 0 {
        return Err("APDRV package does not require W^X protection");
    }
    let pml4 = crate::mm::vmm::get_kernel_pml4();
    if pml4.is_null() {
        return Err("Kernel page tables are unavailable for W^X mapping");
    }

    // Check if module is already loaded (idempotent reload protection)
    {
        let mods = MODULE_REGISTRY.lock();
        for (idx, slot) in mods.iter().enumerate() {
            if slot.state != ModuleState::Unloaded {
                let slot_name_len = slot
                    .name
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(slot.name.len());
                let slot_name = core::str::from_utf8(&slot.name[..slot_name_len]).unwrap_or("");
                if slot_name == mod_name {
                    return Ok(idx);
                }
            }
        }
    }

    // Calculate required memory pages (align each section to 4 KiB for W^X memory protection)
    let text_pages = (hdr.text_size as usize + PAGE_SIZE - 1) / PAGE_SIZE;
    let rodata_pages = (hdr.rodata_size as usize + PAGE_SIZE - 1) / PAGE_SIZE;
    let data_pages = (hdr.data_size as usize + PAGE_SIZE - 1) / PAGE_SIZE;
    let bss_pages = (hdr.bss_size as usize + PAGE_SIZE - 1) / PAGE_SIZE;

    let total_pages = text_pages + rodata_pages + data_pages + bss_pages;
    if total_pages == 0 {
        return Err("Empty module sections");
    }

    let base_phys =
        alloc_frames(total_pages).ok_or("Failed to allocate physical frames for module")?;
    let base_addr = base_phys;

    let text_addr = base_addr;
    let rodata_addr = text_addr + text_pages * PAGE_SIZE;
    let data_addr = rodata_addr + rodata_pages * PAGE_SIZE;
    let bss_addr = data_addr + data_pages * PAGE_SIZE;

    unsafe {
        // Copy .text
        if hdr.text_size > 0 && (hdr.text_offset as usize + hdr.text_size as usize) <= image.len() {
            let src = image.as_ptr().add(hdr.text_offset as usize);
            core::ptr::copy_nonoverlapping(src, text_addr as *mut u8, hdr.text_size as usize);
            if text_pages * PAGE_SIZE > hdr.text_size as usize {
                core::ptr::write_bytes(
                    (text_addr + hdr.text_size as usize) as *mut u8,
                    0,
                    text_pages * PAGE_SIZE - hdr.text_size as usize,
                );
            }
        }

        // Copy .rodata
        if hdr.rodata_size > 0
            && (hdr.rodata_offset as usize + hdr.rodata_size as usize) <= image.len()
        {
            let src = image.as_ptr().add(hdr.rodata_offset as usize);
            core::ptr::copy_nonoverlapping(src, rodata_addr as *mut u8, hdr.rodata_size as usize);
            if rodata_pages * PAGE_SIZE > hdr.rodata_size as usize {
                core::ptr::write_bytes(
                    (rodata_addr + hdr.rodata_size as usize) as *mut u8,
                    0,
                    rodata_pages * PAGE_SIZE - hdr.rodata_size as usize,
                );
            }
        }

        // Copy .data
        if hdr.data_size > 0 && (hdr.data_offset as usize + hdr.data_size as usize) <= image.len() {
            let src = image.as_ptr().add(hdr.data_offset as usize);
            core::ptr::copy_nonoverlapping(src, data_addr as *mut u8, hdr.data_size as usize);
            if data_pages * PAGE_SIZE > hdr.data_size as usize {
                core::ptr::write_bytes(
                    (data_addr + hdr.data_size as usize) as *mut u8,
                    0,
                    data_pages * PAGE_SIZE - hdr.data_size as usize,
                );
            }
        }

        // Zero .bss
        if bss_pages > 0 {
            core::ptr::write_bytes(bss_addr as *mut u8, 0, bss_pages * PAGE_SIZE);
        }

        // Apply only relocations whose targets, symbols, and arithmetic are valid.
        if hdr.reloc_count > 0 {
            let reloc_start = image.as_ptr().add(hdr.reloc_offset as usize) as *const ApdrvReloc;
            let import_count = hdr.import_count;
            for i in 0..hdr.reloc_count as usize {
                let r = core::ptr::read_unaligned(reloc_start.add(i));
                let width = match r.reloc_type {
                    APDRV_RELOC_NONE => continue,
                    APDRV_RELOC_64 => 8usize,
                    APDRV_RELOC_32 | APDRV_RELOC_PC32 | APDRV_RELOC_PLT32 => 4usize,
                    _ => {
                        let _ = crate::mm::pmm::free_frames(base_phys, total_pages);
                        return Err("Unsupported APDRV relocation type");
                    }
                };
                let offset = r.offset as usize;
                if offset
                    .checked_add(width)
                    .map_or(true, |end| end > total_pages * PAGE_SIZE)
                {
                    let _ = crate::mm::pmm::free_frames(base_phys, total_pages);
                    return Err("Relocation target exceeds module memory bounds");
                }
                let target_ptr = (base_addr + offset) as *mut u8;

                let sym_index = r.sym_index;
                let sym_val = match sym_index {
                    APDRV_SEC_TEXT if hdr.text_size > 0 => text_addr as u64,
                    APDRV_SEC_RODATA if hdr.rodata_size > 0 => rodata_addr as u64,
                    APDRV_SEC_DATA if hdr.data_size > 0 => data_addr as u64,
                    APDRV_SEC_BSS if hdr.bss_size > 0 => bss_addr as u64,
                    APDRV_SEC_MODULE => base_addr as u64,
                    idx if idx < import_count => {
                        let imp_ptr =
                            image.as_ptr().add(hdr.import_offset as usize) as *const ApdrvSymbol;
                        let sym = core::ptr::read_unaligned(imp_ptr.add(idx as usize));
                        let name_len = sym
                            .name
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(sym.name.len());
                        if name_len == 0 {
                            let _ = crate::mm::pmm::free_frames(base_phys, total_pages);
                            return Err("APDRV relocation references an empty import name");
                        }
                        let name = match core::str::from_utf8(&sym.name[..name_len]) {
                            Ok(name) => name,
                            Err(_) => {
                                let _ = crate::mm::pmm::free_frames(base_phys, total_pages);
                                return Err("APDRV import name is not valid UTF-8");
                            }
                        };
                        match crate::kernel::ksym::lookup(name) {
                            Some(address) => address,
                            None => {
                                let _ = crate::mm::pmm::free_frames(base_phys, total_pages);
                                return Err("APDRV import symbol could not be resolved");
                            }
                        }
                    }
                    _ => {
                        let _ = crate::mm::pmm::free_frames(base_phys, total_pages);
                        return Err("APDRV relocation references an invalid symbol");
                    }
                };
                let value = sym_val as i128 + r.addend as i128;
                match r.reloc_type {
                    APDRV_RELOC_64 => match u64::try_from(value) {
                        Ok(value) => core::ptr::write_unaligned(target_ptr as *mut u64, value),
                        Err(_) => {
                            let _ = crate::mm::pmm::free_frames(base_phys, total_pages);
                            return Err("64-bit APDRV relocation overflow");
                        }
                    },
                    APDRV_RELOC_32 => match u32::try_from(value) {
                        Ok(value) => core::ptr::write_unaligned(target_ptr as *mut u32, value),
                        Err(_) => {
                            let _ = crate::mm::pmm::free_frames(base_phys, total_pages);
                            return Err("32-bit APDRV relocation overflow");
                        }
                    },
                    APDRV_RELOC_PC32 | APDRV_RELOC_PLT32 => {
                        let displacement = value - target_ptr as i128;
                        match i32::try_from(displacement) {
                            Ok(value) => core::ptr::write_unaligned(target_ptr as *mut i32, value),
                            Err(_) => {
                                let _ = crate::mm::pmm::free_frames(base_phys, total_pages);
                                return Err("PC-relative APDRV relocation overflow");
                            }
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    // Map code, constants, and writable data with separate permissions.
    let ranges = [
        (text_addr, text_pages, crate::mm::vmm::PAGE_PRESENT),
        (
            rodata_addr,
            rodata_pages,
            crate::mm::vmm::PAGE_PRESENT | crate::mm::vmm::PAGE_NO_EXECUTE,
        ),
        (
            data_addr,
            data_pages + bss_pages,
            crate::mm::vmm::PAGE_PRESENT
                | crate::mm::vmm::PAGE_WRITABLE
                | crate::mm::vmm::PAGE_NO_EXECUTE,
        ),
    ];
    for (start_addr, page_count, permissions) in ranges {
        for page in 0..page_count {
            let address = (start_addr + page * PAGE_SIZE) as u64;
            if crate::mm::vmm::map_page(pml4, address, address, permissions).is_err() {
                // Some pages may already be mapped with module permissions. Keep
                // the allocation reserved on failure so it cannot be reused with
                // stale mappings or without its identity mapping.
                return Err("Failed to enforce APDRV W^X page permissions");
            }
        }
    }

    // Function pointers
    let fn_init: Option<extern "C" fn() -> i32> = if hdr.entry_init != APDRV_ENTRY_ABSENT {
        Some(unsafe { core::mem::transmute(text_addr + hdr.entry_init as usize) })
    } else {
        None
    };

    let fn_fini: Option<extern "C" fn() -> i32> = if hdr.entry_fini != APDRV_ENTRY_ABSENT {
        Some(unsafe { core::mem::transmute(text_addr + hdr.entry_fini as usize) })
    } else {
        None
    };

    let fn_probe: Option<extern "C" fn(u32, u32) -> i32> = if hdr.entry_probe != APDRV_ENTRY_ABSENT
    {
        Some(unsafe { core::mem::transmute(text_addr + hdr.entry_probe as usize) })
    } else {
        None
    };

    let fn_ioctl: Option<extern "C" fn(u32, *mut u8) -> i32> =
        if hdr.entry_ioctl != APDRV_ENTRY_ABSENT {
            Some(unsafe { core::mem::transmute(text_addr + hdr.entry_ioctl as usize) })
        } else {
            None
        };

    // Register in slot
    let mut mods = MODULE_REGISTRY.lock();
    for (idx, slot) in mods.iter_mut().enumerate() {
        if slot.state == ModuleState::Unloaded {
            slot.name = hdr.name;
            slot.state = ModuleState::Loaded;
            slot.base_addr = base_addr;
            slot.total_pages = total_pages;
            slot.text_addr = text_addr;
            slot.rodata_addr = rodata_addr;
            slot.data_addr = data_addr;
            slot.bss_addr = bss_addr;
            slot.fn_init = fn_init;
            slot.fn_fini = fn_fini;
            slot.fn_probe = fn_probe;
            slot.fn_ioctl = fn_ioctl;
            slot.target_vendor = hdr.target_vendor;
            slot.target_device = hdr.target_device;
            slot.bound_devices = 0;
            slot.wx_enforced = (hdr.flags & APDRV_FLAG_ENFORCE_WX) != 0;

            // Invoke module init function
            let init_succeeded = match fn_init {
                Some(init_fn) => {
                    if init_fn() == 0 {
                        slot.state = ModuleState::Running;
                        true
                    } else {
                        slot.state = ModuleState::Error;
                        false
                    }
                }
                None => {
                    slot.state = ModuleState::Running;
                    true
                }
            };

            let name_len = hdr
                .name
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(hdr.name.len());
            let name_str = core::str::from_utf8(&hdr.name[0..name_len]).unwrap_or("unknown");
            crate::drivers::serial::print("APDRV: Module '");
            crate::drivers::serial::print(name_str);
            if init_succeeded {
                crate::drivers::serial::println("' loaded with W^X enforcement");
                return Ok(idx);
            }
            crate::drivers::serial::println("' initialization failed");
            return Err("Module initialization failed");
        }
    }

    let _ = crate::mm::pmm::free_frames(base_phys, total_pages);
    Err("Module registry full")
}

pub fn probe_pci_device(vendor: u16, device: u16) -> bool {
    let mut mods = MODULE_REGISTRY.lock();
    for slot in mods.iter_mut() {
        if slot.state == ModuleState::Running {
            let match_vendor = slot.target_vendor == 0xFFFF || slot.target_vendor == vendor;
            let match_device = slot.target_device == 0xFFFF || slot.target_device == device;
            if match_vendor && match_device {
                if let Some(probe_fn) = slot.fn_probe {
                    if probe_fn(vendor as u32, device as u32) == 0 {
                        slot.bound_devices += 1;
                        return true;
                    }
                }
            }
        }
    }
    false
}

pub fn insmod(name: &str) {
    crate::drivers::serial::print("APDRV: Loading module '");
    crate::drivers::serial::print(name);
    crate::drivers::serial::println("' via dynamic APDRV v3 loader");

    let clean = name.trim_start_matches('/');
    if clean.is_empty()
        || clean.as_bytes().contains(&0)
        || clean
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        crate::drivers::serial::println("APDRV: Invalid module path");
        return;
    }
    let mut buf = [0u8; 8192];
    let mut result: Result<(), &'static str> = Err("Module not found");

    crate::kernel::storage::with_fs(|fs| {
        let mut target_ino = fs.lookup(clean);

        if target_ino.is_none() {
            let prefix = b"System/Drivers/";
            let clean_bytes = clean.as_bytes();
            if prefix.len() + clean_bytes.len() <= 64 {
                let mut path = [0u8; 64];
                path[..prefix.len()].copy_from_slice(prefix);
                path[prefix.len()..prefix.len() + clean_bytes.len()].copy_from_slice(clean_bytes);
                if let Ok(path_str) =
                    core::str::from_utf8(&path[..prefix.len() + clean_bytes.len()])
                {
                    target_ino = fs.lookup(path_str);
                }
            }
        }

        if target_ino.is_none() && !clean.ends_with(".apdrv") {
            let prefix = b"System/Drivers/";
            let clean_bytes = clean.as_bytes();
            if prefix.len() + clean_bytes.len() + 6 <= 64 {
                let mut path = [0u8; 64];
                path[..prefix.len()].copy_from_slice(prefix);
                path[prefix.len()..prefix.len() + clean_bytes.len()].copy_from_slice(clean_bytes);
                let suffix_start = prefix.len() + clean_bytes.len();
                path[suffix_start..suffix_start + 6].copy_from_slice(b".apdrv");
                if let Ok(path_str) = core::str::from_utf8(&path[..suffix_start + 6]) {
                    target_ino = fs.lookup(path_str);
                }
            }
        }

        if let Some(ino) = target_ino {
            let len = fs.read_file(ino, &mut buf);
            if len == 0 {
                result = Err("Module file is empty or could not be read");
            } else {
                result = load_binary(&buf[..len]).map(|_| ());
            }
        }
    });

    match result {
        Ok(()) => {
            crate::drivers::serial::print("APDRV: Module '");
            crate::drivers::serial::print(name);
            crate::drivers::serial::println("' loaded successfully");
        }
        Err(reason) => {
            crate::drivers::serial::print("APDRV: Module '");
            crate::drivers::serial::print(name);
            crate::drivers::serial::print("' was not loaded: ");
            crate::drivers::serial::println(reason);
        }
    }
}

pub fn list_modules() {
    let mods = MODULE_REGISTRY.lock();
    crate::drivers::serial::println("=== APDRV v3 Loaded Native Modules ===");
    crate::drivers::serial::println(
        "NAME                             STATE    BASE ADDR          PAGES   PCI VENDOR:DEVICE",
    );
    let mut count = 0;
    for slot in mods.iter() {
        if slot.state != ModuleState::Unloaded {
            count += 1;
            let name_len = slot
                .name
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(slot.name.len());
            let name_str = core::str::from_utf8(&slot.name[0..name_len]).unwrap_or("unknown");
            crate::drivers::serial::print(name_str);
            for _ in name_str.len()..32 {
                crate::drivers::serial::print(" ");
            }
            match slot.state {
                ModuleState::Running => crate::drivers::serial::print("RUNNING  "),
                ModuleState::Loaded => crate::drivers::serial::print("LOADED   "),
                ModuleState::Error => crate::drivers::serial::print("ERROR    "),
                _ => crate::drivers::serial::print("UNKNOWN  "),
            }
            crate::drivers::serial::print("0x");
            crate::drivers::serial::print_hex(slot.base_addr as u64);
            crate::drivers::serial::print("  ");
            crate::drivers::serial::print_u64(slot.total_pages as u64);
            crate::drivers::serial::print("       ");
            crate::drivers::serial::print_hex(slot.target_vendor as u64);
            crate::drivers::serial::print(":");
            crate::drivers::serial::print_hex(slot.target_device as u64);
            crate::drivers::serial::println("");
        }
    }
    if count == 0 {
        crate::drivers::serial::println("No external dynamic modules currently loaded.");
    }
}
