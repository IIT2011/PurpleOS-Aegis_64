use crate::arch::x86_64::boot::{MultibootInfo, AEGIS_UEFI_ACPI_RSDP};

#[repr(C, packed)]
struct RsdpDescriptor {
    signature: [u8; 8], // "RSD PTR "
    checksum: u8,
    oemid: [u8; 6],
    revision: u8,
    rsdt_address: u32,
}

#[repr(C, packed)]
struct AcpiHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oemid: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

// The early identity map covers addresses below 4 GiB. Apply a narrower
// firmware table range before dereferencing either the header or payload.
unsafe fn validated_table(address: usize, signature: &[u8; 4]) -> Option<&'static [u8]> {
    const MIN_ADDRESS: usize = 0x1000;
    const END_ADDRESS: usize = 0xFE000000;
    const HEADER_SIZE: usize = core::mem::size_of::<AcpiHeader>();
    const MAX_TABLE_SIZE: usize = 4096;
    if !(MIN_ADDRESS..=END_ADDRESS - HEADER_SIZE).contains(&address) {
        return None;
    }
    let header = core::slice::from_raw_parts(address as *const u8, HEADER_SIZE);
    if &header[..4] != signature {
        return None;
    }
    let length = u32::from_le_bytes(header[4..8].try_into().ok()?) as usize;
    if !(HEADER_SIZE..=MAX_TABLE_SIZE).contains(&length) || length > END_ADDRESS - address {
        return None;
    }
    let table = core::slice::from_raw_parts(address as *const u8, length);
    (table.iter().fold(0u8, |sum, &byte| sum.wrapping_add(byte)) == 0).then_some(table)
}

fn madt_topology(table: &[u8]) -> Option<(u64, [u8; 8], usize)> {
    let header = core::mem::size_of::<AcpiHeader>();
    let lapic = u32::from_le_bytes(table.get(header..header + 4)?.try_into().ok()?) as u64;
    let mut records = table.get(header + 8..)?;
    let mut ids = [0; 8];
    let mut count = 0;
    while !records.is_empty() {
        let kind = records[0];
        let length = usize::from(*records.get(1)?);
        if length < 2 || length > records.len() {
            return None;
        }
        let (record, rest) = records.split_at(length);
        if kind == 0 {
            let flags = u32::from_le_bytes(record.get(4..8)?.try_into().ok()?);
            if flags & 1 != 0 && count < ids.len() && !ids[..count].contains(&record[3]) {
                ids[count] = record[3];
                count += 1;
            }
        }
        records = rest;
    }
    if lapic == 0 || lapic & 0xfff != 0 || count == 0 {
        return None;
    }
    Some((lapic, ids, count))
}

pub struct MadtInfo {
    pub lapic_phys_addr: u64,
    pub cpu_count: usize,
    pub apic_ids: [u8; 8],
}

static mut MADT_INFO: MadtInfo = MadtInfo {
    lapic_phys_addr: 0xFEE00000,
    cpu_count: 1,
    apic_ids: [0; 8],
};

pub fn init(mbi: Option<&MultibootInfo>) -> Result<(), &'static str> {
    unsafe {
        // Default detected cores if ACPI scan in standard QEMU memory
        let mut count = 0;
        let mut lapic = 0xFEE00000;

        // Search BIOS ROM area 0xE0000..0xFFFFF for "RSD PTR "
        let (start, limit) = mbi
            .filter(|info| info.flags & AEGIS_UEFI_ACPI_RSDP != 0)
            .map_or((0xE0000, 0x100000), |info| {
                let rsdp = info.config_table as usize;
                (rsdp, rsdp + 16)
            });
        let mut addr = start as *const u8;
        let end = limit as *const u8;

        while addr < end {
            if core::slice::from_raw_parts(addr, 8) == b"RSD PTR " {
                let rsdp_bytes = core::slice::from_raw_parts(addr, 20);
                let sum: u8 = rsdp_bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
                if sum == 0 {
                    let rsdp = &*(addr as *const RsdpDescriptor);
                    let rsdt_phys =
                        core::ptr::read_unaligned(core::ptr::addr_of!(rsdp.rsdt_address)) as usize;
                    crate::drivers::serial::print("ACPI: RSDP at 0x");
                    crate::drivers::serial::print_hex(addr as u64);
                    crate::drivers::serial::print(", RSDT at 0x");
                    crate::drivers::serial::print_hex(rsdt_phys as u64);
                    crate::drivers::serial::println("");
                    if let Some(rsdt) = validated_table(rsdt_phys, b"RSDT") {
                        let payload = &rsdt[core::mem::size_of::<AcpiHeader>()..];
                        if payload.len() % 4 == 0 {
                            crate::drivers::serial::print("ACPI: RSDT valid, len=");
                            crate::drivers::serial::print_u64(rsdt.len() as u64);
                            crate::drivers::serial::println("");
                            for entry in payload.chunks_exact(4) {
                                let address = u32::from_le_bytes(entry.try_into().unwrap()) as usize;
                                if validated_table(address, b"FACP").is_some() {
                                    crate::kernel::power::init(address);
                                }
                                if let Some((base, ids, found)) =
                                    validated_table(address, b"APIC").and_then(madt_topology)
                                {
                                    lapic = base;
                                    MADT_INFO.apic_ids = ids;
                                    count = found;
                                }
                            }
                        }
                    }
                }
                break;
            }
            addr = addr.add(16);
        }

        if count == 0 {
            count = 1; // Fallback for uniprocessor system without ACPI MADT
            let bsp_apic_id = (crate::drivers::apic::get_id() & 0xFF) as u8;
            MADT_INFO.apic_ids[0] = bsp_apic_id;
        }

        MADT_INFO.lapic_phys_addr = lapic;
        MADT_INFO.cpu_count = count;

        crate::drivers::serial::print("ACPI MADT: Local APIC Base=0x");
        crate::drivers::serial::print_hex_u64(lapic);
        crate::drivers::serial::print(", CPUs=");
        crate::drivers::serial::print_u64(count as u64);
        crate::drivers::serial::println("");
    }
    Ok(())
}

pub fn get_madt_info() -> &'static MadtInfo {
    unsafe { &*core::ptr::addr_of!(MADT_INFO) }
}
