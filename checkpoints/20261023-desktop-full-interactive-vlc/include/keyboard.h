#ifndef KEYBOARD_H
#define KEYBOARD_H

#include "types.h"

#define KBD_BUFFER_SIZE 256

/* Special Key Codes */
#define KEY_NONE        0
#define KEY_UP          0x80
#define KEY_DOWN        0x81
#define KEY_LEFT        0x82
#define KEY_RIGHT       0x83
#define KEY_HOME        0x84
#define KEY_END         0x85
#define KEY_PAGE_UP     0x86
#define KEY_PAGE_DOWN   0x87
#define KEY_INSERT      0x88
#define KEY_DELETE      0x89
#define KEY_F1          0x90
#define KEY_F2          0x91
#define KEY_F3          0x92
#define KEY_F4          0x93
#define KEY_F5          0x94
#define KEY_F6          0x95
#define KEY_F7          0x96
#define KEY_F8          0x97
#define KEY_F9          0x98
#define KEY_F10         0x99
#define KEY_F11         0x9A
#define KEY_F12         0x9B

#define KEY_ESC         27
#define KEY_BACKSPACE   '\b'
#define KEY_TAB         '\t'
#define KEY_ENTER       '\n'

/* Modifiers */
#define KBD_MOD_SHIFT   0x01
#define KBD_MOD_CTRL    0x02
#define KBD_MOD_ALT     0x04
#define KBD_MOD_CAPS    0x08

typedef struct {
    uint8_t scancode;
    uint8_t key_code;    /* ASCII char or KEY_* code */
    uint8_t pressed;     /* 1 = pressed, 0 = released */
    uint8_t modifiers;   /* Bitmask of KBD_MOD_* */
} key_event_t;

typedef void (*keyboard_event_callback_t)(const key_event_t *event);

void keyboard_init(void);
char keyboard_getchar(void);
int  keyboard_has_char(void);
int  keyboard_poll_event(key_event_t *event);
void keyboard_set_callback(keyboard_event_callback_t cb);
uint8_t keyboard_get_modifiers(void);
void keyboard_process_scancode(uint8_t scancode);

#endif /* KEYBOARD_H */
