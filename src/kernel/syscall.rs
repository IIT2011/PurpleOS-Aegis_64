use crate::arch::x86_64::cpu::{rdmsr, wrmsr};
use crate::kernel::posix::{self, LinuxDirent64, PosixStat, SigAction, Timespec};
use crate::kernel::uaccess;

const MSR_EFER: u32 = 0xC0000080;
const MSR_STAR: u32 = 0xC0000081;
const MSR_LSTAR: u32 = 0xC0000082;
const MSR_SFMASK: u32 = 0xC0000084;
const MSR_KERNEL_GS_BASE: u32 = 0xC0000102;
const EFER_SCE: u64 = 0x00000001;

// Standard Linux / POSIX.1-2024 x86_64 Syscall Numbers
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
pub const SYS_ACCESS: u64          = 21;
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
pub const SYS_UMASK: u64           = 95;
pub const SYS_GETUID: u64          = 102;
pub const SYS_GETGID: u64          = 104;
pub const SYS_GETEUID: u64         = 107;
pub const SYS_GETEGID: u64         = 108;
pub const SYS_GETPPID: u64         = 110;
pub const SYS_GETDENTS64: u64      = 217;
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
    uaccess::validate_user_range(ptr as u64, len, false).is_ok()
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
            let user_buf = a2;
            let count = a3 as usize;
            if count == 0 {
                return 0;
            }
            if let Err(e) = uaccess::validate_user_range(user_buf, count, true) {
                return e;
            }
            let mut kbuf = [0u8; 4096];
            let to_read = count.min(kbuf.len());
            match posix::posix_read(fd, &mut kbuf[..to_read]) {
                Ok(n) => {
                    if n > 0 {
                        if let Err(e) = uaccess::copy_to_user(&kbuf[..n], user_buf) {
                            return e;
                        }
                    }
                    n as i64
                }
                Err(e) => e,
            }
        }

        // POSIX 1: sys_write(fd, buf, count)
        SYS_WRITE => {
            let fd = a1 as usize;
            let user_buf = a2;
            let count = a3 as usize;
            if count == 0 {
                return 0;
            }
            let mut kbuf = [0u8; 4096];
            let to_write = count.min(kbuf.len());
            if let Err(e) = uaccess::copy_from_user(user_buf, &mut kbuf[..to_write]) {
                return e;
            }
            match posix::posix_write(fd, &kbuf[..to_write]) {
                Ok(n) => n as i64,
                Err(e) => e,
            }
        }

        // POSIX 2: sys_open(filename, flags, mode)
        SYS_OPEN => {
            let path_ptr = a1;
            let flags = a2 as u32;
            let mode = a3 as u32;
            let mut path_buf = [0u8; 128];
            match uaccess::copy_str_from_user(path_ptr, &mut path_buf) {
                Ok(len) => {
                    if let Ok(path) = core::str::from_utf8(&path_buf[..len]) {
                        match posix::posix_open(path, flags, mode) {
                            Ok(fd) => fd as i64,
                            Err(e) => e,
                        }
                    } else {
                        -posix::EINVAL
                    }
                }
                Err(e) => e,
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
            let path_ptr = a1;
            let stat_ptr = a2;
            let mut path_buf = [0u8; 128];
            match uaccess::copy_str_from_user(path_ptr, &mut path_buf) {
                Ok(len) => {
                    if let Ok(path) = core::str::from_utf8(&path_buf[..len]) {
                        let mut statbuf = PosixStat::default();
                        match posix::posix_stat(path, &mut statbuf) {
                            Ok(()) => {
                                let slice = unsafe {
                                    core::slice::from_raw_parts(&statbuf as *const PosixStat as *const u8, core::mem::size_of::<PosixStat>())
                                };
                                match uaccess::copy_to_user(slice, stat_ptr) {
                                    Ok(()) => 0,
                                    Err(e) => e,
                                }
                            }
                            Err(e) => e,
                        }
                    } else {
                        -posix::EINVAL
                    }
                }
                Err(e) => e,
            }
        }

        // POSIX 5: sys_fstat(fd, statbuf)
        SYS_FSTAT => {
            let fd = a1 as usize;
            let stat_ptr = a2;
            let mut statbuf = PosixStat::default();
            match posix::posix_fstat(fd, &mut statbuf) {
                Ok(()) => {
                    let slice = unsafe {
                        core::slice::from_raw_parts(&statbuf as *const PosixStat as *const u8, core::mem::size_of::<PosixStat>())
                    };
                    match uaccess::copy_to_user(slice, stat_ptr) {
                        Ok(()) => 0,
                        Err(e) => e,
                    }
                }
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
        SYS_MPROTECT => {
            match posix::posix_mprotect(a1, a2 as usize, a3 as i32) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }

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

        // POSIX 13: sys_rt_sigaction(signum, act, oldact, sigsetsize)
        SYS_RT_SIGACTION => {
            let signum = a1 as i32;
            let act_ptr = a2;
            let oldact_ptr = a3;

            let mut act_val = SigAction::default();
            let act = if act_ptr != 0 {
                let slice = unsafe {
                    core::slice::from_raw_parts_mut(&mut act_val as *mut SigAction as *mut u8, core::mem::size_of::<SigAction>())
                };
                if let Err(e) = uaccess::copy_from_user(act_ptr, slice) {
                    return e;
                }
                Some(&act_val)
            } else {
                None
            };

            let mut oldact_val = SigAction::default();
            let oldact = if oldact_ptr != 0 {
                Some(&mut oldact_val)
            } else {
                None
            };

            match posix::posix_rt_sigaction(signum, act, oldact) {
                Ok(()) => {
                    if oldact_ptr != 0 {
                        let slice = unsafe {
                            core::slice::from_raw_parts(&oldact_val as *const SigAction as *const u8, core::mem::size_of::<SigAction>())
                        };
                        if let Err(e) = uaccess::copy_to_user(slice, oldact_ptr) {
                            return e;
                        }
                    }
                    0
                }
                Err(e) => e,
            }
        }

        // POSIX 14: sys_rt_sigprocmask(how, set, oldset, sigsetsize)
        SYS_RT_SIGPROCMASK => {
            let how = a1 as i32;
            let set_ptr = a2;
            let oldset_ptr = a3;

            let mut set_val = 0u64;
            let set = if set_ptr != 0 {
                let slice = unsafe {
                    core::slice::from_raw_parts_mut(&mut set_val as *mut u64 as *mut u8, 8)
                };
                if let Err(e) = uaccess::copy_from_user(set_ptr, slice) {
                    return e;
                }
                Some(set_val)
            } else {
                None
            };

            let mut oldset_val = 0u64;
            let oldset = if oldset_ptr != 0 {
                Some(&mut oldset_val)
            } else {
                None
            };

            match posix::posix_rt_sigprocmask(how, set, oldset) {
                Ok(()) => {
                    if oldset_ptr != 0 {
                        let slice = unsafe {
                            core::slice::from_raw_parts(&oldset_val as *const u64 as *const u8, 8)
                        };
                        if let Err(e) = uaccess::copy_to_user(slice, oldset_ptr) {
                            return e;
                        }
                    }
                    0
                }
                Err(e) => e,
            }
        }

        // POSIX 16: sys_ioctl(fd, req, arg)
        SYS_IOCTL => {
            let fd = a1 as usize;
            let req = a2;
            let arg = a3;
            match posix::posix_ioctl(fd, req, arg) {
                Ok(res) => res,
                Err(e) => e,
            }
        }

        // POSIX 21: sys_access(path, mode)
        SYS_ACCESS => {
            let path_ptr = a1;
            let mode = a2 as i32;
            let mut path_buf = [0u8; 128];
            match uaccess::copy_str_from_user(path_ptr, &mut path_buf) {
                Ok(len) => {
                    if let Ok(path) = core::str::from_utf8(&path_buf[..len]) {
                        match posix::posix_access(path, mode) {
                            Ok(()) => 0,
                            Err(e) => e,
                        }
                    } else {
                        -posix::EINVAL
                    }
                }
                Err(e) => e,
            }
        }

        // POSIX 22: sys_pipe(pipefd[2])
        SYS_PIPE => {
            let pipe_ptr = a1;
            let mut pipe_fds = [0i32; 2];
            match posix::posix_pipe(&mut pipe_fds) {
                Ok(()) => {
                    let slice = unsafe {
                        core::slice::from_raw_parts(&pipe_fds as *const [i32; 2] as *const u8, 8)
                    };
                    match uaccess::copy_to_user(slice, pipe_ptr) {
                        Ok(()) => 0,
                        Err(e) => e,
                    }
                }
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
            let req_ptr = a1;
            let mut req = Timespec::default();
            let slice = unsafe {
                core::slice::from_raw_parts_mut(&mut req as *mut Timespec as *mut u8, core::mem::size_of::<Timespec>())
            };
            if let Err(e) = uaccess::copy_from_user(req_ptr, slice) {
                return e;
            }
            match posix::posix_nanosleep(&req, None) {
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

        // POSIX 59: sys_execve(filename, argv, envp)
        SYS_EXECVE => {
            let path_ptr = a1;
            let mut path_buf = [0u8; 128];
            match uaccess::copy_str_from_user(path_ptr, &mut path_buf) {
                Ok(len) => {
                    if let Ok(path) = core::str::from_utf8(&path_buf[..len]) {
                        match posix::posix_execve(path, &[], &[]) {
                            Ok(()) => 0,
                            Err(e) => e,
                        }
                    } else {
                        -posix::EINVAL
                    }
                }
                Err(e) => e,
            }
        }

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

        // POSIX 72: sys_fcntl(fd, cmd, arg)
        SYS_FCNTL => {
            let fd = a1 as usize;
            let cmd = a2 as i32;
            let arg = a3;
            match posix::posix_fcntl(fd, cmd, arg) {
                Ok(res) => res,
                Err(e) => e,
            }
        }

        // POSIX 79: sys_getcwd(buf, size)
        SYS_GETCWD => {
            let buf_ptr = a1;
            let size = a2 as usize;
            if size == 0 {
                return -posix::EINVAL;
            }
            let mut kbuf = [0u8; 128];
            match posix::posix_getcwd(&mut kbuf[..size.min(128)]) {
                Ok(len) => {
                    if let Err(e) = uaccess::copy_to_user(&kbuf[..len + 1], buf_ptr) {
                        return e;
                    }
                    len as i64
                }
                Err(e) => e,
            }
        }

        // POSIX 80: sys_chdir(path)
        SYS_CHDIR => {
            let path_ptr = a1;
            let mut path_buf = [0u8; 128];
            match uaccess::copy_str_from_user(path_ptr, &mut path_buf) {
                Ok(len) => {
                    if let Ok(path) = core::str::from_utf8(&path_buf[..len]) {
                        match posix::posix_chdir(path) {
                            Ok(()) => 0,
                            Err(e) => e,
                        }
                    } else {
                        -posix::EINVAL
                    }
                }
                Err(e) => e,
            }
        }

        // POSIX 83: sys_mkdir(path, mode)
        SYS_MKDIR => {
            let path_ptr = a1;
            let mode = a2 as u32;
            let mut path_buf = [0u8; 128];
            match uaccess::copy_str_from_user(path_ptr, &mut path_buf) {
                Ok(len) => {
                    if let Ok(path) = core::str::from_utf8(&path_buf[..len]) {
                        match posix::posix_mkdir(path, mode) {
                            Ok(()) => 0,
                            Err(e) => e,
                        }
                    } else {
                        -posix::EINVAL
                    }
                }
                Err(e) => e,
            }
        }

        // POSIX 84: sys_rmdir(path)
        SYS_RMDIR => {
            let path_ptr = a1;
            let mut path_buf = [0u8; 128];
            match uaccess::copy_str_from_user(path_ptr, &mut path_buf) {
                Ok(len) => {
                    if let Ok(path) = core::str::from_utf8(&path_buf[..len]) {
                        match posix::posix_rmdir(path) {
                            Ok(()) => 0,
                            Err(e) => e,
                        }
                    } else {
                        -posix::EINVAL
                    }
                }
                Err(e) => e,
            }
        }

        // POSIX 87: sys_unlink(path)
        SYS_UNLINK => {
            let path_ptr = a1;
            let mut path_buf = [0u8; 128];
            match uaccess::copy_str_from_user(path_ptr, &mut path_buf) {
                Ok(len) => {
                    if let Ok(path) = core::str::from_utf8(&path_buf[..len]) {
                        match posix::posix_unlink(path) {
                            Ok(()) => 0,
                            Err(e) => e,
                        }
                    } else {
                        -posix::EINVAL
                    }
                }
                Err(e) => e,
            }
        }

        // POSIX 95: sys_umask(mask)
        SYS_UMASK => {
            posix::posix_umask(a1 as u32) as i64
        }

        // POSIX 102, 104, 107, 108: User & Group IDs
        SYS_GETUID => posix::posix_getuid(),
        SYS_GETGID => posix::posix_getgid(),
        SYS_GETEUID => posix::posix_geteuid(),
        SYS_GETEGID => posix::posix_getegid(),
        SYS_GETPPID => posix::posix_getppid(),

        // POSIX 217: sys_getdents64(fd, dirp, count)
        SYS_GETDENTS64 => {
            let fd = a1 as usize;
            let dirp = a2 as *mut LinuxDirent64;
            let count = a3 as usize;
            match posix::posix_getdents64(fd, dirp, count) {
                Ok(n) => n,
                Err(e) => e,
            }
        }

        // POSIX 228: sys_clock_gettime(clock_id, tp)
        SYS_CLOCK_GETTIME => {
            let clock_id = a1 as i32;
            let tp_ptr = a2;
            let mut tp = Timespec::default();
            match posix::posix_clock_gettime(clock_id, &mut tp) {
                Ok(()) => {
                    let slice = unsafe {
                        core::slice::from_raw_parts(&tp as *const Timespec as *const u8, core::mem::size_of::<Timespec>())
                    };
                    match uaccess::copy_to_user(slice, tp_ptr) {
                        Ok(()) => 0,
                        Err(e) => e,
                    }
                }
                Err(e) => e,
            }
        }

        // POSIX 230: sys_clock_nanosleep(clock_id, flags, req, rem)
        SYS_CLOCK_NANOSLEEP => {
            let req_ptr = a3;
            let mut req = Timespec::default();
            let slice = unsafe {
                core::slice::from_raw_parts_mut(&mut req as *mut Timespec as *mut u8, core::mem::size_of::<Timespec>())
            };
            if let Err(e) = uaccess::copy_from_user(req_ptr, slice) {
                return e;
            }
            match posix::posix_nanosleep(&req, None) {
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
            let ptr = a1;
            let len = (a2 as usize).min(1024);
            let mut kbuf = [0u8; 1024];
            if len == 0 {
                return 0;
            }
            match uaccess::copy_from_user(ptr, &mut kbuf[..len]) {
                Ok(()) => {
                    if let Ok(s) = core::str::from_utf8(&kbuf[..len]) {
                        crate::drivers::serial::print("[USER APP] ");
                        crate::drivers::serial::println(s);
                        len as i64
                    } else {
                        -posix::EINVAL
                    }
                }
                Err(e) => e,
            }
        }

        _ => -posix::ENOSYS,
    }
}
