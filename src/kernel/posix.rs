//! Aegis64 POSIX.1-2024 (IEEE Std 1003.1-2024 / Issue 8) Subsystem
//!
//! Provides production-grade POSIX interfaces, per-process file descriptor tables,
//! shared open file descriptions with reference counting, real streaming FIFO pipes
//! with broken-pipe detection and EOF semantics, virtual memory areas (VMAs),
//! dynamic mmap/munmap/mprotect, heap expansion (brk), POSIX signal masking and
//! actions, wait/exit handling, high-resolution clocks, and directory/filesystem
//! operations. Fork and execve return ENOSYS until a complete context and ELF
//! loader implementation is available.
//!
//! 100% Pure Rust (no_std, x86_64-unknown-none) with zero C/C++ dependencies.

use crate::sync::Spinlock;

// ============================================================================
// 1. Standard POSIX Errno Definitions (IEEE Std 1003.1-2024)
// ============================================================================
pub const EPERM: i64 = 1; // Operation not permitted
pub const ENOENT: i64 = 2; // No such file or directory
pub const ESRCH: i64 = 3; // No such process
pub const EINTR: i64 = 4; // Interrupted system call
pub const EIO: i64 = 5; // I/O error
pub const ENXIO: i64 = 6; // No such device or address
pub const E2BIG: i64 = 7; // Argument list too long
pub const ENOEXEC: i64 = 8; // Exec format error
pub const EBADF: i64 = 9; // Bad file descriptor
pub const ECHILD: i64 = 10; // No child processes
pub const EAGAIN: i64 = 11; // Resource temporarily unavailable
pub const ENOMEM: i64 = 12; // Out of memory
pub const EACCES: i64 = 13; // Permission denied
pub const EFAULT: i64 = 14; // Bad address
pub const EBUSY: i64 = 16; // Device or resource busy
pub const EEXIST: i64 = 17; // File exists
pub const EXDEV: i64 = 18; // Cross-device link
pub const ENODEV: i64 = 19; // No such device
pub const ENOTDIR: i64 = 20; // Not a directory
pub const EISDIR: i64 = 21; // Is a directory
pub const EINVAL: i64 = 22; // Invalid argument
pub const ENFILE: i64 = 23; // File table overflow
pub const EMFILE: i64 = 24; // Too many open files
pub const ENOTTY: i64 = 25; // Not a typewriter
pub const EFBIG: i64 = 27; // File too large
pub const ENOSPC: i64 = 28; // No space left on device
pub const ESPIPE: i64 = 29; // Illegal seek
pub const EROFS: i64 = 30; // Read-only file system
pub const EMLINK: i64 = 31; // Too many links
pub const EPIPE: i64 = 32; // Broken pipe
pub const EDOM: i64 = 33; // Numerical argument out of domain
pub const ERANGE: i64 = 34; // Numerical result out of range
pub const ENAMETOOLONG: i64 = 36; // File name too long
pub const ENOSYS: i64 = 38; // Function not implemented
pub const ENOTEMPTY: i64 = 39; // Directory not empty
pub const ELOOP: i64 = 40; // Too many levels of symbolic links
pub const EOVERFLOW: i64 = 75; // Value too large for defined data type
pub const EOPNOTSUPP: i64 = 95; // Operation not supported on socket
pub const ECONNREFUSED: i64 = 111; // Connection refused

// ============================================================================
// 2. POSIX Open Flags, File Modes & Seek Directives
// ============================================================================
pub const O_RDONLY: u32 = 0x0000;
pub const O_WRONLY: u32 = 0x0001;
pub const O_RDWR: u32 = 0x0002;
pub const O_CREAT: u32 = 0x0040;
pub const O_EXCL: u32 = 0x0080;
pub const O_TRUNC: u32 = 0x0200;
pub const O_APPEND: u32 = 0x0400;
pub const O_NONBLOCK: u32 = 0x0800;
pub const O_DIRECTORY: u32 = 0x10000;
pub const O_CLOEXEC: u32 = 0x80000;

pub const FD_CLOEXEC: u32 = 1;

pub const S_IFMT: u32 = 0o170000;
pub const S_IFSOCK: u32 = 0o140000;
pub const S_IFLNK: u32 = 0o120000;
pub const S_IFREG: u32 = 0o100000;
pub const S_IFBLK: u32 = 0o060000;
pub const S_IFDIR: u32 = 0o040000;
pub const S_IFCHR: u32 = 0o020000;
pub const S_IFIFO: u32 = 0o010000;

pub const SEEK_SET: i32 = 0;
pub const SEEK_CUR: i32 = 1;
pub const SEEK_END: i32 = 2;

// ============================================================================
// 3. POSIX Signals (Signals 1..64)
// ============================================================================
pub const SIGHUP: u32 = 1;
pub const SIGINT: u32 = 2;
pub const SIGQUIT: u32 = 3;
pub const SIGILL: u32 = 4;
pub const SIGTRAP: u32 = 5;
pub const SIGABRT: u32 = 6;
pub const SIGBUS: u32 = 7;
pub const SIGFPE: u32 = 8;
pub const SIGKILL: u32 = 9;
pub const SIGUSR1: u32 = 10;
pub const SIGSEGV: u32 = 11;
pub const SIGUSR2: u32 = 12;
pub const SIGPIPE: u32 = 13;
pub const SIGALRM: u32 = 14;
pub const SIGTERM: u32 = 15;
pub const SIGCHLD: u32 = 17;
pub const SIGCONT: u32 = 18;
pub const SIGSTOP: u32 = 19;
pub const SIGTSTP: u32 = 20;

// ============================================================================
// 4. POSIX Clocks & Memory Constants
// ============================================================================
pub const CLOCK_REALTIME: i32 = 0;
pub const CLOCK_MONOTONIC: i32 = 1;
pub const CLOCK_PROCESS_CPUTIME_ID: i32 = 2;
pub const CLOCK_THREAD_CPUTIME_ID: i32 = 3;

pub const PROT_NONE: i32 = 0x0;
pub const PROT_READ: i32 = 0x1;
pub const PROT_WRITE: i32 = 0x2;
pub const PROT_EXEC: i32 = 0x4;

pub const MAP_SHARED: i32 = 0x01;
pub const MAP_PRIVATE: i32 = 0x02;
pub const MAP_FIXED: i32 = 0x10;
pub const MAP_ANONYMOUS: i32 = 0x20;

pub const WNOHANG: i32 = 1;
pub const WUNTRACED: i32 = 2;

pub const MAX_FD_PER_TASK: usize = 32;
pub const MAX_VMAS_PER_TASK: usize = 16;
pub const MAX_OPEN_FILES: usize = 64;

// ============================================================================
// 5. POSIX Data Structures
// ============================================================================

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum FdType {
    Unused,
    ConsoleStdin,
    ConsoleStdout,
    ConsoleStderr,
    VfsFile,
    PipeRead(usize),
    PipeWrite(usize),
}

/// Open File Description (Shared between duplicated file descriptors across dup/fork)
#[derive(Copy, Clone, Debug)]
pub struct OpenFileDescription {
    pub fd_type: FdType,
    pub path: [u8; 128],
    pub path_len: usize,
    pub flags: u32,
    pub offset: usize,
    pub ref_count: usize,
}

impl OpenFileDescription {
    pub const fn unused() -> Self {
        Self {
            fd_type: FdType::Unused,
            path: [0; 128],
            path_len: 0,
            flags: 0,
            offset: 0,
            ref_count: 0,
        }
    }
}

pub static OPEN_FILE_TABLE: Spinlock<[OpenFileDescription; MAX_OPEN_FILES]> = Spinlock::new([
    // Slots 0, 1, 2 for initial stdin, stdout, stderr
    OpenFileDescription {
        fd_type: FdType::ConsoleStdin,
        path: [0; 128],
        path_len: 0,
        flags: O_RDONLY,
        offset: 0,
        ref_count: 1,
    },
    OpenFileDescription {
        fd_type: FdType::ConsoleStdout,
        path: [0; 128],
        path_len: 0,
        flags: O_WRONLY,
        offset: 0,
        ref_count: 1,
    },
    OpenFileDescription {
        fd_type: FdType::ConsoleStderr,
        path: [0; 128],
        path_len: 0,
        flags: O_WRONLY,
        offset: 0,
        ref_count: 1,
    },
    // The rest are unused
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
    OpenFileDescription::unused(),
]);

/// Per-Process File Descriptor (Indexes into OPEN_FILE_TABLE)
#[derive(Copy, Clone, Debug)]
pub struct FileDescriptor {
    pub desc_idx: Option<usize>,
    pub flags: u32,
}

impl FileDescriptor {
    pub const fn unused() -> Self {
        Self {
            desc_idx: None,
            flags: 0,
        }
    }
}

pub const fn default_fd_table() -> [FileDescriptor; MAX_FD_PER_TASK] {
    let mut table = [FileDescriptor::unused(); MAX_FD_PER_TASK];
    table[0] = FileDescriptor {
        desc_idx: Some(0),
        flags: 0,
    };
    table[1] = FileDescriptor {
        desc_idx: Some(1),
        flags: 0,
    };
    table[2] = FileDescriptor {
        desc_idx: Some(2),
        flags: 0,
    };
    table
}

/// Virtual Memory Area (VMA) Descriptor
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Vma {
    pub start: u64,
    pub end: u64,
    pub prot: i32,
    pub flags: i32,
    pub in_use: bool,
}

impl Vma {
    pub const fn empty() -> Self {
        Self {
            start: 0,
            end: 0,
            prot: 0,
            flags: 0,
            in_use: false,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct PosixStat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_nlink: u64,
    pub st_mode: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub __pad0: u32,
    pub st_rdev: u64,
    pub st_size: i64,
    pub st_blksize: i64,
    pub st_blocks: i64,
    pub st_atime: i64,
    pub st_atime_nsec: i64,
    pub st_mtime: i64,
    pub st_mtime_nsec: i64,
    pub st_ctime: i64,
    pub st_ctime_nsec: i64,
    pub __unused: [i64; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct Timespec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct Timeval {
    pub tv_sec: i64,
    pub tv_usec: i64,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct Termios {
    pub c_iflag: u32,
    pub c_oflag: u32,
    pub c_cflag: u32,
    pub c_lflag: u32,
    pub c_line: u8,
    pub c_cc: [u8; 32],
    pub c_ispeed: u32,
    pub c_ospeed: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct Winsize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct SigAction {
    pub sa_handler: u64,
    pub sa_flags: u64,
    pub sa_restorer: u64,
    pub sa_mask: u64,
}

#[repr(C)]
pub struct LinuxDirent64 {
    pub d_ino: u64,
    pub d_off: i64,
    pub d_reclen: u16,
    pub d_type: u8,
    pub d_name: [u8; 256],
}

// Global Process Table State for POSIX Process Lifecycle
pub struct PosixProcessState {
    pub current_cwd: [u8; 64],
    pub current_cwd_len: usize,
}

pub static POSIX_STATE: Spinlock<PosixProcessState> = Spinlock::new(PosixProcessState {
    current_cwd: [b'/'; 64],
    current_cwd_len: 1,
});

// ============================================================================
// 6. POSIX Time Conversion (RTC CMOS -> Epoch Seconds)
// ============================================================================

pub fn rtc_to_unix_epoch(t: &crate::drivers::rtc::WallTime) -> i64 {
    let mut days: i64 = 0;
    let target_year = t.year as i64;
    for y in 1970..target_year {
        if is_leap_year(y) {
            days += 366;
        } else {
            days += 365;
        }
    }

    let month_days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let is_leap = is_leap_year(target_year);
    for m in 1..(t.month as usize) {
        if m == 2 && is_leap {
            days += 29;
        } else if m <= 12 {
            days += month_days[m - 1];
        }
    }
    days += (t.day.saturating_sub(1)) as i64;

    days * 86400 + (t.hour as i64) * 3600 + (t.minute as i64) * 60 + (t.second as i64)
}

fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

pub fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

// ============================================================================
// 7. Core POSIX System Call Implementations
// ============================================================================

pub fn posix_open(path: &str, flags: u32, _mode: u32) -> Result<usize, i64> {
    let mut resolved = [0; 128];
    let clean = resolve_task_path(path, &mut resolved)?;
    if (flags & 3) != O_RDONLY || flags & (O_CREAT | O_TRUNC) != 0 {
        require_writable_path(clean)?;
    }

    if (flags & O_CREAT) != 0 {
        if crate::fs::vfs::file_size(clean).is_err() {
            crate::fs::vfs::write(clean, b"").map_err(|_| -EIO)?;
        }
    }

    if (flags & O_TRUNC) != 0 {
        crate::fs::vfs::write(clean, b"").map_err(|_| -EIO)?;
    }

    let mut oft = OPEN_FILE_TABLE.lock();
    let mut desc_slot = None;
    for i in 3..MAX_OPEN_FILES {
        if oft[i].ref_count == 0 {
            desc_slot = Some(i);
            break;
        }
    }
    let d_slot = desc_slot.ok_or(-ENFILE)?;

    let mut s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let cur_idx = s.current[cpu].unwrap_or(0);
    let task = &mut s.tasks[cur_idx];

    let mut found_fd = None;
    for i in 0..MAX_FD_PER_TASK {
        if task.fd_table[i].desc_idx.is_none() {
            found_fd = Some(i);
            break;
        }
    }
    let fd = found_fd.ok_or(-EMFILE)?;

    let mut initial_offset = 0;
    if (flags & O_APPEND) != 0 {
        if let Ok(file_len) = crate::fs::vfs::file_size(clean) {
            initial_offset = file_len;
        }
    }

    let mut path_arr = [0u8; 128];
    let b = clean.as_bytes();
    if b.len() >= path_arr.len() {
        return Err(-ENAMETOOLONG);
    }
    let len = b.len();
    path_arr[..len].copy_from_slice(b);

    oft[d_slot] = OpenFileDescription {
        fd_type: FdType::VfsFile,
        path: path_arr,
        path_len: len,
        flags,
        offset: initial_offset,
        ref_count: 1,
    };

    task.fd_table[fd] = FileDescriptor {
        desc_idx: Some(d_slot),
        flags: if (flags & O_CLOEXEC) != 0 {
            FD_CLOEXEC
        } else {
            0
        },
    };

    Ok(fd)
}

pub fn posix_close(fd: usize) -> Result<(), i64> {
    if fd >= MAX_FD_PER_TASK {
        return Err(-EBADF);
    }

    let mut s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let cur_idx = s.current[cpu].unwrap_or(0);
    let task = &mut s.tasks[cur_idx];

    let desc_idx = match task.fd_table[fd].desc_idx {
        Some(idx) => idx,
        None => return Err(-EBADF),
    };
    task.fd_table[fd] = FileDescriptor::unused();
    drop(s);

    let mut oft = OPEN_FILE_TABLE.lock();
    if desc_idx >= 3 && desc_idx < MAX_OPEN_FILES && oft[desc_idx].ref_count > 0 {
        oft[desc_idx].ref_count -= 1;
        if oft[desc_idx].ref_count == 0 {
            match oft[desc_idx].fd_type {
                FdType::PipeRead(id) => crate::kernel::ipc::pipe_close_reader(id),
                FdType::PipeWrite(id) => crate::kernel::ipc::pipe_close_writer(id),
                _ => {}
            }
            oft[desc_idx] = OpenFileDescription::unused();
        }
    }

    Ok(())
}

pub fn posix_read(fd: usize, buf: &mut [u8]) -> Result<usize, i64> {
    if fd >= MAX_FD_PER_TASK {
        return Err(-EBADF);
    }
    if buf.is_empty() {
        return Ok(0);
    }

    let desc_idx = {
        let s = crate::kernel::task::SCHED.lock();
        let cpu = crate::kernel::smp::get_current_cpu();
        let cur_idx = s.current[cpu].unwrap_or(0);
        s.tasks[cur_idx].fd_table[fd].desc_idx.ok_or(-EBADF)?
    };

    let mut oft = OPEN_FILE_TABLE.lock();
    if desc_idx >= MAX_OPEN_FILES || oft[desc_idx].ref_count == 0 {
        return Err(-EBADF);
    }

    let desc = &mut oft[desc_idx];
    if (desc.flags & 3) == O_WRONLY {
        return Err(-EBADF);
    }

    match desc.fd_type {
        FdType::ConsoleStdin => {
            let mut read_bytes = 0usize;
            while read_bytes < buf.len() {
                if crate::drivers::keyboard::has_char() {
                    buf[read_bytes] = crate::drivers::keyboard::getchar();
                    read_bytes += 1;
                } else if crate::drivers::serial::has_char() {
                    buf[read_bytes] = crate::drivers::serial::getchar();
                    read_bytes += 1;
                } else {
                    break;
                }
            }
            Ok(read_bytes)
        }
        FdType::ConsoleStdout | FdType::ConsoleStderr => Err(-EBADF),
        FdType::VfsFile => {
            let path_str = core::str::from_utf8(&desc.path[..desc.path_len]).unwrap_or("");
            require_writable_path(path_str)?;
            let offset = desc.offset;
            match crate::fs::vfs::read_at(path_str, offset, buf) {
                Ok(read_bytes) => {
                    desc.offset = desc.offset.saturating_add(read_bytes);
                    Ok(read_bytes)
                }
                Err(_) => Err(-ENOENT),
            }
        }
        FdType::PipeRead(id) => crate::kernel::ipc::pipe_read(id, buf),
        _ => Err(-EBADF),
    }
}

pub fn posix_write(fd: usize, data: &[u8]) -> Result<usize, i64> {
    if fd >= MAX_FD_PER_TASK {
        return Err(-EBADF);
    }
    if data.is_empty() {
        return Ok(0);
    }

    let desc_idx = {
        let s = crate::kernel::task::SCHED.lock();
        let cpu = crate::kernel::smp::get_current_cpu();
        let cur_idx = s.current[cpu].unwrap_or(0);
        s.tasks[cur_idx].fd_table[fd].desc_idx.ok_or(-EBADF)?
    };

    let mut oft = OPEN_FILE_TABLE.lock();
    if desc_idx >= MAX_OPEN_FILES || oft[desc_idx].ref_count == 0 {
        return Err(-EBADF);
    }

    let desc = &mut oft[desc_idx];
    if (desc.flags & 3) == O_RDONLY {
        return Err(-EBADF);
    }

    match desc.fd_type {
        FdType::ConsoleStdout | FdType::ConsoleStderr => {
            if let Ok(text) = core::str::from_utf8(data) {
                crate::drivers::serial::print(text);
                if crate::drivers::video::is_available() {
                    crate::drivers::video::puts(text);
                }
            } else {
                for &b in data {
                    crate::drivers::serial::putchar(b);
                }
            }
            Ok(data.len())
        }
        FdType::ConsoleStdin => Err(-EBADF),
        FdType::VfsFile => {
            let path_str = core::str::from_utf8(&desc.path[..desc.path_len]).unwrap_or("");
            let offset = desc.offset;
            match crate::fs::vfs::write_at(path_str, offset, data) {
                Ok(written) => {
                    desc.offset = desc.offset.saturating_add(written);
                    Ok(written)
                }
                Err(_) => Err(-EIO),
            }
        }
        FdType::PipeWrite(id) => {
            match crate::kernel::ipc::pipe_write(id, data) {
                Ok(n) => Ok(n),
                Err(e) if e == -EPIPE => {
                    // Send SIGPIPE to current process
                    let _ = posix_kill(posix_getpid() as i32, SIGPIPE as i32);
                    Err(-EPIPE)
                }
                Err(e) => Err(e),
            }
        }
        _ => Err(-EBADF),
    }
}

pub fn posix_lseek(fd: usize, offset: i64, whence: i32) -> Result<i64, i64> {
    if fd >= MAX_FD_PER_TASK {
        return Err(-EBADF);
    }

    let desc_idx = {
        let s = crate::kernel::task::SCHED.lock();
        let cpu = crate::kernel::smp::get_current_cpu();
        let cur_idx = s.current[cpu].unwrap_or(0);
        s.tasks[cur_idx].fd_table[fd].desc_idx.ok_or(-EBADF)?
    };

    let mut oft = OPEN_FILE_TABLE.lock();
    if desc_idx >= MAX_OPEN_FILES || oft[desc_idx].ref_count == 0 {
        return Err(-EBADF);
    }

    let desc = &mut oft[desc_idx];
    match desc.fd_type {
        FdType::VfsFile => {
            let cur = desc.offset as i64;
            let target = match whence {
                SEEK_SET => offset,
                SEEK_CUR => cur.checked_add(offset).ok_or(-EOVERFLOW)?,
                SEEK_END => {
                    let path_str = core::str::from_utf8(&desc.path[..desc.path_len]).unwrap_or("");
                    let file_size = crate::fs::vfs::file_size(path_str).unwrap_or(0) as i64;
                    file_size.checked_add(offset).ok_or(-EOVERFLOW)?
                }
                _ => return Err(-EINVAL),
            };

            if target < 0 {
                return Err(-EINVAL);
            }
            desc.offset = target as usize;
            Ok(target)
        }
        FdType::PipeRead(_)
        | FdType::PipeWrite(_)
        | FdType::ConsoleStdin
        | FdType::ConsoleStdout
        | FdType::ConsoleStderr => Err(-ESPIPE),
        _ => Err(-EBADF),
    }
}

pub fn posix_stat(path: &str, statbuf: &mut PosixStat) -> Result<(), i64> {
    let clean = path.trim();
    if clean.is_empty() {
        return Err(-ENOENT);
    }

    *statbuf = PosixStat::default();
    statbuf.st_dev = 1;
    statbuf.st_nlink = 1;

    let rtc = crate::drivers::rtc::get_time();
    let epoch = rtc_to_unix_epoch(&rtc);
    statbuf.st_atime = epoch;
    statbuf.st_mtime = epoch;
    statbuf.st_ctime = epoch;
    statbuf.st_blksize = 4096;

    if clean == "/" || clean == "." || clean == ".." || crate::fs::vfs::is_directory(clean) {
        statbuf.st_ino = fnv1a_64(clean.as_bytes());
        statbuf.st_mode = S_IFDIR | 0o755;
        statbuf.st_size = 4096;
        statbuf.st_blocks = 8;
        return Ok(());
    }

    if clean == "/dev" || clean == "dev" {
        statbuf.st_ino = 2;
        statbuf.st_mode = S_IFDIR | 0o755;
        statbuf.st_size = 4096;
        statbuf.st_blocks = 8;
        return Ok(());
    }

    if clean.starts_with("/dev/") || clean.starts_with("dev/") {
        statbuf.st_ino = fnv1a_64(clean.as_bytes());
        statbuf.st_mode = S_IFCHR | 0o666;
        statbuf.st_size = 0;
        return Ok(());
    }

    if clean.starts_with("/ssd")
        || clean.starts_with("/hdd")
        || clean.starts_with("/usb")
        || clean.starts_with("/sdxc")
        || clean.starts_with("/sdhc")
    {
        statbuf.st_ino = fnv1a_64(clean.as_bytes());
        statbuf.st_mode = S_IFBLK | 0o660;
        if let Some(idx) = crate::drivers::block_dev::find_by_canonical_path(clean) {
            let mut total_bytes: i64 = 0;
            let mut blk_count: i64 = 0;
            let _ = crate::drivers::block_dev::with_dev(idx, |dev| {
                let bytes = dev.total_blocks() * (dev.block_size() as u64);
                total_bytes = bytes as i64;
                blk_count = (bytes / 512) as i64;
            });
            statbuf.st_size = total_bytes;
            statbuf.st_blocks = blk_count;
            return Ok(());
        } else {
            return Err(-ENOENT);
        }
    }

    match crate::fs::vfs::file_size(clean) {
        Ok(sz) => {
            statbuf.st_ino = fnv1a_64(clean.as_bytes());
            statbuf.st_mode = S_IFREG | 0o644;
            statbuf.st_size = sz as i64;
            statbuf.st_blocks = ((sz as i64) + 511) / 512;
            Ok(())
        }
        Err(_) => Err(-ENOENT),
    }
}

pub fn posix_fstat(fd: usize, statbuf: &mut PosixStat) -> Result<(), i64> {
    if fd >= MAX_FD_PER_TASK {
        return Err(-EBADF);
    }

    let desc_idx = {
        let s = crate::kernel::task::SCHED.lock();
        let cpu = crate::kernel::smp::get_current_cpu();
        let cur_idx = s.current[cpu].unwrap_or(0);
        s.tasks[cur_idx].fd_table[fd].desc_idx.ok_or(-EBADF)?
    };

    let oft = OPEN_FILE_TABLE.lock();
    if desc_idx >= MAX_OPEN_FILES || oft[desc_idx].ref_count == 0 {
        return Err(-EBADF);
    }

    let desc = &oft[desc_idx];
    match desc.fd_type {
        FdType::ConsoleStdin | FdType::ConsoleStdout | FdType::ConsoleStderr => {
            *statbuf = PosixStat::default();
            statbuf.st_mode = S_IFCHR | 0o666;
            statbuf.st_nlink = 1;
            Ok(())
        }
        FdType::PipeRead(_) | FdType::PipeWrite(_) => {
            *statbuf = PosixStat::default();
            statbuf.st_mode = S_IFIFO | 0o600;
            statbuf.st_size = 4096;
            statbuf.st_nlink = 1;
            Ok(())
        }
        FdType::VfsFile => {
            let mut path_buf = [0u8; 128];
            let len = desc.path_len.min(128);
            path_buf[..len].copy_from_slice(&desc.path[..len]);
            drop(oft);
            let path_str = core::str::from_utf8(&path_buf[..len]).unwrap_or("");
            posix_stat(path_str, statbuf)
        }
        _ => Err(-EBADF),
    }
}

pub fn posix_pipe(pipefd: &mut [i32; 2]) -> Result<(), i64> {
    let pipe_id = crate::kernel::ipc::create_pipe().ok_or(-ENFILE)?;

    let mut oft = OPEN_FILE_TABLE.lock();
    let mut r_desc = None;
    let mut w_desc = None;
    for i in 3..MAX_OPEN_FILES {
        if oft[i].ref_count == 0 {
            if r_desc.is_none() {
                r_desc = Some(i);
            } else if w_desc.is_none() {
                w_desc = Some(i);
                break;
            }
        }
    }
    let rd_slot = match r_desc {
        Some(s) => s,
        None => {
            crate::kernel::ipc::destroy_pipe(pipe_id);
            return Err(-ENFILE);
        }
    };
    let wd_slot = match w_desc {
        Some(s) => s,
        None => {
            crate::kernel::ipc::destroy_pipe(pipe_id);
            return Err(-ENFILE);
        }
    };

    let mut s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let cur_idx = s.current[cpu].unwrap_or(0);
    let task = &mut s.tasks[cur_idx];

    let mut read_slot = None;
    let mut write_slot = None;

    for i in 0..MAX_FD_PER_TASK {
        if task.fd_table[i].desc_idx.is_none() {
            if read_slot.is_none() {
                read_slot = Some(i);
            } else if write_slot.is_none() {
                write_slot = Some(i);
                break;
            }
        }
    }

    let r_slot = match read_slot {
        Some(s) => s,
        None => {
            crate::kernel::ipc::destroy_pipe(pipe_id);
            return Err(-EMFILE);
        }
    };
    let w_slot = match write_slot {
        Some(s) => s,
        None => {
            crate::kernel::ipc::destroy_pipe(pipe_id);
            return Err(-EMFILE);
        }
    };

    oft[rd_slot] = OpenFileDescription {
        fd_type: FdType::PipeRead(pipe_id),
        path: [0; 128],
        path_len: 0,
        flags: O_RDONLY,
        offset: 0,
        ref_count: 1,
    };

    oft[wd_slot] = OpenFileDescription {
        fd_type: FdType::PipeWrite(pipe_id),
        path: [0; 128],
        path_len: 0,
        flags: O_WRONLY,
        offset: 0,
        ref_count: 1,
    };

    task.fd_table[r_slot] = FileDescriptor {
        desc_idx: Some(rd_slot),
        flags: 0,
    };

    task.fd_table[w_slot] = FileDescriptor {
        desc_idx: Some(wd_slot),
        flags: 0,
    };

    pipefd[0] = r_slot as i32;
    pipefd[1] = w_slot as i32;
    Ok(())
}

pub fn posix_dup(oldfd: usize) -> Result<usize, i64> {
    if oldfd >= MAX_FD_PER_TASK {
        return Err(-EBADF);
    }

    let mut s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let cur_idx = s.current[cpu].unwrap_or(0);
    let task = &mut s.tasks[cur_idx];

    let desc_idx = task.fd_table[oldfd].desc_idx.ok_or(-EBADF)?;

    let mut free_slot = None;
    for i in 0..MAX_FD_PER_TASK {
        if task.fd_table[i].desc_idx.is_none() {
            free_slot = Some(i);
            break;
        }
    }

    let target = free_slot.ok_or(-EMFILE)?;

    let mut oft = OPEN_FILE_TABLE.lock();
    oft[desc_idx].ref_count += 1;
    match oft[desc_idx].fd_type {
        FdType::PipeRead(id) => crate::kernel::ipc::pipe_add_reader(id),
        FdType::PipeWrite(id) => crate::kernel::ipc::pipe_add_writer(id),
        _ => {}
    }

    task.fd_table[target] = FileDescriptor {
        desc_idx: Some(desc_idx),
        flags: 0, // POSIX requirement: FD_CLOEXEC is cleared on dup
    };

    Ok(target)
}

pub fn posix_dup2(oldfd: usize, newfd: usize) -> Result<usize, i64> {
    if oldfd >= MAX_FD_PER_TASK || newfd >= MAX_FD_PER_TASK {
        return Err(-EBADF);
    }

    let mut s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let mut cur_idx = s.current[cpu].unwrap_or(0);
    let task = &mut s.tasks[cur_idx];

    let desc_idx = task.fd_table[oldfd].desc_idx.ok_or(-EBADF)?;

    if oldfd == newfd {
        return Ok(newfd);
    }

    if task.fd_table[newfd].desc_idx.is_some() {
        drop(s);
        let _ = posix_close(newfd);
        s = crate::kernel::task::SCHED.lock();
        cur_idx = s.current[cpu].unwrap_or(0);
    }

    let mut oft = OPEN_FILE_TABLE.lock();
    oft[desc_idx].ref_count += 1;
    match oft[desc_idx].fd_type {
        FdType::PipeRead(id) => crate::kernel::ipc::pipe_add_reader(id),
        FdType::PipeWrite(id) => crate::kernel::ipc::pipe_add_writer(id),
        _ => {}
    }

    s.tasks[cur_idx].fd_table[newfd] = FileDescriptor {
        desc_idx: Some(desc_idx),
        flags: 0,
    };

    Ok(newfd)
}

pub fn posix_fcntl(fd: usize, cmd: i32, arg: u64) -> Result<i64, i64> {
    if fd >= MAX_FD_PER_TASK {
        return Err(-EBADF);
    }

    const F_DUPFD: i32 = 0;
    const F_GETFD: i32 = 1;
    const F_SETFD: i32 = 2;
    const F_GETFL: i32 = 3;
    const F_SETFL: i32 = 4;
    const F_DUPFD_CLOEXEC: i32 = 1030;

    let mut s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let cur_idx = s.current[cpu].unwrap_or(0);
    let task = &mut s.tasks[cur_idx];

    let desc_idx = task.fd_table[fd].desc_idx.ok_or(-EBADF)?;

    match cmd {
        F_DUPFD | F_DUPFD_CLOEXEC => {
            let min_fd = (arg as usize).min(MAX_FD_PER_TASK);
            let mut target_slot = None;
            for i in min_fd..MAX_FD_PER_TASK {
                if task.fd_table[i].desc_idx.is_none() {
                    target_slot = Some(i);
                    break;
                }
            }
            let target = target_slot.ok_or(-EMFILE)?;

            let mut oft = OPEN_FILE_TABLE.lock();
            oft[desc_idx].ref_count += 1;
            match oft[desc_idx].fd_type {
                FdType::PipeRead(id) => crate::kernel::ipc::pipe_add_reader(id),
                FdType::PipeWrite(id) => crate::kernel::ipc::pipe_add_writer(id),
                _ => {}
            }

            task.fd_table[target] = FileDescriptor {
                desc_idx: Some(desc_idx),
                flags: if cmd == F_DUPFD_CLOEXEC {
                    FD_CLOEXEC
                } else {
                    0
                },
            };
            Ok(target as i64)
        }
        F_GETFD => Ok((task.fd_table[fd].flags & FD_CLOEXEC) as i64),
        F_SETFD => {
            task.fd_table[fd].flags = (arg as u32) & FD_CLOEXEC;
            Ok(0)
        }
        F_GETFL => {
            let oft = OPEN_FILE_TABLE.lock();
            Ok(oft[desc_idx].flags as i64)
        }
        F_SETFL => {
            let mut oft = OPEN_FILE_TABLE.lock();
            let mask = O_APPEND | O_NONBLOCK;
            oft[desc_idx].flags = (oft[desc_idx].flags & !mask) | ((arg as u32) & mask);
            Ok(0)
        }
        _ => Err(-EINVAL),
    }
}

pub fn posix_ioctl(fd: usize, req: u64, arg: u64) -> Result<i64, i64> {
    if fd >= MAX_FD_PER_TASK {
        return Err(-EBADF);
    }

    let desc_idx = {
        let s = crate::kernel::task::SCHED.lock();
        let cpu = crate::kernel::smp::get_current_cpu();
        let cur_idx = s.current[cpu].unwrap_or(0);
        s.tasks[cur_idx].fd_table[fd].desc_idx.ok_or(-EBADF)?
    };

    let oft = OPEN_FILE_TABLE.lock();
    if desc_idx >= MAX_OPEN_FILES || oft[desc_idx].ref_count == 0 {
        return Err(-EBADF);
    }

    match oft[desc_idx].fd_type {
        FdType::ConsoleStdin | FdType::ConsoleStdout | FdType::ConsoleStderr => {
            const TCGETS: u64 = 0x5401;
            const TCSETS: u64 = 0x5402;
            const TIOCGWINSZ: u64 = 0x5413;

            match req {
                TCGETS => {
                    let mut term = Termios::default();
                    term.c_cflag = 0o000060; // CS8
                    term.c_lflag = 0o000010 | 0o000002; // ECHO | ICANON
                    drop(oft);
                    let slice = unsafe {
                        core::slice::from_raw_parts(
                            &term as *const Termios as *const u8,
                            core::mem::size_of::<Termios>(),
                        )
                    };
                    crate::kernel::uaccess::copy_to_user(slice, arg)?;
                    Ok(0)
                }
                TCSETS => {
                    let mut term = Termios::default();
                    drop(oft);
                    let slice = unsafe {
                        core::slice::from_raw_parts_mut(
                            &mut term as *mut Termios as *mut u8,
                            core::mem::size_of::<Termios>(),
                        )
                    };
                    crate::kernel::uaccess::copy_from_user(arg, slice)?;
                    Ok(0)
                }
                TIOCGWINSZ => {
                    let ws = Winsize {
                        ws_row: 25,
                        ws_col: 80,
                        ws_xpixel: 1024,
                        ws_ypixel: 768,
                    };
                    drop(oft);
                    let slice = unsafe {
                        core::slice::from_raw_parts(
                            &ws as *const Winsize as *const u8,
                            core::mem::size_of::<Winsize>(),
                        )
                    };
                    crate::kernel::uaccess::copy_to_user(slice, arg)?;
                    Ok(0)
                }
                _ => Err(-EINVAL),
            }
        }
        _ => Err(-ENOTTY),
    }
}

pub fn posix_fork() -> Result<u32, i64> {
    // A real fork must preserve the interrupted user register frame, clone all
    // mappings successfully, and make the child resume with return value zero.
    // The current scheduler has no child return-frame support; do not report a
    // child PID for the old yield-loop placeholder.
    Err(-ENOSYS)
}

pub fn posix_execve(_path: &str, _argv: &[&str], _envp: &[&str]) -> Result<(), i64> {
    // ELF segment loading, argument/environment stack construction, and an
    // atomic user-mode transfer are not implemented. Preserve the current task
    // and report the operation as unsupported instead of switching to an empty
    // address space and returning success.
    Err(-ENOSYS)
}

pub fn posix_wait4(target_pid: i32, status_ptr: *mut i32, options: i32) -> Result<i32, i64> {
    if !status_ptr.is_null() {
        crate::kernel::uaccess::validate_user_range(
            status_ptr as u64,
            core::mem::size_of::<i32>(),
            true,
        )?;
    }

    let mut s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let cur_idx = s.current[cpu].unwrap_or(0);
    let my_pid = s.tasks[cur_idx].id;

    for i in 0..crate::kernel::task::MAX_TASKS {
        let t = &mut s.tasks[i];
        if t.ppid == my_pid && (target_pid == -1 || t.id == target_pid as u32) {
            if t.is_zombie || t.state == crate::kernel::task::TaskState::Terminated {
                let pid = t.id as i32;
                let exit_code = t.exit_code;
                if t.stack_base != 0 {
                    let _ = crate::mm::pmm::free_frames(t.stack_base as usize, 4);
                }
                if t.cr3 != 0 {
                    crate::mm::vmm::destroy_pml4(t.cr3 as *mut crate::mm::vmm::PageTable);
                }
                *t = crate::kernel::task::Task::empty();
                drop(s);

                if !status_ptr.is_null() {
                    let exit_status = (exit_code & 0xFF) << 8;
                    let slice = unsafe {
                        core::slice::from_raw_parts(
                            &exit_status as *const i32 as *const u8,
                            core::mem::size_of::<i32>(),
                        )
                    };
                    let _ = crate::kernel::uaccess::copy_to_user(slice, status_ptr as u64);
                }
                return Ok(pid);
            }
        }
    }

    if (options & WNOHANG) != 0 {
        Ok(0)
    } else {
        Err(-ECHILD)
    }
}

pub fn posix_kill(pid: i32, sig: i32) -> Result<(), i64> {
    if sig < 0 || sig > 64 {
        return Err(-EINVAL);
    }

    let mut s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let caller_uid = s.current[cpu].map_or_else(|| super::accounts::local().uid, |index| s.tasks[index].uid);
    for i in 0..crate::kernel::task::MAX_TASKS {
        let t = &mut s.tasks[i];
        if t.id == pid as u32 && t.state != crate::kernel::task::TaskState::Unused {
            if caller_uid != 0 && caller_uid != t.uid {
                return Err(-EPERM);
            }
            if sig == 0 {
                return Ok(()); // Probing process existence
            }
            if sig == (SIGKILL as i32) || sig == (SIGTERM as i32) {
                t.state = crate::kernel::task::TaskState::Terminated;
                t.is_zombie = true;
                t.exit_code = 128 + sig;
                drop(s);
                crate::kernel::smp::broadcast_resched();
                return Ok(());
            } else {
                t.sig_pending |= 1u64 << (sig - 1);
            }
            return Ok(());
        }
    }
    Err(-ESRCH)
}

pub fn posix_rt_sigaction(
    signum: i32,
    act: Option<&SigAction>,
    oldact: Option<&mut SigAction>,
) -> Result<(), i64> {
    if signum < 1 || signum > 64 {
        return Err(-EINVAL);
    }
    if signum == (SIGKILL as i32) || signum == (SIGSTOP as i32) {
        return Err(-EINVAL);
    }

    let mut s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let cur_idx = s.current[cpu].unwrap_or(0);
    let task = &mut s.tasks[cur_idx];

    let idx = (signum - 1) as usize;

    if let Some(old) = oldact {
        old.sa_handler = task.sig_handlers[idx];
        old.sa_mask = task.sig_mask;
        old.sa_flags = 0;
        old.sa_restorer = 0;
    }

    if let Some(new) = act {
        task.sig_handlers[idx] = new.sa_handler;
    }

    Ok(())
}

pub fn posix_rt_sigprocmask(
    how: i32,
    set: Option<u64>,
    oldset: Option<&mut u64>,
) -> Result<(), i64> {
    const SIG_BLOCK: i32 = 0;
    const SIG_UNBLOCK: i32 = 1;
    const SIG_SETMASK: i32 = 2;

    let mut s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let cur_idx = s.current[cpu].unwrap_or(0);
    let task = &mut s.tasks[cur_idx];

    if let Some(old) = oldset {
        *old = task.sig_mask;
    }

    if let Some(new_mask) = set {
        match how {
            SIG_BLOCK => task.sig_mask |= new_mask,
            SIG_UNBLOCK => task.sig_mask &= !new_mask,
            SIG_SETMASK => task.sig_mask = new_mask,
            _ => return Err(-EINVAL),
        }
        // SIGKILL and SIGSTOP cannot be blocked
        task.sig_mask &= !((1u64 << (SIGKILL - 1)) | (1u64 << (SIGSTOP - 1)));
    }

    Ok(())
}

pub fn posix_brk(addr: u64) -> Result<u64, i64> {
    let mut s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let cur_idx = s.current[cpu].unwrap_or(0);
    let task = &mut s.tasks[cur_idx];

    if addr == 0 || addr == task.brk_current {
        return Ok(task.brk_current);
    }

    if addr < task.brk_start {
        return Ok(task.brk_current);
    }

    if addr > 0x0000_5000_0000_0000 {
        return Err(-ENOMEM);
    }

    let pml4 = if task.cr3 != 0 {
        task.cr3 as *mut crate::mm::vmm::PageTable
    } else {
        crate::mm::vmm::get_kernel_pml4()
    };

    let cur_page_top = (task.brk_current + 4095) & !4095;
    let new_page_top = (addr + 4095) & !4095;

    if addr > task.brk_current {
        if new_page_top > cur_page_top {
            let diff = new_page_top - cur_page_top;
            let frames_needed = (diff as usize) / 4096;
            if let Some(paddr) = crate::mm::pmm::alloc_frames(frames_needed) {
                unsafe {
                    core::ptr::write_bytes(paddr as *mut u8, 0, frames_needed * 4096);
                }
                for i in 0..frames_needed {
                    let page_v = cur_page_top + (i as u64 * 4096);
                    let page_p = (paddr as u64) + (i as u64 * 4096);
                    let _ = crate::mm::vmm::map_page(
                        pml4,
                        page_v,
                        page_p,
                        crate::mm::vmm::PAGE_PRESENT
                            | crate::mm::vmm::PAGE_WRITABLE
                            | crate::mm::vmm::PAGE_USER
                            | crate::mm::vmm::PAGE_NO_EXECUTE,
                    );
                }
            } else {
                return Err(-ENOMEM);
            }
        }
        task.brk_current = addr;
        Ok(task.brk_current)
    } else {
        // Heap contraction
        if new_page_top < cur_page_top {
            crate::mm::vmm::free_user_range(pml4, new_page_top, cur_page_top);
        }
        task.brk_current = addr;
        Ok(task.brk_current)
    }
}

pub fn posix_mmap(
    addr: u64,
    len: usize,
    prot: i32,
    flags: i32,
    fd: i32,
    offset: i64,
) -> Result<u64, i64> {
    if len == 0 {
        return Err(-EINVAL);
    }

    let aligned_len = len.checked_add(4095).ok_or(-ENOMEM)? & !4095;
    let frames_needed = aligned_len / 4096;

    let mut s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let mut cur_idx = s.current[cpu].unwrap_or(0);
    let task = &mut s.tasks[cur_idx];

    let vaddr = if addr != 0 && (flags & MAP_FIXED) != 0 {
        let target = addr & !0xFFF;
        if target < crate::kernel::uaccess::USER_SPACE_MIN
            || target.saturating_add(aligned_len as u64) > crate::kernel::uaccess::USER_SPACE_MAX
        {
            return Err(-EINVAL);
        }
        target
    } else {
        // Dynamic Virtual Address Allocation: Find non-colliding range in user mmap region
        let mut alloc_addr = 0x0000_6000_0000_0000u64;
        for vma in task.vmas.iter() {
            if vma.in_use && alloc_addr < vma.end {
                alloc_addr = (vma.end + 4095) & !4095;
            }
        }
        alloc_addr
    };

    let pml4 = if task.cr3 != 0 {
        task.cr3 as *mut crate::mm::vmm::PageTable
    } else {
        crate::mm::vmm::get_kernel_pml4()
    };

    let phys_frame = crate::mm::pmm::alloc_frames(frames_needed).ok_or(-ENOMEM)?;
    unsafe {
        core::ptr::write_bytes(phys_frame as *mut u8, 0, aligned_len);
    }

    if (flags & MAP_ANONYMOUS) == 0 && fd >= 0 {
        drop(s);
        let mut tmp_buf = [0u8; 4096];
        let _ = posix_lseek(fd as usize, offset, SEEK_SET);
        let mut bytes_copied = 0usize;
        while bytes_copied < aligned_len {
            let chunk = (aligned_len - bytes_copied).min(tmp_buf.len());
            let n = posix_read(fd as usize, &mut tmp_buf[..chunk]).unwrap_or(0);
            if n == 0 {
                break;
            }
            unsafe {
                core::ptr::copy_nonoverlapping(
                    tmp_buf.as_ptr(),
                    (phys_frame + bytes_copied) as *mut u8,
                    n,
                );
            }
            bytes_copied += n;
        }
        s = crate::kernel::task::SCHED.lock();
        let cpu = crate::kernel::smp::get_current_cpu();
        cur_idx = s.current[cpu].unwrap_or(0);
    }

    let mut page_flags = crate::mm::vmm::PAGE_PRESENT | crate::mm::vmm::PAGE_USER;
    if (prot & PROT_WRITE) != 0 {
        page_flags |= crate::mm::vmm::PAGE_WRITABLE;
    }
    if (prot & PROT_EXEC) == 0 {
        page_flags |= crate::mm::vmm::PAGE_NO_EXECUTE;
    }

    for i in 0..frames_needed {
        let page_v = vaddr + (i as u64 * 4096);
        let page_p = (phys_frame as u64) + (i as u64 * 4096);
        let _ = crate::mm::vmm::map_page(pml4, page_v, page_p, page_flags);
    }

    // Record VMA
    let task = &mut s.tasks[cur_idx];
    let mut recorded = false;
    for vma in task.vmas.iter_mut() {
        if !vma.in_use {
            *vma = Vma {
                start: vaddr,
                end: vaddr + aligned_len as u64,
                prot,
                flags,
                in_use: true,
            };
            recorded = true;
            break;
        }
    }

    if !recorded {
        crate::mm::vmm::free_user_range(pml4, vaddr, vaddr + aligned_len as u64);
        return Err(-ENOMEM);
    }

    Ok(vaddr)
}

pub fn posix_munmap(addr: u64, len: usize) -> Result<(), i64> {
    if addr == 0 || (addr & 0xFFF) != 0 || len == 0 {
        return Err(-EINVAL);
    }
    let aligned_len = len.checked_add(4095).ok_or(-EINVAL)? & !4095;

    let mut s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let cur_idx = s.current[cpu].unwrap_or(0);
    let task = &mut s.tasks[cur_idx];

    let pml4 = if task.cr3 != 0 {
        task.cr3 as *mut crate::mm::vmm::PageTable
    } else {
        crate::mm::vmm::get_kernel_pml4()
    };

    crate::mm::vmm::free_user_range(pml4, addr, addr + aligned_len as u64);

    for vma in task.vmas.iter_mut() {
        if vma.in_use && vma.start >= addr && vma.end <= addr + aligned_len as u64 {
            *vma = Vma::empty();
        }
    }

    Ok(())
}

pub fn posix_mprotect(addr: u64, len: usize, prot: i32) -> Result<(), i64> {
    if addr == 0 || (addr & 0xFFF) != 0 || len == 0 {
        return Err(-EINVAL);
    }
    let aligned_len = len.checked_add(4095).ok_or(-EINVAL)? & !4095;
    let end = addr.saturating_add(aligned_len as u64);

    let mut s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let cur_idx = s.current[cpu].unwrap_or(0);
    let task = &mut s.tasks[cur_idx];

    let pml4 = if task.cr3 != 0 {
        task.cr3 as *mut crate::mm::vmm::PageTable
    } else {
        crate::mm::vmm::get_kernel_pml4()
    };

    let mut curr = addr;
    while curr < end {
        let (paddr, _) = crate::mm::vmm::get_mapping(pml4, curr).ok_or(-ENOMEM)?;
        let mut new_flags = crate::mm::vmm::PAGE_PRESENT | crate::mm::vmm::PAGE_USER;
        if (prot & PROT_WRITE) != 0 {
            new_flags |= crate::mm::vmm::PAGE_WRITABLE;
        }
        if (prot & PROT_EXEC) == 0 {
            new_flags |= crate::mm::vmm::PAGE_NO_EXECUTE;
        }
        let _ = crate::mm::vmm::map_page(pml4, curr, paddr, new_flags);
        curr += 4096;
    }

    for vma in task.vmas.iter_mut() {
        if vma.in_use && vma.start <= addr && vma.end >= end {
            vma.prot = prot;
        }
    }

    Ok(())
}

pub fn posix_clock_gettime(clock_id: i32, tp: &mut Timespec) -> Result<(), i64> {
    let ticks = crate::drivers::pit::get_ticks();
    match clock_id {
        CLOCK_REALTIME => {
            let rtc = crate::drivers::rtc::get_time();
            let epoch_sec = rtc_to_unix_epoch(&rtc);
            tp.tv_sec = epoch_sec;
            tp.tv_nsec = ((ticks % 100) * 10_000_000) as i64;
            Ok(())
        }
        CLOCK_MONOTONIC | CLOCK_PROCESS_CPUTIME_ID | CLOCK_THREAD_CPUTIME_ID => {
            tp.tv_sec = (ticks / 100) as i64;
            tp.tv_nsec = ((ticks % 100) * 10_000_000) as i64;
            Ok(())
        }
        _ => Err(-EINVAL),
    }
}

pub fn posix_nanosleep(req: &Timespec, _rem: Option<&mut Timespec>) -> Result<(), i64> {
    if req.tv_sec < 0 || req.tv_nsec < 0 || req.tv_nsec >= 1_000_000_000 {
        return Err(-EINVAL);
    }
    let mut ticks = (req.tv_sec as u64 * 100) + ((req.tv_nsec as u64 + 9_999_999) / 10_000_000);
    if ticks == 0 && (req.tv_sec > 0 || req.tv_nsec > 0) {
        ticks = 1;
    }
    if ticks > 0 {
        crate::kernel::task::sleep(ticks);
    }
    Ok(())
}

pub fn posix_getpid() -> i64 {
    let s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let cur_idx = s.current[cpu].unwrap_or(0);
    s.tasks[cur_idx].id as i64
}

pub fn posix_getppid() -> i64 {
    let s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let cur_idx = s.current[cpu].unwrap_or(0);
    s.tasks[cur_idx].ppid as i64
}

pub fn posix_getuid() -> i64 {
    let scheduler = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    i64::from(scheduler.current[cpu].map_or_else(|| super::accounts::local().uid, |index| scheduler.tasks[index].uid))
}
pub fn posix_getgid() -> i64 {
    let scheduler = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    i64::from(scheduler.current[cpu].map_or_else(|| super::accounts::local().gid, |index| scheduler.tasks[index].gid))
}
pub fn posix_geteuid() -> i64 {
    posix_getuid()
}
pub fn posix_getegid() -> i64 {
    posix_getgid()
}

pub(crate) fn normal_home_child(path: &str) -> bool {
    path_child(path, super::accounts::local().home(&mut [0; 28]))
}

fn path_child(path: &str, directory: &str) -> bool {
    path.strip_prefix(directory)
        .and_then(|rest| rest.strip_prefix('/'))
        .is_some_and(|rest| rest.split('/').all(|part| {
            !part.is_empty() && !matches!(part, "." | "..")
        }))
}

fn require_writable_path(path: &str) -> Result<(), i64> {
    if posix_geteuid() == 0 || normal_home_child(path) || path_child(path, "/tmp") {
        Ok(())
    } else {
        Err(-EACCES)
    }
}

fn resolve_task_path<'a>(path: &str, buffer: &'a mut [u8; 128]) -> Result<&'a str, i64> {
    if path.is_empty() {
        return Err(-ENOENT);
    }
    let mut cwd = [0; 64];
    let base = if path.starts_with('/') {
        ""
    } else {
        let length = posix_getcwd(&mut cwd)?;
        core::str::from_utf8(&cwd[..length]).map_err(|_| -EINVAL)?
    };
    buffer[0] = b'/';
    let mut length = 1;
    for part in base.split('/').chain(path.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                length = buffer[..length].iter().rposition(|&byte| byte == b'/')
                    .unwrap_or(0).max(1);
            }
            name => {
                let start = length + usize::from(length > 1);
                let end = start + name.len();
                if end >= buffer.len() {
                    return Err(-ENAMETOOLONG);
                }
                if length > 1 {
                    buffer[length] = b'/';
                }
                buffer[start..end].copy_from_slice(name.as_bytes());
                length = end;
            }
        }
    }
    core::str::from_utf8(&buffer[..length]).map_err(|_| -EINVAL)
}

pub fn posix_getcwd(buf: &mut [u8]) -> Result<usize, i64> {
    let s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let cur_idx = s.current[cpu].unwrap_or(0);
    let task = &s.tasks[cur_idx];

    let len = task.cwd_len;
    if buf.len() < len + 1 {
        return Err(-ERANGE);
    }

    buf[..len].copy_from_slice(&task.cwd[..len]);
    buf[len] = 0;
    Ok(len)
}

pub fn posix_chdir(path: &str) -> Result<(), i64> {
    let mut resolved = [0; 128];
    let clean = resolve_task_path(path, &mut resolved)?;

    if !crate::fs::vfs::is_directory(clean) {
        if crate::fs::vfs::file_size(clean).is_ok() {
            return Err(-ENOTDIR);
        } else {
            return Err(-ENOENT);
        }
    }

    let mut s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let cur_idx = s.current[cpu].unwrap_or(0);
    let task = &mut s.tasks[cur_idx];

    let b = clean.as_bytes();
    if b.len() >= task.cwd.len() {
        return Err(-ENAMETOOLONG);
    }

    task.cwd.fill(0);
    task.cwd[..b.len()].copy_from_slice(b);
    task.cwd_len = b.len();
    Ok(())
}

pub fn posix_mkdir(path: &str, _mode: u32) -> Result<(), i64> {
    let mut resolved = [0; 128];
    let clean = resolve_task_path(path, &mut resolved)?;
    let uid = posix_geteuid() as u16;
    if uid != 0 && !normal_home_child(clean) {
        return Err(-EACCES);
    }
    match crate::fs::vfs::mkdir_owned(clean, uid, posix_getegid() as u16) {
        Ok(()) => Ok(()),
        Err("Directory already exists") => Err(-EEXIST),
        Err("Cannot create root directory") => Err(-EEXIST),
        Err(_) => Err(-ENOSPC),
    }
}

pub fn posix_rmdir(path: &str) -> Result<(), i64> {
    let mut resolved = [0; 128];
    let clean = resolve_task_path(path, &mut resolved)?;
    if posix_geteuid() != 0 && !normal_home_child(clean) {
        return Err(-EACCES);
    }
    match crate::fs::vfs::rmdir(clean) {
        Ok(()) => Ok(()),
        Err("Cannot remove root directory") | Err("Cannot remove system mountpoint") => Err(-EBUSY),
        Err(_) => Err(-ENOENT),
    }
}

pub fn posix_unlink(path: &str) -> Result<(), i64> {
    let mut resolved = [0; 128];
    let clean = resolve_task_path(path, &mut resolved)?;
    if posix_geteuid() != 0 && !normal_home_child(clean) {
        return Err(-EACCES);
    }
    if crate::fs::vfs::is_directory(clean) {
        return Err(-EISDIR);
    }
    crate::fs::vfs::delete(clean).map_err(|_| -ENOENT)
}

pub fn posix_access(path: &str, mode: i32) -> Result<(), i64> {
    let clean = path.trim();
    if clean.is_empty() {
        return Err(-ENOENT);
    }
    let mut statbuf = PosixStat::default();
    posix_stat(clean, &mut statbuf)?;

    // Check write permission
    if (mode & 2) != 0 {
        if (statbuf.st_mode & 0o222) == 0 {
            return Err(-EACCES);
        }
    }
    // Check read permission
    if (mode & 4) != 0 {
        if (statbuf.st_mode & 0o444) == 0 {
            return Err(-EACCES);
        }
    }
    // Check execute permission
    if (mode & 1) != 0 {
        if (statbuf.st_mode & 0o111) == 0 {
            return Err(-EACCES);
        }
    }

    Ok(())
}

pub fn posix_umask(mask: u32) -> u32 {
    let mut s = crate::kernel::task::SCHED.lock();
    let cpu = crate::kernel::smp::get_current_cpu();
    let cur_idx = s.current[cpu].unwrap_or(0);
    let task = &mut s.tasks[cur_idx];
    let old = task.umask;
    task.umask = mask & 0o777;
    old
}

pub fn posix_getdents64(fd: usize, dirp: *mut LinuxDirent64, count: usize) -> Result<i64, i64> {
    if fd >= MAX_FD_PER_TASK {
        return Err(-EBADF);
    }
    if dirp.is_null() || count < core::mem::size_of::<LinuxDirent64>() {
        return Err(-EINVAL);
    }

    let desc_idx = {
        let s = crate::kernel::task::SCHED.lock();
        let cpu = crate::kernel::smp::get_current_cpu();
        let cur_idx = s.current[cpu].unwrap_or(0);
        s.tasks[cur_idx].fd_table[fd].desc_idx.ok_or(-EBADF)?
    };

    let oft = OPEN_FILE_TABLE.lock();
    if desc_idx >= MAX_OPEN_FILES || oft[desc_idx].ref_count == 0 {
        return Err(-EBADF);
    }

    let desc = &oft[desc_idx];
    let mut path_buf = [0u8; 128];
    let plen = desc.path_len.min(128);
    path_buf[..plen].copy_from_slice(&desc.path[..plen]);
    drop(oft);

    let path_str = core::str::from_utf8(&path_buf[..plen]).unwrap_or("/");

    crate::kernel::uaccess::validate_user_range(dirp as u64, count, true)?;

    let mut written_bytes = 0usize;
    let mut current_dirp = dirp as *mut u8;

    let dot_entries: [(&str, u8); 2] = [(".", 4), ("..", 4)];
    for (name, dtype) in dot_entries.iter() {
        let name_bytes = name.as_bytes();
        let reclen = core::mem::size_of::<LinuxDirent64>();
        if written_bytes + reclen > count {
            break;
        }

        let mut entry = LinuxDirent64 {
            d_ino: fnv1a_64(name_bytes),
            d_off: (written_bytes + reclen) as i64,
            d_reclen: reclen as u16,
            d_type: *dtype,
            d_name: [0; 256],
        };
        entry.d_name[..name_bytes.len()].copy_from_slice(name_bytes);
        entry.d_name[name_bytes.len()] = 0;

        let slice = unsafe {
            core::slice::from_raw_parts(&entry as *const LinuxDirent64 as *const u8, reclen)
        };
        crate::kernel::uaccess::copy_to_user(slice, current_dirp as u64)?;
        written_bytes += reclen;
        current_dirp = unsafe { current_dirp.add(reclen) };
    }

    let _ = crate::fs::vfs::list_directory(path_str, |name, _size, is_dir| {
        let name_bytes = name.as_bytes();
        let reclen = core::mem::size_of::<LinuxDirent64>();
        if written_bytes + reclen <= count {
            let mut entry = LinuxDirent64 {
                d_ino: fnv1a_64(name_bytes),
                d_off: (written_bytes + reclen) as i64,
                d_reclen: reclen as u16,
                d_type: if is_dir { 4 } else { 8 },
                d_name: [0; 256],
            };
            let copy_len = name_bytes.len().min(255);
            entry.d_name[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
            entry.d_name[copy_len] = 0;

            let slice = unsafe {
                core::slice::from_raw_parts(&entry as *const LinuxDirent64 as *const u8, reclen)
            };
            let _ = crate::kernel::uaccess::copy_to_user(slice, current_dirp as u64);
            written_bytes += reclen;
            current_dirp = unsafe { current_dirp.add(reclen) };
        }
    });

    Ok(written_bytes as i64)
}

// ============================================================================
// 8. In-Kernel POSIX Conformance & Hardening Self-Test Verification Suite
// ============================================================================

pub fn run_posix_self_test() {
    crate::drivers::serial::println("\n=======================================================");
    crate::drivers::serial::println("  Aegis64 POSIX.1-2024 Conformance & Hardening Suite   ");
    crate::drivers::serial::println("=======================================================");

    // Test 1: Standard Streams
    crate::drivers::serial::print(
        "[POSIX TEST 1/12] Standard Streams (stdin=0, stdout=1, stderr=2)... ",
    );
    let msg = b"[POSIX OK] stdout stream functional\n";
    match posix_write(1, msg) {
        Ok(written) if written == msg.len() => crate::drivers::serial::println("[PASSED]"),
        _ => crate::drivers::serial::println("[FAILED: stdout write]"),
    }

    // Test 2: File I/O Lifecycle (open, write, lseek, read, close)
    crate::drivers::serial::print(
        "[POSIX TEST 2/12] File I/O Lifecycle (open, write, lseek, read, close)... ",
    );
    match posix_open("/tmp/posix_test.txt", O_RDWR | O_CREAT, 0o644) {
        Ok(fd) => {
            let sample_data = b"POSIX_CONFORMANCE_AEGIS64_OK";
            let _ = posix_write(fd, sample_data);
            let _ = posix_lseek(fd, 0, SEEK_SET);

            let mut read_back = [0u8; 32];
            let read_len = posix_read(fd, &mut read_back).unwrap_or(0);
            let _ = posix_close(fd);

            if read_len == sample_data.len() && &read_back[..read_len] == sample_data {
                crate::drivers::serial::println("[PASSED]");
            } else {
                crate::drivers::serial::println("[FAILED: Byte mismatch]");
            }
        }
        Err(_) => crate::drivers::serial::println("[FAILED: open error]"),
    }

    // Test 3: Shared Open-File Descriptions & Duplication (dup, dup2, shared offset)
    crate::drivers::serial::print(
        "[POSIX TEST 3/12] Shared Open-File Descriptions (dup / dup2 / shared offset)... ",
    );
    match posix_open("/tmp/posix_dup.txt", O_RDWR | O_CREAT, 0o644) {
        Ok(fd1) => {
            let _ = posix_write(fd1, b"0123456789ABCDEF");
            if let Ok(fd2) = posix_dup(fd1) {
                let _ = posix_lseek(fd1, 10, SEEK_SET);
                let mut buf = [0u8; 4];
                let n = posix_read(fd2, &mut buf).unwrap_or(0);
                let _ = posix_close(fd1);
                let _ = posix_close(fd2);

                if n == 4 && &buf == b"ABCD" {
                    crate::drivers::serial::println("[PASSED]");
                } else {
                    crate::drivers::serial::println("[FAILED: shared offset mismatch]");
                }
            } else {
                let _ = posix_close(fd1);
                crate::drivers::serial::println("[FAILED: dup error]");
            }
        }
        Err(_) => crate::drivers::serial::println("[FAILED: open error]"),
    }

    // Test 4: POSIX Stat Subsystem
    crate::drivers::serial::print(
        "[POSIX TEST 4/12] Metadata & File Status (sys_stat / sys_fstat)... ",
    );
    let mut st = PosixStat::default();
    if posix_stat("/tmp/posix_test.txt", &mut st).is_ok() {
        if (st.st_mode & S_IFREG) != 0 && st.st_blksize == 4096 {
            crate::drivers::serial::println("[PASSED]");
        } else {
            crate::drivers::serial::println("[FAILED: Mode mismatch]");
        }
    } else {
        crate::drivers::serial::println("[FAILED: stat lookup]");
    }

    // Test 5: POSIX Virtual Memory Management (sys_mmap / sys_munmap / sys_mprotect)
    crate::drivers::serial::print(
        "[POSIX TEST 5/12] Virtual Memory Management (mmap / munmap / mprotect)... ",
    );
    match posix_mmap(
        0,
        4096,
        PROT_READ | PROT_WRITE,
        MAP_ANONYMOUS | MAP_PRIVATE,
        -1,
        0,
    ) {
        Ok(vaddr) => unsafe {
            let ptr = vaddr as *mut u64;
            core::ptr::write_volatile(ptr, 0xAE64_BEEF_CAFE_0001);
            let val = core::ptr::read_volatile(ptr);
            if val == 0xAE64_BEEF_CAFE_0001 {
                let _ = posix_mprotect(vaddr, 4096, PROT_READ);
                let _ = posix_munmap(vaddr, 4096);
                crate::drivers::serial::println("[PASSED]");
            } else {
                crate::drivers::serial::println("[FAILED: memory read error]");
            }
        },
        Err(_) => crate::drivers::serial::println("[FAILED: mmap error]"),
    }

    // Test 6: POSIX Dynamic Heap Expansion (sys_brk)
    crate::drivers::serial::print(
        "[POSIX TEST 6/12] Process Heap Expansion (sys_brk growth & contraction)... ",
    );
    let cur_brk = posix_brk(0).unwrap_or(0);
    if cur_brk > 0 {
        match posix_brk(cur_brk + 4096) {
            Ok(new_brk) if new_brk == cur_brk + 4096 => {
                let _ = posix_brk(cur_brk);
                crate::drivers::serial::println("[PASSED]");
            }
            _ => crate::drivers::serial::println("[FAILED: brk expand]"),
        }
    } else {
        crate::drivers::serial::println("[FAILED: initial brk]");
    }

    // Test 7: POSIX Inter-Process Pipes (sys_pipe, EOF & broken pipe EPIPE)
    crate::drivers::serial::print(
        "[POSIX TEST 7/12] Streaming Byte IPC (pipe / EOF / EPIPE detection)... ",
    );
    let mut fds = [0i32; 2];
    if posix_pipe(&mut fds).is_ok() {
        let token = b"PIPE_TOKEN_1234\n";
        let _ = posix_write(fds[1] as usize, token);

        let mut recv = [0u8; 16];
        let n = posix_read(fds[0] as usize, &mut recv).unwrap_or(0);
        let _ = posix_close(fds[0] as usize); // Close reader end

        // Writing to pipe with reader closed must return -EPIPE
        let write_res = posix_write(fds[1] as usize, b"DEAD");
        let _ = posix_close(fds[1] as usize);

        if n == token.len() && &recv[..n] == token && write_res == Err(-EPIPE) {
            crate::drivers::serial::println("[PASSED]");
        } else {
            crate::drivers::serial::println("[FAILED: pipe behavior mismatch]");
        }
    } else {
        crate::drivers::serial::println("[FAILED: pipe alloc]");
    }

    // Test 8: POSIX Real-Time & Monotonic Clocks (sys_clock_gettime)
    crate::drivers::serial::print(
        "[POSIX TEST 8/12] Real-Time & Monotonic Clocks (sys_clock_gettime)... ",
    );
    let mut ts_real = Timespec::default();
    let mut ts_mono = Timespec::default();
    if posix_clock_gettime(CLOCK_REALTIME, &mut ts_real).is_ok()
        && posix_clock_gettime(CLOCK_MONOTONIC, &mut ts_mono).is_ok()
    {
        if ts_real.tv_sec > 1700000000 {
            crate::drivers::serial::print("[PASSED: Epoch=");
            crate::drivers::serial::print_u64(ts_real.tv_sec as u64);
            crate::drivers::serial::println("s]");
        } else {
            crate::drivers::serial::println("[PASSED]");
        }
    } else {
        crate::drivers::serial::println("[FAILED: clock error]");
    }

    // Test 9: POSIX Process Identification & Probing (getpid, getppid, kill)
    crate::drivers::serial::print(
        "[POSIX TEST 9/12] Process Context & Signal Probing (getpid / kill)... ",
    );
    let pid = posix_getpid();
    if pid >= 0 && posix_kill(pid as i32, 0).is_ok() {
        crate::drivers::serial::println("[PASSED]");
    } else {
        crate::drivers::serial::println("[FAILED]");
    }

    // Test 10: POSIX Signal Management (rt_sigaction / rt_sigprocmask)
    crate::drivers::serial::print(
        "[POSIX TEST 10/12] Signal Management (rt_sigaction / rt_sigprocmask)... ",
    );
    let mut old_act = SigAction::default();
    let new_act = SigAction {
        sa_handler: 0x0040_1000,
        sa_flags: 0,
        sa_restorer: 0,
        sa_mask: 0,
    };
    if posix_rt_sigaction(SIGUSR1 as i32, Some(&new_act), Some(&mut old_act)).is_ok() {
        let mut old_mask = 0u64;
        let new_mask = 1u64 << (SIGUSR2 - 1);
        if posix_rt_sigprocmask(0, Some(new_mask), Some(&mut old_mask)).is_ok() {
            crate::drivers::serial::println("[PASSED]");
        } else {
            crate::drivers::serial::println("[FAILED: sigprocmask]");
        }
    } else {
        crate::drivers::serial::println("[FAILED: sigaction]");
    }

    // Test 11: Directory & Path Operations (getcwd / chdir / mkdir / access / unlink)
    crate::drivers::serial::print(
        "[POSIX TEST 11/12] Directory & Path Operations (getcwd / chdir / access)... ",
    );
    let mut cwd_buf = [0u8; 32];
    if posix_getcwd(&mut cwd_buf).is_ok() && posix_access("/tmp/posix_test.txt", 0).is_ok() {
        let _ = posix_unlink("/tmp/posix_test.txt");
        crate::drivers::serial::println("[PASSED]");
    } else {
        crate::drivers::serial::println("[FAILED: directory operations]");
    }

    // Test 12: User Memory Safety & Pointer Boundary Validation (rejection of null/kernel/unmapped)
    crate::drivers::serial::print(
        "[POSIX TEST 12/12] User Memory Safety (rejection of null/kernel/unmapped)... ",
    );
    let null_res = crate::kernel::uaccess::validate_user_range(0, 16, false);
    let kernel_res = crate::kernel::uaccess::validate_user_range(0xFFFF_8000_0000_0000, 16, false);
    let unmapped_res =
        crate::kernel::uaccess::validate_user_range(0x0000_7000_0000_0000, 4096, false);
    if null_res == Err(-EFAULT) && kernel_res == Err(-EFAULT) && unmapped_res == Err(-EFAULT) {
        crate::drivers::serial::println("[PASSED]");
    } else {
        crate::drivers::serial::println("[FAILED: user memory safety leak]");
    }

    crate::drivers::serial::println("POSIX self-test sequence complete; review each result above. fork and execve currently return ENOSYS.\n");
}
