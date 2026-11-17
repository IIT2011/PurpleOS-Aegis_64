use crate::sync::Spinlock;

pub const BLOCK_DEV_OK: i32 = 0;
pub const BLOCK_DEV_ERR_IO: i32 = -1;
pub const BLOCK_DEV_ERR_BOUNDS: i32 = -2;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum StorageCategory {
    Ssd,   // /ssd/ssd1, /ssd/ssd2... (NVMe, SATA SSD)
    Hdd,   // /hdd/hdd1, /hdd/hdd2... (ATA/IDE, SATA HDD)
    Usb,   // /usb/usb1, /usb/usb2... (USB 3.x flash drives)
    Sdxc,  // /sdxc/card1... (SDXC high-capacity cards)
    Sdhc,  // /sdhc/card1... (SDHC standard cards)
}

pub trait BlockDevice: Send + Sync {
    fn name(&self) -> &str;
    fn block_size(&self) -> u32;
    fn total_blocks(&self) -> u64;
    fn read_blocks(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> i32;
    fn write_blocks(&mut self, lba: u64, count: u32, buf: &[u8]) -> i32;
    fn flush(&mut self) -> i32 {
        BLOCK_DEV_OK
    }
    fn category(&self) -> StorageCategory {
        StorageCategory::Hdd
    }
}

pub struct BlockDevRegistry {
    devices: [Option<&'static mut dyn BlockDevice>; 16],
    canonical_paths: [[u8; 32]; 16],
    canonical_lens: [usize; 16],
    count: usize,
    ssd_count: usize,
    hdd_count: usize,
    usb_count: usize,
    sdxc_count: usize,
    sdhc_count: usize,
}

pub static REGISTRY: Spinlock<BlockDevRegistry> = Spinlock::new(BlockDevRegistry {
    devices: [None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None],
    canonical_paths: [[0; 32]; 16],
    canonical_lens: [0; 16],
    count: 0,
    ssd_count: 0,
    hdd_count: 0,
    usb_count: 0,
    sdxc_count: 0,
    sdhc_count: 0,
});

fn format_canon_path(prefix: &[u8], num: usize, out: &mut [u8]) -> usize {
    let mut pos = 0;
    for &b in prefix {
        if pos < out.len() {
            out[pos] = b;
            pos += 1;
        }
    }
    let mut temp = [0u8; 10];
    let mut t_idx = 0;
    let mut n = num;
    if n == 0 {
        if pos < out.len() {
            out[pos] = b'0';
            pos += 1;
        }
    } else {
        while n > 0 && t_idx < temp.len() {
            temp[t_idx] = b'0' + (n % 10) as u8;
            n /= 10;
            t_idx += 1;
        }
        while t_idx > 0 && pos < out.len() {
            t_idx -= 1;
            out[pos] = temp[t_idx];
            pos += 1;
        }
    }
    pos
}

pub fn register(dev: &'static mut dyn BlockDevice) {
    let mut reg = REGISTRY.lock();
    if reg.count < reg.devices.len() {
        let idx = reg.count;
        let dname = dev.name().trim_matches(char::from(0)).trim();
        if dname == "sata0" {
            crate::drivers::serial::println("BlockDev: Registered 'sata0' [AHCI/SATA]");
        } else if dname == "nvme0n1" {
            crate::drivers::serial::println("BlockDev: Registered 'nvme0n1' [NVMe PCIe]");
        } else {
            crate::drivers::serial::print("BlockDev: Registered '");
            crate::drivers::serial::print(dname);
            crate::drivers::serial::print("', Blocks=");
            crate::drivers::serial::print_u64(dev.total_blocks());
            crate::drivers::serial::println("");
        }

        // Assign dedicated storage hierarchy path (/ssd/ssd1, /hdd/hdd1, /usb/usb1, ...)
        let (prefix, num) = match dev.category() {
            StorageCategory::Ssd => {
                reg.ssd_count += 1;
                (b"/ssd/ssd" as &[u8], reg.ssd_count)
            }
            StorageCategory::Hdd => {
                reg.hdd_count += 1;
                (b"/hdd/hdd" as &[u8], reg.hdd_count)
            }
            StorageCategory::Usb => {
                reg.usb_count += 1;
                (b"/usb/usb" as &[u8], reg.usb_count)
            }
            StorageCategory::Sdxc => {
                reg.sdxc_count += 1;
                (b"/sdxc/card" as &[u8], reg.sdxc_count)
            }
            StorageCategory::Sdhc => {
                reg.sdhc_count += 1;
                (b"/sdhc/card" as &[u8], reg.sdhc_count)
            }
        };

        let canon_len = format_canon_path(prefix, num, &mut reg.canonical_paths[idx]);
        reg.canonical_lens[idx] = canon_len;

        if let Ok(canon_str) = core::str::from_utf8(&reg.canonical_paths[idx][..canon_len]) {
            crate::drivers::serial::print("  -> Assigned canonical path: ");
            crate::drivers::serial::println(canon_str);
        }

        reg.devices[idx] = Some(dev);
        reg.count += 1;
    }
}

pub fn count() -> usize {
    REGISTRY.lock().count
}

pub fn get_canonical_path(idx: usize) -> Option<&'static str> {
    let reg = REGISTRY.lock();
    if idx < reg.count {
        let len = reg.canonical_lens[idx];
        let ptr = reg.canonical_paths[idx].as_ptr();
        drop(reg);
        unsafe {
            let slice = core::slice::from_raw_parts(ptr, len);
            core::str::from_utf8(slice).ok()
        }
    } else {
        None
    }
}

pub fn find_by_canonical_path(path: &str) -> Option<usize> {
    let clean = path.trim();
    let norm = if !clean.starts_with('/') {
        clean
    } else {
        &clean[1..]
    };

    let reg = REGISTRY.lock();
    for i in 0..reg.count {
        let len = reg.canonical_lens[i];
        if let Ok(canon) = core::str::from_utf8(&reg.canonical_paths[i][..len]) {
            let canon_norm = canon.trim_start_matches('/');
            if canon == clean || canon_norm == norm {
                return Some(i);
            }
        }
        if let Some(ref d) = reg.devices[i] {
            if d.name() == clean || d.name() == norm {
                return Some(i);
            }
        }
    }
    None
}

pub fn with_dev<F, R>(idx: usize, f: F) -> Option<R>
where
    F: FnOnce(&mut dyn BlockDevice) -> R,
{
    let mut reg = REGISTRY.lock();
    if idx < reg.count {
        if let Some(ref mut d) = reg.devices[idx] {
            return Some(f(*d));
        }
    }
    None
}

pub fn for_each_dev<F: FnMut(usize, &'static str, &dyn BlockDevice)>(mut f: F) {
    let reg = REGISTRY.lock();
    for i in 0..reg.count {
        if let Some(ref d) = reg.devices[i] {
            let len = reg.canonical_lens[i];
            let canon = unsafe {
                let slice = core::slice::from_raw_parts(reg.canonical_paths[i].as_ptr(), len);
                core::str::from_utf8(slice).unwrap_or("/unknown")
            };
            f(i, canon, *d);
        }
    }
}
