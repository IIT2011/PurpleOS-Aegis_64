use crate::arch::x86_64::io::{inb, outb};
use crate::sync::Spinlock;
use core::fmt;

const COM1: u16 = 0x3F8;

pub const CAPTURE_LIMIT: usize = 4096;

struct Capture {
    cpu: Option<usize>,
    buffer: [u8; CAPTURE_LIMIT],
    length: usize,
}

static SERIAL_LOCK: Spinlock<Capture> = Spinlock::new(Capture {
    cpu: None,
    buffer: [0; CAPTURE_LIMIT],
    length: 0,
});

pub(crate) fn capturing_here() -> bool {
    SERIAL_LOCK
        .lock()
        .cpu
        .is_some_and(|cpu| cpu == crate::kernel::smp::get_current_cpu())
}

pub(crate) fn capture(output: &mut [u8], action: impl FnOnce()) -> usize {
    {
        let mut serial = SERIAL_LOCK.lock();
        serial.cpu = Some(crate::kernel::smp::get_current_cpu());
        serial.length = 0;
    }
    action();
    let mut serial = SERIAL_LOCK.lock();
    serial.cpu = None;
    let length = serial.length.min(output.len());
    output[..length].copy_from_slice(&serial.buffer[..length]);
    length
}

impl Capture {
    fn write(&mut self, bytes: &[u8]) {
        if self
            .cpu
            .is_some_and(|cpu| cpu == crate::kernel::smp::get_current_cpu())
        {
            let length = bytes.len().min(self.buffer.len() - self.length);
            self.buffer[self.length..self.length + length].copy_from_slice(&bytes[..length]);
            self.length += length;
            return;
        }
        for &byte in bytes {
            unsafe {
                while (inb(COM1 + 5) & 0x20) == 0 {
                    core::hint::spin_loop();
                }
                outb(COM1, byte);
            }
        }
    }

    fn text(&mut self, text: &str) {
        for part in text.split_inclusive('\n') {
            if let Some(line) = part.strip_suffix('\n') {
                self.write(line.as_bytes());
                self.write(b"\r\n");
            } else {
                self.write(part.as_bytes());
            }
        }
    }
}

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
    SERIAL_LOCK.lock().write(&[c]);
}

pub fn print(s: &str) {
    SERIAL_LOCK.lock().text(s);
}

pub fn println(s: &str) {
    let mut serial = SERIAL_LOCK.lock();
    serial.text(s);
    serial.write(b"\r\n");
}

pub fn print_u64(n: u64) {
    print_number(n, 10, 1);
}

pub fn print_hex(n: u64) {
    print_number(n, 16, 1);
}

pub fn print_hex_u64(n: u64) {
    print_number(n, 16, 16);
}

fn print_number(mut value: u64, radix: u64, minimum: usize) {
    let mut bytes = [0; 20];
    let mut index = bytes.len();
    let digits = b"0123456789abcdef";
    while value != 0 || bytes.len() - index < minimum {
        index -= 1;
        bytes[index] = digits[(value % radix) as usize];
        value /= radix;
    }
    SERIAL_LOCK.lock().write(&bytes[index..]);
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
