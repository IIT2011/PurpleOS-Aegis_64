//! Aegis64 Virtual File System (VFS)
//!
//! Provides a unified hierarchical namespace over:
//!   - `/`       -> PurpleFS v2 Persistent Storage
//!   - `/dev`    -> DevFS (character & pseudo devices: null, zero, console)
//!   - `/proc`   -> ProcFS (virtual system introspection: version, meminfo, uptime, cpuinfo, tasks)
//!   - `/tmp`    -> TmpFS (in-memory volatile RAM disk)
//!   - `/ssd`    -> Dedicated Solid State Drive block storage devices (/ssd/ssd1, /ssd/ssd2, ...)
//!   - `/hdd`    -> Dedicated Hard Disk Drive block storage devices (/hdd/hdd1, /hdd/hdd2, ...)
//!   - `/usb`    -> Dedicated USB 3.x Flash Drive block storage devices (/usb/usb1, /usb/usb2, ...)
//!   - `/sdxc`   -> Dedicated SDXC Card storage devices (/sdxc/card1, ...)
//!   - `/sdhc`   -> Dedicated SDHC Card storage devices (/sdhc/card1, ...)

use crate::sync::Spinlock;

pub const MAX_TMPFS_FILES: usize = 8;
pub const MAX_TMPFS_SIZE: usize = 4096;

struct TmpEntry {
    name: [u8; 32],
    data: [u8; MAX_TMPFS_SIZE],
    len: usize,
    in_use: bool,
}

struct TmpFs {
    entries: [TmpEntry; MAX_TMPFS_FILES],
}

impl TmpFs {
    const fn new() -> Self {
        Self {
            entries: [
                TmpEntry { name: [0; 32], data: [0; MAX_TMPFS_SIZE], len: 0, in_use: false },
                TmpEntry { name: [0; 32], data: [0; MAX_TMPFS_SIZE], len: 0, in_use: false },
                TmpEntry { name: [0; 32], data: [0; MAX_TMPFS_SIZE], len: 0, in_use: false },
                TmpEntry { name: [0; 32], data: [0; MAX_TMPFS_SIZE], len: 0, in_use: false },
                TmpEntry { name: [0; 32], data: [0; MAX_TMPFS_SIZE], len: 0, in_use: false },
                TmpEntry { name: [0; 32], data: [0; MAX_TMPFS_SIZE], len: 0, in_use: false },
                TmpEntry { name: [0; 32], data: [0; MAX_TMPFS_SIZE], len: 0, in_use: false },
                TmpEntry { name: [0; 32], data: [0; MAX_TMPFS_SIZE], len: 0, in_use: false },
            ],
        }
    }
}

static TMPFS: Spinlock<TmpFs> = Spinlock::new(TmpFs::new());

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum VfsMountType {
    RootPurpleFs,
    DevFs,
    ProcFs,
    TmpFs,
    SsdDev,   // /ssd/...
    HddDev,   // /hdd/...
    UsbDev,   // /usb/...
    SdxcDev,  // /sdxc/...
    SdhcDev,  // /sdhc/...
}

pub fn classify_path(path: &str) -> (VfsMountType, &str) {
    let p = path.trim_start_matches('/');
    if p.starts_with("dev/") || p == "dev" {
        (VfsMountType::DevFs, if p.len() > 4 { &p[4..] } else { "" })
    } else if p.starts_with("proc/") || p == "proc" {
        (VfsMountType::ProcFs, if p.len() > 5 { &p[5..] } else { "" })
    } else if p.starts_with("tmp/") || p == "tmp" {
        (VfsMountType::TmpFs, if p.len() > 4 { &p[4..] } else { "" })
    } else if p.starts_with("ssd/") || p == "ssd" {
        (VfsMountType::SsdDev, if p.len() > 4 { &p[4..] } else { "" })
    } else if p.starts_with("hdd/") || p == "hdd" {
        (VfsMountType::HddDev, if p.len() > 4 { &p[4..] } else { "" })
    } else if p.starts_with("usb/") || p == "usb" {
        (VfsMountType::UsbDev, if p.len() > 4 { &p[4..] } else { "" })
    } else if p.starts_with("sdxc/") || p == "sdxc" {
        (VfsMountType::SdxcDev, if p.len() > 5 { &p[5..] } else { "" })
    } else if p.starts_with("sdhc/") || p == "sdhc" {
        (VfsMountType::SdhcDev, if p.len() > 5 { &p[5..] } else { "" })
    } else {
        (VfsMountType::RootPurpleFs, p)
    }
}

pub fn read(path: &str, buf: &mut [u8]) -> Result<usize, &'static str> {
    let (mount, sub) = classify_path(path);
    match mount {
        VfsMountType::DevFs => read_devfs(sub, buf),
        VfsMountType::ProcFs => read_procfs(sub, buf),
        VfsMountType::TmpFs => read_tmpfs(sub, buf),
        VfsMountType::SsdDev => read_storage_node(path, buf),
        VfsMountType::HddDev => read_storage_node(path, buf),
        VfsMountType::UsbDev => read_storage_node(path, buf),
        VfsMountType::SdxcDev => read_storage_node(path, buf),
        VfsMountType::SdhcDev => read_storage_node(path, buf),
        VfsMountType::RootPurpleFs => {
            let res = crate::kernel::storage::with_fs(|fs| {
                if let Some(ino) = fs.lookup(sub) {
                    fs.read_file(ino, buf)
                } else {
                    0
                }
            });
            match res {
                Some(len) if len > 0 => Ok(len),
                _ => Err("File not found on filesystem"),
            }
        }
    }
}

pub fn write(path: &str, content: &[u8]) -> Result<usize, &'static str> {
    let (mount, sub) = classify_path(path);
    match mount {
        VfsMountType::DevFs => write_devfs(sub, content),
        VfsMountType::ProcFs => Err("ProcFS is a read-only virtual filesystem"),
        VfsMountType::TmpFs => write_tmpfs(sub, content),
        VfsMountType::SsdDev | VfsMountType::HddDev | VfsMountType::UsbDev |
        VfsMountType::SdxcDev | VfsMountType::SdhcDev => write_storage_node(path, content),
        VfsMountType::RootPurpleFs => {
            let res = crate::kernel::storage::with_fs(|fs| {
                fs.write_file(sub, content)
            });
            match res {
                Some(Ok(len)) => Ok(len),
                Some(Err(e)) => Err(e),
                None => Err("No persistent filesystem mounted"),
            }
        }
    }
}

pub fn delete(path: &str) -> Result<(), &'static str> {
    let (mount, sub) = classify_path(path);
    match mount {
        VfsMountType::TmpFs => {
            let mut tmp = TMPFS.lock();
            for entry in tmp.entries.iter_mut() {
                if entry.in_use {
                    let name_len = entry.name.iter().position(|&c| c == 0).unwrap_or(entry.name.len());
                    if let Ok(n) = core::str::from_utf8(&entry.name[..name_len]) {
                        if n == sub {
                            entry.in_use = false;
                            entry.len = 0;
                            return Ok(());
                        }
                    }
                }
            }
            Err("File not found in /tmp")
        }
        VfsMountType::RootPurpleFs => {
            let res = crate::kernel::storage::with_fs(|fs| {
                fs.delete_file(sub)
            });
            match res {
                Some(Ok(())) => Ok(()),
                Some(Err(e)) => Err(e),
                None => Err("No persistent filesystem mounted"),
            }
        }
        _ => Err("Cannot delete node on this virtual filesystem"),
    }
}

fn read_storage_node(path: &str, buf: &mut [u8]) -> Result<usize, &'static str> {
    if let Some(idx) = crate::drivers::block_dev::find_by_canonical_path(path) {
        let mut blk = [0u8; 512];
        let res = crate::drivers::block_dev::with_dev(idx, |dev| {
            dev.read_blocks(0, 1, &mut blk)
        });
        if res == Some(crate::drivers::block_dev::BLOCK_DEV_OK) {
            let len = buf.len().min(512);
            buf[..len].copy_from_slice(&blk[..len]);
            Ok(len)
        } else {
            Err("I/O error reading storage device sector 0")
        }
    } else {
        Err("Storage device not found")
    }
}

fn write_storage_node(path: &str, content: &[u8]) -> Result<usize, &'static str> {
    if let Some(idx) = crate::drivers::block_dev::find_by_canonical_path(path) {
        let mut blk = [0u8; 512];
        let len = content.len().min(512);
        blk[..len].copy_from_slice(&content[..len]);
        let res = crate::drivers::block_dev::with_dev(idx, |dev| {
            dev.write_blocks(0, 1, &blk)
        });
        if res == Some(crate::drivers::block_dev::BLOCK_DEV_OK) {
            Ok(len)
        } else {
            Err("I/O error writing storage device sector 0")
        }
    } else {
        Err("Storage device not found")
    }
}

fn read_devfs(name: &str, buf: &mut [u8]) -> Result<usize, &'static str> {
    match name {
        "null" => Ok(0),
        "zero" => {
            buf.fill(0);
            Ok(buf.len())
        }
        "console" => Ok(0),
        "storage" => {
            let msg = b"Aegis64 Unified Storage Hierarchy: /ssd, /hdd, /usb, /sdxc, /sdhc active\n";
            let len = msg.len().min(buf.len());
            buf[..len].copy_from_slice(&msg[..len]);
            Ok(len)
        }
        _ => Err("Device not found in /dev"),
    }
}

fn write_devfs(name: &str, content: &[u8]) -> Result<usize, &'static str> {
    match name {
        "null" | "zero" => Ok(content.len()),
        "console" => {
            if let Ok(s) = core::str::from_utf8(content) {
                crate::drivers::serial::print(s);
            }
            Ok(content.len())
        }
        _ => Err("Device is read-only or unsupported in /dev"),
    }
}

fn read_procfs(name: &str, buf: &mut [u8]) -> Result<usize, &'static str> {
    let mut written = 0usize;
    let mut write_str = |s: &str| {
        let b = s.as_bytes();
        let avail = buf.len().saturating_sub(written);
        let len = b.len().min(avail);
        buf[written..written + len].copy_from_slice(&b[..len]);
        written += len;
    };

    match name {
        "version" => {
            write_str("Purple OS / Aegis64 Kernel v0.5.0 (Rust Edition 2021, no_std, x86_64)\n");
            Ok(written)
        }
        "meminfo" => {
            let free = crate::mm::pmm::free_frames_count();
            let free_mib = free * 4 / 1024;
            write_str("MemTotal:        4194304 kB\n");
            write_str("MemFree:         ");
            let mut num_buf = [0u8; 16];
            let num_len = format_u64((free_mib * 1024) as u64, &mut num_buf);
            if let Ok(s) = core::str::from_utf8(&num_buf[..num_len]) {
                write_str(s);
            }
            write_str(" kB\n");
            Ok(written)
        }
        "uptime" => {
            let ticks = crate::drivers::pit::get_ticks();
            let mut num_buf = [0u8; 16];
            let num_len = format_u64(ticks / 100, &mut num_buf);
            if let Ok(s) = core::str::from_utf8(&num_buf[..num_len]) {
                write_str(s);
            }
            write_str(" seconds\n");
            Ok(written)
        }
        "cpuinfo" => {
            let info = crate::arch::x86_64::cpu::get_bsp_info();
            write_str("Vendor: ");
            let vendor = core::str::from_utf8(&info.vendor).unwrap_or("Unknown");
            write_str(vendor);
            write_str("\nCores:  ");
            let mut num_buf = [0u8; 16];
            let num_len = format_u64(crate::kernel::smp::get_online_count() as u64, &mut num_buf);
            if let Ok(s) = core::str::from_utf8(&num_buf[..num_len]) {
                write_str(s);
            }
            write_str("\n");
            Ok(written)
        }
        "tasks" => {
            write_str("PID  NAME       STATUS      CR3\n");
            write_str("------------------------------------\n");
            write_str("0    IdleTask   RUNNING     0x1000\n");
            write_str("1    AegisInit  READY       0x1000\n");
            Ok(written)
        }
        _ => Err("File not found in /proc"),
    }
}

fn read_tmpfs(name: &str, buf: &mut [u8]) -> Result<usize, &'static str> {
    let tmp = TMPFS.lock();
    for entry in tmp.entries.iter() {
        if entry.in_use {
            let name_len = entry.name.iter().position(|&c| c == 0).unwrap_or(entry.name.len());
            if let Ok(n) = core::str::from_utf8(&entry.name[..name_len]) {
                if n == name {
                    let len = entry.len.min(buf.len());
                    buf[..len].copy_from_slice(&entry.data[..len]);
                    return Ok(len);
                }
            }
        }
    }
    Err("File not found in /tmp")
}

fn write_tmpfs(name: &str, content: &[u8]) -> Result<usize, &'static str> {
    let mut tmp = TMPFS.lock();
    let mut slot = None;

    for (i, entry) in tmp.entries.iter_mut().enumerate() {
        if entry.in_use {
            let name_len = entry.name.iter().position(|&c| c == 0).unwrap_or(entry.name.len());
            if let Ok(n) = core::str::from_utf8(&entry.name[..name_len]) {
                if n == name {
                    slot = Some(i);
                    break;
                }
            }
        }
    }

    if slot.is_none() {
        for (i, entry) in tmp.entries.iter_mut().enumerate() {
            if !entry.in_use {
                slot = Some(i);
                break;
            }
        }
    }

    let idx = slot.ok_or("TmpFS full (maximum 8 files)")?;
    let entry = &mut tmp.entries[idx];
    entry.name.fill(0);
    let b = name.as_bytes();
    let len = b.len().min(31);
    entry.name[..len].copy_from_slice(&b[..len]);

    let write_len = content.len().min(MAX_TMPFS_SIZE);
    entry.data[..write_len].copy_from_slice(&content[..write_len]);
    entry.len = write_len;
    entry.in_use = true;

    Ok(write_len)
}

/// Enumerate directory entries across unified VFS namespaces
pub fn list_directory<F: FnMut(&str, usize, bool)>(path: &str, mut cb: F) -> Result<(), &'static str> {
    let (mount, sub) = classify_path(path);
    match mount {
        VfsMountType::DevFs => {
            cb("null", 0, false);
            cb("zero", 0, false);
            cb("console", 0, false);
            Ok(())
        }
        VfsMountType::ProcFs => {
            cb("version", 70, false);
            cb("meminfo", 128, false);
            cb("uptime", 32, false);
            cb("cpuinfo", 64, false);
            cb("tasks", 256, false);
            Ok(())
        }
        VfsMountType::TmpFs => {
            let tmp = TMPFS.lock();
            for entry in tmp.entries.iter() {
                if entry.in_use {
                    let name_len = entry.name.iter().position(|&c| c == 0).unwrap_or(entry.name.len());
                    if let Ok(n) = core::str::from_utf8(&entry.name[..name_len]) {
                        cb(n, entry.len, false);
                    }
                }
            }
            Ok(())
        }
        VfsMountType::SsdDev => {
            crate::drivers::block_dev::for_each_dev(|_idx, canon, dev| {
                if dev.category() == crate::drivers::block_dev::StorageCategory::Ssd {
                    let name = canon.trim_start_matches("/ssd/");
                    cb(name, (dev.total_blocks() * 512) as usize, false);
                }
            });
            Ok(())
        }
        VfsMountType::HddDev => {
            crate::drivers::block_dev::for_each_dev(|_idx, canon, dev| {
                if dev.category() == crate::drivers::block_dev::StorageCategory::Hdd {
                    let name = canon.trim_start_matches("/hdd/");
                    cb(name, (dev.total_blocks() * 512) as usize, false);
                }
            });
            Ok(())
        }
        VfsMountType::UsbDev => {
            crate::drivers::block_dev::for_each_dev(|_idx, canon, dev| {
                if dev.category() == crate::drivers::block_dev::StorageCategory::Usb {
                    let name = canon.trim_start_matches("/usb/");
                    cb(name, (dev.total_blocks() * 512) as usize, false);
                }
            });
            Ok(())
        }
        VfsMountType::SdxcDev => {
            crate::drivers::block_dev::for_each_dev(|_idx, canon, dev| {
                if dev.category() == crate::drivers::block_dev::StorageCategory::Sdxc {
                    let name = canon.trim_start_matches("/sdxc/");
                    cb(name, (dev.total_blocks() * 512) as usize, false);
                }
            });
            Ok(())
        }
        VfsMountType::SdhcDev => {
            crate::drivers::block_dev::for_each_dev(|_idx, canon, dev| {
                if dev.category() == crate::drivers::block_dev::StorageCategory::Sdhc {
                    let name = canon.trim_start_matches("/sdhc/");
                    cb(name, (dev.total_blocks() * 512) as usize, false);
                }
            });
            Ok(())
        }
        VfsMountType::RootPurpleFs => {
            // Root virtual mount folders
            cb("dev", 0, true);
            cb("proc", 0, true);
            cb("tmp", 0, true);
            cb("ssd", 0, true);
            cb("hdd", 0, true);
            cb("usb", 0, true);
            cb("sdxc", 0, true);
            cb("sdhc", 0, true);

            // PurpleFS persistent files
            crate::kernel::storage::with_fs(|fs| {
                fs.list_files(|_ino, ftype, fsize, name| {
                    if sub.is_empty() || name.starts_with(sub) {
                        cb(name, fsize as usize, ftype == 2);
                    }
                });
            });
            Ok(())
        }
    }
}

pub fn format_u64(mut n: u64, buf: &mut [u8]) -> usize {
    if n == 0 {
        if !buf.is_empty() {
            buf[0] = b'0';
            return 1;
        }
        return 0;
    }
    let mut temp = [0u8; 20];
    let mut i = 0;
    while n > 0 && i < 20 {
        temp[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    let len = i.min(buf.len());
    for j in 0..len {
        buf[j] = temp[i - 1 - j];
    }
    len
}
