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

/// Synchronize the PS/2 position with the software pointer when the display starts.
pub fn set_position(x: i32, y: i32) {
    let mut state = MOUSE.lock();
    state.x = x;
    state.y = y;
}

unsafe fn wait_status(mask: u8, expected: u8) -> bool {
    (0..100_000).any(|_| inb(MOUSE_CMD) & mask == expected)
}

unsafe fn write(port: u16, value: u8) -> bool {
    if !wait_status(2, 0) {
        return false;
    }
    outb(port, value);
    true
}

unsafe fn mouse_command(command: u8) -> bool {
    for _ in 0..3 {
        if !write(MOUSE_CMD, 0xD4) || !write(MOUSE_DATA, command) {
            return false;
        }
        for _ in 0..64 {
            if !wait_status(1, 1) {
                return false;
            }
            let status = inb(MOUSE_CMD);
            let byte = inb(MOUSE_DATA);
            if status & 0x20 == 0 {
                crate::drivers::keyboard::process_scancode(byte);
                continue;
            }
            match byte {
                0xFA => return true,
                0xFE => break,
                _ => continue,
            }
        }
    }
    false
}

unsafe fn configure() -> bool {
    // Pause the keyboard port while reading the controller configuration byte.
    if !write(MOUSE_CMD, 0xAD) {
        return false;
    }
    crate::drivers::keyboard::poll_hardware();
    let configured = if write(MOUSE_CMD, 0x20) && wait_status(1, 1) {
        let config = inb(MOUSE_DATA);
        write(MOUSE_CMD, 0x60)
            && write(MOUSE_DATA, (config | 0x02) & !0x20)
            && write(MOUSE_CMD, 0xA8)
    } else {
        false
    };
    let keyboard_enabled = write(MOUSE_CMD, 0xAE);
    configured && keyboard_enabled && mouse_command(0xF6) && mouse_command(0xF4)
}

pub fn init() {
    unsafe {
        if !configure() {
            crate::drivers::serial::println("Mouse: PS/2 initialization failed");
            return;
        }
        crate::arch::x86_64::idt::register_handler(44, on_interrupt);
        crate::arch::x86_64::pic::clear_mask(2); // Slave PIC cascade line
        crate::arch::x86_64::pic::clear_mask(12); // Unmask IRQ 12
    }

    crate::drivers::serial::println("Mouse: PS/2 Mouse Initialized");
}

pub fn reset_packet() {
    MOUSE.lock().cycle = 0;
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
            if flags & 0x40 != 0 {
                dx = 0;
            }
            if flags & 0x80 != 0 {
                dy = 0;
            }

            m.buttons = flags & 0x07;
            let (max_x, max_y) = if video::is_available() {
                ((video::get_width() as i32).saturating_sub(1), (video::get_height() as i32).saturating_sub(1))
            } else {
                (1023, 767)
            };
            m.x = (m.x + dx).clamp(0, max_x);
            m.y = (m.y - dy).clamp(0, max_y);

            let cur_x = m.x;
            let cur_y = m.y;
            let cur_btns = m.buttons;
            drop(m);

            crate::compositor::handle_mouse_event(cur_x, cur_y, cur_btns);
        }
        _ => m.cycle = 0,
    }
}
