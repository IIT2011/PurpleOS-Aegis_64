use crate::drivers::block_dev::BlockDevice;

pub const GPT_SIGNATURE: u64 = 0x5452_4150_2049_4645; // "EFI PART"
pub const GPT_SECTOR_SIZE: usize = 512;
pub const GPT_MAX_PARTITION_ENTRIES: usize = 128;
pub const GPT_MAX_PARTITIONS: usize = 32;
pub const PURPLEFS_GUID: [u8; 16] = [
    0x50, 0x55, 0x52, 0x50, 0x4C, 0x45, 0x2D, 0x4F, 0x53, 0x2D, 0x46, 0x53, 0x2D, 0x56, 0x32, 0x30,
];

#[derive(Copy, Clone)]
pub struct GptPartition {
    pub type_guid: [u8; 16],
    pub unique_guid: [u8; 16],
    pub start_lba: u64,
    pub end_lba: u64,
    pub attributes: u64,
    pub name: [u8; 36],
    pub is_purplefs: bool,
}

impl Default for GptPartition {
    fn default() -> Self {
        Self {
            type_guid: [0; 16],
            unique_guid: [0; 16],
            start_lba: 0,
            end_lba: 0,
            attributes: 0,
            name: [0; 36],
            is_purplefs: false,
        }
    }
}

pub struct GptInfo {
    pub valid_header: bool,
    pub partition_count: usize,
    pub partitions: [GptPartition; GPT_MAX_PARTITIONS],
    pub purplefs_part_index: Option<usize>,
}

impl GptInfo {
    pub const fn new() -> Self {
        Self {
            valid_header: false,
            partition_count: 0,
            partitions: [GptPartition {
                type_guid: [0; 16],
                unique_guid: [0; 16],
                start_lba: 0,
                end_lba: 0,
                attributes: 0,
                name: [0; 36],
                is_purplefs: false,
            }; GPT_MAX_PARTITIONS],
            purplefs_part_index: None,
        }
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}

fn crc32_update(mut crc: u32, bytes: &[u8]) -> u32 {
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    crc
}

fn crc32(bytes: &[u8]) -> u32 {
    !crc32_update(u32::MAX, bytes)
}

fn check_purplefs_superblock(bdev: &mut dyn BlockDevice, start_lba: u64) -> bool {
    let mut sb = [0u8; GPT_SECTOR_SIZE];
    bdev.read_blocks(start_lba, 1, &mut sb) == 0 && &sb[0..8] == b"PURPLEFS"
}

fn read_gpt_entry_table(
    bdev: &mut dyn BlockDevice,
    entry_lba: u64,
    table_bytes: usize,
    expected_crc: u32,
) -> Result<(), &'static str> {
    let mut crc = u32::MAX;
    let mut remaining = table_bytes;
    let mut lba = entry_lba;
    let mut sector = [0u8; GPT_SECTOR_SIZE];
    while remaining > 0 {
        if bdev.read_blocks(lba, 1, &mut sector) != 0 {
            return Err("Failed to read GPT partition entry table");
        }
        let bytes = remaining.min(GPT_SECTOR_SIZE);
        crc = crc32_update(crc, &sector[..bytes]);
        remaining -= bytes;
        lba = lba.checked_add(1).ok_or("GPT entry table LBA overflow")?;
    }
    if (!crc) != expected_crc {
        return Err("GPT partition entry table CRC mismatch");
    }
    Ok(())
}

pub fn parse(bdev: &mut dyn BlockDevice) -> Result<GptInfo, &'static str> {
    if bdev.block_size() as usize != GPT_SECTOR_SIZE {
        return Err("GPT parser requires 512-byte logical sectors");
    }
    let total_blocks = bdev.total_blocks();
    if total_blocks < 34 {
        return Err("Block device too small for GPT");
    }

    let mut mbr = [0u8; GPT_SECTOR_SIZE];
    if bdev.read_blocks(0, 1, &mut mbr) != 0 {
        return Err("Failed to read MBR");
    }
    if mbr[510] != 0x55 || mbr[511] != 0xAA {
        return Err("Invalid MBR boot signature");
    }

    let mut header = [0u8; GPT_SECTOR_SIZE];
    if bdev.read_blocks(1, 1, &mut header) != 0 {
        return Err("Failed to read GPT header");
    }
    if read_u64(&header, 0) != GPT_SIGNATURE {
        return Err("Invalid GPT signature");
    }

    let revision = read_u32(&header, 8);
    let header_size = read_u32(&header, 12) as usize;
    if revision < 0x0001_0000 || !(92..=GPT_SECTOR_SIZE).contains(&header_size) {
        return Err("Invalid GPT header revision or size");
    }
    let expected_header_crc = read_u32(&header, 16);
    let mut crc_header = header;
    crc_header[16..20].fill(0);
    if crc32(&crc_header[..header_size]) != expected_header_crc {
        return Err("GPT header CRC mismatch");
    }

    let current_lba = read_u64(&header, 24);
    let backup_lba = read_u64(&header, 32);
    let first_usable_lba = read_u64(&header, 40);
    let last_usable_lba = read_u64(&header, 48);
    let entry_lba = read_u64(&header, 72);
    let entry_count = read_u32(&header, 80) as usize;
    let entry_size = read_u32(&header, 84) as usize;
    let expected_table_crc = read_u32(&header, 88);

    if current_lba != 1 || backup_lba >= total_blocks || backup_lba == current_lba {
        return Err("GPT header has invalid current or backup LBA");
    }
    if first_usable_lba > last_usable_lba || last_usable_lba >= total_blocks {
        return Err("GPT usable LBA range is invalid");
    }
    if entry_count == 0 || entry_count > GPT_MAX_PARTITION_ENTRIES {
        return Err("Unsupported GPT partition entry count");
    }
    if !(128..=GPT_SECTOR_SIZE).contains(&entry_size) || entry_size % 8 != 0 {
        return Err("Invalid GPT partition entry size");
    }
    let table_bytes = entry_count
        .checked_mul(entry_size)
        .ok_or("GPT entry table size overflow")?;
    let table_sectors = table_bytes
        .checked_add(GPT_SECTOR_SIZE - 1)
        .ok_or("GPT entry table size overflow")?
        / GPT_SECTOR_SIZE;
    let table_end_lba = entry_lba
        .checked_add(table_sectors as u64)
        .ok_or("GPT entry table LBA overflow")?;
    if entry_lba < 2
        || table_end_lba > total_blocks
        || table_end_lba > backup_lba
        || table_end_lba > first_usable_lba
    {
        return Err("GPT entry table is outside the usable device range");
    }
    read_gpt_entry_table(bdev, entry_lba, table_bytes, expected_table_crc)?;

    let mut info = GptInfo::new();
    info.valid_header = true;
    let entries_per_sector = GPT_SECTOR_SIZE / entry_size;
    if entries_per_sector == 0 || GPT_SECTOR_SIZE % entry_size != 0 {
        return Err("GPT entry size is not supported by this parser");
    }

    let mut sector_buf = [0u8; GPT_SECTOR_SIZE];
    let mut cached_sector = u64::MAX;
    for index in 0..entry_count {
        let sector_offset = index / entries_per_sector;
        let entry_offset = (index % entries_per_sector) * entry_size;
        let lba = entry_lba
            .checked_add(sector_offset as u64)
            .ok_or("GPT partition entry LBA overflow")?;
        if lba != cached_sector {
            if bdev.read_blocks(lba, 1, &mut sector_buf) != 0 {
                return Err("Failed to read GPT partition entry");
            }
            cached_sector = lba;
        }

        let entry = &sector_buf[entry_offset..entry_offset + entry_size];
        let mut type_guid = [0u8; 16];
        type_guid.copy_from_slice(&entry[0..16]);
        if type_guid == [0; 16] {
            continue;
        }

        let mut unique_guid = [0u8; 16];
        unique_guid.copy_from_slice(&entry[16..32]);
        let start_lba = read_u64(entry, 32);
        let end_lba = read_u64(entry, 40);
        let attributes = read_u64(entry, 48);
        if start_lba > end_lba || start_lba < first_usable_lba || end_lba > last_usable_lba {
            return Err("GPT partition lies outside the usable LBA range");
        }

        let mut name = [0u8; 36];
        for (idx, byte) in name.iter_mut().enumerate() {
            let offset = 56 + idx * 2;
            let low = entry[offset];
            let high = entry[offset + 1];
            if low == 0 && high == 0 {
                break;
            }
            *byte = if high == 0 { low } else { b'?' };
        }

        let is_purplefs = type_guid == PURPLEFS_GUID || check_purplefs_superblock(bdev, start_lba);
        if info.partition_count == info.partitions.len() {
            return Err("GPT contains more partitions than this kernel supports");
        }
        let idx = info.partition_count;
        info.partitions[idx] = GptPartition {
            type_guid,
            unique_guid,
            start_lba,
            end_lba,
            attributes,
            name,
            is_purplefs,
        };
        if is_purplefs && info.purplefs_part_index.is_none() {
            info.purplefs_part_index = Some(idx);
        }
        info.partition_count += 1;

        crate::drivers::serial::print("  Part ");
        crate::drivers::serial::print_u64(idx as u64);
        crate::drivers::serial::print(": '");
        let name_len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
        let name_str = core::str::from_utf8(&name[..name_len]).unwrap_or("");
        crate::drivers::serial::print(name_str);
        crate::drivers::serial::print("' [");
        crate::drivers::serial::print_u64(start_lba);
        crate::drivers::serial::print("..");
        crate::drivers::serial::print_u64(end_lba);
        crate::drivers::serial::print(", ");
        crate::drivers::serial::print_u64(end_lba - start_lba + 1);
        crate::drivers::serial::print(" sectors] PurpleFS=");
        crate::drivers::serial::println(if is_purplefs { "1" } else { "0" });
    }

    Ok(info)
}
