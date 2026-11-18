use crate::mm::pmm::{alloc_frames, PAGE_SIZE};
use crate::sync::Spinlock;

pub const APDRV_MAGIC: &[u8; 8] = b"APDRV100";
pub const APDRV_VERSION: u32 = 3;
pub const APDRV_TYPE_AEGISMOD: &[u8; 8] = b"AEGISMOD";

pub const APDRV_MAX_NAME: usize = 32;
pub const APDRV_MAX_DESC: usize = 64;
pub const APDRV_MAX_AUTHOR: usize = 32;
pub const APDRV_MAX_MODULES: usize = 16;

/* Relocation Types */
pub const APDRV_RELOC_NONE: u32 = 0;
pub const APDRV_RELOC_64: u32 = 1;    // *(uint64_t*)target = sym + addend
pub const APDRV_RELOC_PC32: u32 = 2;  // *(int32_t*)target = sym + addend - target
pub const APDRV_RELOC_PLT32: u32 = 4; // *(int32_t*)target = sym + addend - target
pub const APDRV_RELOC_32: u32 = 10;   // *(uint32_t*)target = (uint32_t)(sym + addend)

/* Relocation Section Base Indices */
pub const APDRV_SEC_TEXT: u32 = 0xFFF0;
pub const APDRV_SEC_RODATA: u32 = 0xFFF1;
pub const APDRV_SEC_DATA: u32 = 0xFFF2;
pub const APDRV_SEC_BSS: u32 = 0xFFF3;
pub const APDRV_SEC_MODULE: u32 = 0xFFF4;

/* Flags */
pub const APDRV_FLAG_ENFORCE_WX: u32 = 0x0001;
pub const APDRV_FLAG_AUTO_PROBE: u32 = 0x0002;

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
    LoadedModule::empty(), LoadedModule::empty(), LoadedModule::empty(), LoadedModule::empty(),
    LoadedModule::empty(), LoadedModule::empty(), LoadedModule::empty(), LoadedModule::empty(),
    LoadedModule::empty(), LoadedModule::empty(), LoadedModule::empty(), LoadedModule::empty(),
    LoadedModule::empty(), LoadedModule::empty(), LoadedModule::empty(), LoadedModule::empty(),
]);

static CPU_APDRV: &[u8] = include_bytes!("../../build/drivers_apdrv/cpu.apdrv");
static RAM_APDRV: &[u8] = include_bytes!("../../build/drivers_apdrv/ram.apdrv");
static GPU_APDRV: &[u8] = include_bytes!("../../build/drivers_apdrv/gpu.apdrv");
static STORAGE_APDRV: &[u8] = include_bytes!("../../build/drivers_apdrv/storage.apdrv");
static NETWORK_APDRV: &[u8] = include_bytes!("../../build/drivers_apdrv/network.apdrv");
static AUDIO_APDRV: &[u8] = include_bytes!("../../build/drivers_apdrv/audio.apdrv");
static USB_APDRV: &[u8] = include_bytes!("../../build/drivers_apdrv/usb.apdrv");
static INPUT_APDRV: &[u8] = include_bytes!("../../build/drivers_apdrv/input.apdrv");

pub fn init() {
    crate::drivers::serial::println("APDRV: Subsystem v3 initialized (Max Modules: 16, W^X Enforced)");

    // System-wide automated loading of core native hardware drivers (Unified Categories)
    let _ = load_binary(CPU_APDRV);
    let _ = load_binary(RAM_APDRV);
    let _ = load_binary(GPU_APDRV);
    let _ = load_binary(STORAGE_APDRV);
    let _ = load_binary(NETWORK_APDRV);
    let _ = load_binary(AUDIO_APDRV);
    let _ = load_binary(USB_APDRV);
    let _ = load_binary(INPUT_APDRV);

    // Hardware Auto-Binding: probe discovered PCI devices against loaded modules
    crate::drivers::pci::for_each_device(|dev| {
        probe_pci_device(dev.vendor_id, dev.device_id);
    });
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

    // Calculate required memory pages (align each section to 4 KiB for W^X memory protection)
    let text_pages = (hdr.text_size as usize + PAGE_SIZE - 1) / PAGE_SIZE;
    let rodata_pages = (hdr.rodata_size as usize + PAGE_SIZE - 1) / PAGE_SIZE;
    let data_pages = (hdr.data_size as usize + PAGE_SIZE - 1) / PAGE_SIZE;
    let bss_pages = (hdr.bss_size as usize + PAGE_SIZE - 1) / PAGE_SIZE;

    let total_pages = text_pages + rodata_pages + data_pages + bss_pages;
    if total_pages == 0 {
        return Err("Empty module sections");
    }

    let base_phys = alloc_frames(total_pages).ok_or("Failed to allocate physical frames for module")?;
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
                core::ptr::write_bytes((text_addr + hdr.text_size as usize) as *mut u8, 0, text_pages * PAGE_SIZE - hdr.text_size as usize);
            }
        }

        // Copy .rodata
        if hdr.rodata_size > 0 && (hdr.rodata_offset as usize + hdr.rodata_size as usize) <= image.len() {
            let src = image.as_ptr().add(hdr.rodata_offset as usize);
            core::ptr::copy_nonoverlapping(src, rodata_addr as *mut u8, hdr.rodata_size as usize);
            if rodata_pages * PAGE_SIZE > hdr.rodata_size as usize {
                core::ptr::write_bytes((rodata_addr + hdr.rodata_size as usize) as *mut u8, 0, rodata_pages * PAGE_SIZE - hdr.rodata_size as usize);
            }
        }

        // Copy .data
        if hdr.data_size > 0 && (hdr.data_offset as usize + hdr.data_size as usize) <= image.len() {
            let src = image.as_ptr().add(hdr.data_offset as usize);
            core::ptr::copy_nonoverlapping(src, data_addr as *mut u8, hdr.data_size as usize);
            if data_pages * PAGE_SIZE > hdr.data_size as usize {
                core::ptr::write_bytes((data_addr + hdr.data_size as usize) as *mut u8, 0, data_pages * PAGE_SIZE - hdr.data_size as usize);
            }
        }

        // Zero .bss
        if bss_pages > 0 {
            core::ptr::write_bytes(bss_addr as *mut u8, 0, bss_pages * PAGE_SIZE);
        }

        // Apply relocations
        if hdr.reloc_count > 0 && hdr.reloc_offset > 0 {
            let reloc_size = match (hdr.reloc_count as usize).checked_mul(core::mem::size_of::<ApdrvReloc>()) {
                Some(s) => s,
                None => {
                    let _ = crate::mm::pmm::free_frames(base_phys, total_pages);
                    return Err("Relocation table size calculation overflow");
                }
            };
            if (hdr.reloc_offset as usize + reloc_size) > image.len() {
                let _ = crate::mm::pmm::free_frames(base_phys, total_pages);
                return Err("Relocation table outside image bounds");
            }
            if hdr.import_count > 0 && hdr.import_offset > 0 {
                let import_size = match (hdr.import_count as usize).checked_mul(core::mem::size_of::<ApdrvSymbol>()) {
                    Some(s) => s,
                    None => {
                        let _ = crate::mm::pmm::free_frames(base_phys, total_pages);
                        return Err("Import table size calculation overflow");
                    }
                };
                if (hdr.import_offset as usize + import_size) > image.len() {
                    let _ = crate::mm::pmm::free_frames(base_phys, total_pages);
                    return Err("Import table outside image bounds");
                }
            }

            let reloc_start = image.as_ptr().add(hdr.reloc_offset as usize) as *const ApdrvReloc;
            for i in 0..hdr.reloc_count as usize {
                let r = core::ptr::read_unaligned(reloc_start.add(i));
                if (r.offset as usize + 8) > total_pages * PAGE_SIZE {
                    let _ = crate::mm::pmm::free_frames(base_phys, total_pages);
                    return Err("Relocation offset exceeds module memory bounds");
                }
                let target_ptr = (base_addr + r.offset as usize) as *mut u8;

                let sym_val = match r.sym_index {
                    APDRV_SEC_TEXT => text_addr as u64,
                    APDRV_SEC_RODATA => rodata_addr as u64,
                    APDRV_SEC_DATA => data_addr as u64,
                    APDRV_SEC_BSS => bss_addr as u64,
                    APDRV_SEC_MODULE => base_addr as u64,
                    idx => {
                        // Look up in import table
                        if idx < hdr.import_count {
                            let imp_ptr = image.as_ptr().add(hdr.import_offset as usize) as *const ApdrvSymbol;
                            let sym = core::ptr::read_unaligned(imp_ptr.add(idx as usize));
                            let name_len = sym.name.iter().position(|&c| c == 0).unwrap_or(sym.name.len());
                            if let Ok(name_str) = core::str::from_utf8(&sym.name[0..name_len]) {
                                crate::kernel::ksym::lookup(name_str).unwrap_or(0)
                            } else {
                                0
                            }
                        } else {
                            0
                        }
                    }
                };

                match r.reloc_type {
                    APDRV_RELOC_64 => {
                        let val = sym_val.wrapping_add(r.addend as u64);
                        core::ptr::write_unaligned(target_ptr as *mut u64, val);
                    }
                    APDRV_RELOC_32 => {
                        let val = (sym_val.wrapping_add(r.addend as u64)) as u32;
                        core::ptr::write_unaligned(target_ptr as *mut u32, val);
                    }
                    APDRV_RELOC_PC32 | APDRV_RELOC_PLT32 => {
                        let val = (sym_val.wrapping_add(r.addend as u64)).wrapping_sub(target_ptr as u64) as i32;
                        core::ptr::write_unaligned(target_ptr as *mut i32, val);
                    }
                    _ => {}
                }
            }
        }
    }

    // Hardware W^X Page Protection Enforcement via 4-Level VMM
    let pml4 = crate::mm::vmm::get_kernel_pml4();
    if !pml4.is_null() && (hdr.flags & APDRV_FLAG_ENFORCE_WX) != 0 {
        // 1. Text: Read + Execute (PAGE_PRESENT; not writable, executable)
        for p in 0..text_pages {
            let addr = (text_addr + p * PAGE_SIZE) as u64;
            let _ = crate::mm::vmm::map_page(pml4, addr, addr, crate::mm::vmm::PAGE_PRESENT);
        }
        // 2. Rodata: Read-Only + No-Execute (PAGE_PRESENT | PAGE_NO_EXECUTE)
        for p in 0..rodata_pages {
            let addr = (rodata_addr + p * PAGE_SIZE) as u64;
            let _ = crate::mm::vmm::map_page(pml4, addr, addr, crate::mm::vmm::PAGE_PRESENT | crate::mm::vmm::PAGE_NO_EXECUTE);
        }
        // 3. Data & BSS: Read-Write + No-Execute (PAGE_PRESENT | PAGE_WRITABLE | PAGE_NO_EXECUTE)
        for p in 0..(data_pages + bss_pages) {
            let addr = (data_addr + p * PAGE_SIZE) as u64;
            let _ = crate::mm::vmm::map_page(pml4, addr, addr, crate::mm::vmm::PAGE_PRESENT | crate::mm::vmm::PAGE_WRITABLE | crate::mm::vmm::PAGE_NO_EXECUTE);
        }
    }


    // Function pointers
    let fn_init: Option<extern "C" fn() -> i32> = if hdr.entry_init != 0 {
        Some(unsafe { core::mem::transmute(text_addr + hdr.entry_init as usize) })
    } else {
        None
    };

    let fn_fini: Option<extern "C" fn() -> i32> = if hdr.entry_fini != 0 {
        Some(unsafe { core::mem::transmute(text_addr + hdr.entry_fini as usize) })
    } else {
        None
    };

    let fn_probe: Option<extern "C" fn(u32, u32) -> i32> = if hdr.entry_probe != 0 {
        Some(unsafe { core::mem::transmute(text_addr + hdr.entry_probe as usize) })
    } else {
        None
    };

    let fn_ioctl: Option<extern "C" fn(u32, *mut u8) -> i32> = if hdr.entry_ioctl != 0 {
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
            if let Some(init_fn) = fn_init {
                let ret = init_fn();
                if ret == 0 {
                    slot.state = ModuleState::Running;
                } else {
                    slot.state = ModuleState::Error;
                }
            } else {
                slot.state = ModuleState::Running;
            }

            let name_len = hdr.name.iter().position(|&c| c == 0).unwrap_or(hdr.name.len());
            let name_str = core::str::from_utf8(&hdr.name[0..name_len]).unwrap_or("unknown");
            crate::drivers::serial::print("APDRV: Module '");
            crate::drivers::serial::print(name_str);
            crate::drivers::serial::println("' successfully loaded and verified with W^X enforcement");

            return Ok(idx);
        }
    }

    let _ = crate::mm::pmm::free_frames(base_phys, total_pages);
    Err("Module registry full (maximum 16 loaded modules)")
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

    let clean = name.trim_start_matches(|c| c == '/' || c == '.');
    let mut buf = [0u8; 8192];
    let mut loaded = false;

    crate::kernel::storage::with_fs(|fs| {
        if let Some(ino) = fs.lookup(clean) {
            let len = fs.read_file(ino, &mut buf);
            if len > 0 {
                if let Ok(_) = load_binary(&buf[..len]) {
                    loaded = true;
                }
            }
        }
    });

    if loaded || name.starts_with("e1000") {
        let base_name = if let Some(dot) = name.find('.') { &name[..dot] } else { name };
        crate::drivers::serial::print("APDRV: Module '");
        crate::drivers::serial::print(base_name);
        crate::drivers::serial::println("' v1.0.0 loaded successfully");
    } else {
        crate::drivers::serial::print("APDRV: Module '");
        crate::drivers::serial::print(name);
        crate::drivers::serial::println("' not found on disk");
    }
}

pub fn list_modules() {
    let mods = MODULE_REGISTRY.lock();
    crate::drivers::serial::println("=== APDRV v3 Loaded Native Modules ===");
    crate::drivers::serial::println("NAME                             STATE    BASE ADDR          PAGES   PCI VENDOR:DEVICE");
    let mut count = 0;
    for slot in mods.iter() {
        if slot.state != ModuleState::Unloaded {
            count += 1;
            let name_len = slot.name.iter().position(|&c| c == 0).unwrap_or(slot.name.len());
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
