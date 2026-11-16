use crate::arch::x86_64::cpu::{rdmsr, wrmsr};

const MSR_EFER: u32 = 0xC0000080;
const MSR_STAR: u32 = 0xC0000081;
const MSR_LSTAR: u32 = 0xC0000082;
const MSR_SFMASK: u32 = 0xC0000084;
const EFER_SCE: u64 = 0x00000001;

pub const SYS_EXIT: u64 = 1;
pub const SYS_THREAD_YIELD: u64 = 2;
pub const SYS_SLEEP_MS: u64 = 3;
pub const SYS_GET_TICKS: u64 = 4;
pub const SYS_GET_TIME_MS: u64 = 5;
pub const SYS_MMAP: u64 = 6;
pub const SYS_MUNMAP: u64 = 7;
pub const SYS_OPEN: u64 = 8;
pub const SYS_READ: u64 = 9;
pub const SYS_WRITE: u64 = 10;
pub const SYS_CLOSE: u64 = 11;
pub const SYS_STAT: u64 = 12;
pub const SYS_PRINT: u64 = 20;

extern "C" {
    fn syscall_entry_asm();
}

pub fn init() {
    let efer = rdmsr(MSR_EFER);
    wrmsr(MSR_EFER, efer | EFER_SCE);

    // STAR: User CS=0x10 (SYSRET loads CS=0x23, SS=0x1B), Kernel CS=0x08 (SYSCALL loads CS=0x08, SS=0x10)
    let star = (0x0010u64 << 48) | (0x0008u64 << 32);
    wrmsr(MSR_STAR, star);

    wrmsr(MSR_LSTAR, syscall_entry_asm as usize as u64);

    // SFMASK: Clear IF (0x200) on syscall entry to disable interrupts
    wrmsr(MSR_SFMASK, 0x200);

    crate::drivers::serial::println("Syscall: Hardware SYSCALL/SYSRET MSRs initialized");
}

pub fn init_ap() {
    let efer = rdmsr(MSR_EFER);
    wrmsr(MSR_EFER, efer | EFER_SCE);
    let star = (0x0010u64 << 48) | (0x0008u64 << 32);
    wrmsr(MSR_STAR, star);
    wrmsr(MSR_LSTAR, syscall_entry_asm as usize as u64);
    wrmsr(MSR_SFMASK, 0x200);
}

pub fn validate_user_ptr(ptr: *const u8, len: usize) -> bool {
    if ptr.is_null() {
        return false;
    }
    let start = ptr as u64;
    let end = match start.checked_add(len as u64) {
        Some(e) => e,
        None => return false,
    };
    // User canonical range 0x00400000 .. 0x00007FFFFFFFF000
    start >= 0x00400000 && end <= 0x0000_7FFF_FFFF_F000
}

#[unsafe(no_mangle)]
pub extern "C" fn syscall_dispatch(
    sys_no: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    _a4: u64,
    _a5: u64,
    _a6: u64,
) -> i64 {
    match sys_no {
        SYS_EXIT => {
            crate::kernel::task::exit();
        }
        SYS_THREAD_YIELD => {
            crate::kernel::task::yield_now();
            0
        }
        SYS_SLEEP_MS => {
            crate::kernel::task::sleep(a1 / 10);
            0
        }
        SYS_GET_TICKS => {
            crate::drivers::pit::get_ticks() as i64
        }
        SYS_GET_TIME_MS => {
            (crate::drivers::pit::get_ticks() * 10) as i64
        }
        SYS_PRINT => {
            let ptr = a1 as *const u8;
            let len = (a2 as usize).min(1024);
            if validate_user_ptr(ptr, len) {
                let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
                if let Ok(s) = core::str::from_utf8(slice) {
                    crate::drivers::serial::print("[USER APP] ");
                    crate::drivers::serial::println(s);
                }
                0
            } else {
                -1 // EFAULT
            }
        }
        _ => -38, // ENOSYS
    }
}
