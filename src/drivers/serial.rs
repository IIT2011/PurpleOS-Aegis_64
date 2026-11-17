use crate::arch::x86_64::io::{inb, outb};
use crate::sync::Spinlock;
use core::fmt;

const COM1: u16 = 0x3F8;

static SERIAL_LOCK: Spinlock<()> = Spinlock::new(());

pub fn init() {
    let _guard = SERIAL_LOCK.lock();
    unsafe {
        outb(COM1 + 1, 0x00); // Disable interrupts
        outb(COM1 + 3, 0x80); // Enable DLAB
        outb(COM1 + 0, 0x03); // Divisor 3 = 38400 baud
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x03); // 8 bits, no parity, 1 stop bit
        outb(COM1 + 2, 0xC7); // Enable FIFO, 14-byte threshold
        outb(COM1 + 4, 0x0B); // IRQs enabled, RTS/DSR set
    }
}

pub fn has_char() -> bool {
    unsafe { (inb(COM1 + 5) & 1) != 0 }
}

pub fn getchar() -> u8 {
    while !has_char() {
        core::hint::spin_loop();
    }
    unsafe { inb(COM1) }
}

pub fn putchar(c: u8) {
    let _guard = SERIAL_LOCK.lock();
    unsafe {
        while (inb(COM1 + 5) & 0x20) == 0 {
            core::hint::spin_loop();
        }
        outb(COM1, c);
    }
}

pub fn print(s: &str) {
    let _guard = SERIAL_LOCK.lock();
    for b in s.bytes() {
        if b == b'\n' {
            unsafe {
                while (inb(COM1 + 5) & 0x20) == 0 {
                    core::hint::spin_loop();
                }
                outb(COM1, b'\r');
            }
        }
        unsafe {
            while (inb(COM1 + 5) & 0x20) == 0 {
                core::hint::spin_loop();
            }
            outb(COM1, b);
        }
    }
}

pub fn println(s: &str) {
    let _guard = SERIAL_LOCK.lock();
    for b in s.bytes() {
        if b == b'\n' {
            unsafe {
                while (inb(COM1 + 5) & 0x20) == 0 {
                    core::hint::spin_loop();
                }
                outb(COM1, b'\r');
            }
        }
        unsafe {
            while (inb(COM1 + 5) & 0x20) == 0 {
                core::hint::spin_loop();
            }
            outb(COM1, b);
        }
    }
    unsafe {
        while (inb(COM1 + 5) & 0x20) == 0 {
            core::hint::spin_loop();
        }
        outb(COM1, b'\r');
        while (inb(COM1 + 5) & 0x20) == 0 {
            core::hint::spin_loop();
        }
        outb(COM1, b'\n');
    }
}

pub fn print_u64(mut n: u64) {
    let _guard = SERIAL_LOCK.lock();
    if n == 0 {
        unsafe {
            while (inb(COM1 + 5) & 0x20) == 0 {
                core::hint::spin_loop();
            }
            outb(COM1, b'0');
        }
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = 0;
    while n > 0 {
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        unsafe {
            while (inb(COM1 + 5) & 0x20) == 0 {
                core::hint::spin_loop();
            }
            outb(COM1, buf[i]);
        }
    }
}

pub fn print_hex(mut n: u64) {
    let _guard = SERIAL_LOCK.lock();
    if n == 0 {
        unsafe {
            while (inb(COM1 + 5) & 0x20) == 0 {
                core::hint::spin_loop();
            }
            outb(COM1, b'0');
        }
        return;
    }
    let mut buf = [0u8; 16];
    let mut i = 0;
    let hex_chars = b"0123456789abcdef";
    while n > 0 {
        buf[i] = hex_chars[(n & 0xF) as usize];
        n >>= 4;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        unsafe {
            while (inb(COM1 + 5) & 0x20) == 0 {
                core::hint::spin_loop();
            }
            outb(COM1, buf[i]);
        }
    }
}

pub fn print_hex_u64(n: u64) {
    let _guard = SERIAL_LOCK.lock();
    let hex_chars = b"0123456789abcdef";
    for i in (0..16).rev() {
        let digit = ((n >> (i * 4)) & 0xF) as usize;
        unsafe {
            while (inb(COM1 + 5) & 0x20) == 0 {
                core::hint::spin_loop();
            }
            outb(COM1, hex_chars[digit]);
        }
    }
}

struct SerialWriter;

impl fmt::Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        print(s);
        Ok(())
    }
}

pub fn _print_fmt(args: fmt::Arguments) {
    use core::fmt::Write;
    let _ = SerialWriter.write_fmt(args);
}

#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {
        $crate::drivers::serial::_print_fmt(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! kprintln {
    () => {
        $crate::drivers::serial::print("\n")
    };
    ($($arg:tt)*) => {
        $crate::drivers::serial::_print_fmt(format_args!($($arg)*));
        $crate::drivers::serial::print("\n");
    };
}
