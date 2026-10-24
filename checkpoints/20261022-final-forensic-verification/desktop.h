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
    DESKTOP_HIT_WINDOW_BODY,
    DESKTOP_HIT_LAUNCHER_BTN,
    DESKTOP_HIT_LAUNCHER_ITEM,
    DESKTOP_HIT_DOCK_ITEM,
    DESKTOP_HIT_SYSTEM_PANEL,
    DESKTOP_HIT_NOTIF_CLOSE
} desktop_hit_type_t;

typedef struct {
    desktop_hit_type_t type;
    int window_index;     /* -1 if not a window */
    uint32_t menu_index;  /* if topbar or dock */
} desktop_hit_result_t;

typedef enum {
    DESKTOP_EVENT_NONE = 0,
    DESKTOP_EVENT_CURSOR_MOVED,
    DESKTOP_EVENT_WINDOW_FOCUSED,
    DESKTOP_EVENT_WINDOW_CLOSED,
    DESKTOP_EVENT_WINDOW_MOVED,
    DESKTOP_EVENT_MENU_CLICKED,
    DESKTOP_EVENT_KEY_PRESSED
} desktop_event_t;

#define WIN_TYPE_GENERIC  0
#define WIN_TYPE_TERMINAL 1
#define WIN_TYPE_STORAGE  2
#define WIN_TYPE_SECURITY 3

#define TERM_MAX_OUTPUT_LINES 12
#define TERM_LINE_LEN 80

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
    int      type;

    /* Interactive Terminal Fields */
    char     input_line[128];
    uint32_t input_cursor;
    char     history[8][64];
    int      history_count;
    int      history_idx;
    char     output_lines[TERM_MAX_OUTPUT_LINES][TERM_LINE_LEN];
    int      output_count;
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
void desktop_handle_key(char c, uint8_t scancode, int pressed, uint8_t modifiers);
void desktop_terminal_put_line(gui_window_t *win, const char *line);
void desktop_terminal_execute(gui_window_t *win, const char *cmd);
void desktop_run_session(void);
int  desktop_is_active(void);
void desktop_set_active(int active);

#endif /* DESKTOP_H */
