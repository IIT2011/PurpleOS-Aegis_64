#include "keyboard.h"
#include "mouse.h"
#include "io.h"
#include "idt.h"
#include "pic.h"
#include "serial.h"

static char kbd_buffer[KBD_BUFFER_SIZE];
static volatile uint16_t kbd_head = 0;
static volatile uint16_t kbd_tail = 0;

#define EVENT_QUEUE_SIZE 64
static key_event_t event_queue[EVENT_QUEUE_SIZE];
static volatile uint16_t event_head = 0;
static volatile uint16_t event_tail = 0;
static keyboard_event_callback_t event_callback = NULL;

static int shift_pressed = 0;
static int ctrl_pressed = 0;
static int alt_pressed = 0;
static int caps_lock = 0;
static int e0_prefix = 0;

static const char scancode_ascii_lower[128] = {
    0,  27, '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', '\b',
  '\t', 'q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p', '[', ']', '\n',
    0,  'a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', ';', '\'', '`',
    0, '\\', 'z', 'x', 'c', 'v', 'b', 'n', 'm', ',', '.', '/',   0,
  '*',   0, ' ',   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
    0,   0,   0,   0,   0, '-',   0,   0,   0, '+',   0,   0,   0,
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0
};

static const char scancode_ascii_upper[128] = {
    0,  27, '!', '@', '#', '$', '%', '^', '&', '*', '(', ')', '_', '+', '\b',
  '\t', 'Q', 'W', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P', '{', '}', '\n',
    0,  'A', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L', ':', '"', '~',
    0, '|', 'Z', 'X', 'C', 'V', 'B', 'N', 'M', '<', '>', '?',   0,
  '*',   0, ' ',   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,
    0,   0,   0,   0,   0, '-',   0,   0,   0, '+',   0,   0,   0,
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0
};

uint8_t keyboard_get_modifiers(void) {
    uint8_t mods = 0;
    if (shift_pressed) mods |= KBD_MOD_SHIFT;
    if (ctrl_pressed)  mods |= KBD_MOD_CTRL;
    if (alt_pressed)   mods |= KBD_MOD_ALT;
    if (caps_lock)     mods |= KBD_MOD_CAPS;
    return mods;
}

void keyboard_set_callback(keyboard_event_callback_t cb) {
    event_callback = cb;
}

static void enqueue_event(uint8_t scancode, uint8_t key_code, uint8_t pressed, uint8_t modifiers) {
    key_event_t ev;
    ev.scancode = scancode;
    ev.key_code = key_code;
    ev.pressed = pressed;
    ev.modifiers = modifiers;

    uint16_t next_head = (event_head + 1) % EVENT_QUEUE_SIZE;
    if (next_head != event_tail) {
        event_queue[event_head] = ev;
        event_head = next_head;
    }

    if (event_callback) {
        event_callback(&ev);
    }
}

int keyboard_poll_event(key_event_t *event) {
    if (event_head == event_tail || !event) return 0;
    *event = event_queue[event_tail];
    event_tail = (event_tail + 1) % EVENT_QUEUE_SIZE;
    return 1;
}

void keyboard_process_scancode(uint8_t scancode) {
    if (scancode == 0xE0) {
        e0_prefix = 1;
        return;
    }

    uint8_t current_modifiers = keyboard_get_modifiers();

    if (e0_prefix) {
        e0_prefix = 0;
        int is_release = (scancode & 0x80) != 0;
        uint8_t make_code = scancode & 0x7F;
        uint8_t key_code = KEY_NONE;

        switch (make_code) {
            case 0x1D: // Right Ctrl
                ctrl_pressed = !is_release;
                current_modifiers = keyboard_get_modifiers();
                break;
            case 0x38: // Right Alt
                alt_pressed = !is_release;
                current_modifiers = keyboard_get_modifiers();
                break;
            case 0x48: key_code = KEY_UP; break;
            case 0x50: key_code = KEY_DOWN; break;
            case 0x4B: key_code = KEY_LEFT; break;
            case 0x4D: key_code = KEY_RIGHT; break;
            case 0x47: key_code = KEY_HOME; break;
            case 0x4F: key_code = KEY_END; break;
            case 0x49: key_code = KEY_PAGE_UP; break;
            case 0x51: key_code = KEY_PAGE_DOWN; break;
            case 0x52: key_code = KEY_INSERT; break;
            case 0x53: key_code = KEY_DELETE; break;
            case 0x35: key_code = '/'; break;
            default: break;
        }

        if (key_code != KEY_NONE) {
            enqueue_event(scancode, key_code, !is_release, current_modifiers);
            if (!is_release) {
                uint16_t next_head = (kbd_head + 1) % KBD_BUFFER_SIZE;
                if (next_head != kbd_tail) {
                    kbd_buffer[kbd_head] = (char)key_code;
                    kbd_head = next_head;
                }
            }
        }
        return;
    }

    // Standard Set-1 Scancode Handling
    int is_release = (scancode & 0x80) != 0;
    uint8_t make_code = scancode & 0x7F;

    if (make_code == 0x2A || make_code == 0x36) { // Left / Right Shift
        shift_pressed = !is_release;
        current_modifiers = keyboard_get_modifiers();
        enqueue_event(scancode, KEY_NONE, !is_release, current_modifiers);
        return;
    }
    if (make_code == 0x1D) { // Left Ctrl
        ctrl_pressed = !is_release;
        current_modifiers = keyboard_get_modifiers();
        enqueue_event(scancode, KEY_NONE, !is_release, current_modifiers);
        return;
    }
    if (make_code == 0x38) { // Left Alt
        alt_pressed = !is_release;
        current_modifiers = keyboard_get_modifiers();
        enqueue_event(scancode, KEY_NONE, !is_release, current_modifiers);
        return;
    }
    if (make_code == 0x3A) { // Caps Lock
        if (!is_release) {
            caps_lock = !caps_lock;
            current_modifiers = keyboard_get_modifiers();
            enqueue_event(scancode, KEY_NONE, 1, current_modifiers);
        }
        return;
    }

    uint8_t key_code = KEY_NONE;
    if (make_code >= 0x3B && make_code <= 0x44) {
        key_code = KEY_F1 + (make_code - 0x3B);
    } else if (make_code == 0x57) {
        key_code = KEY_F11;
    } else if (make_code == 0x58) {
        key_code = KEY_F12;
    } else if (make_code < 128) {
        int use_upper = shift_pressed ^ caps_lock;
        key_code = (uint8_t)(use_upper ? scancode_ascii_upper[make_code] : scancode_ascii_lower[make_code]);
    }

    enqueue_event(scancode, key_code, !is_release, current_modifiers);

    if (!is_release && key_code != KEY_NONE) {
        uint16_t next_head = (kbd_head + 1) % KBD_BUFFER_SIZE;
        if (next_head != kbd_tail) {
            kbd_buffer[kbd_head] = (char)key_code;
            kbd_head = next_head;
        }
    }
}

#ifndef __KERNEL_HOST_TEST__
static void keyboard_handler(interrupt_frame_t *frame) {
    (void)frame;
    uint8_t status = inb(0x64);
    while (status & 0x01) {
        uint8_t data = inb(0x60);
        if (status & 0x20) {
            /* 8042 output port has mouse data waiting */
            mouse_process_byte(data);
        } else {
            /* Keyboard scancode */
            keyboard_process_scancode(data);
        }
        status = inb(0x64);
    }
}
#endif

int keyboard_has_char(void) {
    return kbd_head != kbd_tail;
}

char keyboard_getchar(void) {
    while (!keyboard_has_char()) {
        __asm__ volatile("pause");
    }
    char ch = kbd_buffer[kbd_tail];
    kbd_tail = (kbd_tail + 1) % KBD_BUFFER_SIZE;
    return ch;
}

void keyboard_init(void) {
    kbd_head = 0;
    kbd_tail = 0;
    event_head = 0;
    event_tail = 0;
    event_callback = NULL;
    shift_pressed = 0;
    ctrl_pressed = 0;
    alt_pressed = 0;
    caps_lock = 0;
    e0_prefix = 0;

#ifndef __KERNEL_HOST_TEST__
    register_interrupt_handler(33, keyboard_handler);

    // 1. Flush any stale bytes in the 8042 output buffer
    int flush_count = 0;
    while ((inb(0x64) & 0x01) && flush_count++ < 100) {
        inb(0x60);
    }

    // 2. Enable first PS/2 port (command 0xAE)
    while (inb(0x64) & 0x02);
    outb(0x64, 0xAE);

    // 3. Read Controller Configuration Byte (command 0x20)
    while (inb(0x64) & 0x02);
    outb(0x64, 0x20);
    while (!(inb(0x64) & 0x01));
    uint8_t cfg = inb(0x60);

    // Set bit 0 (IRQ 1 enable), bit 6 (translation enable), clear bit 4 (port 1 clock enable)
    cfg |= 0x01 | 0x40;
    cfg &= ~0x10;

    // Write back configuration byte (command 0x60)
    while (inb(0x64) & 0x02);
    outb(0x64, 0x60);
    while (inb(0x64) & 0x02);
    outb(0x60, cfg);

    // 4. Send Enable Scanning (0xF4) to keyboard
    while (inb(0x64) & 0x02);
    outb(0x60, 0xF4);
    int ack_tries = 0;
    while (!(inb(0x64) & 0x01) && ack_tries++ < 1000) {
        io_wait();
    }
    if (inb(0x64) & 0x01) {
        inb(0x60); // Read ACK
    }

    // 5. Unmask IRQ 1 on the 8259 PIC
    pic_unmask(1);
    kinfo("Keyboard: 8042 controller initialized, IRQ 1 unmasked, Set-1 scancode handler active");
#endif
}
