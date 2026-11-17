use crate::arch::x86_64::cpu::{rdmsr, wrmsr};
use crate::kernel::posix::{self, PosixStat, Timespec};

const MSR_EFER: u32 = 0xC0000080;
const MSR_STAR: u32 = 0xC0000081;
const MSR_LSTAR: u32 = 0xC0000082;
const MSR_SFMASK: u32 = 0xC0000084;
const MSR_KERNEL_GS_BASE: u32 = 0xC0000102;
const EFER_SCE: u64 = 0x00000001;

// Standard Linux / POSIX x86_64 Syscall Numbers
pub const SYS_READ: u64            = 0;
pub const SYS_WRITE: u64           = 1;
pub const SYS_OPEN: u64            = 2;
pub const SYS_CLOSE: u64           = 3;
pub const SYS_STAT: u64            = 4;
pub const SYS_FSTAT: u64           = 5;
pub const SYS_LSEEK: u64           = 8;
pub const SYS_MMAP: u64            = 9;
pub const SYS_MPROTECT: u64        = 10;
pub const SYS_MUNMAP: u64          = 11;
pub const SYS_BRK: u64             = 12;
pub const SYS_RT_SIGACTION: u64    = 13;
pub const SYS_RT_SIGPROCMASK: u64  = 14;
pub const SYS_IOCTL: u64           = 16;
pub const SYS_PIPE: u64            = 22;
pub const SYS_SCHED_YIELD: u64     = 24;
pub const SYS_DUP: u64             = 32;
pub const SYS_DUP2: u64            = 33;
pub const SYS_PAUSE: u64           = 34;
pub const SYS_NANOSLEEP: u64       = 35;
pub const SYS_GETPID: u64          = 39;
pub const SYS_FORK: u64            = 57;
pub const SYS_EXECVE: u64          = 59;
pub const SYS_EXIT: u64            = 60;
pub const SYS_WAIT4: u64           = 61;
pub const SYS_KILL: u64            = 62;
pub const SYS_FCNTL: u64           = 72;
pub const SYS_GETCWD: u64          = 79;
pub const SYS_CHDIR: u64           = 80;
pub const SYS_MKDIR: u64           = 83;
pub const SYS_RMDIR: u64           = 84;
pub const SYS_UNLINK: u64          = 87;
pub const SYS_GETUID: u64          = 102;
pub const SYS_GETGID: u64          = 104;
pub const SYS_GETEUID: u64         = 107;
pub const SYS_GETEGID: u64         = 108;
pub const SYS_GETPPID: u64         = 110;
pub const SYS_CLOCK_GETTIME: u64   = 228;
pub const SYS_CLOCK_NANOSLEEP: u64 = 230;
pub const SYS_EXIT_GROUP: u64      = 231;

// Internal backward-compatible syscall aliases
pub const SYS_LEGACY_EXIT: u64     = 60;
pub const SYS_LEGACY_YIELD: u64    = 24;
pub const SYS_PRINT: u64           = 20;

#[repr(C)]
pub struct PerCpuSyscall {
    pub kernel_rsp: u64,
    pub user_rsp: u64,
}

static mut PER_CPU_SYSCALL: [PerCpuSyscall; 8] = [
    PerCpuSyscall { kernel_rsp: 0, user_rsp: 0 },
    PerCpuSyscall { kernel_rsp: 0, user_rsp: 0 },
    PerCpuSyscall { kernel_rsp: 0, user_rsp: 0 },
    PerCpuSyscall { kernel_rsp: 0, user_rsp: 0 },
    PerCpuSyscall { kernel_rsp: 0, user_rsp: 0 },
    PerCpuSyscall { kernel_rsp: 0, user_rsp: 0 },
    PerCpuSyscall { kernel_rsp: 0, user_rsp: 0 },
    PerCpuSyscall { kernel_rsp: 0, user_rsp: 0 },
];

extern "C" {
    fn syscall_entry_asm();
}

pub fn init() {
    let efer = rdmsr(MSR_EFER);
    wrmsr(MSR_EFER, efer | EFER_SCE);

    // STAR: User CS=0x10 (SYSRET loads CS=0x23, SS=0x1B), Kernel CS=0x08 (SYSCALL loads CS=0x08, SS=0x10)
    let star = (0x0010u64 << 48) | (0x0008u64 << 32);
    wrmsr(MSR_STAR, star);

    wrmsr(MSR_LSTAR, syscall_entry_asm as *const () as usize as u64);

    // SFMASK: Clear IF (0x200) on syscall entry to disable interrupts
    wrmsr(MSR_SFMASK, 0x200);

    // Configure per-CPU kernel GS base with dedicated kernel syscall stack
    let stack_top = crate::arch::x86_64::gdt::get_default_stack_top(0);
    unsafe {
        PER_CPU_SYSCALL[0].kernel_rsp = stack_top;
        PER_CPU_SYSCALL[0].user_rsp = 0;
        wrmsr(MSR_KERNEL_GS_BASE, &raw const PER_CPU_SYSCALL[0] as u64);
    }

    crate::drivers::serial::println("Syscall: Hardware SYSCALL/SYSRET MSRs initialized with secure stack switching");
}

pub fn init_ap(cpu_id: usize) {
    let efer = rdmsr(MSR_EFER);
    wrmsr(MSR_EFER, efer | EFER_SCE);
    let star = (0x0010u64 << 48) | (0x0008u64 << 32);
    wrmsr(MSR_STAR, star);
    wrmsr(MSR_LSTAR, syscall_entry_asm as *const () as usize as u64);
    wrmsr(MSR_SFMASK, 0x200);

    let cpu = cpu_id.min(7);
    let stack_top = crate::arch::x86_64::gdt::get_default_stack_top(cpu);
    unsafe {
        PER_CPU_SYSCALL[cpu].kernel_rsp = stack_top;
        PER_CPU_SYSCALL[cpu].user_rsp = 0;
        wrmsr(MSR_KERNEL_GS_BASE, &raw const PER_CPU_SYSCALL[cpu] as u64);
    }
}

pub fn validate_user_ptr(ptr: *const u8, len: usize) -> bool {
    if ptr.is_null() || len == 0 {
        return false;
    }
    let start = ptr as u64;
    let end = match start.checked_add(len as u64) {
        Some(e) => e,
        None => return false,
    };
    // User canonical range 0x00400000 .. 0x00007FFFFFFFF000
    if start < 0x00400000 || end > 0x0000_7FFF_FFFF_F000 {
        return false;
    }

    let pml4 = crate::mm::vmm::get_kernel_pml4();
    if pml4.is_null() {
        return false;
    }

    let page_start = start & !0xFFF;
    let page_end = (end + 0xFFF) & !0xFFF;
    let mut curr_page = page_start;

    while curr_page < page_end {
        match crate::mm::vmm::get_mapping(pml4, curr_page) {
            Some((_paddr, flags)) => {
                if (flags & crate::mm::vmm::PAGE_PRESENT) == 0
                    || (flags & crate::mm::vmm::PAGE_USER) == 0
                {
                    return false;
                }
            }
            None => return false,
        }
        curr_page = match curr_page.checked_add(crate::mm::pmm::PAGE_SIZE as u64) {
            Some(next) => next,
            None => return false,
        };
    }

    true
}

fn read_user_string(ptr: *const u8, max_len: usize) -> Option<&'static str> {
    if ptr.is_null() {
        return None;
    }
    let mut len = 0;
    unsafe {
        while len < max_len {
            let b = *ptr.add(len);
            if b == 0 {
                break;
            }
            len += 1;
        }
        let slice = core::slice::from_raw_parts(ptr, len);
        core::str::from_utf8(slice).ok()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn syscall_dispatch(
    sys_no: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
    a6: u64,
) -> i64 {
    match sys_no {
        // POSIX 0: sys_read(fd, buf, count)
        SYS_READ => {
            let fd = a1 as usize;
            let ptr = a2 as *mut u8;
            let count = a3 as usize;
            if ptr.is_null() { return -posix::EFAULT; }
            let slice = unsafe { core::slice::from_raw_parts_mut(ptr, count) };
            match posix::posix_read(fd, slice) {
                Ok(n) => n as i64,
                Err(e) => e,
            }
        }

        // POSIX 1: sys_write(fd, buf, count)
        SYS_WRITE => {
            let fd = a1 as usize;
            let ptr = a2 as *const u8;
            let count = a3 as usize;
            if ptr.is_null() { return -posix::EFAULT; }
            let slice = unsafe { core::slice::from_raw_parts(ptr, count) };
            match posix::posix_write(fd, slice) {
                Ok(n) => n as i64,
                Err(e) => e,
            }
        }

        // POSIX 2: sys_open(filename, flags, mode)
        SYS_OPEN => {
            let path_ptr = a1 as *const u8;
            let flags = a2 as u32;
            let mode = a3 as u32;
            if let Some(path) = read_user_string(path_ptr, 128) {
                match posix::posix_open(path, flags, mode) {
                    Ok(fd) => fd as i64,
                    Err(e) => e,
                }
            } else {
                -posix::EFAULT
            }
        }

        // POSIX 3: sys_close(fd)
        SYS_CLOSE => {
            match posix::posix_close(a1 as usize) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

        // POSIX 4: sys_stat(path, statbuf)
        SYS_STAT => {
            let path_ptr = a1 as *const u8;
            let stat_ptr = a2 as *mut PosixStat;
            if stat_ptr.is_null() { return -posix::EFAULT; }
            if let Some(path) = read_user_string(path_ptr, 128) {
                let statbuf = unsafe { &mut *stat_ptr };
                match posix::posix_stat(path, statbuf) {
                    Ok(()) => 0,
                    Err(e) => e,
                }
            } else {
                -posix::EFAULT
            }
        }

        // POSIX 5: sys_fstat(fd, statbuf)
        SYS_FSTAT => {
            let fd = a1 as usize;
            let stat_ptr = a2 as *mut PosixStat;
            if stat_ptr.is_null() { return -posix::EFAULT; }
            let statbuf = unsafe { &mut *stat_ptr };
            match posix::posix_fstat(fd, statbuf) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

        // POSIX 8: sys_lseek(fd, offset, whence)
        SYS_LSEEK => {
            let fd = a1 as usize;
            let offset = a2 as i64;
            let whence = a3 as i32;
            match posix::posix_lseek(fd, offset, whence) {
                Ok(pos) => pos,
                Err(e) => e,
            }
        }

        // POSIX 9: sys_mmap(addr, len, prot, flags, fd, offset)
        SYS_MMAP => {
            match posix::posix_mmap(a1, a2 as usize, a3 as i32, a4 as i32, a5 as i32, a6 as i64) {
                Ok(vaddr) => vaddr as i64,
                Err(e) => e,
            }
        }

        // POSIX 10: sys_mprotect(addr, len, prot)
        SYS_MPROTECT => 0,

        // POSIX 11: sys_munmap(addr, len)
        SYS_MUNMAP => {
            match posix::posix_munmap(a1, a2 as usize) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

        // POSIX 12: sys_brk(addr)
        SYS_BRK => {
            match posix::posix_brk(a1) {
                Ok(cur) => cur as i64,
                Err(e) => e,
            }
        }

        // POSIX 13: sys_rt_sigaction
        SYS_RT_SIGACTION => 0,

        // POSIX 14: sys_rt_sigprocmask
        SYS_RT_SIGPROCMASK => 0,

        // POSIX 16: sys_ioctl
        SYS_IOCTL => 0,

        // POSIX 22: sys_pipe(pipefd[2])
        SYS_PIPE => {
            let pipe_ptr = a1 as *mut [i32; 2];
            if pipe_ptr.is_null() { return -posix::EFAULT; }
            let pipe_slice = unsafe { &mut *pipe_ptr };
            match posix::posix_pipe(pipe_slice) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

        // POSIX 24: sys_sched_yield
        SYS_SCHED_YIELD => {
            crate::kernel::task::yield_now();
            0
        }

        // POSIX 32: sys_dup(oldfd)
        SYS_DUP => {
            match posix::posix_dup(a1 as usize) {
                Ok(newfd) => newfd as i64,
                Err(e) => e,
            }
        }

        // POSIX 33: sys_dup2(oldfd, newfd)
        SYS_DUP2 => {
            match posix::posix_dup2(a1 as usize, a2 as usize) {
                Ok(newfd) => newfd as i64,
                Err(e) => e,
            }
        }

        // POSIX 34: sys_pause
        SYS_PAUSE => {
            crate::kernel::task::yield_now();
            0
        }

        // POSIX 35: sys_nanosleep(req, rem)
        SYS_NANOSLEEP => {
            let req_ptr = a1 as *const Timespec;
            if req_ptr.is_null() { return -posix::EFAULT; }
            let req = unsafe { &*req_ptr };
            match posix::posix_nanosleep(req, None) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

        // POSIX 39: sys_getpid
        SYS_GETPID => posix::posix_getpid(),

        // POSIX 57: sys_fork
        SYS_FORK => {
            match posix::posix_fork() {
                Ok(child_pid) => child_pid as i64,
                Err(e) => e,
            }
        }

        // POSIX 59: sys_execve
        SYS_EXECVE => 0,

        // POSIX 60: sys_exit(status)
        SYS_EXIT => {
            crate::kernel::task::exit();
        }

        // POSIX 61: sys_wait4(pid, status, options, rusage)
        SYS_WAIT4 => {
            let target_pid = a1 as i32;
            let status_ptr = a2 as *mut i32;
            let options = a3 as i32;
            match posix::posix_wait4(target_pid, status_ptr, options) {
                Ok(pid) => pid as i64,
                Err(e) => e,
            }
        }

        // POSIX 62: sys_kill(pid, sig)
        SYS_KILL => {
            let pid = a1 as i32;
            let sig = a2 as i32;
            match posix::posix_kill(pid, sig) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

        // POSIX 72: sys_fcntl
        SYS_FCNTL => 0,

        // POSIX 79: sys_getcwd(buf, size)
        SYS_GETCWD => {
            let ptr = a1 as *mut u8;
            let size = a2 as usize;
            if ptr.is_null() { return -posix::EFAULT; }
            let slice = unsafe { core::slice::from_raw_parts_mut(ptr, size) };
            match posix::posix_getcwd(slice) {
                Ok(len) => len as i64,
                Err(e) => e,
            }
        }

        // POSIX 80: sys_chdir(path)
        SYS_CHDIR => {
            let ptr = a1 as *const u8;
            if let Some(path) = read_user_string(ptr, 128) {
                match posix::posix_chdir(path) {
                    Ok(()) => 0,
                    Err(e) => e,
                }
            } else {
                -posix::EFAULT
            }
        }

        // POSIX 83: sys_mkdir(path, mode)
        SYS_MKDIR => {
            let ptr = a1 as *const u8;
            let mode = a2 as u32;
            if let Some(path) = read_user_string(ptr, 128) {
                match posix::posix_mkdir(path, mode) {
                    Ok(()) => 0,
                    Err(e) => e,
                }
            } else {
                -posix::EFAULT
            }
        }

        // POSIX 84: sys_rmdir(path)
        SYS_RMDIR => {
            let ptr = a1 as *const u8;
            if let Some(path) = read_user_string(ptr, 128) {
                match posix::posix_rmdir(path) {
                    Ok(()) => 0,
                    Err(e) => e,
                }
            } else {
                -posix::EFAULT
            }
        }

        // POSIX 87: sys_unlink(path)
        SYS_UNLINK => {
            let ptr = a1 as *const u8;
            if let Some(path) = read_user_string(ptr, 128) {
                match posix::posix_unlink(path) {
                    Ok(()) => 0,
                    Err(e) => e,
                }
            } else {
                -posix::EFAULT
            }
        }

        // POSIX 102, 104, 107, 108: User & Group IDs
        SYS_GETUID => posix::posix_getuid(),
        SYS_GETGID => posix::posix_getgid(),
        SYS_GETEUID => posix::posix_geteuid(),
        SYS_GETEGID => posix::posix_getegid(),
        SYS_GETPPID => posix::posix_getppid(),

        // POSIX 228: sys_clock_gettime(clock_id, tp)
        SYS_CLOCK_GETTIME => {
            let clock_id = a1 as i32;
            let tp_ptr = a2 as *mut Timespec;
            if tp_ptr.is_null() { return -posix::EFAULT; }
            let tp = unsafe { &mut *tp_ptr };
            match posix::posix_clock_gettime(clock_id, tp) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

        // POSIX 230: sys_clock_nanosleep(clock_id, flags, req, rem)
        SYS_CLOCK_NANOSLEEP => {
            let req_ptr = a3 as *const Timespec;
            if req_ptr.is_null() { return -posix::EFAULT; }
            let req = unsafe { &*req_ptr };
            match posix::posix_nanosleep(req, None) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

        // POSIX 231: sys_exit_group(status)
        SYS_EXIT_GROUP => {
            crate::kernel::task::exit();
        }

        // Backward compatibility: Print message
        SYS_PRINT => {
            let ptr = a1 as *const u8;
            let len = (a2 as usize).min(1024);
            if validate_user_ptr(ptr, len) {
                let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
                if let Ok(s) = core::str::from_utf8(slice) {
                    crate::drivers::serial::print("[USER APP] ");
                    crate::drivers::serial::println(s);
                    len as i64
                } else {
                    -posix::EINVAL
                }
            } else {
                -posix::EFAULT
            }
        }

        _ => -posix::ENOSYS,
    }
}
