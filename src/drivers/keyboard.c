#include "keyboard.h"
#include "mouse.h"
#include "io.h"
#include "idt.h"
#include "serial.h"

static char kbd_buffer[KBD_BUFFER_SIZE];
static volatile uint16_t kbd_head = 0;
static volatile uint16_t kbd_tail = 0;

static int shift_pressed = 0;
static int caps_lock = 0;

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

void keyboard_process_scancode(uint8_t scancode) {
    if (scancode == 0x2A || scancode == 0x36) { // Left / Right Shift pressed
        shift_pressed = 1;
        return;
    }
    if (scancode == 0xAA || scancode == 0xB6) { // Shift released
        shift_pressed = 0;
        return;
    }
    if (scancode == 0x3A) { // Caps Lock
        caps_lock = !caps_lock;
        return;
    }

    if (scancode & 0x80) {
        // Key release
        return;
    }

    char ch = 0;
    if (scancode < 128) {
        int use_upper = shift_pressed ^ caps_lock;
        ch = use_upper ? scancode_ascii_upper[scancode] : scancode_ascii_lower[scancode];
    }

    if (ch != 0) {
        uint16_t next_head = (kbd_head + 1) % KBD_BUFFER_SIZE;
        if (next_head != kbd_tail) {
            kbd_buffer[kbd_head] = ch;
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
    shift_pressed = 0;
    caps_lock = 0;

#ifndef __KERNEL_HOST_TEST__
    register_interrupt_handler(33, keyboard_handler);
    kinfo("Keyboard: PS/2 Set-1 handler registered on IRQ 1");
#endif
}
