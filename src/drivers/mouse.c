#include "mouse.h"
#include "keyboard.h"
#include "io.h"
#include "idt.h"
#include "pic.h"
#include "serial.h"
#include "video.h"
#include "purple_detect.h"

static mouse_state_t mouse_state;
static int mouse_detected = 0;
static uint8_t mouse_cycle = 0;
static uint8_t mouse_packet[3];
static mouse_event_callback_t mouse_cb = NULL;

void mouse_set_callback(mouse_event_callback_t cb) {
    mouse_cb = cb;
}

void mouse_set_position(int32_t x, int32_t y) {
    int32_t max_x = video_is_available() ? (int32_t)video_get_width() - 1 : 1024;
    int32_t max_y = video_is_available() ? (int32_t)video_get_height() - 1 : 768;

    if (x < 0) x = 0;
    if (x > max_x) x = max_x;
    if (y < 0) y = 0;
    if (y > max_y) y = max_y;

    mouse_state.x = x;
    mouse_state.y = y;

    if (mouse_cb) {
        mouse_cb(mouse_state.x, mouse_state.y, mouse_state.buttons);
    }
}

void mouse_process_byte(uint8_t data) {
    if (mouse_cycle == 0) {
        if (data & 0x08) { // Bit 3 must be 1 in standard PS/2 packet
            mouse_packet[0] = data;
            mouse_cycle = 1;
        }
    } else if (mouse_cycle == 1) {
        mouse_packet[1] = data;
        mouse_cycle = 2;
    } else if (mouse_cycle == 2) {
        mouse_packet[2] = data;
        mouse_cycle = 0;

        uint8_t flags = mouse_packet[0];
        /* Reject packet if X or Y overflow bit is set */
        if (flags & 0xC0) {
            return;
        }

        int32_t dx = (int32_t)mouse_packet[1] - ((flags << 4) & 0x100);
        int32_t dy = (int32_t)mouse_packet[2] - ((flags << 3) & 0x100);

        mouse_state.buttons = flags & 0x07;
        mouse_state.dx = dx;
        mouse_state.dy = dy;

        mouse_state.x += dx;
        mouse_state.y -= dy; // Invert Y (downward screen coordinates)

        int32_t max_x = video_is_available() ? (int32_t)video_get_width() - 1 : 1024;
        int32_t max_y = video_is_available() ? (int32_t)video_get_height() - 1 : 768;

        if (mouse_state.x < 0) mouse_state.x = 0;
        if (mouse_state.x > max_x) mouse_state.x = max_x;
        if (mouse_state.y < 0) mouse_state.y = 0;
        if (mouse_state.y > max_y) mouse_state.y = max_y;

        if (mouse_cb) {
            mouse_cb(mouse_state.x, mouse_state.y, mouse_state.buttons);
        }
    }
}

#ifndef __KERNEL_HOST_TEST__
static void mouse_wait_write(void) {
    uint32_t timeout = 100000;
    while (timeout--) {
        if ((inb(0x64) & 0x02) == 0) return;
    }
}

static void mouse_wait_read(void) {
    uint32_t timeout = 100000;
    while (timeout--) {
        if ((inb(0x64) & 0x01) == 1) return;
    }
}

static void mouse_write(uint8_t val) {
    mouse_wait_write();
    outb(0x64, 0xD4);
    mouse_wait_write();
    outb(0x60, val);
}

static uint8_t mouse_read(void) {
    mouse_wait_read();
    return inb(0x60);
}

static void mouse_handler(interrupt_frame_t *frame) {
    (void)frame;
    uint8_t status = inb(0x64);
    while (status & 0x01) {
        uint8_t data = inb(0x60);
        if (status & 0x20) {
            /* Auxiliary PS/2 mouse packet */
            mouse_process_byte(data);
        } else {
            /* Keyboard scancode arrived on mouse IRQ */
            keyboard_process_scancode(data);
        }
        status = inb(0x64);
    }
}
#endif

int mouse_init(void) {
    mouse_state.x = 320;
    mouse_state.y = 240;
    mouse_state.dx = 0;
    mouse_state.dy = 0;
    mouse_state.buttons = 0;
    mouse_cycle = 0;

#ifndef __KERNEL_HOST_TEST__
    // 1. Enable Auxiliary Device (second PS/2 port)
    mouse_wait_write();
    outb(0x64, 0xA8);

    // 2. Read controller configuration byte
    mouse_wait_write();
    outb(0x64, 0x20);
    uint8_t comp_status = mouse_read();

    // Enable both IRQ 1 (Keyboard) and IRQ 12 (Mouse), and clear both clock disables
    comp_status |= 0x03;
    comp_status &= ~0x30;

    mouse_wait_write();
    outb(0x64, 0x60);
    mouse_wait_write();
    outb(0x60, comp_status);

    // 3. Set Default Settings (0xF6)
    mouse_write(0xF6);
    mouse_read(); // ACK (0xFA)

    // 4. Enable Streaming Data Reporting (0xF4)
    mouse_write(0xF4);
    uint8_t ack = mouse_read();
    if (ack == 0xFA) {
        mouse_detected = 1;
    } else {
        mouse_detected = 1; // Fallback assume present on VM
    }

    // 5. Register IRQ 12 (vector 44)
    register_interrupt_handler(44, mouse_handler);
    pic_unmask(2);  // Cascade IRQ
    pic_unmask(12); // Mouse IRQ

    kinfo("Mouse: PS/2 Auxiliary device initialized on IRQ 12");
    purple_detect_register(PURPLE_CAT_INPUT, "PS2MOU", "Standard", "PS/2 Mouse", "ISA", 0x60, 12,
                           PURPLE_STATE_ACTIVE, PURPLE_DEV_FLAG_BUILTIN);
#else
    mouse_detected = 1;
#endif

    return 0;
}

int mouse_is_present(void) {
    return mouse_detected;
}

void mouse_get_state(mouse_state_t *out_state) {
    if (out_state) {
        *out_state = mouse_state;
    }
}

int32_t mouse_get_x(void) {
    return mouse_state.x;
}

int32_t mouse_get_y(void) {
    return mouse_state.y;
}

uint8_t mouse_get_buttons(void) {
    return mouse_state.buttons;
}
