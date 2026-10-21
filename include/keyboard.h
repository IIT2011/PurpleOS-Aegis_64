#ifndef KEYBOARD_H
#define KEYBOARD_H

#include "types.h"

#define KBD_BUFFER_SIZE 256

void keyboard_init(void);
char keyboard_getchar(void);
int keyboard_has_char(void);
void keyboard_process_scancode(uint8_t scancode);

#endif /* KEYBOARD_H */
