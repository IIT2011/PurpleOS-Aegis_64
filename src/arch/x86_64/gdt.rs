use core::arch::asm;
use core::mem::size_of;

pub const GDT_NULL: u16 = 0x00;
pub const GDT_KERNEL_CS: u16 = 0x08;
pub const GDT_KERNEL_DS: u16 = 0x10;
pub const GDT_USER_DS: u16 = 0x1B; // 0x18 | 3
pub const GDT_USER_CS: u16 = 0x23; // 0x20 | 3
pub const GDT_TSS_BASE: u16 = 0x28;

pub const MAX_GDT_CPUS: usize = 8;

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

// 5 base entries + (2 entries * 8 CPUs) for 64-bit TSS descriptors = 21 entries
#[repr(C, align(16))]
pub struct GdtTable {
    pub entries: [u64; 5 + 2 * MAX_GDT_CPUS],
}

static mut GDT: GdtTable = GdtTable {
    entries: [0; 5 + 2 * MAX_GDT_CPUS],
};
static mut TSS_ARRAY: [Tss64; MAX_GDT_CPUS] = [Tss64 {
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
    iopb_offset: 104,
}; MAX_GDT_CPUS];

static mut GDT_PTR: GdtPointer = GdtPointer { limit: 0, base: 0 };
static mut DEFAULT_STACKS: [[u8; 16384]; MAX_GDT_CPUS] = [[0; 16384]; MAX_GDT_CPUS];
static mut DOUBLE_FAULT_STACKS: [[u8; 8192]; MAX_GDT_CPUS] = [[0; 8192]; MAX_GDT_CPUS];

pub fn init() {
    unsafe {
        // Base segments
        GDT.entries[0] = 0x0000000000000000; // Null
        GDT.entries[1] = 0x00AF9A000000FFFF; // Kernel CS 64-bit (DPL 0, Exec/Read, L=1)
        GDT.entries[2] = 0x00CF92000000FFFF; // Kernel DS 64-bit (DPL 0, Read/Write)
        GDT.entries[3] = 0x00CFF2000000FFFF; // User DS 64-bit (DPL 3, Read/Write)
        GDT.entries[4] = 0x00AFFA000000FFFF; // User CS 64-bit (DPL 3, Exec/Read, L=1)

        let tss_limit = (size_of::<Tss64>() - 1) as u64;

        // Populate per-CPU TSS descriptors
        for cpu in 0..MAX_GDT_CPUS {
            let tss_ptr = &raw mut TSS_ARRAY[cpu];
            let default_stack_top = (&raw const DEFAULT_STACKS[cpu] as u64) + 16384;
            let df_stack_top = (&raw const DOUBLE_FAULT_STACKS[cpu] as u64) + 8192;

            (*tss_ptr).rsp0 = default_stack_top;
            (*tss_ptr).ist1 = df_stack_top;
            (*tss_ptr).iopb_offset = size_of::<Tss64>() as u16;

            let tss_base = tss_ptr as u64;

            // 16-byte TSS descriptor (Available 64-bit TSS type 9, Present)
            let tss_low = (tss_limit & 0xFFFF)
                | ((tss_base & 0xFFFF) << 16)
                | (((tss_base >> 16) & 0xFF) << 32)
                | (0x89u64 << 40)
                | (((tss_limit >> 16) & 0x0F) << 48)
                | (((tss_base >> 24) & 0xFF) << 56);
            let tss_high = tss_base >> 32;

            let entry_idx = 5 + cpu * 2;
            GDT.entries[entry_idx] = tss_low;
            GDT.entries[entry_idx + 1] = tss_high;
        }

        GDT_PTR.limit = (size_of::<GdtTable>() - 1) as u16;
        GDT_PTR.base = &raw const GDT as u64;

        load_gdt_and_tr();
    }
}

pub fn init_ap(cpu_id: usize) {
    let cpu = cpu_id.min(MAX_GDT_CPUS - 1);
    let tss_sel = (GDT_TSS_BASE + (cpu as u16 * 16)) as u16;

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

        // Load per-CPU Task Register
        asm!(
            "ltr {0:x}",
            in(reg) tss_sel,
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

    // Load BSP Task Register (0x28)
    asm!("ltr ax", in("ax") GDT_TSS_BASE, options(nomem, nostack, preserves_flags));
}

pub fn get_default_stack_top(cpu: usize) -> u64 {
    let cpu = cpu.min(MAX_GDT_CPUS - 1);
    unsafe { (&raw const DEFAULT_STACKS[cpu] as u64) + 16384 }
}

pub fn set_tss_rsp0(cpu: usize, stack_top: u64) {
    let cpu = cpu.min(MAX_GDT_CPUS - 1);
    unsafe {
        TSS_ARRAY[cpu].rsp0 = stack_top;
    }
}

pub fn get_tss_rsp0(cpu: usize) -> u64 {
    let cpu = cpu.min(MAX_GDT_CPUS - 1);
    unsafe { TSS_ARRAY[cpu].rsp0 }
}
