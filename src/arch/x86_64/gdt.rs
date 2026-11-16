use core::arch::asm;
use core::mem::size_of;

pub const GDT_NULL: u16 = 0x00;
pub const GDT_KERNEL_CS: u16 = 0x08;
pub const GDT_KERNEL_DS: u16 = 0x10;
pub const GDT_USER_DS: u16 = 0x1B; // 0x18 | 3
pub const GDT_USER_CS: u16 = 0x23; // 0x20 | 3
pub const GDT_TSS: u16 = 0x28;

#[repr(C, packed)]
#[derive(Copy, Clone, Default)]
pub struct Tss64 {
    reserved0: u32,
    pub rsp0: u64,
    pub rsp1: u64,
    pub rsp2: u64,
    reserved1: u64,
    pub ist1: u64,
    pub ist2: u64,
    pub ist3: u64,
    pub ist4: u64,
    pub ist5: u64,
    pub ist6: u64,
    pub ist7: u64,
    reserved2: u64,
    reserved3: u16,
    pub iopb_offset: u16,
}

#[repr(C, packed)]
pub struct GdtPointer {
    pub limit: u16,
    pub base: u64,
}

#[repr(C, align(16))]
pub struct GdtTable {
    pub entries: [u64; 7],
}

static mut GDT: GdtTable = GdtTable { entries: [0; 7] };
static mut TSS: Tss64 = Tss64 {
    reserved0: 0,
    rsp0: 0,
    rsp1: 0,
    rsp2: 0,
    reserved1: 0,
    ist1: 0,
    ist2: 0,
    ist3: 0,
    ist4: 0,
    ist5: 0,
    ist6: 0,
    ist7: 0,
    reserved2: 0,
    reserved3: 0,
    iopb_offset: size_of::<Tss64>() as u16,
};
static mut GDT_PTR: GdtPointer = GdtPointer { limit: 0, base: 0 };
static mut DEFAULT_STACK: [u8; 16384] = [0; 16384];
static mut DOUBLE_FAULT_STACK: [u8; 8192] = [0; 8192];

pub fn init() {
    unsafe {
        let tss_base = &raw const TSS as u64;
        let tss_limit = (size_of::<Tss64>() - 1) as u64;

        TSS.rsp0 = &raw const DEFAULT_STACK as u64 + 16384;
        TSS.ist1 = &raw const DOUBLE_FAULT_STACK as u64 + 8192;
        TSS.iopb_offset = size_of::<Tss64>() as u16;

        GDT.entries[0] = 0x0000000000000000; // Null
        GDT.entries[1] = 0x00AF9A000000FFFF; // Kernel CS 64-bit (DPL 0, Exec/Read, L=1)
        GDT.entries[2] = 0x00CF92000000FFFF; // Kernel DS 64-bit (DPL 0, Read/Write)
        GDT.entries[3] = 0x00CFF2000000FFFF; // User DS 64-bit (DPL 3, Read/Write)
        GDT.entries[4] = 0x00AFFA000000FFFF; // User CS 64-bit (DPL 3, Exec/Read, L=1)

        // 16-byte TSS descriptor
        let tss_low = (tss_limit & 0xFFFF)
            | ((tss_base & 0xFFFF) << 16)
            | (((tss_base >> 16) & 0xFF) << 32)
            | (0x89u64 << 40) // Present, DPL 0, 64-bit TSS Available (type 9)
            | (((tss_limit >> 16) & 0x0F) << 48)
            | (((tss_base >> 24) & 0xFF) << 56);
        let tss_high = tss_base >> 32;

        GDT.entries[5] = tss_low;
        GDT.entries[6] = tss_high;

        GDT_PTR.limit = (size_of::<GdtTable>() - 1) as u16;
        GDT_PTR.base = &raw const GDT as u64;

        load_gdt_and_tr();
    }
}

pub fn init_ap() {
    unsafe {
        asm!("lgdt [{}]", in(reg) &raw const GDT_PTR, options(readonly, nostack, preserves_flags));

        // Reload CS via far return
        asm!(
            "push 0x08",
            "lea rax, [2f + rip]",
            "push rax",
            "retfq",
            "2:",
            out("rax") _,
            options(preserves_flags)
        );

        // Reload Data segments
        asm!(
            "mov ax, 0x10",
            "mov ds, ax",
            "mov es, ax",
            "mov ss, ax",
            "mov fs, ax",
            "mov gs, ax",
            out("ax") _,
            options(nomem, nostack, preserves_flags)
        );
    }
}

unsafe fn load_gdt_and_tr() {
    asm!("lgdt [{}]", in(reg) &raw const GDT_PTR, options(readonly, nostack, preserves_flags));

    // Reload CS via far return
    asm!(
        "push 0x08",
        "lea rax, [2f + rip]",
        "push rax",
        "retfq",
        "2:",
        out("rax") _,
        options(preserves_flags)
    );

    // Reload Data segments
    asm!(
        "mov ax, 0x10",
        "mov ds, ax",
        "mov es, ax",
        "mov ss, ax",
        "mov fs, ax",
        "mov gs, ax",
        out("ax") _,
        options(nomem, nostack, preserves_flags)
    );

    // Load Task Register
    asm!("ltr ax", in("ax") GDT_TSS, options(nomem, nostack, preserves_flags));
}

pub fn set_kernel_stack(rsp0: u64) {
    unsafe {
        TSS.rsp0 = rsp0;
    }
}
