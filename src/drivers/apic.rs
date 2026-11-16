use crate::arch::x86_64::cpu::{rdmsr, wrmsr};

const IA32_APIC_BASE_MSR: u32 = 0x1B;
const LAPIC_DEFAULT_PHYS: u64 = 0xFEE00000;

pub const LAPIC_ID: u32 = 0x020;
pub const LAPIC_VER: u32 = 0x030;
pub const LAPIC_TPR: u32 = 0x080;
pub const LAPIC_EOI: u32 = 0x0B0;
pub const LAPIC_SVR: u32 = 0x0F0;
pub const LAPIC_ESR: u32 = 0x280;
pub const LAPIC_ICR_LOW: u32 = 0x300;
pub const LAPIC_ICR_HIGH: u32 = 0x310;
pub const LAPIC_LVT_TIMER: u32 = 0x320;
pub const LAPIC_TIMER_INIT: u32 = 0x380;
pub const LAPIC_TIMER_CURR: u32 = 0x390;
pub const LAPIC_TIMER_DIV: u32 = 0x3E0;

pub const ICR_DELIV_FIXED: u32 = 0 << 8;
pub const ICR_DELIV_INIT: u32 = 5 << 8;
pub const ICR_DELIV_STARTUP: u32 = 6 << 8;
pub const ICR_TRIGGER_EDGE: u32 = 0 << 15;
pub const ICR_LEVEL_ASSERT: u32 = 1 << 14;
pub const ICR_DEST_PHYSICAL: u32 = 0 << 11;
pub const ICR_DEST_ALL_EXC_SELF: u32 = 3 << 18;

static mut LAPIC_BASE: u64 = LAPIC_DEFAULT_PHYS;

#[inline]
pub unsafe fn read_reg(reg: u32) -> u32 {
    let ptr = (LAPIC_BASE + reg as u64) as *const u32;
    core::ptr::read_volatile(ptr)
}

#[inline]
pub unsafe fn write_reg(reg: u32, val: u32) {
    let ptr = (LAPIC_BASE + reg as u64) as *mut u32;
    core::ptr::write_volatile(ptr, val);
}

pub fn init(base_phys: u64) -> Result<(), &'static str> {
    unsafe {
        LAPIC_BASE = if base_phys != 0 { base_phys } else { LAPIC_DEFAULT_PHYS };

        // Enable APIC via MSR (bit 11)
        let msr = rdmsr(IA32_APIC_BASE_MSR);
        wrmsr(IA32_APIC_BASE_MSR, (msr & 0xFFF) | LAPIC_BASE | (1 << 11));

        // Enable spurious interrupt vector (Spurious vector = 0xFF, APIC Software Enable = bit 8)
        write_reg(LAPIC_SVR, 0x1FF);

        // Clear task priority register to accept all interrupts
        write_reg(LAPIC_TPR, 0);

        crate::drivers::serial::print("Local APIC Initialized: Base=0x");
        crate::drivers::serial::print_hex(LAPIC_BASE);
        crate::drivers::serial::println("");

        // Calibrate APIC timer against PIT Channel 2
        calibrate_timer();
    }
    Ok(())
}

static mut APIC_TICKS_PER_10MS: u32 = 100000;

fn on_apic_timer(_frame: &mut crate::arch::x86_64::idt::InterruptFrame) {
    crate::kernel::task::timer_tick();
}

pub fn calibrate_timer() -> u32 {
    unsafe {
        // Divide by 16 (0x03)
        write_reg(LAPIC_TIMER_DIV, 0x03);

        // Mask timer during calibration (bit 16 = 1)
        write_reg(LAPIC_LVT_TIMER, (1 << 16) | 48);

        // Load maximum count
        write_reg(LAPIC_TIMER_INIT, 0xFFFFFFFF);

        // Precise 10 ms delay via PIT Channel 2
        crate::drivers::pit::delay_ms(10);

        let curr = read_reg(LAPIC_TIMER_CURR);
        let ticks_10ms = 0xFFFFFFFFu32.saturating_sub(curr);

        APIC_TICKS_PER_10MS = if ticks_10ms > 1000 { ticks_10ms } else { 100000 };

        // Register IDT handler for vector 48
        crate::arch::x86_64::idt::register_handler(48, on_apic_timer);

        // Set Periodic mode (bit 17 = 1) on Vector 48
        write_reg(LAPIC_LVT_TIMER, (1 << 17) | 48);
        write_reg(LAPIC_TIMER_INIT, APIC_TICKS_PER_10MS);

        crate::drivers::serial::print("Local APIC Timer: Calibrated via PIT (");
        crate::drivers::serial::print_u64((APIC_TICKS_PER_10MS as u64) * 100);
        crate::drivers::serial::println(" Hz base, 100 Hz periodic tick active)");

        APIC_TICKS_PER_10MS
    }
}

pub fn init_ap() {
    unsafe {
        // Enable APIC via MSR (bit 11)
        let msr = rdmsr(IA32_APIC_BASE_MSR);
        wrmsr(IA32_APIC_BASE_MSR, (msr & 0xFFF) | LAPIC_BASE | (1 << 11));

        write_reg(LAPIC_SVR, 0x1FF);
        write_reg(LAPIC_TPR, 0);

        // Synchronize APIC timer on AP core with calibrated frequency
        write_reg(LAPIC_TIMER_DIV, 0x03);
        write_reg(LAPIC_LVT_TIMER, (1 << 17) | 48);
        write_reg(LAPIC_TIMER_INIT, APIC_TICKS_PER_10MS);
    }
}

pub fn get_id() -> u32 {
    unsafe { read_reg(LAPIC_ID) >> 24 }
}

pub fn send_eoi() {
    unsafe {
        write_reg(LAPIC_EOI, 0);
    }
}

pub fn send_ipi(dest_apic_id: u32, flags: u32) {
    unsafe {
        write_reg(LAPIC_ICR_HIGH, dest_apic_id << 24);
        write_reg(LAPIC_ICR_LOW, flags);

        // Wait until delivery status (bit 12) is idle
        let mut timeout = 100000;
        while (read_reg(LAPIC_ICR_LOW) & (1 << 12)) != 0 && timeout > 0 {
            core::hint::spin_loop();
            timeout -= 1;
        }
    }
}
