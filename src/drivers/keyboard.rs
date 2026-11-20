use crate::arch::x86_64::io::{inb, outb};
use crate::sync::Spinlock;

const KBD_DATA_PORT: u16 = 0x60;
const KBD_STATUS_PORT: u16 = 0x64;

pub const KBD_MOD_SHIFT: u8 = 1 << 0;
pub const KBD_MOD_CTRL: u8 = 1 << 1;
pub const KBD_MOD_ALT: u8 = 1 << 2;
pub const KBD_MOD_CAPS: u8 = 1 << 3;

struct KbdState {
    modifiers: u8,
    extended: bool,
    last_scancode: u8,
    last_tick: u64,
    buf: [u8; 256],
    buf_modifiers: [u8; 256],
    head: usize,
    tail: usize,
}

static KBD: Spinlock<KbdState> = Spinlock::new(KbdState {
    modifiers: 0,
    extended: false,
    last_scancode: 0,
    last_tick: 0,
    buf: [0; 256],
    buf_modifiers: [0; 256],
    head: 0,
    tail: 0,
});

unsafe fn wait_write() {
    let mut timeout = 100000;
    while (inb(KBD_STATUS_PORT) & 2) != 0 && timeout > 0 {
        core::hint::spin_loop();
        timeout -= 1;
    }
}

unsafe fn wait_read() -> bool {
    let mut timeout = 100000;
    while (inb(KBD_STATUS_PORT) & 1) == 0 && timeout > 0 {
        core::hint::spin_loop();
        timeout -= 1;
    }
    (inb(KBD_STATUS_PORT) & 1) != 0
}

pub fn init() {
    unsafe {
        // 1. Drain any pending bytes from 8042 controller output buffer
        let mut drain_limit = 64;
        while (inb(KBD_STATUS_PORT) & 1) != 0 && drain_limit > 0 {
            let _ = inb(KBD_DATA_PORT);
            drain_limit -= 1;
        }

        // 2. Read 8042 Controller Configuration Byte (cmd 0x20)
        wait_write();
        outb(KBD_STATUS_PORT, 0x20);
        let mut config = if wait_read() {
            inb(KBD_DATA_PORT)
        } else {
            0x01 | 0x40
        };

        // Bit 0: First PS/2 port interrupt enable (1)
        // Bit 4: First PS/2 port clock disable (0 = enabled)
        // Bit 6: First PS/2 port translation (1 = scancode translation to Set 1)
        config |= 0x01; // Enable IRQ 1
        config &= !0x10; // Enable clock
        config |= 0x40; // Enable scancode translation

        // 3. Write Controller Configuration Byte (cmd 0x60)
        wait_write();
        outb(KBD_STATUS_PORT, 0x60);
        wait_write();
        outb(KBD_DATA_PORT, config);

        // 4. Enable First PS/2 port (cmd 0xAE)
        wait_write();
        outb(KBD_STATUS_PORT, 0xAE);

        // 5. Configure Typematic Rate & Delay (cmd 0xF3)
        // Delay: 1000 ms (0b11 << 5 = 0x60), Rate: 2 Hz (0b11111 = 0x1F) -> 0x7F
        wait_write();
        outb(KBD_DATA_PORT, 0xF3);
        if wait_read() {
            let _ = inb(KBD_DATA_PORT);
        }
        wait_write();
        outb(KBD_DATA_PORT, 0b01111111);
        if wait_read() {
            let _ = inb(KBD_DATA_PORT);
        }

        // 6. Send Enable Scanning (0xF4) command to keyboard
        wait_write();
        outb(KBD_DATA_PORT, 0xF4);
        if wait_read() {
            let _ack = inb(KBD_DATA_PORT);
        }
    }

    crate::arch::x86_64::pic::clear_mask(1); // Unmask IRQ 1 (Keyboard)
    crate::arch::x86_64::idt::register_handler(33, on_interrupt);
    crate::drivers::serial::println("Keyboard: 8042 controller initialized, IRQ 1 unmasked");
    crate::drivers::serial::println(
        "Keyboard: PS/2 Keyboard Initialized (256-Byte Ring Buffer Active)",
    );
}

pub fn poll_hardware() {
    // Both IRQs and shell polling consume the same controller output buffer.
    static CONTROLLER_READ: Spinlock<()> = Spinlock::new(());
    let _controller = CONTROLLER_READ.lock();
    unsafe {
        let mut limit = 64;
        while limit > 0 {
            let status = inb(KBD_STATUS_PORT);
            if (status & 1) == 0 {
                break;
            }
            let data = inb(KBD_DATA_PORT);
            if status & 0xC0 != 0 {
                if status & 0x20 != 0 {
                    crate::drivers::mouse::reset_packet();
                }
            } else if (status & 0x20) == 0 {
                // First PS/2 port (Keyboard)
                process_scancode(data);
            } else {
                // Second PS/2 port (Mouse)
                crate::drivers::mouse::handle_byte(data);
            }
            limit -= 1;
        }
    }
}

fn on_interrupt(_frame: &mut crate::arch::x86_64::idt::InterruptFrame) {
    poll_hardware();
}

const SCANCODE_TABLE: [u8; 135] = [
    0, 27, b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', b'-', b'=', b'\x08', b'\t',
    b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i', b'o', b'p', b'[', b']', b'\n',
    0, /* Ctrl */
    b'a', b's', b'd', b'f', b'g', b'h', b'j', b'k', b'l', b';', b'\'', b'`',
    0, /* Left Shift */
    b'\\', b'z', b'x', b'c', b'v', b'b', b'n', b'm', b',', b'.', b'/', 0, /* Right Shift */
    b'*', 0, /* Alt */ b' ', 0, /* Caps */
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, /* F1-F10: 0x3B..0x44 */
    0, 0, /* Num/Scroll lock: 0x45, 0x46 */
    b'7', b'8', b'9', b'-', b'4', b'5', b'6', b'+', b'1', b'2', b'3', b'0',
    b'.', /* Keypad: 0x47..0x53 */
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

const SCANCODE_SHIFT_TABLE: [u8; 135] = [
    0, 27, b'!', b'@', b'#', b'$', b'%', b'^', b'&', b'*', b'(', b')', b'_', b'+', b'\x08', b'\t',
    b'Q', b'W', b'E', b'R', b'T', b'Y', b'U', b'I', b'O', b'P', b'{', b'}', b'\n',
    0, /* Ctrl */
    b'A', b'S', b'D', b'F', b'G', b'H', b'J', b'K', b'L', b':', b'"', b'~',
    0, /* Left Shift */
    b'|', b'Z', b'X', b'C', b'V', b'B', b'N', b'M', b'<', b'>', b'?', 0, /* Right Shift */
    b'*', 0, /* Alt */ b' ', 0, /* Caps */
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, /* F1-F10: 0x3B..0x44 */
    0, 0, /* Num/Scroll lock: 0x45, 0x46 */
    b'7', b'8', b'9', b'-', b'4', b'5', b'6', b'+', b'1', b'2', b'3', b'0',
    b'.', /* Keypad: 0x47..0x53 */
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

pub fn process_scancode(sc: u8) {
    let mut k = KBD.lock();

    // Check extended scancode prefix
    if sc == 0xE0 {
        k.extended = true;
        return;
    }

    if k.extended {
        k.extended = false;
        // Extended release code
        if (sc & 0x80) != 0 {
            k.last_scancode = 0;
            return;
        }
        let ext_seq: &[u8] = match sc {
            0x48 => b"\x1b[A",  // Up Arrow
            0x50 => b"\x1b[B",  // Down Arrow
            0x4D => b"\x1b[C",  // Right Arrow
            0x4B => b"\x1b[D",  // Left Arrow
            0x47 => b"\x1b[H",  // Home
            0x4F => b"\x1b[F",  // End
            0x53 => b"\x1b[3~", // Delete
            0x49 => b"\x1b[5~", // Page Up
            0x51 => b"\x1b[6~", // Page Down
            0x1C => b"\n",      // Keypad Enter
            0x35 => b"/",       // Keypad /
            _ => b"",
        };
        for &byte in ext_seq {
            let next = (k.head + 1) % k.buf.len();
            if next != k.tail {
                let head = k.head;
                k.buf[head] = byte;
                k.buf_modifiers[head] = k.modifiers;
                k.head = next;
            }
        }
        return;
    }

    // Check release bit (Break code)
    if (sc & 0x80) != 0 {
        k.last_scancode = 0;
        let code = sc & 0x7F;
        if code == 0x2A || code == 0x36 {
            k.modifiers &= !KBD_MOD_SHIFT;
        } else if code == 0x1D {
            k.modifiers &= !KBD_MOD_CTRL;
        } else if code == 0x38 {
            k.modifiers &= !KBD_MOD_ALT;
        }
        return;
    }

    // Typematic repeat debounce:
    // If the identical make code arrives before key release, suppress repeat within 350 ms (35 PIT ticks)
    let now = crate::drivers::pit::get_ticks();
    if sc == k.last_scancode && now.saturating_sub(k.last_tick) < 35 {
        return;
    }
    k.last_scancode = sc;
    k.last_tick = now;

    if sc == 0x2A || sc == 0x36 {
        k.modifiers |= KBD_MOD_SHIFT;
        return;
    }
    if sc == 0x1D {
        k.modifiers |= KBD_MOD_CTRL;
        return;
    }
    if sc == 0x38 {
        k.modifiers |= KBD_MOD_ALT;
        return;
    }
    if sc == 0x3A {
        k.modifiers ^= KBD_MOD_CAPS;
        return;
    }

    if (sc as usize) < SCANCODE_TABLE.len() {
        let base_ch = SCANCODE_TABLE[sc as usize];
        let ch = if (k.modifiers & KBD_MOD_CTRL) != 0 {
            if base_ch >= b'a' && base_ch <= b'z' {
                base_ch - b'a' + 1
            } else if base_ch >= b'A' && base_ch <= b'Z' {
                base_ch - b'A' + 1
            } else {
                base_ch
            }
        } else if (k.modifiers & KBD_MOD_SHIFT) != 0 {
            let shift_ch = SCANCODE_SHIFT_TABLE[sc as usize];
            if (k.modifiers & KBD_MOD_CAPS) != 0 && shift_ch >= b'A' && shift_ch <= b'Z' {
                shift_ch + 32
            } else {
                shift_ch
            }
        } else if (k.modifiers & KBD_MOD_CAPS) != 0 && base_ch >= b'a' && base_ch <= b'z' {
            base_ch - 32
        } else {
            base_ch
        };

        if ch != 0 {
            let next = (k.head + 1) % k.buf.len();
            if next != k.tail {
                let head = k.head;
                k.buf[head] = ch;
                k.buf_modifiers[head] = k.modifiers;
                k.head = next;
            }
        }
    }
}

pub fn has_char() -> bool {
    poll_hardware();
    let k = KBD.lock();
    k.head != k.tail
}

pub fn try_getchar_with_modifiers() -> Option<(u8, u8)> {
    poll_hardware();
    let mut k = KBD.lock();
    if k.head == k.tail {
        return None;
    }
    let at = k.tail;
    let result = (k.buf[at], k.buf_modifiers[at]);
    k.tail = (k.tail + 1) % k.buf.len();
    Some(result)
}

pub fn try_getchar() -> Option<u8> {
    try_getchar_with_modifiers().map(|(byte, _)| byte)
}

pub fn getchar() -> u8 {
    loop {
        if let Some(ch) = try_getchar() {
            return ch;
        }
        core::hint::spin_loop();
    }
}

/// Current keyboard modifier flags for compositor shortcuts.
pub fn modifier_state() -> u8 {
    KBD.lock().modifiers
}
