//! Aegis64 Virtual File System (VFS)
//!
//! Provides a unified hierarchical namespace over:
//!   - `/`       -> PurpleFS v2 Persistent Storage
//!   - `/dev`    -> DevFS (null, zero, console, storage)
//!   - `/proc`   -> ProcFS (version, meminfo, uptime, cpuinfo, tasks)
//!   - `/tmp`    -> TmpFS (In-memory volatile RAM disk)

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

pub enum VfsMountType {
    RootPurpleFs,
    DevFs,
    ProcFs,
    TmpFs,
}

pub fn classify_path(path: &str) -> (VfsMountType, &str) {
    let p = path.trim_start_matches('/');
    if p.starts_with("dev/") {
        (VfsMountType::DevFs, &p[4..])
    } else if p.starts_with("proc/") {
        (VfsMountType::ProcFs, &p[5..])
    } else if p.starts_with("tmp/") {
        (VfsMountType::TmpFs, &p[4..])
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

fn read_devfs(name: &str, buf: &mut [u8]) -> Result<usize, &'static str> {
    match name {
        "null" => Ok(0),
        "zero" => {
            buf.fill(0);
            Ok(buf.len())
        }
        "console" => Ok(0),
        "storage" => {
            let msg = b"Aegis64 Unified Storage: ATA, AHCI, NVMe block devices active\n";
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

fn format_u64(mut n: u64, buf: &mut [u8]) -> usize {
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
