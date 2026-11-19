//! Pure Rust Bare-Metal FAT12 / FAT16 / FAT32 Filesystem Driver
//!
//! Provides structural parsing, BPB validation, cluster chain traversal,
//! and directory record inspection for removable storage devices (USB, SD, ESP).

use crate::drivers::block_dev::BlockDevice;

pub const FAT_ATTR_READ_ONLY: u8 = 0x01;
pub const FAT_ATTR_HIDDEN: u8    = 0x02;
pub const FAT_ATTR_SYSTEM: u8    = 0x04;
pub const FAT_ATTR_VOLUME_ID: u8 = 0x08;
pub const FAT_ATTR_DIRECTORY: u8 = 0x10;
pub const FAT_ATTR_ARCHIVE: u8   = 0x20;
pub const FAT_ATTR_LFN: u8       = 0x0F;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FatType {
    Fat12,
    Fat16,
    Fat32,
}

impl FatType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FatType::Fat12 => "FAT12",
            FatType::Fat16 => "FAT16",
            FatType::Fat32 => "FAT32",
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct BiosParameterBlock {
    pub jmp_boot: [u8; 3],
    pub oem_name: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub num_fats: u8,
    pub root_entry_count: u16,
    pub total_sectors_16: u16,
    pub media_type: u8,
    pub fat_size_16: u16,
    pub sectors_per_track: u16,
    pub num_heads: u16,
    pub hidden_sectors: u32,
    pub total_sectors_32: u32,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct Fat32Header {
    pub fat_size_32: u32,
    pub ext_flags: u16,
    pub fs_version: u16,
    pub root_cluster: u32,
    pub fs_info: u16,
    pub backup_boot_sector: u16,
    pub reserved: [u8; 12],
    pub drive_num: u8,
    pub nt_flags: u8,
    pub signature: u8,
    pub volume_id: u32,
    pub volume_label: [u8; 11],
    pub fs_type_str: [u8; 8],
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct FatDirEntry {
    pub name: [u8; 11],
    pub attributes: u8,
    pub nt_reserved: u8,
    pub creation_time_tenth: u8,
    pub creation_time: u16,
    pub creation_date: u16,
    pub last_access_date: u16,
    pub first_cluster_hi: u16,
    pub write_time: u16,
    pub write_date: u16,
    pub first_cluster_lo: u16,
    pub file_size: u32,
}

#[derive(Copy, Clone)]
pub struct FatFsInfo {
    pub fat_type: FatType,
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub cluster_size_bytes: u32,
    pub reserved_sectors: u16,
    pub num_fats: u8,
    pub fat_size: u32,
    pub root_cluster: u32,
    pub root_dir_sectors: u32,
    pub first_data_sector: u32,
    pub total_clusters: u32,
}

pub struct FatDriver {
    pub info: Option<FatFsInfo>,
    pub start_lba: u64,
}

impl FatDriver {
    pub const fn new() -> Self {
        Self {
            info: None,
            start_lba: 0,
        }
    }

    /// Check if a 512-byte block contains a valid FAT BPB boot sector
    pub fn probe_sector(buf: &[u8]) -> Option<FatType> {
        if buf.len() < 512 {
            return None;
        }
        // Boot signature 0x55, 0xAA at 510..512
        if buf[510] != 0x55 || buf[511] != 0xAA {
            return None;
        }

        let bytes_per_sec = u16::from_le_bytes([buf[11], buf[12]]);
        // The block-device API uses 512-byte logical sectors; reject BPBs
        // whose sector geometry cannot be addressed by this probe/driver.
        if bytes_per_sec != 512 {
            return None;
        }

        let sec_per_clus = buf[13];
        if sec_per_clus == 0 || (sec_per_clus & (sec_per_clus - 1)) != 0 {
            return None;
        }

        let res_sec = u16::from_le_bytes([buf[14], buf[15]]);
        let num_fats = buf[16];
        if num_fats == 0 || res_sec == 0 {
            return None;
        }

        let root_ent_cnt = u16::from_le_bytes([buf[17], buf[18]]);
        let tot_sec_16 = u16::from_le_bytes([buf[19], buf[20]]);
        let fat_sz_16 = u16::from_le_bytes([buf[22], buf[23]]);
        let tot_sec_32 = u32::from_le_bytes([buf[32], buf[33], buf[34], buf[35]]);

        let total_sectors = if tot_sec_16 != 0 { tot_sec_16 as u32 } else { tot_sec_32 };
        if total_sectors == 0 {
            return None;
        }

        let root_dir_sectors = ((root_ent_cnt as u32 * 32) + (bytes_per_sec as u32 - 1)) / (bytes_per_sec as u32);
        let fat_sz = if fat_sz_16 != 0 {
            fat_sz_16 as u32
        } else {
            u32::from_le_bytes([buf[36], buf[37], buf[38], buf[39]])
        };

        if fat_sz == 0 {
            return None;
        }
        let overhead = (res_sec as u32)
            .checked_add((num_fats as u32).checked_mul(fat_sz)?)?
            .checked_add(root_dir_sectors)?;
        if total_sectors <= overhead {
            return None;
        }
        let data_sec = total_sectors - overhead;
        let cluster_count = data_sec / (sec_per_clus as u32);

        if cluster_count < 4085 {
            Some(FatType::Fat12)
        } else if cluster_count < 65525 {
            Some(FatType::Fat16)
        } else {
            Some(FatType::Fat32)
        }
    }

    /// Probe a block device for a FAT filesystem at start_lba
    pub fn probe(&mut self, dev: &mut dyn BlockDevice, start_lba: u64) -> Option<FatType> {
        let mut buf = [0u8; 512];
        if dev.block_size() != 512 || start_lba >= dev.total_blocks()
            || dev.read_blocks(start_lba, 1, &mut buf) != 0 {
            return None;
        }
        let fat_t = Self::probe_sector(&buf)?;
        self.start_lba = start_lba;
        Some(fat_t)
    }
}
