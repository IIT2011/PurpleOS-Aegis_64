#ifndef DESKTOP_H
#define DESKTOP_H

#include "types.h"

#define DESKTOP_MAX_WINDOWS 8

typedef enum {
    DESKTOP_HIT_NONE = 0,
    DESKTOP_HIT_DESKTOP,
    DESKTOP_HIT_TOPBAR,
    DESKTOP_HIT_WINDOW_TITLE,
    DESKTOP_HIT_WINDOW_CLOSE,
    DESKTOP_HIT_WINDOW_BODY
} desktop_hit_type_t;

typedef struct {
    desktop_hit_type_t type;
    int window_index;     /* -1 if not a window */
    uint32_t menu_index;  /* if topbar */
} desktop_hit_result_t;

typedef enum {
    DESKTOP_EVENT_NONE = 0,
    DESKTOP_EVENT_CURSOR_MOVED,
    DESKTOP_EVENT_WINDOW_FOCUSED,
    DESKTOP_EVENT_WINDOW_CLOSED,
    DESKTOP_EVENT_WINDOW_MOVED,
    DESKTOP_EVENT_MENU_CLICKED
} desktop_event_t;

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
int  desktop_close_window(uint32_t id);
int  desktop_focus_window(uint32_t id);
int  desktop_toggle_window(uint32_t id);
gui_window_t *desktop_get_window(uint32_t id);
uint32_t desktop_get_window_count(void);
desktop_hit_result_t desktop_hit_test(int32_t x, int32_t y);

void desktop_render(void);
void desktop_render_preview(void);
void desktop_draw_cursor(uint32_t x, uint32_t y);
void desktop_erase_cursor(void);
void desktop_move_cursor(int32_t nx, int32_t ny);

int  desktop_handle_mouse(int32_t x, int32_t y, uint8_t buttons);
void desktop_run_session(void);
int  desktop_is_active(void);
void desktop_set_active(int active);

#endif /* DESKTOP_H */
