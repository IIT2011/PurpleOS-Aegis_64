use crate::arch::x86_64::io::{inb, outb};
use core::sync::atomic::{AtomicU64, Ordering};

const PIT_CHANNEL0: u16 = 0x40;
const PIT_CHANNEL2: u16 = 0x42;
const PIT_COMMAND: u16 = 0x43;
const PIT_BASE_FREQ: u32 = 1193182;

static SYSTEM_TICKS: AtomicU64 = AtomicU64::new(0);

pub fn init(frequency_hz: u32) {
    let divisor = (PIT_BASE_FREQ / frequency_hz) as u16;

    unsafe {
        outb(PIT_COMMAND, 0x36); // Channel 0, lobyte/hibyte, rate generator (mode 2)
        outb(PIT_CHANNEL0, (divisor & 0xFF) as u8);
        outb(PIT_CHANNEL0, (divisor >> 8) as u8);
    }

    crate::arch::x86_64::pic::clear_mask(0); // Unmask IRQ 0
    crate::arch::x86_64::idt::register_handler(32, on_tick);

    crate::drivers::serial::print("PIT: 8254 Timer Initialized at ");
    crate::drivers::serial::print_u64(frequency_hz as u64);
    crate::drivers::serial::println(" Hz");
}

fn on_tick(_frame: &mut crate::arch::x86_64::idt::InterruptFrame) {
    SYSTEM_TICKS.fetch_add(1, Ordering::Relaxed);
    crate::kernel::task::timer_tick();
}

pub fn get_ticks() -> u64 {
    SYSTEM_TICKS.load(Ordering::Relaxed)
}

pub fn delay_us(us: u32) {
    if us == 0 {
        return;
    }
    let mut count = ((PIT_BASE_FREQ as u64 * us as u64) / 1_000_000) as u32;
    if count == 0 {
        count = 1;
    }
    if count > 0xFFFF {
        count = 0xFFFF;
    }

    unsafe {
        outb(0x61, (inb(0x61) & !0x02) | 0x01); // Gate high, speaker low
        outb(PIT_COMMAND, 0xB0); // Channel 2, lobyte/hibyte, mode 0
        outb(PIT_CHANNEL2, (count & 0xFF) as u8);
        outb(PIT_CHANNEL2, ((count >> 8) & 0xFF) as u8);

        let val = inb(0x61) & 0xFE;
        outb(0x61, val);
        outb(0x61, val | 0x01);

        while (inb(0x61) & 0x20) == 0 {
            core::hint::spin_loop();
        }
    }
}

pub fn delay_ms(ms: u64) {
    for _ in 0..ms {
        delay_us(1000);
    }
}
