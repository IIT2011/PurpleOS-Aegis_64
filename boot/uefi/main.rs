#![no_std]
#![no_main]

use core::{ffi::c_void, panic::PanicInfo, ptr, slice};

#[path = "../../src/arch/x86_64/boot.rs"]
#[allow(dead_code)]
mod boot;

#[path = "../../src/crypto/mod.rs"]
#[allow(dead_code)]
mod crypto;

use boot::{MultibootInfo, MultibootMmapEntry};

const PAGE: usize = 4096;
const LIMIT: u64 = 1 << 32;
const BUFFER_PAGES: usize = 32;
const MAP_OFFSET: usize = PAGE * 16;
const EFI_ERROR: usize = 1 << (usize::BITS - 1);
const EFI_LOAD_ERROR: usize = EFI_ERROR | 1;
const EFI_SECURITY_VIOLATION: usize = EFI_ERROR | 26;
const EFI_INVALID_PARAMETER: usize = EFI_ERROR | 2;
const REQUIRE_SECURE_BOOT: bool = option_env!("AEGIS_ALLOW_INSECURE_UEFI").is_none();
const KERNEL: &[u8] = include_bytes!(env!("AEGIS_KERNEL_ELF"));
const ENTRY: &[u8; 8] = include_bytes!(env!("AEGIS_UEFI_ENTRY"));

const KERNEL_SIGNATURE: &[u8; crypto::rsa::RSA_2048_BYTES] =
    include_bytes!(env!("AEGIS_KERNEL_SIGNATURE"));

fn verify_kernel() -> bool {
    let mut digest = crypto::sha256::Sha256::new();
    digest.update(b"PurpleOS/UEFI-KERNEL/v1\0");
    digest.update(ENTRY);
    digest.update(KERNEL);
    crypto::rsa::verify_rsa_2048_sha256_digest(
        &crypto::creator::CREATOR_RSA_MODULUS,
        &digest.finalize(),
        KERNEL_SIGNATURE,
    )
}

unsafe fn print_status(table: &SystemTable, message: &[u8]) {
    if let Some(console) = table.console_out.as_ref() {
        let mut utf16 = [0; 96];
        for (slot, byte) in utf16.iter_mut().take(95).zip(message) {
            *slot = u16::from(*byte);
        }
        (console.output_string)(table.console_out, utf16.as_ptr());
    }
}

type Status = usize;
type Handle = *mut c_void;
type AllocatePages = unsafe extern "efiapi" fn(u32, u32, usize, *mut u64) -> Status;
type GetMemoryMap = unsafe extern "efiapi" fn(*mut usize, *mut u8, *mut usize, *mut usize, *mut u32) -> Status;
type ExitBootServices = unsafe extern "efiapi" fn(Handle, usize) -> Status;
type LocateProtocol = unsafe extern "efiapi" fn(*const Guid, *const c_void, *mut *mut c_void) -> Status;

#[repr(C, align(8))]
struct BootServices {
    header: [u8; 24],
    tpl: [usize; 2],
    allocate_pages: AllocatePages,
    free_pages: usize,
    get_memory_map: GetMemoryMap,
    before_exit_boot_services: [usize; 21],
    exit_boot_services: ExitBootServices,
    before_locate_protocol: [usize; 10],
    locate_protocol: LocateProtocol,
}

type GetVariable = unsafe extern "efiapi" fn(
    *const u16, *const Guid, *mut u32, *mut usize, *mut u8,
) -> Status;

#[repr(C)]
struct RuntimeServices {
    header: [u8; 24],
    before_get_variable: [usize; 6],
    get_variable: GetVariable,
}

unsafe fn secure_boot_enabled(table: &SystemTable) -> bool {
    const EFI_GLOBAL_VARIABLE: Guid = Guid(
        0x8be4df61, 0x93ca, 0x11d2, [0xaa, 0x0d, 0x00, 0xe0, 0x98, 0x03, 0x2b, 0x8c],
    );
    const SECURE_BOOT: [u16; 11] = [83, 101, 99, 117, 114, 101, 66, 111, 111, 116, 0];
    const SETUP_MODE: [u16; 10] = [83, 101, 116, 117, 112, 77, 111, 100, 101, 0];
    let Some(runtime) = table.runtime_services.as_ref() else { return false };
    let read = |name: &[u16]| {
        let mut value = 0;
        let mut size = 1;
        let status = (runtime.get_variable)(
            name.as_ptr(), &EFI_GLOBAL_VARIABLE, ptr::null_mut(), &mut size, &mut value,
        );
        (status == 0 && size == 1).then_some(value)
    };
    read(&SECURE_BOOT) == Some(1) && read(&SETUP_MODE) == Some(0)
}

#[repr(C)]
struct Console {
    reset: usize,
    output_string: unsafe extern "efiapi" fn(*mut Console, *const u16) -> Status,
}

#[repr(C)]
pub struct SystemTable {
    header: [u8; 24],
    firmware_vendor: usize,
    firmware_revision: u32,
    console_in_handle: usize,
    console_in: usize,
    console_out_handle: usize,
    console_out: *mut Console,
    error_handle: usize,
    error: usize,
    runtime_services: *const RuntimeServices,
    boot_services: *const BootServices,
    configuration_count: usize,
    configuration: *const ConfigurationTable,
}

#[repr(C)]
#[derive(PartialEq, Eq)]
struct Guid(u32, u16, u16, [u8; 8]);

#[repr(C)]
struct ConfigurationTable {
    guid: Guid,
    address: u64,
}

#[repr(C)]
struct GraphicsOutput {
    methods: [usize; 3],
    mode: *const GraphicsMode,
}

#[repr(C)]
struct GraphicsMode {
    max_mode: u32,
    mode: u32,
    info: *const GraphicsInfo,
    info_size: usize,
    framebuffer: u64,
    framebuffer_size: usize,
}

#[repr(C)]
struct GraphicsInfo {
    version: u32,
    width: u32,
    height: u32,
    pixel_format: u32,
    masks: [u32; 4],
    stride: u32,
}

fn u16_at(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap())
}

fn u32_at(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}

fn u64_at(data: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap())
}

unsafe fn load_kernel(services: &BootServices, entry: u64) -> Option<()> {
    let elf = KERNEL;
    if elf.len() < 64 || &elf[..7] != b"\x7fELF\x02\x01\x01"
        || u16_at(elf, 16) != 2 || u16_at(elf, 18) != 62 || u16_at(elf, 54) != 56
    {
        return None;
    }
    let offset = usize::try_from(u64_at(elf, 32)).ok()?;
    let count = usize::from(u16_at(elf, 56));
    let table = elf.get(offset..offset.checked_add(count.checked_mul(56)?)?)?;
    let loads = || table.chunks_exact(56).filter(|header| u32_at(header, 0) == 1);
    let mut start = LIMIT;
    let mut end = 0;
    let mut entry_valid = false;
    for header in loads() {
        let address = u64_at(header, 24);
        let file_size = u64_at(header, 32);
        let memory = u64_at(header, 40);
        let file_offset = u64_at(header, 8);
        let limit = address.checked_add(memory)?;
        if address != u64_at(header, 16) || address < 0x100000 || limit > LIMIT
            || file_size > memory || file_offset.checked_add(file_size)? > elf.len() as u64
        {
            return None;
        }
        if memory == 0 { continue; }
        start = start.min(address & !(PAGE as u64 - 1));
        end = end.max(limit.next_multiple_of(PAGE as u64));
        entry_valid |= u32_at(header, 4) & 1 != 0
            && (address..address + file_size).contains(&entry);
    }
    if !entry_valid || end <= start { return None; }
    let length = usize::try_from(end - start).ok()?;
    let mut destination = start;
    if (services.allocate_pages)(2, 1, length / PAGE, &mut destination) != 0
        || destination != start
    {
        return None;
    }
    ptr::write_bytes(destination as *mut u8, 0, length);
    for header in loads() {
        let size = u64_at(header, 32) as usize;
        if size > 0 {
            ptr::copy_nonoverlapping(
                elf.as_ptr().add(u64_at(header, 8) as usize),
                u64_at(header, 24) as *mut u8,
                size,
            );
        }
    }
    Some(())
}

unsafe fn graphics(services: &BootServices, info: &mut MultibootInfo) {
    const GOP: Guid = Guid(0x9042a9de, 0x23dc, 0x4a38, [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a]);
    let mut interface = ptr::null_mut();
    if (services.locate_protocol)(&GOP, ptr::null(), &mut interface) != 0 { return; }
    let Some(gop) = (interface as *const GraphicsOutput).as_ref() else { return };
    let Some(mode) = gop.mode.as_ref() else { return };
    let Some(geometry) = mode.info.as_ref() else { return };
    // The kernel writes XRGB8888 words, which match GOP's BGR byte order.
    let Some(pitch) = geometry.stride.checked_mul(4) else { return };
    if geometry.pixel_format != 1 || mode.framebuffer == 0
        || mode.framebuffer.checked_add(mode.framebuffer_size as u64).is_none_or(|end| end > LIMIT)
        || geometry.stride < geometry.width || geometry.height == 0
        || (pitch as u64) * u64::from(geometry.height) > mode.framebuffer_size as u64
    {
        return;
    }
    info.flags |= boot::MULTIBOOT_INFO_FRAMEBUFFER;
    info.framebuffer_addr = mode.framebuffer;
    info.framebuffer_width = geometry.width;
    info.framebuffer_height = geometry.height;
    info.framebuffer_pitch = pitch;
    info.framebuffer_bpp = 32;
    info.framebuffer_type = 1;
}

unsafe fn handoff(image: Handle, table: &SystemTable, services: &BootServices, entry: u64, secure_boot: bool) -> Status {
    if load_kernel(services, entry).is_none() { return EFI_LOAD_ERROR; }
    let mut buffer = LIMIT - 1;
    if (services.allocate_pages)(1, 2, BUFFER_PAGES, &mut buffer) != 0 {
        return EFI_LOAD_ERROR;
    }
    let mut info: MultibootInfo = core::mem::zeroed();
    if secure_boot {
        info.flags |= boot::AEGIS_UEFI_SECURE_BOOT;
    }
    graphics(services, &mut info);
    const ACPI_2: Guid = Guid(0x8868e871, 0xe4f1, 0x11d3, [0xbc, 0x22, 0, 0x80, 0xc7, 0x3c, 0x88, 0x81]);
    if !table.configuration.is_null() && table.configuration_count <= 1024 {
        let configuration = slice::from_raw_parts(table.configuration, table.configuration_count);
        if let Some(acpi) = configuration.iter().find(|item| item.guid == ACPI_2) {
            if (0x1000..LIMIT - 20).contains(&acpi.address) {
                info.config_table = acpi.address as u32;
                info.flags |= boot::AEGIS_UEFI_ACPI_RSDP;
            }
        }
    }
    let entries = (buffer as *mut u8).add(PAGE) as *mut MultibootMmapEntry;
    let memory_map = (buffer as *mut u8).add(MAP_OFFSET);
    for _ in 0..3 {
        let mut length = BUFFER_PAGES * PAGE - MAP_OFFSET;
        let mut key = 0;
        let mut stride = 0;
        let mut version = 0;
        let status = (services.get_memory_map)(&mut length, memory_map, &mut key, &mut stride, &mut version);
        if status != 0 { return status; }
        if stride < 40 || version != 1 || length > BUFFER_PAGES * PAGE - MAP_OFFSET
            || length % stride != 0
        {
            return EFI_LOAD_ERROR;
        }
        let mut count = 0;
        for descriptor in slice::from_raw_parts(memory_map, length).chunks_exact(stride) {
            let kind = u32_at(descriptor, 0);
            let address = u64_at(descriptor, 8);
            let Some(size) = u64_at(descriptor, 24).checked_mul(PAGE as u64) else { return EFI_LOAD_ERROR };
            if address.checked_add(size).is_none() { return EFI_LOAD_ERROR; }
            if address >= LIMIT || size == 0 { continue; }
            if count >= (MAP_OFFSET - PAGE) / size_of::<MultibootMmapEntry>() {
                return EFI_LOAD_ERROR;
            }
            entries.add(count).write(MultibootMmapEntry {
                size: 20, addr: address, len: size.min(LIMIT - address),
                entry_type: match kind { 3 | 4 | 7 => 1, 9 => 3, 10 => 4, 8 => 5, _ => 2 },
            });
            count += 1;
        }
        if count == 0 { return EFI_LOAD_ERROR; }
        info.flags |= boot::MULTIBOOT_INFO_MEM_MAP;
        info.mmap_addr = entries as u32;
        info.mmap_length = (count * size_of::<MultibootMmapEntry>()) as u32;
        (buffer as *mut MultibootInfo).write(info);
        match (services.exit_boot_services)(image, key) {
            0 => {
                let start: unsafe extern "efiapi" fn(u64) -> ! = core::mem::transmute(entry);
                start(buffer);
            }
            EFI_INVALID_PARAMETER => continue,
            error => return error,
        }
    }
    EFI_LOAD_ERROR
}

#[unsafe(no_mangle)]
pub unsafe extern "efiapi" fn efi_main(image: Handle, table: *const SystemTable) -> Status {
    let Some(table) = table.as_ref() else { return EFI_INVALID_PARAMETER };
    let Some(services) = table.boot_services.as_ref() else { return EFI_INVALID_PARAMETER };
    if !verify_kernel() {
        print_status(table, b"AegisX64: kernel signature rejected. Boot halted.\r\n");
        return EFI_SECURITY_VIOLATION;
    }
    let secure_boot = secure_boot_enabled(table);
    if REQUIRE_SECURE_BOOT && !secure_boot {
        print_status(table, b"AegisX64: Secure Boot required. Boot halted.\r\n");
        return EFI_SECURITY_VIOLATION;
    }
    print_status(table, b"AegisX64: verified embedded kernel\r\n");
    handoff(image, table, services, u64::from_le_bytes(*ENTRY), secure_boot)
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    loop { core::hint::spin_loop(); }
}
