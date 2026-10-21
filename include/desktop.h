#ifndef DESKTOP_H
#define DESKTOP_H

#include "types.h"

#define DESKTOP_MAX_WINDOWS 8

typedef struct {
    uint32_t id;
    char     title[32];
    uint32_t x;
    uint32_t y;
    uint32_t width;
    uint32_t height;
    uint32_t bg_color;
    uint32_t title_bg;
    int      visible;
    int      focused;
} gui_window_t;

int  desktop_init(void);
int  desktop_create_window(const char *title, uint32_t x, uint32_t y, uint32_t w, uint32_t h, uint32_t bg);
void desktop_render(void);
void desktop_render_preview(void);
void desktop_draw_cursor(uint32_t x, uint32_t y);

#endif /* DESKTOP_H */
