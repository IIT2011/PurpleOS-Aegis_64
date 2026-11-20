use crate::arch::x86_64::io::{inb, outb};
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

const PIT_CHANNEL0: u16 = 0x40;
const PIT_CHANNEL2: u16 = 0x42;
const PIT_COMMAND: u16 = 0x43;
pub const PIT_BASE_FREQ_HZ: u32 = 1_193_182;
pub const PIT_DEFAULT_TICK_HZ: u32 = 100;
pub const PIT_MAX_TONE_FREQUENCY_HZ: u32 = 20_000;
const PIT_MIN_TICK_HZ: u32 = PIT_BASE_FREQ_HZ / u16::MAX as u32;
const PIT_MAX_COUNTER: u16 = u16::MAX;
pub const PIT_MAX_DELAY_US: u32 =
    ((PIT_MAX_COUNTER as u64 * 1_000_000) / PIT_BASE_FREQ_HZ as u64) as u32;
static SYSTEM_TICK_HZ: AtomicU32 = AtomicU32::new(PIT_DEFAULT_TICK_HZ);

static SYSTEM_TICKS: AtomicU64 = AtomicU64::new(0);

pub fn init(frequency_hz: u32) {
    let requested_hz = if frequency_hz == 0 {
        PIT_DEFAULT_TICK_HZ
    } else {
        frequency_hz
    };
    let bounded_hz = requested_hz.clamp(PIT_MIN_TICK_HZ, PIT_BASE_FREQ_HZ);
    let divisor = (PIT_BASE_FREQ_HZ / bounded_hz).clamp(1, u16::MAX as u32) as u16;
    let actual_hz = PIT_BASE_FREQ_HZ / divisor as u32;
    SYSTEM_TICK_HZ.store(actual_hz, Ordering::Relaxed);

    unsafe {
        outb(PIT_COMMAND, 0x36); // Channel 0, lobyte/hibyte, rate generator (mode 2)
        outb(PIT_CHANNEL0, (divisor & 0xFF) as u8);
        outb(PIT_CHANNEL0, (divisor >> 8) as u8);
    }

    crate::arch::x86_64::pic::clear_mask(0); // Unmask IRQ 0
    crate::arch::x86_64::idt::register_handler(32, on_tick);

    crate::drivers::serial::print("PIT: 8254 Timer Initialized at ");
    crate::drivers::serial::print_u64(actual_hz as u64);
    crate::drivers::serial::println(" Hz");
}

fn on_tick(_frame: &mut crate::arch::x86_64::idt::InterruptFrame) {
    SYSTEM_TICKS.fetch_add(1, Ordering::Relaxed);
    crate::kernel::task::timer_tick();
}

pub fn get_ticks() -> u64 {
    SYSTEM_TICKS.load(Ordering::Relaxed)
}

pub fn ticks_to_millis(ticks: u64) -> u64 {
    let hz = SYSTEM_TICK_HZ.load(Ordering::Relaxed).max(1) as u64;
    ticks.saturating_mul(1_000) / hz
}

pub fn ticks_to_nanos(ticks: u64) -> u64 {
    let hz = SYSTEM_TICK_HZ.load(Ordering::Relaxed).max(1) as u64;
    ticks.saturating_mul(1_000_000_000) / hz
}

pub fn delay_us(us: u32) {
    if us == 0 {
        return;
    }
    let count =
        ((PIT_BASE_FREQ_HZ as u64 * us as u64) / 1_000_000).clamp(1, PIT_MAX_COUNTER as u64) as u16;

    unsafe {
        outb(0x61, (inb(0x61) & !0x02) | 0x01); // Gate high, speaker low
        outb(PIT_COMMAND, 0xB0); // Channel 2, lobyte/hibyte, mode 0
        outb(PIT_CHANNEL2, (count & 0xFF) as u8);
        outb(PIT_CHANNEL2, (count >> 8) as u8);

        let val = inb(0x61) & 0xFE;
        outb(0x61, val);
        outb(0x61, val | 0x01);

        while (inb(0x61) & 0x20) == 0 {
            core::hint::spin_loop();
        }
    }
}

pub fn delay_us_long(mut us: u64) {
    while us > 0 {
        let chunk = us.min(PIT_MAX_DELAY_US as u64) as u32;
        delay_us(chunk);
        us -= chunk as u64;
    }
}

pub fn delay_ms(ms: u64) {
    for _ in 0..ms {
        delay_us(1000);
    }
}

pub fn play_tone(freq_hz: u32, duration_ms: u32) {
    if freq_hz == 0 || freq_hz > PIT_MAX_TONE_FREQUENCY_HZ {
        return;
    }
    let divisor = (PIT_BASE_FREQ_HZ / freq_hz.max(1)).clamp(1, u16::MAX as u32) as u16;
    unsafe {
        outb(PIT_COMMAND, 0xB6); // Channel 2, square wave
        outb(PIT_CHANNEL2, (divisor & 0xFF) as u8);
        outb(PIT_CHANNEL2, ((divisor >> 8) & 0xFF) as u8);
        let val = inb(0x61);
        outb(0x61, val | 0x03);
    }
    let hz = SYSTEM_TICK_HZ.load(Ordering::Relaxed).max(1) as u64;
    let wait_ticks = (duration_ms as u64).saturating_mul(hz).saturating_add(999) / 1_000;
    if crate::arch::x86_64::cpu::read_rflags() & (1 << 9) != 0 {
        let start_ticks = get_ticks();
        while get_ticks().wrapping_sub(start_ticks) < wait_ticks {
            core::hint::spin_loop();
        }
    }
    unsafe {
        let val = inb(0x61);
        outb(0x61, val & !0x03);
    }
}
