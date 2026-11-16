use core::arch::asm;
use core::mem::size_of;

#[repr(C, packed)]
#[derive(Copy, Clone, Default)]
pub struct IdtEntry {
    pub offset_low: u16,
    pub selector: u16,
    pub ist: u8,
    pub type_attr: u8,
    pub offset_mid: u16,
    pub offset_high: u32,
    pub zero: u32,
}

#[repr(C, packed)]
pub struct IdtPointer {
    pub limit: u16,
    pub base: u64,
}

#[repr(C)]
#[derive(Debug)]
pub struct InterruptFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub int_no: u64,
    pub err_code: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

pub type HandlerFn = fn(&mut InterruptFrame);

static mut IDT: [IdtEntry; 256] = [IdtEntry {
    offset_low: 0,
    selector: 0,
    ist: 0,
    type_attr: 0,
    offset_mid: 0,
    offset_high: 0,
    zero: 0,
}; 256];

static mut IDT_PTR: IdtPointer = IdtPointer { limit: 0, base: 0 };
static mut HANDLERS: [Option<HandlerFn>; 256] = [None; 256];

extern "C" {
    static isr_stub_table: [u64; 256];
}

pub fn set_gate(num: usize, base: u64, sel: u16, flags: u8, ist: u8) {
    unsafe {
        IDT[num].offset_low = (base & 0xFFFF) as u16;
        IDT[num].selector = sel;
        IDT[num].ist = ist;
        IDT[num].type_attr = flags;
        IDT[num].offset_mid = ((base >> 16) & 0xFFFF) as u16;
        IDT[num].offset_high = (base >> 32) as u32;
        IDT[num].zero = 0;
    }
}

pub fn register_handler(num: usize, handler: HandlerFn) {
    unsafe {
        HANDLERS[num] = Some(handler);
    }
}

pub fn init() {
    unsafe {
        IDT_PTR.limit = (size_of::<[IdtEntry; 256]>() - 1) as u16;
        IDT_PTR.base = &raw const IDT as u64;

        for i in 0..256 {
            let stub = isr_stub_table[i];
            let ist = if i == 8 { 1 } else { 0 }; // Vector 8 (Double Fault) uses IST1!
            let flags = if i == 128 { 0xEE } else { 0x8E }; // 0xEE = DPL 3 for Ring 3 syscall
            set_gate(i, stub, 0x08, flags, ist);
        }

        load_idt();
    }
}

pub fn load_idt() {
    unsafe {
        asm!("lidt [{}]", in(reg) &raw const IDT_PTR, options(readonly, nostack, preserves_flags));
    }
}

const EXCEPTION_MESSAGES: [&str; 32] = [
    "Division By Zero", "Debug", "Non-Maskable Interrupt", "Breakpoint",
    "Into Detected Overflow", "Out of Bounds", "Invalid Opcode", "No Coprocessor",
    "Double Fault", "Coprocessor Segment Overrun", "Bad TSS", "Segment Not Present",
    "Stack Fault", "General Protection Fault", "Page Fault", "Unknown Interrupt",
    "Coprocessor Fault", "Alignment Check", "Machine Check", "SIMD Floating-Point",
    "Virtualization", "Control Protection", "Reserved", "Reserved",
    "Reserved", "Reserved", "Reserved", "Reserved",
    "Reserved", "Reserved", "Security", "Reserved"
];

#[unsafe(no_mangle)]
pub extern "C" fn interrupt_dispatch(frame: &mut InterruptFrame) {
    let int_no = frame.int_no as usize;

    if int_no < 32 {
        // CPU Exception
        if (frame.cs & 3) == 3 {
            // User mode exception - terminate task safely
            crate::drivers::serial::print("[SECURITY] User Process Exception: ");
            crate::drivers::serial::println(EXCEPTION_MESSAGES[int_no]);
            crate::kernel::task::exit();
        }

        // Kernel panic
        crate::drivers::serial::println("=================================================");
        crate::drivers::serial::print("!!! KERNEL PANIC: CPU EXCEPTION ");
        crate::drivers::serial::print(EXCEPTION_MESSAGES[int_no]);
        crate::drivers::serial::println(" !!!");
        crate::drivers::serial::println("=================================================");
        if int_no == 14 {
            let cr2 = crate::arch::x86_64::cpu::read_cr2();
            let err = frame.err_code;
            crate::drivers::serial::print("CR2 (Faulting Address): 0x");
            crate::drivers::serial::print_hex(cr2);
            crate::drivers::serial::println("");
            crate::drivers::serial::print("Page Fault Reason: ");
            if (err & (1 << 0)) == 0 {
                crate::drivers::serial::print("[NOT_PRESENT] ");
            } else {
                crate::drivers::serial::print("[PROTECTION_VIOLATION] ");
            }
            if (err & (1 << 1)) != 0 {
                crate::drivers::serial::print("[WRITE] ");
            } else {
                crate::drivers::serial::print("[READ] ");
            }
            if (err & (1 << 2)) != 0 {
                crate::drivers::serial::print("[USER_MODE] ");
            } else {
                crate::drivers::serial::print("[SUPERVISOR] ");
            }
            if (err & (1 << 3)) != 0 {
                crate::drivers::serial::print("[RSVD_BIT_VIOLATION] ");
            }
            if (err & (1 << 4)) != 0 {
                crate::drivers::serial::print("[INSTRUCTION_FETCH_NX] ");
            }
            crate::drivers::serial::println("");
        }
        loop {
            crate::arch::x86_64::cpu::cli();
            crate::arch::x86_64::cpu::hlt();
        }
    }

    // Call registered driver / kernel handler if present
    if let Some(handler) = unsafe { HANDLERS[int_no] } {
        handler(frame);
    }

    // Acknowledge hardware PIC / APIC
    if int_no >= 32 && int_no < 48 {
        crate::arch::x86_64::pic::send_eoi((int_no - 32) as u8);
    }
    if int_no >= 48 && int_no <= 255 {
        crate::drivers::apic::send_eoi();
    }
}
