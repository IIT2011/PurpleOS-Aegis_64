#ifndef KEYBOARD_H
#define KEYBOARD_H

#include "types.h"

#define KBD_BUFFER_SIZE 256

/* Common Special Key Scancodes (Set 1 Make codes) */
#define KEY_ESC         0x01
#define KEY_BACKSPACE   0x0E
#define KEY_TAB         0x0F
#define KEY_ENTER       0x1C
#define KEY_LCTRL       0x1D
#define KEY_LSHIFT      0x2A
#define KEY_RSHIFT      0x36
#define KEY_LALT        0x38
#define KEY_CAPSLOCK    0x3A
#define KEY_F1          0x3B
#define KEY_F2          0x3C
#define KEY_F3          0x3D
#define KEY_F4          0x3E
#define KEY_HOME        0x47
#define KEY_UP          0x48
#define KEY_PAGEUP      0x49
#define KEY_LEFT        0x4B
#define KEY_RIGHT       0x4D
#define KEY_END         0x4F
#define KEY_DOWN        0x50
#define KEY_PAGEDOWN    0x51
#define KEY_DELETE      0x53

typedef void (*keyboard_event_callback_t)(char ch, uint8_t scancode, int is_press);

void keyboard_init(void);
char keyboard_getchar(void);
int  keyboard_has_char(void);
void keyboard_process_scancode(uint8_t scancode);
void keyboard_set_callback(keyboard_event_callback_t cb);

int  keyboard_is_shift(void);
int  keyboard_is_ctrl(void);
int  keyboard_is_alt(void);
int  keyboard_is_caps(void);

#endif /* KEYBOARD_H */
