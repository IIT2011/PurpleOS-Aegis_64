use crate::drivers::block_dev::BlockDevice;

pub const GPT_SIGNATURE: u64 = 0x5452_4150_2049_4645; // "EFI PART"
pub const PURPLEFS_GUID: [u8; 16] = [
    0x50, 0x55, 0x52, 0x50, 0x4C, 0x45, 0x2D, 0x4F,
    0x53, 0x2D, 0x46, 0x53, 0x2D, 0x56, 0x32, 0x30,
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
    pub partitions: [GptPartition; 8],
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
            }; 8],
            purplefs_part_index: None,
        }
    }
}

fn check_purplefs_superblock(bdev: &mut dyn BlockDevice, start_lba: u64) -> bool {
    let mut sb = [0u8; 512];
    if bdev.read_blocks(start_lba, 1, &mut sb) == 0 {
        &sb[0..8] == b"PURPLEFS"
    } else {
        false
    }
}

pub fn parse(bdev: &mut dyn BlockDevice) -> Result<GptInfo, &'static str> {
    if bdev.total_blocks() < 34 {
        return Err("Block device too small for GPT");
    }

    let mut mbr = [0u8; 512];
    if bdev.read_blocks(0, 1, &mut mbr) != 0 {
        return Err("Failed to read MBR");
    }

    if mbr[510] != 0x55 || mbr[511] != 0xAA {
        return Err("Invalid MBR boot signature");
    }

    let mut gpt_hdr = [0u8; 512];
    if bdev.read_blocks(1, 1, &mut gpt_hdr) != 0 {
        return Err("Failed to read GPT header");
    }

    let sig = u64::from_le_bytes(gpt_hdr[0..8].try_into().unwrap());
    if sig != GPT_SIGNATURE {
        return Err("Invalid GPT signature");
    }

    let part_entry_lba = u64::from_le_bytes(gpt_hdr[72..80].try_into().unwrap());
    let num_part_entries = u32::from_le_bytes(gpt_hdr[80..84].try_into().unwrap()) as usize;
    let part_entry_size = u32::from_le_bytes(gpt_hdr[84..88].try_into().unwrap()) as usize;

    if !(128..=512).contains(&part_entry_size) {
        return Err("Invalid GPT partition entry size");
    }

    let mut info = GptInfo::new();
    info.valid_header = true;

    let mut sector_buf = [0u8; 512];
    let entries_per_sector = 512 / part_entry_size;
    if entries_per_sector == 0 {
        return Err("Zero entries per sector in GPT");
    }

    for i in 0..num_part_entries.min(32) {
        let sector_offset = i / entries_per_sector;
        let entry_offset = (i % entries_per_sector) * part_entry_size;

        if entry_offset + part_entry_size > 512 {
            break;
        }

        if i % entries_per_sector == 0 {
            if bdev.read_blocks(part_entry_lba + sector_offset as u64, 1, &mut sector_buf) != 0 {
                break;
            }
        }

        let entry = &sector_buf[entry_offset..entry_offset + part_entry_size];
        let mut type_guid = [0u8; 16];
        type_guid.copy_from_slice(&entry[0..16]);

        if type_guid == [0; 16] {
            continue;
        }

        let mut unique_guid = [0u8; 16];
        unique_guid.copy_from_slice(&entry[16..32]);

        let start_lba = u64::from_le_bytes(entry[32..40].try_into().unwrap());
        let end_lba = u64::from_le_bytes(entry[40..48].try_into().unwrap());
        let attributes = u64::from_le_bytes(entry[48..56].try_into().unwrap());

        // Extract UTF-16LE partition name
        let mut name = [0u8; 36];
        let mut name_idx = 0;
        for c in 0..35 {
            let off = 56 + c * 2;
            if off + 1 < part_entry_size {
                let ch = entry[off];
                if ch == 0 {
                    break;
                }
                name[name_idx] = ch;
                name_idx += 1;
            }
        }

        let is_purplefs = type_guid == PURPLEFS_GUID || check_purplefs_superblock(bdev, start_lba);

        if info.partition_count < info.partitions.len() {
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
    }

    Ok(info)
}
