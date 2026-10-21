#ifndef MOUSE_H
#define MOUSE_H

#include "types.h"

#define MOUSE_BTN_LEFT   (1 << 0)
#define MOUSE_BTN_RIGHT  (1 << 1)
#define MOUSE_BTN_MIDDLE (1 << 2)

typedef struct {
    int32_t x;
    int32_t y;
    int32_t dx;
    int32_t dy;
    uint8_t buttons;
} mouse_state_t;

int  mouse_init(void);
int  mouse_is_present(void);
void mouse_get_state(mouse_state_t *out_state);
int32_t mouse_get_x(void);
int32_t mouse_get_y(void);
uint8_t mouse_get_buttons(void);

#endif /* MOUSE_H */
