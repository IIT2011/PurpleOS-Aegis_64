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

    pub fn get_file_size(&mut self, inode: u32) -> Option<usize> {
        if inode == 0 || inode >= 256 {
            return None;
        }
        let blk_idx = 2 + (inode / 32);
        let ino_off = ((inode % 32) * 128) as usize;
        let mut inode_block = [0u8; 4096];
        if self.read_fs_block(blk_idx, &mut inode_block).is_err() {
            return None;
        }
        let size_bytes = u64::from_le_bytes(
            inode_block[ino_off + 8..ino_off + 16].try_into().unwrap(),
        ) as usize;
        Some(size_bytes)
    }

    pub fn read_file_at(&mut self, inode: u32, offset: usize, buf: &mut [u8]) -> usize {
        if inode == 0 || inode >= 256 || buf.is_empty() {
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

        if offset >= size_bytes {
            return 0;
        }

        let total_file_blocks = (size_bytes + 4095) / 4096;
        let mut bytes_copied = 0;
        let mut block_buf = [0u8; 4096];
        let mut indirect_buf = [0u8; 4096];
        let mut indirect_loaded = false;
        let indirect_blk = u32::from_le_bytes(inode_block[ino_off + 0x5C..ino_off + 0x60].try_into().unwrap());

        for i in 0..total_file_blocks.min(12 + 1024) {
            if bytes_copied >= buf.len() {
                break;
            }
            let blk_start = i * 4096;
            let blk_end = blk_start + 4096;
            let current_pos = offset + bytes_copied;

            if current_pos >= size_bytes {
                break;
            }

            if current_pos >= blk_end {
                continue;
            }

            let data_blk = if i < 12 {
                let ptr_off = ino_off + 0x2C + i * 4;
                u32::from_le_bytes(inode_block[ptr_off..ptr_off + 4].try_into().unwrap())
            } else {
                if indirect_blk == 0 {
                    0
                } else {
                    if !indirect_loaded {
                        if self.read_fs_block(indirect_blk, &mut indirect_buf).is_ok() {
                            indirect_loaded = true;
                        } else {
                            break;
                        }
                    }
                    let ind_idx = i - 12;
                    u32::from_le_bytes(indirect_buf[ind_idx * 4..(ind_idx + 1) * 4].try_into().unwrap())
                }
            };
            if data_blk == 0 {
                break;
            }

            if self.read_fs_block(data_blk, &mut block_buf).is_ok() {
                let off_in_blk = current_pos - blk_start;
                let avail_in_blk = 4096 - off_in_blk;
                let avail_in_file = size_bytes - current_pos;
                let chunk_len = avail_in_blk.min(avail_in_file).min(buf.len() - bytes_copied);

                buf[bytes_copied..bytes_copied + chunk_len].copy_from_slice(&block_buf[off_in_blk..off_in_blk + chunk_len]);
                bytes_copied += chunk_len;
            } else {
                break;
            }
        }

        bytes_copied
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
        let b = filename.as_bytes();
        let name_len = b.len().min(55);
        let rec_len = if name_len > 23 { 64u16 } else { 32u16 };

        while off + (rec_len as usize) <= 4096 {
            let ino = u32::from_le_bytes(dir_block[off..off + 4].try_into().unwrap());
            let cur_rec_len = u16::from_le_bytes(dir_block[off + 4..off + 6].try_into().unwrap()) as usize;
            if ino == 0 || cur_rec_len == 0 {
                dir_block[off..off + 4].copy_from_slice(&new_ino.to_le_bytes());
                dir_block[off + 4..off + 6].copy_from_slice(&rec_len.to_le_bytes());
                dir_block[off + 6] = name_len as u8;
                dir_block[off + 7] = 1; // regular file
                dir_block[off + 8..off + (rec_len as usize)].fill(0);
                dir_block[off + 8..off + 8 + name_len].copy_from_slice(&b[..name_len]);
                slot_found = true;
                break;
            }
            off += cur_rec_len.max(32);
        }

        if !slot_found {
            return Err("Root directory full");
        }

        self.write_fs_block(10, &dir_block)?;
        Ok(new_ino)
    }

    pub fn mkdir(&mut self, dirname: &str) -> Result<u32, &'static str> {
        if self.read_only {
            return Err("Filesystem is mounted read-only");
        }
        let clean = dirname.trim_start_matches('/').trim_end_matches('/');
        if clean.is_empty() {
            return Err("Cannot create directory with empty name");
        }
        if self.lookup(clean).is_some() {
            return Err("Directory already exists");
        }

        // Find free inode (scan inodes 2..256)
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

        // Initialize new directory inode (Directory, 0755, size 4096, links 2)
        inode_block[target_ino_off..target_ino_off + 2].copy_from_slice(&0x41EDu16.to_le_bytes());
        inode_block[target_ino_off + 2..target_ino_off + 4].copy_from_slice(&0u16.to_le_bytes()); // uid=0
        inode_block[target_ino_off + 4..target_ino_off + 6].copy_from_slice(&0u16.to_le_bytes()); // gid=0
        inode_block[target_ino_off + 6..target_ino_off + 8].copy_from_slice(&2u16.to_le_bytes()); // links=2
        inode_block[target_ino_off + 8..target_ino_off + 16].copy_from_slice(&4096u64.to_le_bytes()); // size=4096
        self.write_fs_block(target_blk, &inode_block)?;

        // Add dirent into root dir block (block 10)
        let mut dir_block = [0u8; 4096];
        self.read_fs_block(10, &mut dir_block)?;

        let mut off = 0;
        let mut slot_found = false;
        let b = clean.as_bytes();
        let name_len = b.len().min(55);
        let rec_len = if name_len > 23 { 64u16 } else { 32u16 };

        while off + (rec_len as usize) <= 4096 {
            let ino = u32::from_le_bytes(dir_block[off..off + 4].try_into().unwrap());
            let cur_rec_len = u16::from_le_bytes(dir_block[off + 4..off + 6].try_into().unwrap()) as usize;
            if ino == 0 || cur_rec_len == 0 {
                dir_block[off..off + 4].copy_from_slice(&new_ino.to_le_bytes());
                dir_block[off + 4..off + 6].copy_from_slice(&rec_len.to_le_bytes());
                dir_block[off + 6] = name_len as u8;
                dir_block[off + 7] = 2; // Directory file type
                dir_block[off + 8..off + (rec_len as usize)].fill(0);
                dir_block[off + 8..off + 8 + name_len].copy_from_slice(&b[..name_len]);
                slot_found = true;
                break;
            }
            off += cur_rec_len.max(32);
        }

        if !slot_found {
            return Err("Root directory full");
        }

        self.write_fs_block(10, &dir_block)?;
        Ok(new_ino)
    }

    pub fn rmdir(&mut self, dirname: &str) -> Result<(), &'static str> {
        if self.read_only {
            return Err("Filesystem is mounted read-only");
        }
        let clean = dirname.trim_start_matches('/').trim_end_matches('/');
        let ino = self.lookup(clean).ok_or("Directory not found")?;

        // Read inode to verify it is a directory
        let blk_idx = 2 + (ino / 32);
        let ino_off = ((ino % 32) * 128) as usize;
        let mut inode_block = [0u8; 4096];
        self.read_fs_block(blk_idx, &mut inode_block)?;

        let mode = u16::from_le_bytes(inode_block[ino_off..ino_off + 2].try_into().unwrap());
        if (mode & 0xF000) != 0x4000 {
            return Err("Not a directory");
        }

        // Check if directory has any children in the filesystem
        let mut has_children = false;
        self.list_files(|_child_ino, _ftype, _fsize, name| {
            if name.starts_with(clean) && name.as_bytes().get(clean.len()) == Some(&b'/') {
                has_children = true;
            }
        });
        if has_children {
            return Err("Directory not empty");
        }

        // Clear inode
        inode_block[ino_off..ino_off + 128].fill(0);
        self.write_fs_block(blk_idx, &inode_block)?;

        // Remove dirent from root dir block (block 10)
        let mut dir_block = [0u8; 4096];
        self.read_fs_block(10, &mut dir_block)?;

        let mut off = 0;
        while off + 32 <= 4096 {
            let d_ino = u32::from_le_bytes(dir_block[off..off + 4].try_into().unwrap());
            let rec_len = u16::from_le_bytes(dir_block[off + 4..off + 6].try_into().unwrap()) as usize;
            if d_ino == ino {
                let clear_len = rec_len.max(32).min(4096 - off);
                dir_block[off..off + clear_len].fill(0);
                break;
            }
            if rec_len == 0 {
                break;
            }
            off += rec_len.max(32);
        }

        self.write_fs_block(10, &dir_block)?;
        Ok(())
    }

    pub fn write_file(&mut self, filename: &str, content: &[u8]) -> Result<usize, &'static str> {
        self.write_file_at(filename, 0, content)
    }

    pub fn write_file_at(&mut self, filename: &str, offset: usize, content: &[u8]) -> Result<usize, &'static str> {
        if self.read_only {
            return Err("Filesystem is mounted read-only");
        }
        if content.is_empty() {
            return Ok(0);
        }
        if offset >= 12 * 4096 {
            return Err("File write offset exceeds maximum direct block capacity (48 KiB)");
        }

        let ino = match self.lookup(filename) {
            Some(i) => i,
            None => self.touch(filename)?,
        };

        let blk_idx = 2 + (ino / 32);
        let ino_off = ((ino % 32) * 128) as usize;
        let mut inode_block = [0u8; 4096];
        self.read_fs_block(blk_idx, &mut inode_block)?;

        let current_size = u64::from_le_bytes(
            inode_block[ino_off + 8..ino_off + 16].try_into().unwrap(),
        ) as usize;

        let mut bytes_written = 0usize;
        let mut bitmap = [0u8; 4096];
        let mut bitmap_loaded = false;

        while bytes_written < content.len() {
            let cur_pos = offset + bytes_written;
            let block_idx = cur_pos / 4096;
            if block_idx >= 12 {
                break;
            }
            let off_in_block = cur_pos % 4096;
            let avail_in_block = 4096 - off_in_block;
            let chunk_len = (content.len() - bytes_written).min(avail_in_block);

            let ptr_off = ino_off + 0x2C + (block_idx * 4);
            let mut blk = u32::from_le_bytes(inode_block[ptr_off..ptr_off + 4].try_into().unwrap());

            if blk == 0 {
                if !bitmap_loaded {
                    self.read_fs_block(1, &mut bitmap)?;
                    bitmap_loaded = true;
                }
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
                blk = free_blk.ok_or("No free data blocks available")?;
                inode_block[ptr_off..ptr_off + 4].copy_from_slice(&blk.to_le_bytes());
            }

            let mut data_buf = [0u8; 4096];
            if off_in_block > 0 || chunk_len < 4096 {
                let _ = self.read_fs_block(blk, &mut data_buf);
            }
            data_buf[off_in_block..off_in_block + chunk_len].copy_from_slice(&content[bytes_written..bytes_written + chunk_len]);
            self.write_fs_block(blk, &data_buf)?;
            bytes_written += chunk_len;
        }

        if bitmap_loaded {
            self.write_fs_block(1, &bitmap)?;
        }

        let new_size = current_size.max(offset + bytes_written);
        inode_block[ino_off + 8..ino_off + 16].copy_from_slice(&(new_size as u64).to_le_bytes());
        self.write_fs_block(blk_idx, &inode_block)?;

        Ok(bytes_written)
    }

    pub fn copy_file(&mut self, src_name: &str, dst_name: &str) -> Result<usize, &'static str> {
        if self.read_only {
            return Err("Filesystem is mounted read-only");
        }
        let ino = self.lookup(src_name).ok_or("Source file not found")?;
        let file_size = self.get_file_size(ino).ok_or("Failed to get file size")?;

        let mut total_copied = 0;
        let mut chunk_buf = [0u8; 4096];
        while total_copied < file_size {
            let to_read = (file_size - total_copied).min(4096);
            let n = self.read_file_at(ino, total_copied, &mut chunk_buf[..to_read]);
            if n == 0 {
                break;
            }
            self.write_file_at(dst_name, total_copied, &chunk_buf[..n])?;
            total_copied += n;
        }
        Ok(total_copied)
    }

    pub fn move_file(&mut self, src_name: &str, dst_name: &str) -> Result<(), &'static str> {
        if self.read_only {
            return Err("Filesystem is mounted read-only");
        }
        let ino = self.lookup(src_name).ok_or("Source file not found")?;
        if self.lookup(dst_name).is_some() {
            return Err("Destination file already exists");
        }

        let mut dir_block = [0u8; 4096];
        self.read_fs_block(10, &mut dir_block)?;
        let mut off = 0;
        let mut updated = false;
        while off + 32 <= 4096 {
            let d_ino = u32::from_le_bytes(dir_block[off..off + 4].try_into().unwrap());
            if d_ino == ino {
                let b = dst_name.as_bytes();
                let len = b.len().min(24);
                dir_block[off + 6] = len as u8;
                dir_block[off + 8..off + 32].fill(0);
                dir_block[off + 8..off + 8 + len].copy_from_slice(&b[..len]);
                updated = true;
                break;
            }
            off += 32;
        }

        if !updated {
            return Err("Dirent not found in directory block");
        }

        self.write_fs_block(10, &dir_block)?;
        Ok(())
    }

    pub fn delete_file(&mut self, filename: &str) -> Result<(), &'static str> {
        if self.read_only {
            return Err("Filesystem is mounted read-only");
        }
        let ino = self.lookup(filename).ok_or("File not found")?;

        let blk_idx = 2 + (ino / 32);
        let ino_off = ((ino % 32) * 128) as usize;
        let mut inode_block = [0u8; 4096];
        self.read_fs_block(blk_idx, &mut inode_block)?;

        let mut data_blks = [0u32; 12];
        for i in 0..12 {
            let ptr_off = ino_off + 0x2C + i * 4;
            data_blks[i] = u32::from_le_bytes(inode_block[ptr_off..ptr_off + 4].try_into().unwrap());
        }

        inode_block[ino_off..ino_off + 128].fill(0);
        self.write_fs_block(blk_idx, &inode_block)?;

        let mut bitmap = [0u8; 4096];
        self.read_fs_block(1, &mut bitmap)?;
        for &blk in data_blks.iter() {
            if blk >= 11 && blk < self.total_blocks {
                let byte_idx = (blk / 8) as usize;
                let bit_idx = (blk % 8) as usize;
                if byte_idx < 4096 {
                    bitmap[byte_idx] &= !(1 << bit_idx);
                }
            }
        }
        self.write_fs_block(1, &bitmap)?;

        let mut dir_block = [0u8; 4096];
        self.read_fs_block(10, &mut dir_block)?;
        let mut off = 0;
        while off + 32 <= 4096 {
            let d_ino = u32::from_le_bytes(dir_block[off..off + 4].try_into().unwrap());
            if d_ino == ino {
                dir_block[off..off + 32].fill(0);
                break;
            }
            off += 32;
        }
        self.write_fs_block(10, &dir_block)?;

        Ok(())
    }

    pub fn sync(&mut self) -> Result<(), &'static str> {
        if self.dev.flush() == 0 {
            Ok(())
        } else {
            Err("Failed to flush device")
        }
    }
}
