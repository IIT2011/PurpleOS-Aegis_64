use crate::drivers::block_dev::BlockDevice;

pub const PFS2_MAGIC: [u8; 8] = *b"PURPLEFS";
pub const PFS2_BLOCK_SIZE: usize = 4096;
pub const PFS2_ROOT_INODE: u32 = 1;

pub struct PurpleFsV2<'a> {
    pub dev: &'a mut dyn BlockDevice,
    pub start_lba: u64,
    pub volume_label: [u8; 32],
    pub total_blocks: u32,
    pub free_blocks: u32,
    pub read_only: bool,
}

impl<'a> PurpleFsV2<'a> {
    pub fn mount(dev: &'a mut dyn BlockDevice, start_lba: u64) -> Result<Self, &'static str> {
        let mut sb_buf = [0u8; 512];
        if dev.read_blocks(start_lba, 1, &mut sb_buf) != 0 {
            return Err("Failed to read superblock");
        }

        if &sb_buf[0..8] != &PFS2_MAGIC {
            return Err("Invalid PurpleFS v2 signature");
        }

        let total_blocks = u32::from_le_bytes(sb_buf[24..28].try_into().unwrap());
        let free_blocks = u32::from_le_bytes(sb_buf[28..32].try_into().unwrap());

        let mut label = [0u8; 32];
        label.copy_from_slice(&sb_buf[0x70..0x70 + 32]);

        Ok(Self {
            dev,
            start_lba,
            volume_label: label,
            total_blocks: if total_blocks > 0 { total_blocks } else { 32768 },
            free_blocks: if free_blocks > 0 { free_blocks } else { 30000 },
            read_only: true,
        })
    }

    /// Read an entire 4096-byte filesystem block (8 sectors of 512 bytes)
    pub fn read_fs_block(&mut self, block_idx: u32, buf: &mut [u8; 4096]) -> Result<(), &'static str> {
        let lba = self.start_lba + (block_idx as u64) * 8;
        if self.dev.read_blocks(lba, 8, buf) == 0 {
            Ok(())
        } else {
            Err("Block device read failure")
        }
    }

    /// Write an entire 4096-byte filesystem block (8 sectors of 512 bytes)
    pub fn write_fs_block(&mut self, block_idx: u32, buf: &[u8; 4096]) -> Result<(), &'static str> {
        let lba = self.start_lba + (block_idx as u64) * 8;
        if self.dev.write_blocks(lba, 8, buf) == 0 {
            Ok(())
        } else {
            Err("Block device write failure")
        }
    }

    pub fn lookup(&mut self, filename: &str) -> Option<u32> {
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
                    return Some(ino);
                }
            }

            if rec_len == 0 {
                break;
            }
            off += rec_len.max(32);
        }

        None
    }

    pub fn read_file(&mut self, inode: u32, buf: &mut [u8]) -> usize {
        if inode == 0 || inode >= 256 {
            return 0;
        }

        // Each inode is 128 bytes; inode table starts at block 2 (32 inodes per 4 KiB block)
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

            if self.read_fs_block(data_blk, &mut block_buf).is_ok() {
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

    pub fn list_files<F: FnMut(u32, u8, u64, &str)>(&mut self, mut callback: F) {
        let mut dir_block = [0u8; 4096];
        if self.read_fs_block(10, &mut dir_block).is_err() {
            return;
        }

        let mut off = 0;
        while off + 32 <= 4096 {
            let ino = u32::from_le_bytes(dir_block[off..off + 4].try_into().unwrap());
            let rec_len = u16::from_le_bytes(dir_block[off + 4..off + 6].try_into().unwrap()) as usize;
            let name_len = dir_block[off + 6] as usize;
            let ftype = dir_block[off + 7];

            if ino != 0 && name_len > 0 && off + 8 + name_len <= 4096 {
                if let Ok(name_str) = core::str::from_utf8(&dir_block[off + 8..off + 8 + name_len]) {
                    if name_str != "." && name_str != ".." {
                        // Lookup size from inode
                        let blk_idx = 2 + (ino / 32);
                        let ino_off = ((ino % 32) * 128) as usize;
                        let mut inode_block = [0u8; 4096];
                        let mut fsize = 0u64;
                        if self.read_fs_block(blk_idx, &mut inode_block).is_ok() {
                            fsize = u64::from_le_bytes(inode_block[ino_off + 8..ino_off + 16].try_into().unwrap());
                        }
                        callback(ino, ftype, fsize, name_str);
                    }
                }
            }

            if rec_len == 0 {
                break;
            }
            off += rec_len.max(32);
        }
    }

    pub fn touch(&mut self, filename: &str) -> Result<u32, &'static str> {
        if self.read_only {
            return Err("Filesystem is mounted read-only");
        }
        if let Some(existing) = self.lookup(filename) {
            return Ok(existing);
        }

        // Find free inode (scan inodes 2..255)
        let mut free_ino = None;
        let mut target_blk = 0;
        let mut target_ino_off = 0;
        let mut inode_block = [0u8; 4096];

        for ino in 2..256 {
            let blk = 2 + (ino / 32);
            let off = ((ino % 32) * 128) as usize;
            if self.read_fs_block(blk, &mut inode_block).is_ok() {
                let mode = u16::from_le_bytes(inode_block[off..off + 2].try_into().unwrap());
                if mode == 0 {
                    free_ino = Some(ino);
                    target_blk = blk;
                    target_ino_off = off;
                    break;
                }
            }
        }

        let new_ino = free_ino.ok_or("No free inodes available")?;

        // Initialize new inode (Regular file, 0644, size 0)
        inode_block[target_ino_off..target_ino_off + 2].copy_from_slice(&0x81A4u16.to_le_bytes());
        inode_block[target_ino_off + 6..target_ino_off + 8].copy_from_slice(&1u16.to_le_bytes()); // links=1
        inode_block[target_ino_off + 8..target_ino_off + 16].copy_from_slice(&0u64.to_le_bytes()); // size=0
        self.write_fs_block(target_blk, &inode_block)?;

        // Add dirent into root dir block (block 10)
        let mut dir_block = [0u8; 4096];
        self.read_fs_block(10, &mut dir_block)?;

        let mut off = 0;
        let mut slot_found = false;
        while off + 32 <= 4096 {
            let ino = u32::from_le_bytes(dir_block[off..off + 4].try_into().unwrap());
            if ino == 0 {
                let b = filename.as_bytes();
                let len = b.len().min(24);
                dir_block[off..off + 4].copy_from_slice(&new_ino.to_le_bytes());
                dir_block[off + 4..off + 6].copy_from_slice(&32u16.to_le_bytes());
                dir_block[off + 6] = len as u8;
                dir_block[off + 7] = 1; // regular file
                dir_block[off + 8..off + 8 + len].copy_from_slice(&b[..len]);
                slot_found = true;
                break;
            }
            off += 32;
        }

        if !slot_found {
            return Err("Root directory full");
        }

        self.write_fs_block(10, &dir_block)?;
        Ok(new_ino)
    }

    pub fn write_file(&mut self, filename: &str, content: &[u8]) -> Result<usize, &'static str> {
        if self.read_only {
            return Err("Filesystem is mounted read-only");
        }

        let ino = match self.lookup(filename) {
            Some(i) => i,
            None => self.touch(filename)?,
        };

        // Allocate a data block (find first free block in bitmap block 1)
        let mut bitmap = [0u8; 4096];
        self.read_fs_block(1, &mut bitmap)?;

        let mut free_blk = None;
        for b in 11..self.total_blocks {
            let byte_idx = (b / 8) as usize;
            let bit_idx = (b % 8) as usize;
            if byte_idx < 4096 && (bitmap[byte_idx] & (1 << bit_idx)) == 0 {
                bitmap[byte_idx] |= 1 << bit_idx;
                free_blk = Some(b);
                break;
            }
        }

        let blk = free_blk.ok_or("No free data blocks available")?;
        self.write_fs_block(1, &bitmap)?;

        // Write content into block blk
        let mut data_buf = [0u8; 4096];
        let write_len = content.len().min(4096);
        data_buf[0..write_len].copy_from_slice(&content[0..write_len]);
        self.write_fs_block(blk, &data_buf)?;

        // Update inode direct_blocks[0] and size
        let blk_idx = 2 + (ino / 32);
        let ino_off = ((ino % 32) * 128) as usize;
        let mut inode_block = [0u8; 4096];
        self.read_fs_block(blk_idx, &mut inode_block)?;

        inode_block[ino_off + 8..ino_off + 16].copy_from_slice(&(write_len as u64).to_le_bytes());
        inode_block[ino_off + 0x2C..ino_off + 0x30].copy_from_slice(&blk.to_le_bytes());
        self.write_fs_block(blk_idx, &inode_block)?;

        Ok(write_len)
    }

    pub fn sync(&mut self) -> Result<(), &'static str> {
        if self.dev.flush() == 0 {
            Ok(())
        } else {
            Err("Failed to flush device")
        }
    }
}
