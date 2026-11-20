//! Aegis64 / Purple OS Ring 3 (Userland / User Mode) Architecture & Test Harness
//!
//! Enforces strict x86_64 privilege separation between Kernel Mode (Ring 0, CPL=0)
//! and User Mode (Ring 3, CPL=3). Zero unchecked user memory access, canonical
//! user address range validation, hardware SYSCALL/SYSRET MSR configuration,
//! and per-CPU TSS rsp0 / GS base stack switching.

use crate::drivers::serial;
use crate::kernel::task::{
    spawn_user_program, TaskPriority, USER_CODE_BASE, USER_STACK_PAGES, USER_STACK_TOP,
};
use crate::kernel::uaccess::{USER_SPACE_MAX, USER_SPACE_MIN};

/// Ring 3 Full Feature Demonstration Userland Binary (124 bytes code + 2 embedded string constants)
pub static RING3_DEMO_CODE: [u8; 1024] = {
    let mut buf = [0u8; 1024];

    // .text (124 bytes)
    // 0x00: movw %cs, %ax
    buf[0] = 0x66;
    buf[1] = 0x8c;
    buf[2] = 0xc8;
    // 0x03: andw $3, %ax
    buf[3] = 0x66;
    buf[4] = 0x83;
    buf[5] = 0xe0;
    buf[6] = 0x03;
    // 0x07: cmpw $3, %ax
    buf[7] = 0x66;
    buf[8] = 0x83;
    buf[9] = 0xf8;
    buf[10] = 0x03;
    // 0x0b: je +2 (to 0x0f)
    buf[11] = 0x74;
    buf[12] = 0x02;
    // 0x0d: ud2 (fault if not CPL=3)
    buf[13] = 0x0f;
    buf[14] = 0x0b;
    // 0x0f: movq $20, %rax (SYS_PRINT)
    buf[15] = 0x48;
    buf[16] = 0xc7;
    buf[17] = 0xc0;
    buf[18] = 0x14;
    buf[19] = 0x00;
    buf[20] = 0x00;
    buf[21] = 0x00;
    // 0x16: movabsq $0x00400200, %rdi (pointer to msg1)
    buf[22] = 0x48;
    buf[23] = 0xbf;
    buf[24] = 0x00;
    buf[25] = 0x02;
    buf[26] = 0x40;
    buf[27] = 0x00;
    buf[28] = 0x00;
    buf[29] = 0x00;
    buf[30] = 0x00;
    buf[31] = 0x00;
    // 0x20: movq $62, %rsi (len of msg1)
    buf[32] = 0x48;
    buf[33] = 0xc7;
    buf[34] = 0xc6;
    buf[35] = 0x3e;
    buf[36] = 0x00;
    buf[37] = 0x00;
    buf[38] = 0x00;
    // 0x27: syscall
    buf[39] = 0x0f;
    buf[40] = 0x05;
    // 0x29: movabsq $0x12345678, %rbx
    buf[41] = 0x48;
    buf[42] = 0xbb;
    buf[43] = 0x78;
    buf[44] = 0x56;
    buf[45] = 0x34;
    buf[46] = 0x12;
    buf[47] = 0x00;
    buf[48] = 0x00;
    buf[49] = 0x00;
    buf[50] = 0x00;
    // 0x33: pushq %rbx (test user stack)
    buf[51] = 0x53;
    // 0x34: popq %rdx
    buf[52] = 0x5a;
    // 0x35: cmpq %rbx, %rdx
    buf[53] = 0x48;
    buf[54] = 0x39;
    buf[55] = 0xda;
    // 0x38: je +2 (to 0x3c)
    buf[56] = 0x74;
    buf[57] = 0x02;
    // 0x3a: ud2 (fault if stack mismatch)
    buf[58] = 0x0f;
    buf[59] = 0x0b;
    // 0x3c: movq $1, %rax (SYS_WRITE)
    buf[60] = 0x48;
    buf[61] = 0xc7;
    buf[62] = 0xc0;
    buf[63] = 0x01;
    buf[64] = 0x00;
    buf[65] = 0x00;
    buf[66] = 0x00;
    // 0x43: movq $1, %rdi (stdout fd)
    buf[67] = 0x48;
    buf[68] = 0xc7;
    buf[69] = 0xc7;
    buf[70] = 0x01;
    buf[71] = 0x00;
    buf[72] = 0x00;
    buf[73] = 0x00;
    // 0x4a: movabsq $0x00400300, %rsi (pointer to msg2)
    buf[74] = 0x48;
    buf[75] = 0xbe;
    buf[76] = 0x00;
    buf[77] = 0x03;
    buf[78] = 0x40;
    buf[79] = 0x00;
    buf[80] = 0x00;
    buf[81] = 0x00;
    buf[82] = 0x00;
    buf[83] = 0x00;
    // 0x54: movq $59, %rdx (len of msg2: 59 bytes)
    buf[84] = 0x48;
    buf[85] = 0xc7;
    buf[86] = 0xc2;
    buf[87] = 0x3b;
    buf[88] = 0x00;
    buf[89] = 0x00;
    buf[90] = 0x00;
    // 0x5b: syscall
    buf[91] = 0x0f;
    buf[92] = 0x05;
    // 0x5d: movq $39, %rax (SYS_GETPID)
    buf[93] = 0x48;
    buf[94] = 0xc7;
    buf[95] = 0xc0;
    buf[96] = 0x27;
    buf[97] = 0x00;
    buf[98] = 0x00;
    buf[99] = 0x00;
    // 0x64: syscall
    buf[100] = 0x0f;
    buf[101] = 0x05;
    // 0x66: movq $24, %rax (SYS_SCHED_YIELD)
    buf[102] = 0x48;
    buf[103] = 0xc7;
    buf[104] = 0xc0;
    buf[105] = 0x18;
    buf[106] = 0x00;
    buf[107] = 0x00;
    buf[108] = 0x00;
    // 0x6d: syscall
    buf[109] = 0x0f;
    buf[110] = 0x05;
    // 0x6f: movq $60, %rax (SYS_EXIT)
    buf[111] = 0x48;
    buf[112] = 0xc7;
    buf[113] = 0xc0;
    buf[114] = 0x3c;
    buf[115] = 0x00;
    buf[116] = 0x00;
    buf[117] = 0x00;
    // 0x76: xorq %rdi, %rdi (exit status 0)
    buf[118] = 0x48;
    buf[119] = 0x31;
    buf[120] = 0xff;
    // 0x79: syscall
    buf[121] = 0x0f;
    buf[122] = 0x05;
    // 0x7b: hlt
    buf[123] = 0xf4;

    // 0x200 (offset 512): msg1 (62 bytes)
    let msg1 = b"[RING3] Aegis64 Ring 3 User Mode Execution Confirmed (CPL=3)!\n";
    let mut i = 0;
    while i < msg1.len() {
        buf[512 + i] = msg1[i];
        i += 1;
    }

    // 0x300 (offset 768): msg2 (57 bytes)
    let msg2 = b"[RING3] POSIX SYS_WRITE: User Stack & Sycall ABI Verified!\n";
    let mut j = 0;
    while j < msg2.len() {
        buf[768 + j] = msg2[j];
        j += 1;
    }

    buf
};

/// Ring 3 Deliberate Fault 1: Division By Zero (#DE, vector 0)
pub static RING3_FAULT_DIV0: [u8; 13] = [
    0x48, 0x31, 0xc9, // xorq %rcx, %rcx
    0x48, 0xc7, 0xc0, 0x64, 0x00, 0x00, 0x00, // movq $100, %rax
    0x48, 0xf7, 0xf1, // divq %rcx (#DE)
];

/// Ring 3 Deliberate Fault 2: Kernel Memory Access / Protection Violation (#PF, vector 14)
pub static RING3_FAULT_PAGEFAULT: [u8; 13] = [
    0x48, 0xb8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0xff,
    0xff, // movabsq $0xffff800000000000, %rax
    0x48, 0x8b, 0x18, // movq (%rax), %rbx (#PF)
];

/// Displays comprehensive Ring 3 architecture, segment selectors, MSRs, and memory bounds.
pub fn print_info() {
    serial::println("=======================================================================");
    serial::println("  Aegis64 / Purple OS Ring 3 (Userland / User Mode) Subsystem");
    serial::println("=======================================================================");
    serial::println("Privilege Architecture: x86_64 Dual-Privilege Model (Ring 0 / Ring 3)");
    serial::println("Hardware Enforced:      CPL=0 (Kernel Supervisor), CPL=3 (Userland)");
    serial::println("");
    serial::println("Segment Selectors (GDT):");
    serial::println("  Kernel CS (DPL 0):    0x08 (64-bit Code, Long Mode L=1)");
    serial::println("  Kernel DS (DPL 0):    0x10 (64-bit Data, Writable)");
    serial::println("  User DS   (DPL 3):    0x1B (0x18 | 3, RPL 3, Writable)");
    serial::println("  User CS   (DPL 3):    0x23 (0x20 | 3, RPL 3, 64-bit Code, L=1)");
    serial::println("  TSS Selector:         0x28 (Per-CPU 64-bit Task State Segment)");
    serial::println("");
    serial::println("Hardware System Call Registers (MSR):");
    serial::println(
        "  MSR_STAR (0xC0000081): 0x0010000800000000 (Target CS/SS for SYSCALL/SYSRET)",
    );
    serial::println("  MSR_LSTAR (0xC0000082): syscall_entry_asm (Hardware SYSCALL Entry Point)");
    serial::println("  MSR_SFMASK (0xC0000084): 0x0000000000000200 (Clears IF flag on entry)");
    serial::println("  MSR_KERNEL_GS_BASE:    Per-CPU dedicated kernel syscall stack anchor");
    serial::println("");
    serial::println("Canonical User Memory Layout:");
    serial::print("  User Space Min:       0x");
    serial::print_hex(USER_SPACE_MIN);
    serial::println(" (Protects against null-page dereferences)");
    serial::print("  User Space Max:       0x");
    serial::print_hex(USER_SPACE_MAX);
    serial::println(" (Canonical 48-bit VA user ceiling)");
    serial::print("  User Code Base:       0x");
    serial::print_hex(USER_CODE_BASE);
    serial::println(" (4 MiB canonical code entry point)");
    serial::print("  User Stack Top:       0x");
    serial::print_hex(USER_STACK_TOP);
    serial::print(" (");
    serial::print_u64((USER_STACK_PAGES * 4096 / 1024) as u64);
    serial::println(" KiB user execution stack)");
    serial::println("");
    serial::println("Memory Isolation & Safety Enforcements:");
    serial::println("  - Higher-half kernel space (PML4 entries 256..511) has PAGE_USER=0.");
    serial::println("  - All user pages require PAGE_PRESENT | PAGE_USER permissions.");
    serial::println("  - Zero unchecked user pointer dereferencing (Strict uaccess validation).");
    serial::println("  - Stack switching: SWAPGS + TSS.rsp0 on interrupts and SYSCALL entry.");
    serial::println("  - Fault isolation: IDT detects user mode (CS & 3 == 3) and reaps task.");
    serial::println("=======================================================================");
}

/// Spawns and executes the full Ring 3 verification task.
pub fn run_test() -> Option<u32> {
    serial::println("[RING3] Spawning Ring 3 Userland Demonstration Process...");
    let tid = spawn_user_program("Ring3Demo", &RING3_DEMO_CODE, TaskPriority::Normal)?;
    serial::print("[RING3] Process spawned successfully with TID ");
    serial::print_u64(tid as u64);
    serial::println(" (Private PML4, CPL=3 transition ready).");
    Some(tid)
}

/// Spawns a Ring 3 task that deliberately executes a Division by Zero instruction (#DE).
pub fn run_fault_div0() -> Option<u32> {
    serial::println("[RING3] Spawning Ring 3 Process to Trigger Fault: Division By Zero (#DE)...");
    let tid = spawn_user_program("FaultDiv0", &RING3_FAULT_DIV0, TaskPriority::Normal)?;
    serial::print("[RING3] Fault process spawned with TID ");
    serial::print_u64(tid as u64);
    serial::println("; IDT will intercept without kernel crash.");
    Some(tid)
}

/// Spawns a Ring 3 task that deliberately accesses supervisor kernel memory (#PF).
pub fn run_fault_pagefault() -> Option<u32> {
    serial::println(
        "[RING3] Spawning Ring 3 Process to Trigger Fault: Kernel Memory Access (#PF)...",
    );
    let tid = spawn_user_program("FaultPage", &RING3_FAULT_PAGEFAULT, TaskPriority::Normal)?;
    serial::print("[RING3] Fault process spawned with TID ");
    serial::print_u64(tid as u64);
    serial::println("; IDT will intercept without kernel crash.");
    Some(tid)
}
