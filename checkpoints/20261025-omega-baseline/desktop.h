#ifndef DESKTOP_H
#define DESKTOP_H

#include "types.h"
#include "icon_data.h"

#define DESKTOP_MAX_WINDOWS 16

typedef enum {
    DESKTOP_HIT_NONE = 0,
    DESKTOP_HIT_DESKTOP,
    DESKTOP_HIT_TOPBAR,
    DESKTOP_HIT_WINDOW_TITLE,
    DESKTOP_HIT_WINDOW_CLOSE,
    DESKTOP_HIT_WINDOW_MINIMIZE,
    DESKTOP_HIT_WINDOW_MAXIMIZE,
    DESKTOP_HIT_WINDOW_BODY,
    DESKTOP_HIT_WINDOW_BUTTON,
    DESKTOP_HIT_WINDOW_TAB,
    DESKTOP_HIT_LAUNCHER_BTN,
    DESKTOP_HIT_LAUNCHER_ITEM,
    DESKTOP_HIT_DOCK_ITEM,
    DESKTOP_HIT_SYSTEM_PANEL,
    DESKTOP_HIT_NOTIF_CLOSE
} desktop_hit_type_t;

typedef struct {
    desktop_hit_type_t type;
    int window_index;     /* -1 if not a window */
    uint32_t menu_index;  /* if topbar, dock, or window control */
    int32_t click_x;      /* local click offset inside target */
    int32_t click_y;
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

#define WIN_TYPE_GENERIC    0
#define WIN_TYPE_TERMINAL   1
#define WIN_TYPE_STORAGE    2
#define WIN_TYPE_FILES      2
#define WIN_TYPE_SECURITY   3
#define WIN_TYPE_SYSMON     3
#define WIN_TYPE_SETTINGS   4
#define WIN_TYPE_APPMGR     5
#define WIN_TYPE_EDITOR     6
#define WIN_TYPE_CALCULATOR 7
#define WIN_TYPE_HELP       8
#define WIN_TYPE_VLC        9
#define WIN_TYPE_VIEWER     10
#define WIN_TYPE_DISKMGR    11

#define TERM_MAX_OUTPUT_LINES 128
#define TERM_LINE_LEN 96

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
    uint32_t icon_id;

    /* Window Bounded Drawing Surface */
    uint32_t *surface;
    uint32_t surface_w;
    uint32_t surface_h;
    uint32_t surface_pitch;

    /* Window geometry state for maximize/restore */
    int      is_maximized;
    uint32_t saved_x;
    uint32_t saved_y;
    uint32_t saved_w;
    uint32_t saved_h;

    /* Window animation state */
    int      anim_active;
    uint32_t anim_start_tick;
    uint32_t anim_duration;
    int      anim_type;     /* 1=open, 2=close, 3=minimize, 4=restore, 5=maximize */
    int      anim_progress; /* 0..255 */

    /* Interactive Terminal Fields */
    char     input_line[128];
    uint32_t input_cursor;
    char     history[32][64];
    int      history_count;
    int      history_idx;
    char     output_lines[TERM_MAX_OUTPUT_LINES][TERM_LINE_LEN];
    int      output_count;
    int      term_scroll_offset; /* 0 = bottom live output, >0 = scrolled up */

    /* Calculator Application Fields */
    char     calc_display[32];
    int64_t  calc_prev_val;
    char     calc_op;
    int      calc_new_entry;

    /* Text Editor Application Fields */
    char     editor_filename[64];
    char     editor_content[2048];
    uint32_t editor_cursor;
    int      editor_modified;

    /* Settings & Preferences Fields */
    int      settings_active_tab;
    int      settings_volume;
    int      settings_dark_mode;
    int      settings_night_mode;
    int      settings_reduced_motion;
    int      settings_anim_enabled;
    int      settings_theme_accent; /* 0=Cosmic Purple, 1=Violet, 2=Neon Magenta, 3=Deep Midnight */

    /* Files / Storage Explorer Fields */
    char     files_current_path[64];
    int      files_selected_idx;
    int      files_item_count;
    char     files_items[16][32];
    uint8_t  files_is_dir[16];
    uint32_t files_sizes[16];
    char     files_status[64];

    /* App Manager Fields */
    int      appmgr_selected_idx;

    /* VLC Media Player Fields (Default Media Player) */
    int      vlc_state;          /* 0=stopped, 1=playing, 2=paused */
    char     vlc_track[64];
    char     vlc_artist[32];
    uint32_t vlc_elapsed_sec;
    uint32_t vlc_total_sec;
    uint32_t vlc_pos_permille;   /* 0..1000 */
    int      vlc_volume;         /* 0..100 */
    int      vlc_muted;
    int      vlc_playlist_idx;
    int      vlc_playlist_count;
    char     vlc_playlist[6][32];
    uint8_t  vlc_spectrum[16];

    /* Purple View Document Viewer Fields (.txt, .md, .pdf) */
    char     viewer_filename[64];
    int      viewer_type;        /* 1=txt, 2=md, 3=pdf */
    int      viewer_scroll;
    int      viewer_total_lines;
    int      viewer_page_curr;
    int      viewer_page_total;
    int      viewer_zoom;
    char     viewer_content[4096];

    /* Purple Partition Manager & Disk Utility Fields */
    int      diskmgr_selected_disk;
    int      diskmgr_selected_part;
    int      diskmgr_confirm_action; /* 0=none, 1=format, 2=unmount, 3=mount */
    char     diskmgr_action_desc[64];
} gui_window_t;

int  desktop_init(void);
int  desktop_create_window(const char *title, uint32_t x, uint32_t y, uint32_t w, uint32_t h, uint32_t bg);
int  desktop_close_window(uint32_t id);
int  desktop_focus_window(uint32_t id);
int  desktop_toggle_window(uint32_t id);
int  desktop_minimize_window(uint32_t id);
int  desktop_maximize_window(uint32_t id);
int  desktop_open_application(const char *app_name);
gui_window_t *desktop_get_window(uint32_t id);
uint32_t desktop_get_window_count(void);
desktop_hit_result_t desktop_hit_test(int32_t x, int32_t y);

void desktop_render(void);
void desktop_composite(void);
void desktop_render_preview(void);
void desktop_draw_cursor(uint32_t x, uint32_t y);
void desktop_erase_cursor(void);
void desktop_move_cursor(int32_t nx, int32_t ny);

int  desktop_handle_mouse(int32_t x, int32_t y, uint8_t buttons);
void desktop_handle_key(char c, uint8_t scancode, int pressed, uint8_t modifiers);
void desktop_terminal_put_line(gui_window_t *win, const char *line);
void desktop_terminal_write(const char *s);
void desktop_terminal_clear(void);
void desktop_terminal_execute(gui_window_t *win, const char *cmd);
void desktop_run_session(void);
int  desktop_is_active(void);
void desktop_set_active(int active);

#endif /* DESKTOP_H */
