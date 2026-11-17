use crate::drivers::block_dev::BlockDevice;

pub const PFS3_MAGIC: u64 = 0x3353_4645_4C50_5255; // "PURPLEFS3"
pub const PFS3_BLOCK_SIZE: usize = 4096;
pub const PFS3_ROOT_INODE: u64 = 1;

pub struct PurpleFsV3<'a> {
    pub dev: &'a mut dyn BlockDevice,
    pub start_lba: u64,
    pub sector_count: u64,
    pub volume_label: [u8; 32],
    pub total_blocks: u64,
    pub free_blocks: u64,
}

impl<'a> PurpleFsV3<'a> {
    pub fn mount(dev: &'a mut dyn BlockDevice, start_lba: u64, sector_count: u64) -> Result<Self, &'static str> {
        let mut sb = [0u8; 512];
        if dev.read_blocks(start_lba, 1, &mut sb) != 0 {
            return Err("Failed to read superblock");
        }

        let mut label = [0u8; 32];
        label.copy_from_slice(&sb[0x70..0x70 + 32]);

        let sb_total = u32::from_le_bytes(sb[24..28].try_into().unwrap_or([0; 4])) as u64;
        let sb_free = u32::from_le_bytes(sb[28..32].try_into().unwrap_or([0; 4])) as u64;
        let total = if sector_count >= 8 { sector_count / 8 } else { sb_total };
        let free = if sb_free > 0 { sb_free } else { total };

        Ok(Self {
            dev,
            start_lba,
            sector_count,
            volume_label: label,
            total_blocks: total,
            free_blocks: free,
        })
    }

    pub fn read_fs_block(&mut self, block_idx: u64, buf: &mut [u8; 4096]) -> Result<(), &'static str> {
        let lba = self.start_lba + block_idx * 8;
        if self.dev.read_blocks(lba, 8, buf) == 0 {
            Ok(())
        } else {
            Err("Device read error")
        }
    }

    pub fn lookup_path(&mut self, path: &str) -> Option<u64> {
        let filename = path.trim_start_matches('/').rsplit('/').next().unwrap_or(path);
        let mut dir_block = [0u8; 4096];
        if self.read_fs_block(10, &mut dir_block).is_err() {
            return None;
        }

        let mut off = 0;
        while off + 32 <= 4096 {
            let ino = u32::from_le_bytes(dir_block[off..off + 4].try_into().unwrap());
            let rec_len = u16::from_le_bytes(dir_block[off + 4..off + 6].try_into().unwrap()) as usize;
            let name_len = dir_block[off + 6] as usize;

            if ino != 0 && name_len > 0 && off + 8 + name_len <= 4096 {
                let name = &dir_block[off + 8..off + 8 + name_len];
                if name == filename.as_bytes() {
                    return Some(ino as u64);
                }
            }

            if rec_len == 0 {
                break;
            }
            off += rec_len.max(32);
        }

        None
    }

    pub fn read_file(&mut self, inode: u64, buf: &mut [u8]) -> usize {
        if inode == 0 || inode >= 256 {
            return 0;
        }

        let blk_idx = 2 + (inode / 32);
        let ino_off = ((inode % 32) * 128) as usize;

        let mut inode_block = [0u8; 4096];
        if self.read_fs_block(blk_idx, &mut inode_block).is_err() {
            return 0;
        }

        let size_bytes = u64::from_le_bytes(
            inode_block[ino_off + 8..ino_off + 16].try_into().unwrap(),
        ) as usize;

        let mut bytes_copied = 0;
        let mut block_buf = [0u8; 4096];

        for i in 0..12 {
            if bytes_copied >= size_bytes || bytes_copied >= buf.len() {
                break;
            }

            let ptr_off = ino_off + 0x2C + i * 4;
            let data_blk = u32::from_le_bytes(inode_block[ptr_off..ptr_off + 4].try_into().unwrap());
            if data_blk == 0 {
                break;
            }

            if self.read_fs_block(data_blk as u64, &mut block_buf).is_ok() {
                let remaining = size_bytes - bytes_copied;
                let chunk_len = remaining.min(4096).min(buf.len() - bytes_copied);
                buf[bytes_copied..bytes_copied + chunk_len].copy_from_slice(&block_buf[0..chunk_len]);
                bytes_copied += chunk_len;
            } else {
                break;
            }
        }

        bytes_copied
    }
}
