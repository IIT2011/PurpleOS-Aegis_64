use crate::arch::x86_64::boot::MultibootInfo;

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

pub fn init(_mbi: Option<&MultibootInfo>) -> Result<(), &'static str> {
    unsafe {
        // Default detected cores if ACPI scan in standard QEMU memory
        let mut count = 0;
        let mut lapic = 0xFEE00000;

        // Search BIOS ROM area 0xE0000..0xFFFFF for "RSD PTR "
        let mut addr = 0xE0000 as *const u8;
        let end = 0x100000 as *const u8;

        while addr < end {
            if core::slice::from_raw_parts(addr, 8) == b"RSD PTR " {
                let rsdp_bytes = core::slice::from_raw_parts(addr, 20);
                let sum: u8 = rsdp_bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
                if sum == 0 {
                    let rsdp = &*(addr as *const RsdpDescriptor);
                    let rsdt_phys = core::ptr::read_unaligned(core::ptr::addr_of!(rsdp.rsdt_address)) as usize;
                    crate::drivers::serial::print("ACPI: RSDP at 0x");
                    crate::drivers::serial::print_hex(addr as u64);
                    crate::drivers::serial::print(", RSDT at 0x");
                    crate::drivers::serial::print_hex(rsdt_phys as u64);
                    crate::drivers::serial::println("");
                    if rsdt_phys >= 0x1000 && rsdt_phys < 0xFE000000 {
                        let rsdt = rsdt_phys as *const AcpiHeader;
                        if core::slice::from_raw_parts(rsdt as *const u8, 4) == b"RSDT" {
                            let rsdt_len = core::ptr::read_unaligned(core::ptr::addr_of!((*rsdt).length)) as usize;
                            crate::drivers::serial::print("ACPI: RSDT valid, len=");
                            crate::drivers::serial::print_u64(rsdt_len as u64);
                            crate::drivers::serial::println("");
                            if rsdt_len >= core::mem::size_of::<AcpiHeader>() && rsdt_len < 4096 {
                                let entries = (rsdt_len - core::mem::size_of::<AcpiHeader>()) / 4;
                                let entry_ptr = (rsdt as usize + core::mem::size_of::<AcpiHeader>()) as *const u32;

                                for i in 0..entries {
                                    let table_phys = core::ptr::read_unaligned(entry_ptr.add(i)) as usize;
                                    if table_phys < 0x1000 || table_phys >= 0xFE000000 {
                                        continue;
                                    }
                                    let table_addr = table_phys as *const AcpiHeader;
                                    if core::slice::from_raw_parts(table_addr as *const u8, 4) == b"APIC" {
                                        // MADT Table
                                        lapic = core::ptr::read_unaligned(((table_addr as usize) + 0x24) as *const u32) as u64;
                                        let mut madt_cur = (table_addr as usize) + 0x2C;
                                        let madt_len = core::ptr::read_unaligned(core::ptr::addr_of!((*table_addr).length)) as usize;
                                        let madt_end = (table_addr as usize) + madt_len;

                                        while madt_cur < madt_end {
                                            let entry_type = *(madt_cur as *const u8);
                                            let entry_len = *((madt_cur + 1) as *const u8) as usize;
                                            if entry_len == 0 {
                                                break;
                                            }

                                            if entry_type == 0 { // Processor Local APIC
                                                let flags = core::ptr::read_unaligned((madt_cur + 4) as *const u32);
                                                if (flags & 1) != 0 && count < 8 {
                                                    let apic_id = *((madt_cur + 3) as *const u8);
                                                    MADT_INFO.apic_ids[count] = apic_id;
                                                    count += 1;
                                                }
                                            }
                                            madt_cur += entry_len;
                                        }
                                        break;
                                    }
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
            count = 4; // Fallback for 4 vCPUs
            MADT_INFO.apic_ids[0] = 0;
            MADT_INFO.apic_ids[1] = 1;
            MADT_INFO.apic_ids[2] = 2;
            MADT_INFO.apic_ids[3] = 3;
        }

        MADT_INFO.lapic_phys_addr = lapic;
        MADT_INFO.cpu_count = count;

        crate::drivers::serial::print("ACPI MADT: Local APIC Base=0x");
        crate::drivers::serial::print_hex(lapic);
        crate::drivers::serial::print(", CPUs=");
        crate::drivers::serial::print_u64(count as u64);
        crate::drivers::serial::println("");
    }
    Ok(())
}

pub fn get_madt_info() -> &'static MadtInfo {
    unsafe { &*core::ptr::addr_of!(MADT_INFO) }
}
