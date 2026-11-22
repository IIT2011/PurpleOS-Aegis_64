// SPDX-License-Identifier: LGPL-2.1-or-later
//! Native PurpleOS Ring 3 Syscall Invocations in Pure Rust.

pub const SYS_PURPLE_CLOCK_NANOS: u64 = 0x1000;
pub const SYS_PURPLE_SLEEP_NANOS: u64 = 0x1001;

pub const SYS_PURPLE_AUDIO_STREAM_START: u64 = 0x1021;
pub const SYS_PURPLE_AUDIO_STREAM_STOP: u64  = 0x1022;
pub const SYS_PURPLE_AUDIO_SUBMIT_PCM: u64   = 0x1023;
pub const SYS_PURPLE_AUDIO_SET_VOLUME: u64   = 0x1024;

pub const SYS_PURPLE_FB_GET_INFO: u64        = 0x1000;
pub const SYS_PURPLE_GPU_PAGE_FLIP: u64      = 0x1003;
pub const SYS_PURPLE_GPU_CLEAR: u64          = 0x1004;
pub const SYS_PURPLE_GPU_RECT_FILL: u64      = 0x1005;
pub const SYS_PURPLE_GPU_BLIT: u64           = 0x1006;
pub const SYS_PURPLE_GPU_DRAW_LINE: u64      = 0x1007;
pub const SYS_PURPLE_GPU_SUBMIT_3D: u64      = 0x1008;
pub const SYS_PURPLE_GPU_GET_CAPS: u64       = 0x1009;

pub const SYS_GUI_GET_METRICS: u64    = 0x1100;
pub const SYS_GUI_CREATE_WINDOW: u64  = 0x1101;
pub const SYS_GUI_RESIZE_WINDOW: u64  = 0x1102;
pub const SYS_GUI_MAP_WINDOW: u64     = 0x1103;
pub const SYS_GUI_PRESENT: u64        = 0x1104;
pub const SYS_GUI_POLL: u64           = 0x1105;
pub const SYS_GUI_CLOSE_WINDOW: u64   = 0x1106;

#[inline(always)]
pub unsafe fn syscall0(n: u64) -> i64 {
    #[cfg(target_arch = "x86_64")]
    {
        let ret: i64;
        core::arch::asm!(
            "syscall",
            in("rax") n,
            lateout("rax") ret,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
        ret
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = n;
        0
    }
}

#[inline(always)]
pub unsafe fn syscall1(n: u64, a1: u64) -> i64 {
    #[cfg(target_arch = "x86_64")]
    {
        let ret: i64;
        core::arch::asm!(
            "syscall",
            in("rax") n,
            in("rdi") a1,
            lateout("rax") ret,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
        ret
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = (n, a1);
        0
    }
}

#[inline(always)]
pub unsafe fn syscall2(n: u64, a1: u64, a2: u64) -> i64 {
    #[cfg(target_arch = "x86_64")]
    {
        let ret: i64;
        core::arch::asm!(
            "syscall",
            in("rax") n,
            in("rdi") a1,
            in("rsi") a2,
            lateout("rax") ret,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
        ret
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = (n, a1, a2);
        0
    }
}

#[inline(always)]
pub unsafe fn syscall3(n: u64, a1: u64, a2: u64, a3: u64) -> i64 {
    #[cfg(target_arch = "x86_64")]
    {
        let ret: i64;
        core::arch::asm!(
            "syscall",
            in("rax") n,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            lateout("rax") ret,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
        ret
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = (n, a1, a2, a3);
        0
    }
}

#[inline(always)]
pub unsafe fn syscall4(n: u64, a1: u64, a2: u64, a3: u64, a4: u64) -> i64 {
    #[cfg(target_arch = "x86_64")]
    {
        let ret: i64;
        core::arch::asm!(
            "syscall",
            in("rax") n,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            in("r10") a4,
            lateout("rax") ret,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
        ret
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = (n, a1, a2, a3, a4);
        0
    }
}

#[inline(always)]
pub unsafe fn syscall5(n: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> i64 {
    #[cfg(target_arch = "x86_64")]
    {
        let ret: i64;
        core::arch::asm!(
            "syscall",
            in("rax") n,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            in("r10") a4,
            in("r8") a5,
            lateout("rax") ret,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
        ret
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = (n, a1, a2, a3, a4, a5);
        0
    }
}

#[inline(always)]
pub unsafe fn syscall6(n: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64) -> i64 {
    #[cfg(target_arch = "x86_64")]
    {
        let ret: i64;
        core::arch::asm!(
            "syscall",
            in("rax") n,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            in("r10") a4,
            in("r8") a5,
            in("r9") a6,
            lateout("rax") ret,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
        ret
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = (n, a1, a2, a3, a4, a5, a6);
        0
    }
}
