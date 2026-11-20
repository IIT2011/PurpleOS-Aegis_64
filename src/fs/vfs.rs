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

use crate::drivers::block_dev::BlockDevice;
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
                TmpEntry {
                    name: [0; 32],
                    data: [0; MAX_TMPFS_SIZE],
                    len: 0,
                    in_use: false,
                },
                TmpEntry {
                    name: [0; 32],
                    data: [0; MAX_TMPFS_SIZE],
                    len: 0,
                    in_use: false,
                },
                TmpEntry {
                    name: [0; 32],
                    data: [0; MAX_TMPFS_SIZE],
                    len: 0,
                    in_use: false,
                },
                TmpEntry {
                    name: [0; 32],
                    data: [0; MAX_TMPFS_SIZE],
                    len: 0,
                    in_use: false,
                },
                TmpEntry {
                    name: [0; 32],
                    data: [0; MAX_TMPFS_SIZE],
                    len: 0,
                    in_use: false,
                },
                TmpEntry {
                    name: [0; 32],
                    data: [0; MAX_TMPFS_SIZE],
                    len: 0,
                    in_use: false,
                },
                TmpEntry {
                    name: [0; 32],
                    data: [0; MAX_TMPFS_SIZE],
                    len: 0,
                    in_use: false,
                },
                TmpEntry {
                    name: [0; 32],
                    data: [0; MAX_TMPFS_SIZE],
                    len: 0,
                    in_use: false,
                },
            ],
        }
    }
}

static TMPFS: Spinlock<TmpFs> = Spinlock::new(TmpFs::new());

fn block_device_size(dev: &dyn BlockDevice) -> usize {
    dev.total_blocks()
        .saturating_mul(dev.block_size() as u64)
        .min(usize::MAX as u64) as usize
}

#[derive(Clone, Copy)]
pub struct VfsDir {
    pub in_use: bool,
    pub path: [u8; 64],
    pub len: usize,
}

pub static VFS_DIRS: Spinlock<[VfsDir; 32]> = Spinlock::new(
    [VfsDir {
        in_use: false,
        path: [0; 64],
        len: 0,
    }; 32],
);

pub fn is_directory(path: &str) -> bool {
    let p = path.trim_start_matches('/').trim_end_matches('/');
    if p.is_empty() {
        return true;
    }
    if p == crate::kernel::accounts::local().home(&mut [0; 28]).trim_start_matches('/') {
        return true;
    }
    match p {
        "dev" | "proc" | "tmp" => return true,
        _ => {}
    }
    match p {
        "Boot" | "Boot/EFI" | "Boot/EFI/Boot" => return true,
        "System"
        | "System/Kernel"
        | "System/Applications"
        | "System/Drivers"
        | "System/CLI"
        | "System/CLI/scripts"
        | "System/Frameworks"
        | "System/Services"
        | "System/Configuration"
        | "System/Resources"
        | "System/Resources/Fonts"
        | "System/Resources/Icons"
        | "System/Resources/Wallpapers"
        | "System/Libraries" => return true,
        "Users"
        | "Users/aegis"
        | "Users/aegis/Applications"
        | "Users/aegis/Desktop"
        | "Users/aegis/Documents"
        | "Users/Admin" => return true,
        "Recovery" => return true,
        _ => {}
    }
    {
        let dirs = VFS_DIRS.lock();
        for d in dirs.iter() {
            if d.in_use {
                if let Ok(s) = core::str::from_utf8(&d.path[..d.len]) {
                    if s == p {
                        return true;
                    }
                }
            }
        }
    }
    let mut is_dir = false;
    let _ = crate::kernel::storage::with_fs(|fs| {
        fs.list_files(|_ino, ftype, _fsize, name| {
            if name == p && ftype == 2 {
                is_dir = true;
            } else if name.starts_with(p) && name.as_bytes().get(p.len()) == Some(&b'/') {
                is_dir = true;
            }
        });
    });
    is_dir
}

pub fn mkdir(path: &str) -> Result<(), &'static str> {
    mkdir_owned(path, 0, 0)
}

pub(crate) fn mkdir_owned(path: &str, uid: u16, gid: u16) -> Result<(), &'static str> {
    let p = path.trim_start_matches('/').trim_end_matches('/');
    if p.is_empty() {
        return Err("Cannot create root directory");
    }
    if is_directory(p) {
        return Err("Directory already exists");
    }

    // 1. If persistent storage filesystem is mounted, create persistent directory in PurpleFS on disk!
    let disk_res = crate::kernel::storage::with_fs(|fs| fs.mkdir_owned(p, uid, gid));

    match disk_res {
        Some(Ok(_ino)) => {
            // Mirror to in-memory VFS_DIRS for fast lookup
            let mut dirs = VFS_DIRS.lock();
            for d in dirs.iter_mut() {
                if !d.in_use {
                    d.path.fill(0);
                    let b = p.as_bytes();
                    let len = b.len().min(63);
                    d.path[..len].copy_from_slice(&b[..len]);
                    d.len = len;
                    d.in_use = true;
                    break;
                }
            }
            Ok(())
        }
        Some(Err(err)) => {
            // If disk is read-only (e.g. before remount rw during tests), fall back to in-memory VFS_DIRS
            if err == "Filesystem is mounted read-only" {
                let mut dirs = VFS_DIRS.lock();
                for d in dirs.iter_mut() {
                    if !d.in_use {
                        d.path.fill(0);
                        let b = p.as_bytes();
                        let len = b.len().min(63);
                        d.path[..len].copy_from_slice(&b[..len]);
                        d.len = len;
                        d.in_use = true;
                        return Ok(());
                    }
                }
                Err("VFS directory table full (maximum 32 custom directories)")
            } else {
                Err(err)
            }
        }
        None => {
            // No persistent storage mounted, register in VFS_DIRS
            let mut dirs = VFS_DIRS.lock();
            for d in dirs.iter_mut() {
                if !d.in_use {
                    d.path.fill(0);
                    let b = p.as_bytes();
                    let len = b.len().min(63);
                    d.path[..len].copy_from_slice(&b[..len]);
                    d.len = len;
                    d.in_use = true;
                    return Ok(());
                }
            }
            Err("VFS directory table full (maximum 32 custom directories)")
        }
    }
}

pub fn rmdir(path: &str) -> Result<(), &'static str> {
    let p = path.trim_start_matches('/').trim_end_matches('/');
    if p.is_empty() {
        return Err("Cannot remove root directory");
    }
    match p {
        "dev" | "proc" | "tmp" | "Boot" | "System" | "Users" | "Recovery" => {
            return Err("Cannot remove system mountpoint");
        }
        _ => {}
    }

    // Clean up from in-memory VFS_DIRS if present
    let mut found_in_memory = false;
    {
        let mut dirs = VFS_DIRS.lock();
        for d in dirs.iter_mut() {
            if d.in_use {
                if let Ok(s) = core::str::from_utf8(&d.path[..d.len]) {
                    if s == p {
                        d.in_use = false;
                        found_in_memory = true;
                    }
                }
            }
        }
    }

    // Remove from persistent PurpleFS on disk
    let disk_res = crate::kernel::storage::with_fs(|fs| fs.rmdir(p));

    match disk_res {
        Some(Ok(())) => Ok(()),
        Some(Err(err)) => {
            if found_in_memory {
                Ok(())
            } else {
                Err(err)
            }
        }
        None => {
            if found_in_memory {
                Ok(())
            } else {
                Err("Directory not found")
            }
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum VfsMountType {
    RootPurpleFs,
    DevFs,
    ProcFs,
    TmpFs,
    SsdDev,  // /ssd/...
    HddDev,  // /hdd/...
    UsbDev,  // /usb/...
    SdxcDev, // /sdxc/...
    SdhcDev, // /sdhc/...
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
        (
            VfsMountType::SdxcDev,
            if p.len() > 5 { &p[5..] } else { "" },
        )
    } else if p.starts_with("sdhc/") || p == "sdhc" {
        (
            VfsMountType::SdhcDev,
            if p.len() > 5 { &p[5..] } else { "" },
        )
    } else {
        (VfsMountType::RootPurpleFs, p)
    }
}

pub fn file_size(path: &str) -> Result<usize, &'static str> {
    let (mount, sub) = classify_path(path);
    match mount {
        VfsMountType::DevFs => {
            if sub == "null" || sub == "zero" || sub == "console" {
                Ok(0)
            } else if sub == "compositor" {
                Ok(256)
            } else if sub == "fb0" {
                let v = crate::drivers::video::VIDEO.lock();
                Ok(v.pitch.saturating_mul(v.height).saturating_mul(v.bpp / 8))
            } else {
                let mut sz = 0;
                let mut found = false;
                crate::drivers::block_dev::for_each_dev(|_idx, canon, dev| {
                    let dname = canon.trim_start_matches('/');
                    let base = dname.rsplit('/').next().unwrap_or(dname);
                    if base == sub {
                        sz = block_device_size(dev);
                        found = true;
                    }
                });
                if found {
                    Ok(sz)
                } else {
                    Err("Device not found in /dev")
                }
            }
        }
        VfsMountType::ProcFs => {
            let mut buf = [0u8; 1024];
            read_procfs(sub, &mut buf)
        }
        VfsMountType::TmpFs => {
            let tmp = TMPFS.lock();
            for entry in tmp.entries.iter() {
                if entry.in_use {
                    let name_len = entry
                        .name
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.name.len());
                    if let Ok(n) = core::str::from_utf8(&entry.name[..name_len]) {
                        if n == sub {
                            return Ok(entry.len);
                        }
                    }
                }
            }
            Err("File not found in /tmp")
        }
        VfsMountType::SsdDev
        | VfsMountType::HddDev
        | VfsMountType::UsbDev
        | VfsMountType::SdxcDev
        | VfsMountType::SdhcDev => {
            if let Some(idx) = crate::drivers::block_dev::find_by_canonical_path(path) {
                let mut sz = 0;
                let _ = crate::drivers::block_dev::with_dev(idx, |dev| {
                    sz = block_device_size(dev);
                });
                Ok(sz)
            } else {
                Err("Block device not found")
            }
        }
        VfsMountType::RootPurpleFs => {
            let res = crate::kernel::storage::with_fs(|fs| {
                if let Some(ino) = fs.lookup(sub) {
                    fs.get_file_size(ino)
                } else if let Some(slash_pos) = sub.rfind('/') {
                    let base_name = &sub[slash_pos + 1..];
                    if let Some(ino) = fs.lookup(base_name) {
                        fs.get_file_size(ino)
                    } else {
                        None
                    }
                } else {
                    let mut found_ino = None;
                    fs.list_files(|ino, _ftype, _fsize, name| {
                        if found_ino.is_some() {
                            return;
                        }
                        if name == sub
                            || (name.ends_with(sub)
                                && name
                                    .as_bytes()
                                    .get(name.len().saturating_sub(sub.len() + 1))
                                    == Some(&b'/'))
                        {
                            found_ino = Some(ino);
                        }
                    });
                    if let Some(ino) = found_ino {
                        fs.get_file_size(ino)
                    } else {
                        None
                    }
                }
            });
            match res {
                Some(Some(len)) => Ok(len),
                _ => Err("File not found on filesystem"),
            }
        }
    }
}

pub fn read(path: &str, buf: &mut [u8]) -> Result<usize, &'static str> {
    read_at(path, 0, buf)
}

pub fn read_at(path: &str, offset: usize, buf: &mut [u8]) -> Result<usize, &'static str> {
    if buf.is_empty() {
        return Ok(0);
    }
    let (mount, sub) = classify_path(path);
    match mount {
        VfsMountType::DevFs => read_devfs(sub, buf),
        VfsMountType::ProcFs => {
            let mut scratch = [0u8; 1024];
            let total = read_procfs(sub, &mut scratch)?;
            if offset >= total {
                return Ok(0);
            }
            let avail = total - offset;
            let to_copy = avail.min(buf.len());
            buf[..to_copy].copy_from_slice(&scratch[offset..offset + to_copy]);
            Ok(to_copy)
        }
        VfsMountType::TmpFs => {
            let tmp = TMPFS.lock();
            for entry in tmp.entries.iter() {
                if entry.in_use {
                    let name_len = entry
                        .name
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.name.len());
                    if let Ok(n) = core::str::from_utf8(&entry.name[..name_len]) {
                        if n == sub {
                            if offset >= entry.len {
                                return Ok(0);
                            }
                            let avail = entry.len - offset;
                            let to_copy = avail.min(buf.len());
                            buf[..to_copy].copy_from_slice(&entry.data[offset..offset + to_copy]);
                            return Ok(to_copy);
                        }
                    }
                }
            }
            Err("File not found in /tmp")
        }
        VfsMountType::SsdDev
        | VfsMountType::HddDev
        | VfsMountType::UsbDev
        | VfsMountType::SdxcDev
        | VfsMountType::SdhcDev => read_storage_node(path, buf),
        VfsMountType::RootPurpleFs => {
            let res = crate::kernel::storage::with_fs(|fs| {
                if let Some(ino) = fs.lookup(sub) {
                    fs.read_file_at(ino, offset, buf)
                } else if let Some(slash_pos) = sub.rfind('/') {
                    let base_name = &sub[slash_pos + 1..];
                    if let Some(ino) = fs.lookup(base_name) {
                        fs.read_file_at(ino, offset, buf)
                    } else {
                        0
                    }
                } else {
                    let mut found_ino = None;
                    fs.list_files(|ino, _ftype, _fsize, name| {
                        if found_ino.is_some() {
                            return;
                        }
                        if name == sub
                            || (name.ends_with(sub)
                                && name
                                    .as_bytes()
                                    .get(name.len().saturating_sub(sub.len() + 1))
                                    == Some(&b'/'))
                        {
                            found_ino = Some(ino);
                        }
                    });
                    if let Some(ino) = found_ino {
                        fs.read_file_at(ino, offset, buf)
                    } else {
                        0
                    }
                }
            });
            match res {
                Some(len) if len > 0 => Ok(len),
                Some(_) => {
                    let exists = crate::kernel::storage::with_fs(|fs| {
                        if fs.lookup(sub).is_some() {
                            return true;
                        }
                        if let Some(p) = sub.rfind('/') {
                            if fs.lookup(&sub[p + 1..]).is_some() {
                                return true;
                            }
                        }
                        let mut found = false;
                        fs.list_files(|_ino, _ftype, _fsize, name| {
                            if found {
                                return;
                            }
                            if name == sub
                                || (name.ends_with(sub)
                                    && name
                                        .as_bytes()
                                        .get(name.len().saturating_sub(sub.len() + 1))
                                        == Some(&b'/'))
                            {
                                found = true;
                            }
                        });
                        found
                    })
                    .unwrap_or(false);
                    if exists {
                        Ok(0)
                    } else {
                        Err("File not found on filesystem")
                    }
                }
                None => Err("No persistent filesystem mounted"),
            }
        }
    }
}

pub fn write(path: &str, content: &[u8]) -> Result<usize, &'static str> {
    write_at(path, 0, content)
}

pub fn write_at(path: &str, offset: usize, content: &[u8]) -> Result<usize, &'static str> {
    let (mount, sub) = classify_path(path);
    match mount {
        VfsMountType::DevFs => write_devfs(sub, content),
        VfsMountType::ProcFs => Err("ProcFS is a read-only virtual filesystem"),
        VfsMountType::TmpFs => write_tmpfs_at(sub, offset, content),
        VfsMountType::SsdDev
        | VfsMountType::HddDev
        | VfsMountType::UsbDev
        | VfsMountType::SdxcDev
        | VfsMountType::SdhcDev => write_storage_node(path, content),
        VfsMountType::RootPurpleFs => {
            let res = crate::kernel::storage::with_fs(|fs| fs.write_file_at(sub, offset, content));
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
                    let name_len = entry
                        .name
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.name.len());
                    if let Ok(n) = core::str::from_utf8(&entry.name[..name_len]) {
                        if n == sub {
                            entry.in_use = false;
                            entry.name.fill(0);
                            entry.data.fill(0);
                            entry.len = 0;
                            return Ok(());
                        }
                    }
                }
            }
            Err("File not found in /tmp")
        }
        VfsMountType::RootPurpleFs => {
            let res = crate::kernel::storage::with_fs(|fs| fs.delete_file(sub));
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
    let idx = crate::drivers::block_dev::find_by_canonical_path(path)
        .ok_or("Storage device not found")?;
    let mut block = [0u8; crate::mm::pmm::PAGE_SIZE];
    crate::drivers::block_dev::with_dev(idx, |dev| {
        let sector_size = dev.block_size() as usize;
        if !(1..=block.len()).contains(&sector_size) {
            return Err("Unsupported storage logical block size");
        }
        if dev.read_blocks(0, 1, &mut block[..sector_size])
            != crate::drivers::block_dev::BLOCK_DEV_OK
        {
            return Err("I/O error reading storage device sector 0");
        }
        let len = buf.len().min(sector_size);
        buf[..len].copy_from_slice(&block[..len]);
        Ok(len)
    })
    .ok_or("Storage device not found")?
}

pub(crate) fn is_storage_path(path: &str) -> bool {
    matches!(
        classify_path(path).0,
        VfsMountType::SsdDev
            | VfsMountType::HddDev
            | VfsMountType::UsbDev
            | VfsMountType::SdxcDev
            | VfsMountType::SdhcDev
    )
}

fn write_storage_node(_path: &str, _content: &[u8]) -> Result<usize, &'static str> {
    Err("Raw block device nodes are read-only; use authorized block I/O")
}

fn read_devfs(name: &str, buf: &mut [u8]) -> Result<usize, &'static str> {
    match name {
        "null" => Ok(0),
        "zero" => {
            buf.fill(0);
            Ok(buf.len())
        }
        "console" => Ok(0),
        "compositor" => {
            let len = crate::compositor::format_status(buf);
            Ok(len)
        }
        "fb0" => {
            let (w, h, bpp, pitch) = {
                let v = crate::drivers::video::VIDEO.lock();
                (v.width, v.height, v.bpp, v.pitch)
            };
            let mut info_buf = [0u8; 128];
            let mut pos = 0;
            let append_str = |dest: &mut [u8], pos: &mut usize, s: &str| {
                for b in s.bytes() {
                    if *pos < dest.len() {
                        dest[*pos] = b;
                        *pos += 1;
                    }
                }
            };
            let append_u64 = |dest: &mut [u8], pos: &mut usize, mut val: u64| {
                if val == 0 {
                    if *pos < dest.len() {
                        dest[*pos] = b'0';
                        *pos += 1;
                    }
                    return;
                }
                let mut tmp = [0u8; 20];
                let mut i = 0;
                while val > 0 && i < 20 {
                    tmp[i] = b'0' + (val % 10) as u8;
                    val /= 10;
                    i += 1;
                }
                for j in 0..i {
                    if *pos < dest.len() {
                        dest[*pos] = tmp[i - 1 - j];
                        *pos += 1;
                    }
                }
            };
            append_str(&mut info_buf, &mut pos, "fb0: ");
            append_u64(&mut info_buf, &mut pos, w as u64);
            append_str(&mut info_buf, &mut pos, "x");
            append_u64(&mut info_buf, &mut pos, h as u64);
            append_str(&mut info_buf, &mut pos, "x");
            append_u64(&mut info_buf, &mut pos, bpp as u64);
            append_str(&mut info_buf, &mut pos, ", pitch=");
            append_u64(&mut info_buf, &mut pos, pitch as u64);
            append_str(&mut info_buf, &mut pos, ", format=");
            append_u64(&mut info_buf, &mut pos, bpp as u64);
            append_str(&mut info_buf, &mut pos, "bpp\n");
            let len = pos.min(buf.len());
            buf[..len].copy_from_slice(&info_buf[..len]);
            Ok(len)
        }
        "storage" => {
            let msg = b"Aegis64 Unified Device VFS: /dev/null, /dev/zero, /dev/console, dynamic block devices online\n";
            let len = msg.len().min(buf.len());
            buf[..len].copy_from_slice(&msg[..len]);
            Ok(len)
        }
        _ => {
            let mut found_idx = None;
            crate::drivers::block_dev::for_each_dev(|idx, canon, _dev| {
                let dname = canon.trim_start_matches('/');
                let base = dname.rsplit('/').next().unwrap_or(dname);
                if base == name {
                    found_idx = Some(idx);
                }
            });
            if let Some(idx) = found_idx {
                let mut blk = [0u8; 512];
                let res =
                    crate::drivers::block_dev::with_dev(idx, |dev| dev.read_blocks(0, 1, &mut blk));
                if res == Some(crate::drivers::block_dev::BLOCK_DEV_OK) {
                    let len = buf.len().min(512);
                    buf[..len].copy_from_slice(&blk[..len]);
                    Ok(len)
                } else {
                    Err("I/O error reading block device")
                }
            } else {
                Err("Device not found in /dev")
            }
        }
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
        "compositor" => {
            if let Ok(cmd_str) = core::str::from_utf8(content) {
                let trimmed = cmd_str.trim();
                let mut parts = trimmed.split_whitespace();
                match parts.next().unwrap_or("") {
                    "start" | "run" => crate::compositor::start(),
                    "stop" => crate::compositor::stop(),
                    "demo" => {
                        crate::compositor::start();
                        crate::compositor::setup_demo_windows();
                        crate::compositor::render_frame();
                    }
                    "render" | "frame" => crate::compositor::render_frame(),
                    _ => {}
                }
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
            let total = crate::mm::pmm::total_frames_count();
            let total_kb = (total * 4) as u64;
            let free = crate::mm::pmm::free_frames_count();
            let free_kb = (free * 4) as u64;
            write_str("MemTotal:        ");
            let mut num_buf = [0u8; 16];
            let num_len = format_u64(total_kb, &mut num_buf);
            if let Ok(s) = core::str::from_utf8(&num_buf[..num_len]) {
                write_str(s);
            }
            write_str(" kB\nMemFree:         ");
            let num_len = format_u64(free_kb, &mut num_buf);
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
            write_str("PID   NAME             STATUS      CPU  CR3\n");
            write_str("----------------------------------------------------\n");
            let s = crate::kernel::task::SCHED.lock();
            for i in 0..crate::kernel::task::MAX_TASKS {
                let t = &s.tasks[i];
                if t.state != crate::kernel::task::TaskState::Unused {
                    let mut num_buf = [0u8; 16];
                    let nlen = format_u64(t.id as u64, &mut num_buf);
                    if let Ok(ns) = core::str::from_utf8(&num_buf[..nlen]) {
                        write_str(ns);
                        for _ in ns.len()..6 {
                            write_str(" ");
                        }
                    }
                    let name_len = t.name.iter().position(|&c| c == 0).unwrap_or(t.name.len());
                    if let Ok(name_str) = core::str::from_utf8(&t.name[..name_len]) {
                        write_str(name_str);
                        for _ in name_str.len()..17 {
                            write_str(" ");
                        }
                    } else {
                        write_str("unknown          ");
                    }
                    if t.is_zombie {
                        write_str("ZOMBIE      ");
                    } else {
                        match t.state {
                            crate::kernel::task::TaskState::Running => write_str("RUNNING     "),
                            crate::kernel::task::TaskState::Ready => write_str("READY       "),
                            crate::kernel::task::TaskState::Blocked => write_str("BLOCKED     "),
                            crate::kernel::task::TaskState::Sleeping => write_str("SLEEPING    "),
                            crate::kernel::task::TaskState::Terminated => write_str("TERMINATED  "),
                            _ => write_str("UNKNOWN     "),
                        }
                    }
                    let clen = format_u64(t.last_cpu as u64, &mut num_buf);
                    if let Ok(cs) = core::str::from_utf8(&num_buf[..clen]) {
                        write_str(cs);
                        for _ in cs.len()..5 {
                            write_str(" ");
                        }
                    }
                    write_str("0x");
                    let mut hex_buf = [0u8; 16];
                    let hlen = format_hex(t.cr3, &mut hex_buf);
                    if let Ok(hs) = core::str::from_utf8(&hex_buf[..hlen]) {
                        write_str(hs);
                    }
                    write_str("\n");
                }
            }
            Ok(written)
        }
        _ => Err("File not found in /proc"),
    }
}

fn write_tmpfs_at(name: &str, offset: usize, content: &[u8]) -> Result<usize, &'static str> {
    write_tmpfs(name, Some(offset), false, content)
}

/// GUI redirection writes only volatile regular files, with atomic append or replacement.
pub(crate) fn redirect_tmpfs(
    path: &str,
    append: bool,
    content: &[u8],
) -> Result<usize, &'static str> {
    let name = path
        .strip_prefix("/tmp/")
        .ok_or("GUI redirection writes only /tmp files.")?;
    write_tmpfs(name, if append { None } else { Some(0) }, true, content)
}

fn write_tmpfs(
    name: &str,
    offset: Option<usize>,
    redirect: bool,
    content: &[u8],
) -> Result<usize, &'static str> {
    if name.is_empty() || name.len() > 31 || name.contains('/') || name.as_bytes().contains(&0) {
        return Err("Invalid TmpFS file name");
    }
    if !redirect && offset.is_some_and(|offset| offset >= MAX_TMPFS_SIZE) {
        return Err("Write offset exceeds maximum TmpFS file size");
    }
    let mut tmp = TMPFS.lock();
    let mut slot = None;

    for (i, entry) in tmp.entries.iter_mut().enumerate() {
        if entry.in_use {
            let name_len = entry
                .name
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.name.len());
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
    let start = offset.unwrap_or(entry.len);
    if redirect && content.len() > MAX_TMPFS_SIZE - start {
        return Err("Redirection exceeds maximum TmpFS file size.");
    }
    if !entry.in_use {
        entry.name.fill(0);
        let b = name.as_bytes();
        let len = b.len().min(31);
        entry.name[..len].copy_from_slice(&b[..len]);
        entry.data.fill(0);
        entry.len = 0;
        entry.in_use = true;
    }

    let write_len = content.len().min(MAX_TMPFS_SIZE - start);
    entry.data[start..start + write_len].copy_from_slice(&content[..write_len]);
    entry.len = if redirect {
        start + write_len
    } else {
        entry.len.max(start + write_len)
    };

    Ok(write_len)
}

/// Enumerate directory entries across unified VFS namespaces
pub fn list_directory<F: FnMut(&str, usize, bool)>(
    path: &str,
    mut cb: F,
) -> Result<(), &'static str> {
    let (mount, sub) = classify_path(path);
    match mount {
        VfsMountType::DevFs => {
            cb("null", 0, false);
            cb("zero", 0, false);
            cb("console", 0, false);
            cb("compositor", 256, false);
            if let Some((_width, height, pitch, bits_per_pixel, _address)) =
                crate::drivers::video::get_fb_info()
            {
                let bytes = pitch
                    .saturating_mul(height)
                    .saturating_mul(bits_per_pixel / 8);
                cb("fb0", bytes, false);
            }
            crate::drivers::block_dev::for_each_dev(|_idx, canon, dev| {
                let name = canon.trim_start_matches('/');
                let dev_name = name.rsplit('/').next().unwrap_or(name);
                let bytes = dev.total_blocks().saturating_mul(dev.block_size() as u64);
                cb(dev_name, bytes.min(usize::MAX as u64) as usize, false);
            });
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
                    let name_len = entry
                        .name
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.name.len());
                    if let Ok(n) = core::str::from_utf8(&entry.name[..name_len]) {
                        cb(n, entry.len, false);
                    }
                }
            }
            Ok(())
        }
        VfsMountType::SsdDev
        | VfsMountType::HddDev
        | VfsMountType::UsbDev
        | VfsMountType::SdxcDev
        | VfsMountType::SdhcDev => Ok(()),
        VfsMountType::RootPurpleFs => {
            let clean_sub = sub.trim_start_matches('/').trim_end_matches('/');
            if clean_sub.is_empty() {
                // 1. Root virtual mounts (DevFS, ProcFS, TmpFS)
                cb("dev", 0, true);
                cb("proc", 0, true);
                cb("tmp", 0, true);

                // 2. Canonical PurpleOS root architecture directories
                cb("Boot", 0, true);
                cb("System", 0, true);
                cb("Users", 0, true);
                cb("Recovery", 0, true);

                // 3. User-created directories in VFS_DIRS (if not already on disk)
                {
                    let dirs = VFS_DIRS.lock();
                    for d in dirs.iter() {
                        if d.in_use {
                            if let Ok(s) = core::str::from_utf8(&d.path[..d.len]) {
                                if !s.contains('/') {
                                    if s != "dev"
                                        && s != "proc"
                                        && s != "tmp"
                                        && s != "Boot"
                                        && s != "System"
                                        && s != "Users"
                                        && s != "Recovery"
                                    {
                                        let on_disk = crate::kernel::storage::with_fs(|fs| {
                                            fs.lookup(s).is_some()
                                        })
                                        .unwrap_or(false);
                                        if !on_disk {
                                            cb(s, 0, true);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // 4. Root persistent files from storage (do NOT list items containing '/')
                crate::kernel::storage::with_fs(|fs| {
                    fs.list_files(|_ino, ftype, fsize, name| {
                        if !name.contains('/') {
                            if name != "dev"
                                && name != "proc"
                                && name != "tmp"
                                && name != "Boot"
                                && name != "System"
                                && name != "Users"
                                && name != "Recovery"
                            {
                                cb(name, fsize as usize, ftype == 2);
                            }
                        }
                    });
                });
                Ok(())
            } else if is_directory(path) {
                // Canonical subdirectories
                match clean_sub {
                    "System" => {
                        cb("Applications", 0, true);
                        cb("CLI", 0, true);
                        cb("Configuration", 0, true);
                        cb("Drivers", 0, true);
                        cb("Frameworks", 0, true);
                        cb("Kernel", 0, true);
                        cb("Libraries", 0, true);
                        cb("Resources", 0, true);
                        cb("Services", 0, true);
                    }
                    "System/Resources" => {
                        cb("Fonts", 0, true);
                        cb("Icons", 0, true);
                        cb("Wallpapers", 0, true);
                    }
                    "Users" => {
                        cb("Admin", 0, true);
                        cb("aegis", 0, true);
                    }
                    "Users/aegis" => {
                        cb("Applications", 0, true);
                        cb("Desktop", 0, true);
                        cb("Documents", 0, true);
                    }
                    "Boot" => {
                        cb("EFI", 0, true);
                    }
                    "Boot/EFI" => {
                        cb("Boot", 0, true);
                    }
                    _ => {}
                }

                // Custom subdirectories in VFS_DIRS (if not already on disk)
                {
                    let dirs = VFS_DIRS.lock();
                    for d in dirs.iter() {
                        if d.in_use {
                            if let Ok(s) = core::str::from_utf8(&d.path[..d.len]) {
                                if s.starts_with(clean_sub)
                                    && s.as_bytes().get(clean_sub.len()) == Some(&b'/')
                                {
                                    let rel = &s[clean_sub.len() + 1..];
                                    if !rel.is_empty() && !rel.contains('/') {
                                        let on_disk = crate::kernel::storage::with_fs(|fs| {
                                            fs.lookup(s).is_some()
                                        })
                                        .unwrap_or(false);
                                        if !on_disk {
                                            cb(rel, 0, true);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Storage files matching this directory prefix
                crate::kernel::storage::with_fs(|fs| {
                    fs.list_files(|_ino, ftype, fsize, name| {
                        if name.starts_with(clean_sub)
                            && name.as_bytes().get(clean_sub.len()) == Some(&b'/')
                        {
                            let rel = &name[clean_sub.len() + 1..];
                            if let Some(slash_idx) = rel.find('/') {
                                let sub_dir = &rel[..slash_idx];
                                if clean_sub == "System"
                                    && matches!(
                                        sub_dir,
                                        "Applications"
                                            | "CLI"
                                            | "Configuration"
                                            | "Drivers"
                                            | "Frameworks"
                                            | "Kernel"
                                            | "Libraries"
                                            | "Resources"
                                            | "Services"
                                    )
                                {
                                    return;
                                }
                                if clean_sub == "System/Resources"
                                    && matches!(sub_dir, "Fonts" | "Icons" | "Wallpapers")
                                {
                                    return;
                                }
                                cb(sub_dir, 0, true);
                            } else {
                                cb(rel, fsize as usize, ftype == 2);
                            }
                        }
                    });
                });
                Ok(())
            } else {
                Err("No such file or directory")
            }
        }
    }
}

pub fn format_hex(val: u64, buf: &mut [u8]) -> usize {
    const HEX: &[u8] = b"0123456789abcdef";
    if val == 0 {
        if !buf.is_empty() {
            buf[0] = b'0';
            return 1;
        }
        return 0;
    }
    let mut temp = [0u8; 16];
    let mut i = 0;
    let mut v = val;
    while v > 0 && i < 16 {
        temp[i] = HEX[(v & 0xF) as usize];
        v >>= 4;
        i += 1;
    }
    let len = i.min(buf.len());
    for j in 0..len {
        buf[j] = temp[i - 1 - j];
    }
    len
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
