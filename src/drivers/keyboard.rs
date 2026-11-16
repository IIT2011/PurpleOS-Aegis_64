use crate::arch::x86_64::io::inb;
use crate::sync::Spinlock;

const KBD_DATA_PORT: u16 = 0x60;
const KBD_STATUS_PORT: u16 = 0x64;

pub const KBD_MOD_SHIFT: u8 = 1 << 0;
pub const KBD_MOD_CTRL: u8 = 1 << 1;
pub const KBD_MOD_ALT: u8 = 1 << 2;
pub const KBD_MOD_CAPS: u8 = 1 << 3;

struct KbdState {
    modifiers: u8,
    buf: [u8; 128],
    head: usize,
    tail: usize,
}

static KBD: Spinlock<KbdState> = Spinlock::new(KbdState {
    modifiers: 0,
    buf: [0; 128],
    head: 0,
    tail: 0,
});

pub fn init() {
    crate::arch::x86_64::pic::clear_mask(1); // Unmask IRQ 1 (Keyboard)
    crate::arch::x86_64::idt::register_handler(33, on_interrupt);
    crate::drivers::serial::println("Keyboard: PS/2 Keyboard Initialized");
}

fn on_interrupt(_frame: &mut crate::arch::x86_64::idt::InterruptFrame) {
    unsafe {
        let status = inb(KBD_STATUS_PORT);
        if (status & 1) != 0 {
            let scancode = inb(KBD_DATA_PORT);
            process_scancode(scancode);
        }
    }
}

const SCANCODE_TABLE: [u8; 135] = [
    0, 27, b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', b'-', b'=', b'\x08',
    b'\t', b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i', b'o', b'p', b'[', b']', b'\n',
    0, /* Ctrl */
    b'a', b's', b'd', b'f', b'g', b'h', b'j', b'k', b'l', b';', b'\'', b'`',
    0, /* Left Shift */
    b'\\', b'z', b'x', b'c', b'v', b'b', b'n', b'm', b',', b'.', b'/',
    0, /* Right Shift */
    b'*', 0, /* Alt */ b' ', 0, /* Caps */
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, /* F1-F10 */
    0, 0, /* Num/Scroll lock */
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

const SCANCODE_SHIFT_TABLE: [u8; 135] = [
    0, 27, b'!', b'@', b'#', b'$', b'%', b'^', b'&', b'*', b'(', b')', b'_', b'+', b'\x08',
    b'\t', b'Q', b'W', b'E', b'R', b'T', b'Y', b'U', b'I', b'O', b'P', b'{', b'}', b'\n',
    0, /* Ctrl */
    b'A', b'S', b'D', b'F', b'G', b'H', b'J', b'K', b'L', b':', b'"', b'~',
    0, /* Left Shift */
    b'|', b'Z', b'X', b'C', b'V', b'B', b'N', b'M', b'<', b'>', b'?',
    0, /* Right Shift */
    b'*', 0, /* Alt */ b' ', 0, /* Caps */
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

pub fn process_scancode(sc: u8) {
    let mut k = KBD.lock();

    // Check release bit
    if (sc & 0x80) != 0 {
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
        let is_shifted = ((k.modifiers & KBD_MOD_SHIFT) != 0) ^ ((k.modifiers & KBD_MOD_CAPS) != 0);
        let ch = if is_shifted {
            SCANCODE_SHIFT_TABLE[sc as usize]
        } else {
            SCANCODE_TABLE[sc as usize]
        };

        if ch != 0 {
            let next = (k.head + 1) % k.buf.len();
            if next != k.tail {
                let head = k.head;
                k.buf[head] = ch;
                k.head = next;
            }
        }
    }
}

pub fn has_char() -> bool {
    let k = KBD.lock();
    k.head != k.tail
}

pub fn getchar() -> u8 {
    loop {
        {
            let mut k = KBD.lock();
            if k.head != k.tail {
                let ch = k.buf[k.tail];
                k.tail = (k.tail + 1) % k.buf.len();
                return ch;
            }
        }
        core::hint::spin_loop();
    }
}
