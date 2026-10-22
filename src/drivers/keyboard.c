#include "keyboard.h"
#include "mouse.h"
#include "io.h"
#include "idt.h"
#include "pic.h"
#include "serial.h"

static char kbd_buffer[KBD_BUFFER_SIZE];
static volatile uint16_t kbd_head = 0;
static volatile uint16_t kbd_tail = 0;

static int shift_pressed = 0;
static int ctrl_pressed = 0;
static int alt_pressed = 0;
static int caps_lock = 0;

static keyboard_event_callback_t kbd_callback = NULL;

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

void keyboard_set_callback(keyboard_event_callback_t cb) {
    kbd_callback = cb;
}

int keyboard_is_shift(void) { return shift_pressed; }
int keyboard_is_ctrl(void)  { return ctrl_pressed; }
int keyboard_is_alt(void)   { return alt_pressed; }
int keyboard_is_caps(void)  { return caps_lock; }

void keyboard_process_scancode(uint8_t scancode) {
    int is_press = !(scancode & 0x80);
    uint8_t code = scancode & 0x7F;

    /* Handle modifier key changes */
    if (code == KEY_LSHIFT || code == KEY_RSHIFT) {
        shift_pressed = is_press;
    } else if (code == KEY_LCTRL) {
        ctrl_pressed = is_press;
    } else if (code == KEY_LALT) {
        alt_pressed = is_press;
    } else if (code == KEY_CAPSLOCK && is_press) {
        caps_lock = !caps_lock;
    }

    char ch = 0;
    if (is_press && code < 128) {
        int use_upper = shift_pressed ^ caps_lock;
        ch = use_upper ? scancode_ascii_upper[code] : scancode_ascii_lower[code];

        /* Store into FIFO char buffer */
        if (ch != 0) {
            uint16_t next_head = (kbd_head + 1) % KBD_BUFFER_SIZE;
            if (next_head != kbd_tail) {
                kbd_buffer[kbd_head] = ch;
                kbd_head = next_head;
            }
        }
    }

    /* Deliver real-time event to registered desktop/window callback */
    if (kbd_callback) {
        kbd_callback(ch, code, is_press);
    }
}

#ifndef __KERNEL_HOST_TEST__
static void kbd_wait_write(void) {
    uint32_t timeout = 100000;
    while (timeout--) {
        if ((inb(0x64) & 0x02) == 0) return;
    }
}

static void kbd_wait_read(void) {
    uint32_t timeout = 100000;
    while (timeout--) {
        if ((inb(0x64) & 0x01) == 1) return;
    }
}

static void keyboard_handler(interrupt_frame_t *frame) {
    (void)frame;
    uint8_t status = inb(0x64);
    while (status & 0x01) {
        uint8_t data = inb(0x60);
        if (status & 0x20) {
            /* 8042 auxiliary port has mouse data */
            mouse_process_byte(data);
        } else {
            /* 8042 primary port has keyboard scancode */
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
    shift_pressed = 0;
    ctrl_pressed = 0;
    alt_pressed = 0;
    caps_lock = 0;
    kbd_callback = NULL;

#ifndef __KERNEL_HOST_TEST__
    /* 1. Enable first PS/2 port (Keyboard) on 8042 controller */
    kbd_wait_write();
    outb(0x64, 0xAE);

    /* 2. Configure 8042 command byte: enable IRQ 1 and enable keyboard clock */
    kbd_wait_write();
    outb(0x64, 0x20);
    kbd_wait_read();
    uint8_t comp = inb(0x60);

    comp |= 0x01;  /* Enable IRQ 1 (first port) */
    comp &= ~0x10; /* Clear first port clock disable */

    kbd_wait_write();
    outb(0x64, 0x60);
    kbd_wait_write();
    outb(0x60, comp);

    /* 3. Register IDT Interrupt Vector 33 (IRQ 1) */
    register_interrupt_handler(33, keyboard_handler);

    /* 4. Unmask IRQ 1 on Master PIC */
    pic_unmask(1);

    kinfo("Keyboard: PS/2 Set-1 controller configured and handler active on IRQ 1");
#endif
}
