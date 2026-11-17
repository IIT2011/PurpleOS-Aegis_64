use crate::arch::x86_64::io::{inb, outb};
use crate::drivers::video;
use crate::sync::Spinlock;

const MOUSE_DATA: u16 = 0x60;
const MOUSE_CMD: u16 = 0x64;

pub struct MouseState {
    pub x: i32,
    pub y: i32,
    pub buttons: u8,
    pub cycle: u8,
    pub packet: [u8; 3],
}

pub static MOUSE: Spinlock<MouseState> = Spinlock::new(MouseState {
    x: 0,
    y: 0,
    buttons: 0,
    cycle: 0,
    packet: [0; 3],
});

unsafe fn mouse_wait(a_type: u8) {
    let mut timeout = 100000;
    while timeout > 0 {
        let status = inb(MOUSE_CMD);
        if a_type == 0 && (status & 1) == 1 {
            return;
        }
        if a_type == 1 && (status & 2) == 0 {
            return;
        }
        timeout -= 1;
    }
}

unsafe fn mouse_write(write: u8) {
    mouse_wait(1);
    outb(MOUSE_CMD, 0xD4);
    mouse_wait(1);
    outb(MOUSE_DATA, write);
}

unsafe fn mouse_read() -> u8 {
    mouse_wait(0);
    inb(MOUSE_DATA)
}

pub fn init() {
    unsafe {
        mouse_wait(1);
        outb(MOUSE_CMD, 0xA8); // Enable auxiliary device (mouse)

        mouse_wait(1);
        outb(MOUSE_CMD, 0x20); // Get compo pack
        mouse_wait(0);
        let status = inb(MOUSE_DATA) | 2; // Enable mouse interrupt
        mouse_wait(1);
        outb(MOUSE_CMD, 0x60);
        mouse_wait(1);
        outb(MOUSE_DATA, status);

        mouse_write(0xF6); // Set default settings
        let _ = mouse_read();

        mouse_write(0xF4); // Enable packet streaming
        let _ = mouse_read();

        crate::arch::x86_64::pic::clear_mask(2); // Slave PIC cascade line
        crate::arch::x86_64::pic::clear_mask(12); // Unmask IRQ 12
        crate::arch::x86_64::idt::register_handler(44, on_interrupt);
    }

    crate::drivers::serial::println("Mouse: PS/2 Mouse Initialized");
}

fn on_interrupt(_frame: &mut crate::arch::x86_64::idt::InterruptFrame) {
    crate::drivers::keyboard::poll_hardware();
}

pub fn handle_byte(b: u8) {
    let mut m = MOUSE.lock();
    match m.cycle {
        0 => {
            if (b & 0x08) != 0 { // Check sync bit
                m.packet[0] = b;
                m.cycle = 1;
            }
        }
        1 => {
            m.packet[1] = b;
            m.cycle = 2;
        }
        2 => {
            m.packet[2] = b;
            m.cycle = 0;

            let flags = m.packet[0];
            let mut dx = m.packet[1] as i32;
            let mut dy = m.packet[2] as i32;

            if (flags & 0x10) != 0 {
                dx |= !0xFF;
            }
            if (flags & 0x20) != 0 {
                dy |= !0xFF;
            }

            m.buttons = flags & 0x07;
            let (max_x, max_y) = if video::is_available() {
                ((video::get_width() as i32).saturating_sub(1), (video::get_height() as i32).saturating_sub(1))
            } else {
                (1023, 767)
            };
            m.x = (m.x + dx).clamp(0, max_x);
            m.y = (m.y - dy).clamp(0, max_y);
        }
        _ => m.cycle = 0,
    }
}
