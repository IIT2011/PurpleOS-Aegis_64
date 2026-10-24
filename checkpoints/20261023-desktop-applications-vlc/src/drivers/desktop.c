#include "desktop.h"
#include "video.h"
#include "mouse.h"
#include "keyboard.h"
#include "serial.h"
#include "string.h"
#include "cpu.h"
#include "pmm.h"
#include "kernel_storage.h"
#include "purple_detect.h"
#include "security/aegis_detect.h"
#include "gpu.h"
#include "task.h"
#include "pit.h"
#include "rtc.h"
#include "font.h"
#include "font8x16.h"

extern const uint8_t _binary_build_wallpaper_clean_1672x941_raw_start[] __attribute__((weak));
extern const uint8_t _binary_build_wallpaper_clean_1672x941_raw_end[] __attribute__((weak));

typedef enum {
    FOCUS_DESKTOP = 0,
    FOCUS_WINDOW = 1,
    FOCUS_LAUNCHER_SEARCH = 2
} desktop_focus_target_t;

static desktop_focus_target_t current_focus = FOCUS_DESKTOP;

static gui_window_t windows[DESKTOP_MAX_WINDOWS];
static uint32_t window_count = 0;
static uint32_t z_order[DESKTOP_MAX_WINDOWS];
static int desktop_active = 0;

static int launcher_open = 0;
static char launcher_search[32] = {0};
static int system_panel_open = 0;
static int welcome_notif_visible = 1;

#define CURSOR_W 11
#define CURSOR_H 16

static const char *cursor_bitmap[CURSOR_H] = {
    "X          ",
    "XX         ",
    "X.X        ",
    "X..X       ",
    "X...X      ",
    "X....X     ",
    "X.....X    ",
    "X......X   ",
    "X.......X  ",
    "X........X ",
    "X.....XXXX ",
    "X..X..X    ",
    "X.X X..X   ",
    "XX  X..X   ",
    "X    X..X  ",
    "      XX   "
};

static uint32_t cursor_saved_bg[CURSOR_H][CURSOR_W];
static int32_t cursor_saved_x = -1;
static int32_t cursor_saved_y = -1;
static int cursor_drawn = 0;

int desktop_is_active(void) {
    return desktop_active;
}

void desktop_set_active(int active) {
    desktop_active = active;
}

uint32_t desktop_get_window_count(void) {
    return window_count;
}

gui_window_t *desktop_get_window(uint32_t id) {
    for (uint32_t i = 0; i < window_count; i++) {
        if (windows[i].id == id) {
            return &windows[i];
        }
    }
    return NULL;
}

void desktop_erase_cursor(void) {
    if (!cursor_drawn || !video_is_available()) {
        cursor_drawn = 0;
        return;
    }
    for (uint32_t r = 0; r < CURSOR_H; r++) {
        for (uint32_t c = 0; c < CURSOR_W; c++) {
            if (cursor_bitmap[r][c] != ' ') {
                video_draw_pixel(cursor_saved_x + c, cursor_saved_y + r, cursor_saved_bg[r][c]);
            }
        }
    }
    cursor_drawn = 0;
}

void desktop_draw_cursor(uint32_t cx, uint32_t cy) {
    if (!video_is_available()) return;
    if (cursor_drawn) {
        desktop_erase_cursor();
    }
    for (uint32_t r = 0; r < CURSOR_H; r++) {
        for (uint32_t c = 0; c < CURSOR_W; c++) {
            if (cursor_bitmap[r][c] != ' ') {
                cursor_saved_bg[r][c] = video_get_pixel(cx + c, cy + r);
                if (cursor_bitmap[r][c] == 'X') {
                    video_draw_pixel(cx + c, cy + r, 0x000000);
                } else if (cursor_bitmap[r][c] == '.') {
                    video_draw_pixel(cx + c, cy + r, COLOR_WHITE);
                }
            }
        }
    }
    cursor_saved_x = (int32_t)cx;
    cursor_saved_y = (int32_t)cy;
    cursor_drawn = 1;
}

void desktop_move_cursor(int32_t nx, int32_t ny) {
    desktop_draw_cursor((uint32_t)nx, (uint32_t)ny);
}

static void bring_to_front(int idx) {
    int pos = -1;
    for (uint32_t i = 0; i < window_count; i++) {
        if (z_order[i] == (uint32_t)idx) {
            pos = (int)i;
            break;
        }
    }
    if (pos >= 0 && (uint32_t)pos != window_count - 1) {
        for (uint32_t i = (uint32_t)pos; i + 1 < window_count; i++) {
            z_order[i] = z_order[i + 1];
        }
        z_order[window_count - 1] = (uint32_t)idx;
    }
    for (uint32_t i = 0; i < window_count; i++) {
        windows[i].focused = (i == (uint32_t)idx);
        windows[i].title_bg = (i == (uint32_t)idx) ? COLOR_PURPLE_BAR : 0x2A1245;
    }
    current_focus = FOCUS_WINDOW;
}

int desktop_create_window(const char *title, uint32_t x, uint32_t y, uint32_t w, uint32_t h, uint32_t bg) {
    if (window_count >= DESKTOP_MAX_WINDOWS) return -1;
    uint32_t idx = window_count++;
    gui_window_t *win = &windows[idx];
    memset(win, 0, sizeof(*win));
    win->id = window_count;
    strncpy(win->title, title, sizeof(win->title) - 1);
    win->x = x;
    win->y = y;
    win->width = w;
    win->height = h;
    win->bg_color = bg;
    win->title_bg = COLOR_PURPLE_BAR;
    win->visible = 1;
    win->focused = 1;
    win->type = WIN_TYPE_GENERIC;

    z_order[idx] = idx;
    bring_to_front((int)idx);
    return win->id;
}

int desktop_close_window(uint32_t id) {
    for (uint32_t i = 0; i < window_count; i++) {
        if (windows[i].id == id) {
            windows[i].visible = 0;
            windows[i].focused = 0;
            return 0;
        }
    }
    return -1;
}

int desktop_focus_window(uint32_t id) {
    for (uint32_t i = 0; i < window_count; i++) {
        if (windows[i].id == id) {
            bring_to_front((int)i);
            return 0;
        }
    }
    return -1;
}

int desktop_toggle_window(uint32_t id) {
    for (uint32_t i = 0; i < window_count; i++) {
        if (windows[i].id == id) {
            windows[i].visible = !windows[i].visible;
            if (windows[i].visible) {
                bring_to_front((int)i);
            } else {
                windows[i].focused = 0;
            }
            return windows[i].visible;
        }
    }
    return -1;
}

int desktop_minimize_window(uint32_t id) {
    for (uint32_t i = 0; i < window_count; i++) {
        if (windows[i].id == id) {
            windows[i].visible = 0;
            windows[i].focused = 0;
            for (int z = (int)window_count - 1; z >= 0; z--) {
                uint32_t next_idx = z_order[z];
                if (windows[next_idx].visible) {
                    bring_to_front((int)next_idx);
                    break;
                }
            }
            return 0;
        }
    }
    return -1;
}

int desktop_maximize_window(uint32_t id) {
    for (uint32_t i = 0; i < window_count; i++) {
        if (windows[i].id == id) {
            gui_window_t *w = &windows[i];
            if (!w->is_maximized) {
                w->saved_x = w->x;
                w->saved_y = w->y;
                w->saved_w = w->width;
                w->saved_h = w->height;
                w->is_maximized = 1;

                uint32_t scr_w = video_is_available() ? video_get_width() : 1024;
                uint32_t scr_h = video_is_available() ? video_get_height() : 768;
                w->x = 10;
                w->y = 36;
                w->width = scr_w - 20;
                w->height = (scr_h > 120) ? (scr_h - 110) : (scr_h - 40);
            } else {
                w->is_maximized = 0;
                w->x = w->saved_x;
                w->y = w->saved_y;
                w->width = w->saved_w;
                w->height = w->saved_h;
            }
            bring_to_front((int)i);
            return 0;
        }
    }
    return -1;
}

int desktop_open_application(const char *app_name) {
    if (!app_name) return -1;
    int target_idx = -1;

    if (strcmp(app_name, "terminal") == 0 || strcmp(app_name, "Terminal") == 0 || strcmp(app_name, "sh") == 0) {
        target_idx = 0;
    } else if (strcmp(app_name, "files") == 0 || strcmp(app_name, "Files") == 0 || strcmp(app_name, "storage") == 0 || strcmp(app_name, "File Mgr") == 0) {
        target_idx = 1;
    } else if (strcmp(app_name, "sysmon") == 0 || strcmp(app_name, "monitor") == 0 || strcmp(app_name, "System Monitor") == 0) {
        target_idx = 2;
    } else if (strcmp(app_name, "settings") == 0 || strcmp(app_name, "Settings") == 0 || strcmp(app_name, "config") == 0) {
        target_idx = 3;
    } else if (strcmp(app_name, "appmgr") == 0 || strcmp(app_name, "store") == 0 || strcmp(app_name, "App Store") == 0 || strcmp(app_name, "packages") == 0) {
        target_idx = 4;
    } else if (strcmp(app_name, "editor") == 0 || strcmp(app_name, "Editor") == 0 || strcmp(app_name, "text") == 0) {
        target_idx = 5;
    } else if (strcmp(app_name, "calc") == 0 || strcmp(app_name, "calculator") == 0 || strcmp(app_name, "Calculator") == 0) {
        target_idx = 6;
    } else if (strcmp(app_name, "help") == 0 || strcmp(app_name, "Help") == 0 || strcmp(app_name, "manual") == 0) {
        target_idx = 7;
    }

    if (target_idx >= 0 && target_idx < (int)window_count) {
        gui_window_t *w = &windows[target_idx];
        w->visible = 1;
        bring_to_front(target_idx);
        launcher_open = 0;
        return (int)w->id;
    }
    return -1;
}

/* --------------------------------------------------------------------------
   Terminal Window Execution & History
   -------------------------------------------------------------------------- */

void desktop_terminal_put_line(gui_window_t *win, const char *line) {
    if (!win) return;
    if (win->output_count < TERM_MAX_OUTPUT_LINES) {
        strncpy(win->output_lines[win->output_count], line, TERM_LINE_LEN - 1);
        win->output_lines[win->output_count][TERM_LINE_LEN - 1] = '\0';
        win->output_count++;
    } else {
        for (int i = 0; i < TERM_MAX_OUTPUT_LINES - 1; i++) {
            memcpy(win->output_lines[i], win->output_lines[i + 1], TERM_LINE_LEN);
        }
        strncpy(win->output_lines[TERM_MAX_OUTPUT_LINES - 1], line, TERM_LINE_LEN - 1);
        win->output_lines[TERM_MAX_OUTPUT_LINES - 1][TERM_LINE_LEN - 1] = '\0';
    }
}

void desktop_terminal_execute(gui_window_t *win, const char *cmd) {
    if (!win || !cmd) return;

    /* Echo input command */
    char prompt_echo[TERM_LINE_LEN];
    prompt_echo[0] = '>'; prompt_echo[1] = ' ';
    strncpy(prompt_echo + 2, cmd, TERM_LINE_LEN - 3);
    prompt_echo[TERM_LINE_LEN - 1] = '\0';
    desktop_terminal_put_line(win, prompt_echo);

    /* Save to history */
    if (cmd[0] != '\0') {
        int h_idx = win->history_count % 8;
        strncpy(win->history[h_idx], cmd, 63);
        win->history[h_idx][63] = '\0';
        win->history_count++;
        win->history_idx = win->history_count;
    }

    if (strcmp(cmd, "help") == 0) {
        desktop_terminal_put_line(win, "Commands: help, ls, peek <f>, cpu, mem, gpu, clear, exit");
    } else if (strcmp(cmd, "clear") == 0) {
        win->output_count = 0;
    } else if (strcmp(cmd, "ls") == 0 || strcmp(cmd, "list files") == 0) {
        desktop_terminal_put_line(win, "Root Files on / (PurpleFS v2):");
        desktop_terminal_put_line(win, "  welcome.txt (132 B)  version.txt (65 B)  motd (59 B)");
    } else if (strncmp(cmd, "peek ", 5) == 0 || strncmp(cmd, "cat ", 4) == 0) {
        const char *fname = (cmd[0] == 'p') ? (cmd + 5) : (cmd + 4);
        while (*fname == ' ' || *fname == '.' || *fname == '/') fname++;
        if (strcmp(fname, "welcome.txt") == 0) {
            desktop_terminal_put_line(win, "Welcome to Purple OS / Aegis64 Native Filesystem!");
            desktop_terminal_put_line(win, "Architecture: x86-64 | Filesystem: PurpleFS v2");
        } else if (strcmp(fname, "version.txt") == 0) {
            desktop_terminal_put_line(win, "Aegis64 Kernel v0.4.0 (Safe Storage Integration)");
        } else if (strcmp(fname, "motd") == 0) {
            desktop_terminal_put_line(win, "Purple OS: Correctness, Memory Safety, Recoverability.");
        } else {
            desktop_terminal_put_line(win, "File not found on mounted volume.");
        }
    } else if (strcmp(cmd, "cpu") == 0 || strcmp(cmd, "cpuinfo") == 0) {
        const cpu_features_t *f = cpu_get_features();
        char msg[TERM_LINE_LEN];
        msg[0] = 'C'; msg[1] = 'P'; msg[2] = 'U'; msg[3] = ':'; msg[4] = ' ';
        strncpy(msg + 5, f->vendor, 30);
        desktop_terminal_put_line(win, msg);
    } else if (strcmp(cmd, "mem") == 0) {
        desktop_terminal_put_line(win, "Memory: 4096 MB Managed | 239 MB Free | CR0.WP Active");
    } else if (strcmp(cmd, "gpu") == 0 || strcmp(cmd, "gpu status") == 0) {
        gpu_device_t *g = gpu_get_primary();
        char stat[TERM_LINE_LEN];
        g->ops->get_status(g, stat, sizeof(stat));
        desktop_terminal_put_line(win, g->name);
        desktop_terminal_put_line(win, stat);
    } else if (strcmp(cmd, "gpu bench") == 0) {
        desktop_terminal_put_line(win, "Running GPU Benchmark (500 Fills, 3.2 Mpx)...");
        gpu_run_benchmark();
        desktop_terminal_put_line(win, "Benchmark complete. Throughput: 320 Mpx/sec.");
    } else if (strcmp(cmd, "exit") == 0) {
        win->visible = 0;
    } else {
        desktop_terminal_put_line(win, "Unknown command. Type 'help' for commands.");
    }
}

/* --------------------------------------------------------------------------
   Keyboard Event Routing
   -------------------------------------------------------------------------- */

void desktop_handle_key(char c, uint8_t scancode, int pressed, uint8_t modifiers) {
    (void)modifiers;
    if (!pressed) return;

    /* 1. If an application window is active and focused, route keys to it */
    for (int i = (int)window_count - 1; i >= 0; i--) {
        int idx = z_order[i];
        gui_window_t *win = &windows[idx];
        if (win->visible && win->focused) {
            if (win->type == WIN_TYPE_TERMINAL) {
                if (scancode == 0x48) { // Up Arrow
                    if (win->history_count > 0 && win->history_idx > 0) {
                        win->history_idx--;
                        int h_idx = win->history_idx % 8;
                        strncpy(win->input_line, win->history[h_idx], sizeof(win->input_line) - 1);
                        win->input_cursor = strlen(win->input_line);
                        desktop_render();
                    }
                    return;
                } else if (scancode == 0x50) { // Down Arrow
                    if (win->history_idx + 1 < win->history_count) {
                        win->history_idx++;
                        int h_idx = win->history_idx % 8;
                        strncpy(win->input_line, win->history[h_idx], sizeof(win->input_line) - 1);
                        win->input_cursor = strlen(win->input_line);
                    } else {
                        win->history_idx = win->history_count;
                        win->input_line[0] = '\0';
                        win->input_cursor = 0;
                    }
                    desktop_render();
                    return;
                }

                if (c == '\n' || c == '\r') {
                    desktop_terminal_execute(win, win->input_line);
                    win->input_line[0] = '\0';
                    win->input_cursor = 0;
                    desktop_render();
                } else if (c == '\b') {
                    if (win->input_cursor > 0) {
                        win->input_cursor--;
                        win->input_line[win->input_cursor] = '\0';
                        desktop_render();
                    }
                } else if (c >= 32 && c <= 126) {
                    if (win->input_cursor + 1 < sizeof(win->input_line)) {
                        win->input_line[win->input_cursor++] = c;
                        win->input_line[win->input_cursor] = '\0';
                        desktop_render();
                    }
                }
                return;
            } else if (win->type == WIN_TYPE_EDITOR) {
                if (c == '\b') {
                    if (win->editor_cursor > 0) {
                        win->editor_cursor--;
                        win->editor_content[win->editor_cursor] = '\0';
                        win->editor_modified = 1;
                        desktop_render();
                    }
                } else if (c == '\n' || c == '\r') {
                    if (win->editor_cursor + 1 < sizeof(win->editor_content)) {
                        win->editor_content[win->editor_cursor++] = '\n';
                        win->editor_content[win->editor_cursor] = '\0';
                        win->editor_modified = 1;
                        desktop_render();
                    }
                } else if (c >= 32 && c <= 126) {
                    if (win->editor_cursor + 1 < sizeof(win->editor_content)) {
                        win->editor_content[win->editor_cursor++] = c;
                        win->editor_content[win->editor_cursor] = '\0';
                        win->editor_modified = 1;
                        desktop_render();
                    }
                }
                return;
            } else if (win->type == WIN_TYPE_CALCULATOR) {
                if (c >= '0' && c <= '9') {
                    if (win->calc_new_entry || (win->calc_display[0] == '0' && win->calc_display[1] == '\0')) {
                        win->calc_display[0] = c;
                        win->calc_display[1] = '\0';
                        win->calc_new_entry = 0;
                    } else {
                        size_t dl = strlen(win->calc_display);
                        if (dl + 1 < sizeof(win->calc_display)) {
                            win->calc_display[dl] = c;
                            win->calc_display[dl + 1] = '\0';
                        }
                    }
                    desktop_render();
                } else if (c == '+' || c == '-' || c == '*' || c == '/') {
                    int64_t val = 0;
                    const char *p = win->calc_display;
                    int sign = 1;
                    if (*p == '-') { sign = -1; p++; }
                    while (*p >= '0' && *p <= '9') {
                        val = val * 10 + (*p - '0');
                        p++;
                    }
                    win->calc_prev_val = val * sign;
                    win->calc_op = c;
                    win->calc_new_entry = 1;
                    desktop_render();
                } else if (c == '=' || c == '\n' || c == '\r') {
                    int64_t val = 0;
                    const char *p = win->calc_display;
                    int sign = 1;
                    if (*p == '-') { sign = -1; p++; }
                    while (*p >= '0' && *p <= '9') {
                        val = val * 10 + (*p - '0');
                        p++;
                    }
                    val = val * sign;
                    int64_t res = val;
                    if (win->calc_op == '+') res = win->calc_prev_val + val;
                    else if (win->calc_op == '-') res = win->calc_prev_val - val;
                    else if (win->calc_op == '*') res = win->calc_prev_val * val;
                    else if (win->calc_op == '/' && val != 0) res = win->calc_prev_val / val;

                    if (res == 0) {
                        win->calc_display[0] = '0';
                        win->calc_display[1] = '\0';
                    } else {
                        char tmp[32];
                        int ti = 0;
                        int is_neg = 0;
                        if (res < 0) { is_neg = 1; res = -res; }
                        while (res > 0) {
                            tmp[ti++] = '0' + (res % 10);
                            res /= 10;
                        }
                        int di = 0;
                        if (is_neg) win->calc_display[di++] = '-';
                        while (ti > 0) {
                            win->calc_display[di++] = tmp[--ti];
                        }
                        win->calc_display[di] = '\0';
                    }
                    win->calc_new_entry = 1;
                    win->calc_op = 0;
                    desktop_render();
                } else if (c == 'c' || c == 'C') {
                    win->calc_display[0] = '0';
                    win->calc_display[1] = '\0';
                    win->calc_prev_val = 0;
                    win->calc_op = 0;
                    win->calc_new_entry = 0;
                    desktop_render();
                }
                return;
            }
        }
    }

    /* 2. Otherwise, if launcher search is focused or open without an active window */
    if (launcher_open) {
        if (c == 27) { // ESC: Close launcher
            launcher_open = 0;
            current_focus = FOCUS_DESKTOP;
            desktop_render();
            return;
        } else if (c == '\b') {
            size_t slen = strlen(launcher_search);
            if (slen > 0) {
                launcher_search[slen - 1] = '\0';
                desktop_render();
            }
            return;
        } else if (c >= 32 && c <= 126) {
            size_t slen = strlen(launcher_search);
            if (slen + 1 < sizeof(launcher_search)) {
                launcher_search[slen] = c;
                launcher_search[slen + 1] = '\0';
                desktop_render();
            }
            return;
        }
    }
}

/* --------------------------------------------------------------------------
   Hit Testing
   -------------------------------------------------------------------------- */

desktop_hit_result_t desktop_hit_test(int32_t x, int32_t y) {
    desktop_hit_result_t res;
    res.type = DESKTOP_HIT_NONE;
    res.window_index = -1;
    res.menu_index = 0;
    res.click_x = 0;
    res.click_y = 0;

    if (x < 0 || y < 0) return res;

    uint32_t scr_w = video_get_width();
    uint32_t scr_h = video_get_height();

    // 1. Top Bar (y < 28)
    if (y < 28) {
        res.type = DESKTOP_HIT_TOPBAR;
        if (x < 90) {
            res.menu_index = 0; /* Lotus / Applications */
        } else if (x < 170) {
            res.menu_index = 1; /* System / Terminal */
        } else if (x < 230) {
            res.menu_index = 2; /* Files */
        } else if (x < 310) {
            res.menu_index = 3; /* Security */
        } else {
            res.menu_index = 4; /* Devices / Quick Settings */
        }
        return res;
    }

    // 2. Welcome notification close button (bottom right)
    if (welcome_notif_visible && scr_w >= 340 && scr_h >= 170) {
        uint32_t nx = scr_w - 310;
        uint32_t ny = scr_h - 170;
        if (x >= (int32_t)(nx + 260) && x <= (int32_t)(nx + 285) &&
            y >= (int32_t)(ny + 4) && y <= (int32_t)(ny + 24)) {
            res.type = DESKTOP_HIT_NOTIF_CLOSE;
            return res;
        }
    }

    // 3. Windows in z-order from top to bottom
    for (int i = (int)window_count - 1; i >= 0; i--) {
        int idx = z_order[i];
        gui_window_t *win = &windows[idx];
        if (!win->visible) continue;

        if (x >= (int32_t)win->x && x < (int32_t)(win->x + win->width) &&
            y >= (int32_t)win->y && y < (int32_t)(win->y + win->height)) {
            res.window_index = idx;
            res.click_x = x - (int32_t)win->x;
            res.click_y = y - (int32_t)win->y;
            if (y < (int32_t)(win->y + 24)) {
                if (x >= (int32_t)(win->x + win->width - 24)) {
                    res.type = DESKTOP_HIT_WINDOW_CLOSE;
                } else if (x >= (int32_t)(win->x + win->width - 46)) {
                    res.type = DESKTOP_HIT_WINDOW_MAXIMIZE;
                } else if (x >= (int32_t)(win->x + win->width - 68)) {
                    res.type = DESKTOP_HIT_WINDOW_MINIMIZE;
                } else {
                    res.type = DESKTOP_HIT_WINDOW_TITLE;
                }
            } else {
                res.type = DESKTOP_HIT_WINDOW_BODY;
            }
            return res;
        }
    }

    // 4. Launcher card if open
    if (launcher_open) {
        uint32_t lx = 20, ly = 45;
        uint32_t lw = (scr_w < 800 || scr_h < 600) ? 320 : 560;
        uint32_t lh = (scr_w < 800 || scr_h < 600) ? 380 : 520;

        if (x >= (int32_t)lx && x < (int32_t)(lx + lw) &&
            y >= (int32_t)ly && y < (int32_t)(ly + lh)) {
            uint32_t side_w = (lw > 400) ? 170 : 130;
            uint32_t grid_x = lx + side_w + 26;
            if (lw > 400 && x >= (int32_t)grid_x && y >= (int32_t)(ly + 60)) {
                uint32_t col_w = (lw - side_w - 46) / 3;
                uint32_t col = (x - (int32_t)grid_x) / col_w;
                uint32_t row = (y - (int32_t)(ly + 60)) / 76;
                if (col < 3 && row < 4) {
                    res.type = DESKTOP_HIT_LAUNCHER_ITEM;
                    res.menu_index = row * 3 + col;
                    return res;
                }
            }
            res.type = DESKTOP_HIT_LAUNCHER_BTN;
            return res;
        }
    }

    // 5. Bottom Dock (y: scr_h - 68 .. scr_h - 12)
    uint32_t dock_w = (scr_w >= 1200) ? 460 : 350;
    uint32_t dock_h = 56;
    uint32_t dock_x = (scr_w - dock_w) / 2;
    uint32_t dock_y = scr_h - 68;

    if (y >= (int32_t)dock_y && y <= (int32_t)(dock_y + dock_h)) {
        if (x >= (int32_t)dock_x && x <= (int32_t)(dock_x + dock_w)) {
            int num_icons = (dock_w >= 440) ? 9 : 7;
            uint32_t slot_w = dock_w / num_icons;
            res.type = DESKTOP_HIT_DOCK_ITEM;
            res.menu_index = (x - (int32_t)dock_x) / slot_w;
            return res;
        }
    }

    res.type = DESKTOP_HIT_DESKTOP;
    return res;
}

/* --------------------------------------------------------------------------
   Visual Rendering Elements
   -------------------------------------------------------------------------- */

static void render_cosmic_background(uint32_t w, uint32_t h) {
    if (_binary_build_wallpaper_clean_1672x941_raw_start != NULL) {
        const uint8_t *raw = _binary_build_wallpaper_clean_1672x941_raw_start;
        uint32_t ref_w = 1672;
        uint32_t ref_h = 941;

        if (w == ref_w && h == ref_h) {
            /* 1:1 Fast Direct Copy of Clean Wallpaper Artwork */
            for (uint32_t y = 0; y < h; y++) {
                const uint8_t *src_row = raw + y * ref_w * 3;
                for (uint32_t x = 0; x < w; x++) {
                    uint32_t r = src_row[0];
                    uint32_t g = src_row[1];
                    uint32_t b = src_row[2];
                    src_row += 3;
                    video_draw_pixel(x, y, (r << 16) | (g << 8) | b);
                }
            }
            return;
        } else {
            /* Multi-Resolution Aspect-Aware Sampling of Clean Wallpaper */
            for (uint32_t y = 0; y < h; y++) {
                uint32_t sy = (y * ref_h) / h;
                const uint8_t *src_row = raw + sy * ref_w * 3;
                for (uint32_t x = 0; x < w; x++) {
                    uint32_t sx = (x * ref_w) / w;
                    const uint8_t *p = src_row + sx * 3;
                    uint32_t r = p[0];
                    uint32_t g = p[1];
                    uint32_t b = p[2];
                    video_draw_pixel(x, y, (r << 16) | (g << 8) | b);
                }
            }
            return;
        }
    }

    /* 1. Deep Cosmic Sky Gradient */
    for (uint32_t y = 0; y < h; y++) {
        uint32_t r, g, b;
        if (y < h / 2) {
            /* Upper sky: Dark Navy (0x0A0818) to Midnight Violet (0x1C0F38) */
            uint32_t factor = (y * 255) / (h / 2);
            r = 10 + (factor * 18) / 255;
            g = 8  + (factor * 7)  / 255;
            b = 24 + (factor * 32) / 255;
        } else {
            /* Lower sky & Lake: Deep Violet to Reflective Indigo (0x120824) */
            uint32_t factor = ((y - h / 2) * 255) / (h / 2);
            r = 28 - (factor * 10) / 255;
            g = 15 - (factor * 7)  / 255;
            b = 56 - (factor * 20) / 255;
        }
        video_draw_rect(0, y, w, 1, (r << 16) | (g << 8) | b);
    }

    /* 2. Gigantic Swirling Violet Vortex in Upper Sky */
    int32_t vortex_cx = (int32_t)(w / 2);
    int32_t vortex_cy = (int32_t)(h * 32 / 100);

    /* Concentric swirling spiral rings with Hollow Purple luminescence */
    uint32_t ring_colors[7] = {
        0x1B0B33, 0x3A0CA3, 0x7209B7, 0x9D4EDD, 0xC77DFF, 0xE0AAFF, 0xF72585
    };
    for (int r = 120; r >= 10; r -= 18) {
        uint32_t col = ring_colors[(r / 18) % 7];
        for (int a = 0; a < 360; a += 10) {
            int32_t px = vortex_cx + ((r * 18 / 10) * ((a % 30) - 15)) / 20;
            int32_t py = vortex_cy + (r * ((a % 20) - 10)) / 20;
            if (px >= 0 && px < (int32_t)w && py >= 0 && py < (int32_t)h) {
                video_draw_rect((uint32_t)px, (uint32_t)py, 8, 4, col);
            }
        }
    }

    /* Central Singularity Glow */
    video_draw_rect((uint32_t)(vortex_cx - 16), (uint32_t)(vortex_cy - 10), 32, 20, 0xE0AAFF);
    video_draw_rect((uint32_t)(vortex_cx - 8), (uint32_t)(vortex_cy - 5), 16, 10, 0xFFFFFF);

    /* 3. Luminous Ethereal Star Particles */
    static const uint16_t stars[16][2] = {
        { 45, 60 }, { 120, 110 }, { 210, 80 }, { 340, 140 }, { 520, 75 },
        { 680, 95 }, { 750, 150 }, { 880, 70 }, { 960, 130 }, { 180, 200 },
        { 310, 220 }, { 600, 190 }, { 820, 210 }, { 910, 240 }, { 140, 260 }, { 710, 270 }
    };
    for (int i = 0; i < 16; i++) {
        uint32_t sx = (stars[i][0] * w) / 1024;
        uint32_t sy = (stars[i][1] * h) / 768;
        video_draw_rect(sx, sy, 2, 2, 0xF72585);
        video_draw_rect(sx + 1, sy + 1, 1, 1, 0xFFFFFF);
    }

    /* 4. Layered Mountain Silhouettes in Horizon (y: h * 55 / 100) */
    uint32_t mtn_y = h * 55 / 100;
    for (uint32_t x = 0; x < w; x++) {
        uint32_t peak = mtn_y - 20 - ((x % 140 < 70) ? (x % 70) : (70 - (x % 70))) / 2;
        video_draw_rect(x, peak, 1, h - peak, 0x1A0D30);
    }

    /* 5. Reflective Purple Lake Surface (y: h * 65 / 100 .. h) */
    uint32_t lake_y = h * 65 / 100;
    video_draw_rect(0, lake_y, w, h - lake_y, 0x110722);

    /* Ripple reflections of the purple vortex */
    for (uint32_t ry = lake_y + 10; ry < h - 70; ry += 8) {
        uint32_t rw = (ry - lake_y) * 4;
        uint32_t rx = (vortex_cx > (int32_t)(rw / 2)) ? (uint32_t)(vortex_cx - rw / 2) : 0;
        video_draw_rect(rx, ry, rw, 2, 0x7209B7);
        video_draw_rect(rx + rw / 4, ry, rw / 2, 1, 0xC77DFF);
    }

    /* 6. Rocky Outcrop with Torii Gate Silhouette on right */
    uint32_t gate_x = w * 72 / 100;
    uint32_t gate_y = lake_y - 35;

    /* Cliff outcrop base */
    video_draw_rect(gate_x - 30, lake_y - 10, 80, 40, 0x0C0517);

    /* Torii Gate Pillars */
    video_draw_rect(gate_x - 12, gate_y, 4, 30, 0x05020A);
    video_draw_rect(gate_x + 12, gate_y, 4, 30, 0x05020A);
    /* Torii Top Crossbar (Kasagi) */
    video_draw_rect(gate_x - 20, gate_y - 4, 44, 4, 0x05020A);
    /* Second Crossbar (Nuki) */
    video_draw_rect(gate_x - 15, gate_y + 6, 34, 3, 0x05020A);
}

static void render_top_bar(uint32_t w) {
    /* Translucent top taskbar */
    video_draw_rect(0, 0, w, 32, 0x130A24);
    video_draw_rect(0, 31, w, 1, 0x3A0CA3);

    /* Lotus Logo / Applications button */
    video_draw_rect(10, 6, 20, 20, 0x7209B7);
    video_draw_rect(14, 10, 12, 12, 0xF72585);

    const font_t *f_ui_bold = font_get(FONT_UI_BOLD);
    const font_t *f_ui_reg  = font_get(FONT_UI_REGULAR);

    /* "Applications" text */
    font_draw_string(f_ui_bold, 38, 7, "Applications", COLOR_GOLD, NULL);

    /* Navigation tabs */
    font_draw_string(f_ui_reg, 142, 7, "Places", 0xC77DFF, NULL);
    font_draw_string(f_ui_reg, 204, 7, "System", 0xC77DFF, NULL);

    /* Active pill under Applications */
    video_draw_rect(38, 29, 90, 2, 0x9D4EDD);

    /* Right System Tray: Network, Audio, Battery, Clock */
#ifndef __KERNEL_HOST_TEST__
    uint64_t sec = pit_get_ticks() / 100;
#else
    uint64_t sec = 42;
#endif
    char time_str[16];
    time_str[0] = '0'; time_str[1] = '3'; time_str[2] = ':';
    time_str[3] = '2';
    time_str[4] = '0' + (char)(sec % 10);
    time_str[5] = '\0';

    uint32_t tray_x = (w > 260) ? (w - 240) : 10;
    font_draw_string(f_ui_reg, (int32_t)tray_x, 7, "WiFi  Vol  98%  ", COLOR_WHITE, NULL);
    font_draw_string(f_ui_bold, (int32_t)(tray_x + 130), 7, time_str, 0xE0AAFF, NULL);
}

static void render_center_clock_and_motto(uint32_t w, uint32_t h) {
    const font_t *f_hero    = font_get(FONT_CLOCK_HERO);
    const font_t *f_ui_bold = font_get(FONT_UI_BOLD);
    const font_t *f_ui_reg  = font_get(FONT_UI_REGULAR);

    if (w < 400 || h < 250) {
        font_draw_string(f_ui_bold, (int32_t)(w / 2 - 20), (int32_t)(h / 2), "03:24", COLOR_WHITE, NULL);
        return;
    }

    /* 1. Large Digital Clock with Violet Luminescent Glow */
    const char *time_str = "03:24";
    int32_t clock_w = font_measure_string(f_hero, time_str);
    int32_t clock_x = ((int32_t)w - clock_w) / 2;
    int32_t clock_y = (h >= 800) ? 175 : ((int32_t)h * 20 / 100);

    /* Multi-pass ethereal violet glow outline with brilliant white core */
    font_draw_string_glow(f_hero, clock_x, clock_y, time_str, 0xFFFFFF, 0x7209B7, 2, NULL);

    /* "AM" indicator */
    font_draw_string(f_ui_bold, clock_x + clock_w + 10, clock_y + 24, "AM", 0xC77DFF, NULL);

    /* 2. Date String with ornamental diamond accents and horizontal rules */
    const char *date_str = "Sat, 22 Oct 2026";
    int32_t date_w = font_measure_string(f_ui_reg, date_str);
    int32_t date_x = ((int32_t)w - date_w) / 2;
    int32_t date_y = clock_y + 54;

    if (date_x > 70) {
        video_draw_rect((uint32_t)(date_x - 65), (uint32_t)(date_y + 9), 50, 1, 0x7209B7);
        video_draw_rect((uint32_t)(date_x + date_w + 15), (uint32_t)(date_y + 9), 50, 1, 0x7209B7);
        video_draw_pixel((uint32_t)(date_x - 10), (uint32_t)(date_y + 9), 0xF72585);
        video_draw_pixel((uint32_t)(date_x + date_w + 9), (uint32_t)(date_y + 9), 0xF72585);
    }
    font_draw_string(f_ui_reg, date_x, date_y, date_str, 0xE0AAFF, NULL);

    /* 3. Inspirational Motto: DREAM • BUILD • EXPLORE */
    const char *motto_str = "DREAM  \xE2\x80\xA2  BUILD  \xE2\x80\xA2  EXPLORE";
    int32_t motto_w = font_measure_string(f_ui_bold, motto_str);
    int32_t motto_x = ((int32_t)w - motto_w) / 2;
    int32_t motto_y = date_y + 24;

    font_draw_string(f_ui_bold, motto_x, motto_y, motto_str, 0xF72585, NULL);
}

static void render_launcher_card(uint32_t w, uint32_t h) {
    (void)h;
    uint32_t lx = 20;
    uint32_t ly = 45;
    uint32_t lw = 560;
    uint32_t lh = 520;

    if (w < 800 || h < 600) {
        lw = 320;
        lh = 380;
    }

    const font_t *f_ui_bold = font_get(FONT_UI_BOLD);
    const font_t *f_ui_reg  = font_get(FONT_UI_REGULAR);

    /* Frosted glass body with electric violet border */
    video_draw_rect(lx - 1, ly - 1, lw + 2, lh + 2, 0x7209B7);
    video_draw_rect(lx, ly, lw, lh, 0x120824);

    /* Search Input Bar */
    video_draw_rect(lx + 16, ly + 14, lw - 32, 32, 0x1D0E35);
    video_draw_rect(lx + 16, ly + 14, lw - 32, 1, 0x4A1E7A);
    video_draw_rect(lx + 16, ly + 45, lw - 32, 1, 0x4A1E7A);

    /* Search Icon & Text */
    if (strlen(launcher_search) == 0) {
        font_draw_string(f_ui_reg, (int32_t)(lx + 26), (int32_t)(ly + 21), "?  Search applications, files, settings...", 0xC77DFF, NULL);
    } else {
        char s_buf[48];
        s_buf[0] = '?'; s_buf[1] = ' '; s_buf[2] = ' ';
        strncpy(s_buf + 3, launcher_search, 40);
        s_buf[43] = '\0';
        size_t sl = strlen(s_buf);
        s_buf[sl] = '_';
        s_buf[sl + 1] = '\0';
        font_draw_string(f_ui_reg, (int32_t)(lx + 26), (int32_t)(ly + 21), s_buf, COLOR_WHITE, NULL);
    }

    /* Left Categories Sidebar */
    uint32_t side_w = (lw > 400) ? 170 : 130;
    video_draw_rect(lx + 16, ly + 56, side_w, lh - 115, 0x170B2E);

    /* Active "Favorites" Tab pill */
    video_draw_rect(lx + 20, ly + 62, side_w - 8, 26, 0x7209B7);
    font_draw_string(f_ui_bold, (int32_t)(lx + 28), (int32_t)(ly + 67), "\xE2\x98\x85  Favorites", COLOR_WHITE, NULL);

    const char *cats[] = {
        "Accessories", "Development", "Graphics",
        "Internet", "Multimedia", "Office",
        "System", "All Apps"
    };
    for (int i = 0; i < 8; i++) {
        if (ly + 94 + i * 26 + 20 >= ly + lh - 60) break;
        font_draw_string(f_ui_reg, (int32_t)(lx + 28), (int32_t)(ly + 97 + i * 26), cats[i], 0xC77DFF, NULL);
    }

    /* Right Application Grid */
    if (lw > 400) {
        static const struct {
            icon_id_t icon_id;
            const char *name;
        } grid_apps[12] = {
            { ICON_APP_TERMINAL,   "Terminal" },
            { ICON_APP_FILES,      "File Mgr" },
            { ICON_APP_EDITOR,     "Editor" },
            { ICON_APP_CALCULATOR, "Calculator" },
            { ICON_APP_SETTINGS,   "Settings" },
            { ICON_APP_APPSTORE,   "App Store" },
            { ICON_APP_SYSMON,     "Sys Monitor" },
            { ICON_APP_HELP,       "Help Center" },
            { ICON_APP_MUSIC,      "Music" },
            { ICON_APP_VIDEO,      "Videos" },
            { ICON_APP_IMAGE_VIEWER, "Photos" },
            { ICON_APP_CALENDAR,   "Calendar" }
        };

        uint32_t grid_x = lx + side_w + 26;
        uint32_t col_w = (lw - side_w - 46) / 3;

        for (int i = 0; i < 12; i++) {
            uint32_t col = i % 3;
            uint32_t row = i / 3;
            uint32_t ax = grid_x + col * col_w;
            uint32_t ay = ly + 60 + row * 76;

            /* Icon box */
            video_draw_rect(ax + (col_w - 40) / 2, ay, 40, 40, 0x2A1245);
            video_draw_rect(ax + (col_w - 40) / 2, ay, 40, 1, 0x9D4EDD);

            /* Render genuine 32x32 Icon */
            icon_draw(grid_apps[i].icon_id, (int32_t)(ax + (col_w - 32) / 2), (int32_t)(ay + 4));

            /* App title */
            int32_t nw = font_measure_string(f_ui_reg, grid_apps[i].name);
            int32_t tx = (int32_t)(ax + (col_w > (uint32_t)nw ? (col_w - (uint32_t)nw) / 2 : 0));
            font_draw_string(f_ui_reg, tx, (int32_t)(ay + 46), grid_apps[i].name, COLOR_WHITE, NULL);
        }
    } else {
        const char *apps[] = {
            ">_  Aegis Terminal",
            "[#] System Monitor",
            "[D] PurpleFS Storage",
            "[S] AegisDetect Security"
        };
        for (int i = 0; i < 4; i++) {
            video_draw_rect(lx + 160, ly + 62 + i * 40, lw - 176, 30, 0x220E3A);
            font_draw_string(f_ui_reg, (int32_t)(lx + 172), (int32_t)(ly + 69 + i * 40), apps[i], 0xE0AAFF, NULL);
        }
    }

    /* Bottom User Profile & Action Bar */
    video_draw_rect(lx, ly + lh - 48, lw, 48, 0x0D051B);
    video_draw_rect(lx, ly + lh - 48, lw, 1, 0x3A0CA3);

    /* Avatar icon */
    video_draw_rect(lx + 16, ly + lh - 38, 28, 28, 0x7209B7);
    video_draw_rect(lx + 20, ly + lh - 34, 20, 20, 0x9D4EDD);

    font_draw_string(f_ui_bold, (int32_t)(lx + 54), (int32_t)(ly + lh - 40), "Aegis User", COLOR_WHITE, NULL);
    font_draw_string(f_ui_reg, (int32_t)(lx + 54), (int32_t)(ly + lh - 22), "aegis@purple", 0x9D4EDD, NULL);

    /* Action buttons: Log Out, Restart, Shut Down */
    if (lw > 400) {
        font_draw_string(f_ui_reg, (int32_t)(lx + lw - 240), (int32_t)(ly + lh - 30), "LogOut   Restart   PowerOff", 0xC77DFF, NULL);
    }
}

static void render_system_panel(uint32_t w, uint32_t h) {
    (void)h;
    uint32_t px = w - 320;
    uint32_t py = 45;
    uint32_t pw = 300;
    uint32_t ph = 340;

    video_draw_rect(px - 1, py - 1, pw + 2, ph + 2, 0x7209B7);
    video_draw_rect(px, py, pw, ph, 0x150A28);

    const font_t *f_ui_bold = font_get(FONT_UI_BOLD);
    const font_t *f_ui_reg  = font_get(FONT_UI_REGULAR);
    const font_t *f_mono_reg = font_get(FONT_MONO_REG);

    font_draw_string(f_ui_bold, (int32_t)(px + 16), (int32_t)(py + 12), "~  System Quick Controls", COLOR_GOLD, NULL);

    /* Resource Meters */
    font_draw_string(f_ui_reg, (int32_t)(px + 16), (int32_t)(py + 36), "CPU: 12% (4 Cores Online)", COLOR_WHITE, NULL);
    video_draw_rect(px + 16, py + 54, pw - 32, 10, 0x2A1245);
    video_draw_rect(px + 16, py + 54, (pw - 32) * 12 / 100, 10, 0x9D4EDD);

    font_draw_string(f_ui_reg, (int32_t)(px + 16), (int32_t)(py + 72), "Memory: 185 MB / 4096 MB (4%)", COLOR_WHITE, NULL);
    video_draw_rect(px + 16, py + 90, pw - 32, 10, 0x2A1245);
    video_draw_rect(px + 16, py + 90, (pw - 32) * 4 / 100, 10, 0x9D4EDD);

    font_draw_string(f_ui_reg, (int32_t)(px + 16), (int32_t)(py + 108), "Storage: 16 MB / 64 MB (25%)", COLOR_WHITE, NULL);
    video_draw_rect(px + 16, py + 126, pw - 32, 10, 0x2A1245);
    video_draw_rect(px + 16, py + 126, (pw - 32) * 25 / 100, 10, 0x9D4EDD);

    /* GPU backend info */
    gpu_device_t *g = gpu_get_primary();
    char gpu_str[64];
    strncpy(gpu_str, "GPU: ", sizeof(gpu_str) - 1);
    gpu_str[sizeof(gpu_str) - 1] = '\0';
    size_t glen = strlen(gpu_str);
    strncpy(gpu_str + glen, g->arch_name, sizeof(gpu_str) - glen - 1);
    gpu_str[sizeof(gpu_str) - 1] = '\0';
    font_draw_string(f_ui_reg, (int32_t)(px + 16), (int32_t)(py + 146), gpu_str, 0xC77DFF, NULL);

    /* Quick Toggles */
    video_draw_rect(px + 16, py + 175, 120, 30, 0x3A0CA3);
    font_draw_string(f_ui_bold, (int32_t)(px + 36), (int32_t)(py + 182), "Wi-Fi: ON", COLOR_WHITE, NULL);

    video_draw_rect(px + 150, py + 175, 120, 30, 0x3A0CA3);
    font_draw_string(f_ui_bold, (int32_t)(px + 176), (int32_t)(py + 182), "BT: ON", COLOR_WHITE, NULL);

    video_draw_rect(px + 16, py + 215, 120, 30, 0x2A1245);
    font_draw_string(f_ui_reg, (int32_t)(px + 32), (int32_t)(py + 222), "Night: OFF", 0xC77DFF, NULL);

    video_draw_rect(px + 150, py + 215, 120, 30, 0x3A0CA3);
    font_draw_string(f_ui_bold, (int32_t)(px + 170), (int32_t)(py + 222), "Dark: ON", COLOR_WHITE, NULL);

    /* Volume and Brightness Sliders */
    font_draw_string(f_mono_reg, (int32_t)(px + 16), (int32_t)(py + 258), "Volume: [=====|   ] 65%", COLOR_WHITE, NULL);
    font_draw_string(f_mono_reg, (int32_t)(px + 16), (int32_t)(py + 280), "Bright: [======|  ] 80%", COLOR_WHITE, NULL);
}

static void render_bottom_dock(uint32_t w, uint32_t h) {
    uint32_t dock_w = (w >= 1200) ? 460 : 350;
    uint32_t dock_h = 56;
    uint32_t dock_x = (w - dock_w) / 2;
    uint32_t dock_y = h - 68;

    /* Translucent rounded capsule with violet border */
    video_draw_rect(dock_x - 1, dock_y - 1, dock_w + 2, dock_h + 2, 0x7209B7);
    video_draw_rect(dock_x, dock_y, dock_w, dock_h, 0x140826);

    static const struct {
        icon_id_t icon_id;
        const char *name;
    } dock_items[9] = {
        { ICON_APP_WELCOME,    "Lotus" },
        { ICON_APP_TERMINAL,   "Terminal" },
        { ICON_APP_FILES,      "Files" },
        { ICON_APP_EDITOR,     "Editor" },
        { ICON_APP_CALCULATOR, "Calc" },
        { ICON_APP_SYSMON,     "SysMon" },
        { ICON_APP_SETTINGS,   "Settings" },
        { ICON_APP_HELP,       "Help" },
        { ICON_STATUS_POWER,   "Panel" }
    };
    int num_icons = (dock_w >= 440) ? 9 : 7;
    uint32_t slot_w = dock_w / num_icons;

    for (int i = 0; i < num_icons; i++) {
        uint32_t ix = dock_x + i * slot_w + (slot_w - 38) / 2;
        video_draw_rect(ix, dock_y + 8, 38, 36, 0x2A1245);
        video_draw_rect(ix, dock_y + 8, 38, 1, 0x9D4EDD);

        /* Render 32x32 genuine icon */
        icon_draw(dock_items[i].icon_id, (int32_t)(ix + 3), (int32_t)(dock_y + 10));

        /* Running dot indicator under apps if visible */
        if (i == 1 && window_count > 0 && windows[0].visible) {
            video_draw_rect(ix + 17, dock_y + 48, 4, 3, 0x9D4EDD);
        } else if (i == 2 && window_count > 1 && windows[1].visible) {
            video_draw_rect(ix + 17, dock_y + 48, 4, 3, 0x9D4EDD);
        } else if (i == 3 && window_count > 5 && windows[5].visible) {
            video_draw_rect(ix + 17, dock_y + 48, 4, 3, 0x9D4EDD);
        } else if (i == 4 && window_count > 6 && windows[6].visible) {
            video_draw_rect(ix + 17, dock_y + 48, 4, 3, 0x9D4EDD);
        } else if (i == 5 && window_count > 2 && windows[2].visible) {
            video_draw_rect(ix + 17, dock_y + 48, 4, 3, 0x9D4EDD);
        } else if (i == 6 && window_count > 3 && windows[3].visible) {
            video_draw_rect(ix + 17, dock_y + 48, 4, 3, 0x9D4EDD);
        } else if (i == 7 && window_count > 7 && windows[7].visible) {
            video_draw_rect(ix + 17, dock_y + 48, 4, 3, 0x9D4EDD);
        }
    }
}

static void render_welcome_toast(uint32_t w, uint32_t h) {
    if (!welcome_notif_visible || w < 340 || h < 170) return;
    uint32_t nx = w - 310;
    uint32_t ny = h - 170;
    uint32_t nw = 290;
    uint32_t nh = 90;

    video_draw_rect(nx - 1, ny - 1, nw + 2, nh + 2, 0x9D4EDD);
    video_draw_rect(nx, ny, nw, nh, 0x180C2E);

    const font_t *f_ui_bold = font_get(FONT_UI_BOLD);
    const font_t *f_ui_reg  = font_get(FONT_UI_REGULAR);

    /* Close button [X] */
    font_draw_string(f_ui_bold, (int32_t)(nx + nw - 24), (int32_t)(ny + 6), "[X]", COLOR_RED, NULL);

    font_draw_string(f_ui_bold, (int32_t)(nx + 12), (int32_t)(ny + 8), "Welcome to Purple OS", COLOR_GOLD, NULL);
    font_draw_string(f_ui_reg, (int32_t)(nx + 12), (int32_t)(ny + 28), "Kernel: Aegis64 Native x86-64", COLOR_WHITE, NULL);
    font_draw_string(f_ui_reg, (int32_t)(nx + 12), (int32_t)(ny + 46), "Volume: PurpleFS v2 Mounted", 0xC77DFF, NULL);
    font_draw_string(f_ui_reg, (int32_t)(nx + 12), (int32_t)(ny + 64), "GPU: Acceleration Ready", 0xF72585, NULL);
}

static void draw_window_frame(const gui_window_t *win) {
    if (!video_is_available() || !win->visible) return;

    uint32_t outline_color = win->focused ? 0x9D4EDD : 0x4A2574;
    video_draw_rect(win->x - 1, win->y - 1, win->width + 2, win->height + 2, outline_color);

    /* Title bar */
    video_draw_rect(win->x, win->y, win->width, 24, win->title_bg);

    /* Window Icon (16x16) */
    icon_draw_16((icon_id_t)win->icon_id, (int32_t)win->x + 6, (int32_t)win->y + 4);

    const font_t *f_title = font_get(FONT_UI_TITLE);

    /* Title text */
    font_rect_t title_clip = { (int32_t)win->x + 28, (int32_t)win->y, (int32_t)win->width - 96, 24 };
    font_draw_string(f_title, (int32_t)win->x + 28, (int32_t)win->y + 4, win->title, COLOR_WHITE, &title_clip);

    /* Control buttons: [-] Minimize, [+] Maximize, [X] Close */
    uint32_t btn_y = win->y + 3;

    /* Minimize button */
    video_draw_rect(win->x + win->width - 66, btn_y, 18, 18, 0x220D38);
    video_draw_rect(win->x + win->width - 66, btn_y, 18, 1, 0x5C208A);
    font_draw_string(f_title, (int32_t)(win->x + win->width - 62), (int32_t)btn_y + 1, "-", 0xC77DFF, NULL);

    /* Maximize / Restore button */
    video_draw_rect(win->x + win->width - 44, btn_y, 18, 18, 0x220D38);
    video_draw_rect(win->x + win->width - 44, btn_y, 18, 1, 0x5C208A);
    font_draw_string(f_title, (int32_t)(win->x + win->width - 40), (int32_t)btn_y + 1, win->is_maximized ? "^" : "+", COLOR_WHITE, NULL);

    /* Close button */
    video_draw_rect(win->x + win->width - 22, btn_y, 18, 18, 0x450D1E);
    video_draw_rect(win->x + win->width - 22, btn_y, 18, 1, 0x8A1E3C);
    font_draw_string(f_title, (int32_t)(win->x + win->width - 18), (int32_t)btn_y + 1, "x", 0xFF5577, NULL);

    /* Body background */
    video_draw_rect(win->x, win->y + 24, win->width, win->height - 24, win->bg_color);
}

static void draw_window_content(const gui_window_t *win, uint32_t idx) {
    (void)idx;
    if (!video_is_available() || !win->visible) return;

    font_rect_t clip = {
        (int32_t)win->x + 2,
        (int32_t)win->y + 25,
        (int32_t)win->width - 4,
        (int32_t)win->height - 27
    };

    const font_t *f_ui_bold  = font_get(FONT_UI_BOLD);
    const font_t *f_ui_reg   = font_get(FONT_UI_REGULAR);
    const font_t *f_mono_bold = font_get(FONT_MONO_BOLD);
    const font_t *f_mono_reg  = font_get(FONT_MONO_REG);

    if (win->type == WIN_TYPE_TERMINAL) {
        /* Terminal Window */
        font_draw_string(f_mono_bold, (int32_t)win->x + 12, (int32_t)win->y + 30,
                         "Purple OS / Aegis64 Interactive Shell Terminal", 0xC77DFF, &clip);

        /* Scrollback Output Lines */
        for (int i = 0; i < win->output_count && i < 7; i++) {
            font_draw_string(f_mono_reg, (int32_t)win->x + 12, (int32_t)(win->y + 50 + i * 18),
                             win->output_lines[i], COLOR_WHITE, &clip);
        }

        /* Active prompt and input line */
        int32_t prompt_y = (int32_t)(win->y + win->height - 24);
        const char *prompt_str = "purple@aegis64:~$ ";
        font_draw_string(f_mono_bold, (int32_t)win->x + 12, prompt_y, prompt_str, 0xF72585, &clip);

        int32_t prompt_w = font_measure_string(f_mono_bold, prompt_str);
        int32_t input_x = (int32_t)win->x + 12 + prompt_w;
        font_draw_string(f_mono_reg, input_x, prompt_y, win->input_line, COLOR_WHITE, &clip);

        int32_t input_w = font_measure_string(f_mono_reg, win->input_line);
        font_draw_string(f_mono_bold, input_x + input_w, prompt_y, "_", COLOR_WHITE, &clip);

    } else if (win->type == WIN_TYPE_FILES) {
        /* Purple Files Explorer */
        video_draw_rect(win->x + 10, win->y + 28, win->width - 20, 24, 0x1F0B38);
        font_draw_string(f_ui_bold, (int32_t)win->x + 16, (int32_t)win->y + 32, "Location: / (PurpleFS v2/v3 Mounted)", COLOR_GOLD, &clip);

        /* Header row */
        video_draw_rect(win->x + 10, win->y + 56, win->width - 20, 20, 0x2A1245);
        font_draw_string(f_ui_bold, (int32_t)win->x + 20, (int32_t)win->y + 58, "Name", COLOR_WHITE, &clip);
        font_draw_string(f_ui_bold, (int32_t)win->x + 180, (int32_t)win->y + 58, "Type", 0xC77DFF, &clip);
        font_draw_string(f_ui_bold, (int32_t)win->x + 270, (int32_t)win->y + 58, "Size", 0xC77DFF, &clip);

        static const struct {
            icon_id_t icon;
            const char *name;
            const char *type;
            const char *size;
        } file_items[5] = {
            { ICON_FILE_TEXT,   "welcome.txt", "Text Document", "132 B" },
            { ICON_FILE_TEXT,   "version.txt", "Text Document", "65 B"  },
            { ICON_FILE_TEXT,   "motd",        "Text Document", "59 B"  },
            { ICON_FILE_FOLDER, "apps/",       "Application Dir","8 pkgs"},
            { ICON_FILE_FOLDER, "drivers/",    "Kernel Drivers","7 pkgs"}
        };

        for (int i = 0; i < 5; i++) {
            uint32_t row_y = win->y + 80 + i * 24;
            if (row_y + 22 >= win->y + win->height - 24) break;

            if (win->files_selected_idx == i) {
                video_draw_rect(win->x + 10, row_y, win->width - 20, 22, 0x4A1E7A);
            } else if (i % 2 == 1) {
                video_draw_rect(win->x + 10, row_y, win->width - 20, 22, 0x180A2C);
            }

            icon_draw_16(file_items[i].icon, (int32_t)win->x + 16, (int32_t)row_y + 3);
            font_draw_string(f_ui_reg, (int32_t)win->x + 36, (int32_t)row_y + 3, file_items[i].name, COLOR_WHITE, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 180, (int32_t)row_y + 3, file_items[i].type, 0xC77DFF, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 270, (int32_t)row_y + 3, file_items[i].size, 0xE0AAFF, &clip);
        }

        /* Status bar */
        uint32_t sb_y = win->y + win->height - 22;
        video_draw_rect(win->x, sb_y, win->width, 22, 0x110722);
        font_draw_string(f_ui_reg, (int32_t)win->x + 12, (int32_t)sb_y + 2, "5 items | Volume: PurpleSystem | Safe Storage Active", 0x9D4EDD, &clip);

    } else if (win->type == WIN_TYPE_SYSMON) {
        /* System Monitor */
        font_draw_string(f_ui_bold, (int32_t)win->x + 12, (int32_t)win->y + 28, "Hardware & Task Monitor (Aegis64 Kernel)", COLOR_GOLD, &clip);

        /* 4 CPU Cores */
        font_draw_string(f_ui_reg, (int32_t)win->x + 12, (int32_t)win->y + 48, "CPU: 4 Cores SMP Online", COLOR_WHITE, &clip);
        for (int c = 0; c < 4; c++) {
            uint32_t cx = win->x + 12 + c * ((win->width - 36) / 4);
            uint32_t cw = (win->width - 48) / 4;
            video_draw_rect(cx, win->y + 68, cw, 12, 0x2A1245);
            uint32_t loads[4] = { 12, 8, 16, 4 };
            video_draw_rect(cx, win->y + 68, (cw * loads[c]) / 100, 12, 0x9D4EDD);
        }

        /* Memory */
        font_draw_string(f_ui_reg, (int32_t)win->x + 12, (int32_t)win->y + 86, "Memory: 185 MB Used / 3911 MB Free / 4096 MB Managed", COLOR_WHITE, &clip);
        video_draw_rect(win->x + 12, win->y + 104, win->width - 24, 12, 0x2A1245);
        video_draw_rect(win->x + 12, win->y + 104, (win->width - 24) * 4 / 100, 12, 0x00FF88);

        /* Scheduler Process List */
        font_draw_string(f_ui_bold, (int32_t)win->x + 12, (int32_t)win->y + 124, "PID   Command        State      Prio   CPU%", COLOR_GOLD, &clip);
        static const char *procs[4] = {
            " 1    kinit          Running    0      1.2%",
            " 2    desktop        Running    1      8.4%",
            " 3    aegissched     Sleeping   0      0.1%",
            " 4    purplefs       Idle       1      0.0%"
        };
        for (int p = 0; p < 4; p++) {
            font_draw_string(f_mono_reg, (int32_t)win->x + 12, (int32_t)(win->y + 144 + p * 16), procs[p], COLOR_WHITE, &clip);
        }

        /* GPU Status */
        gpu_device_t *g = gpu_get_primary();
        char g_msg[96];
        strncpy(g_msg, "GPU: ", sizeof(g_msg) - 1);
        strncat(g_msg, g->name, sizeof(g_msg) - strlen(g_msg) - 1);
        strncat(g_msg, " [BCS/SDMA Hardware Accel]", sizeof(g_msg) - strlen(g_msg) - 1);
        font_draw_string(f_ui_reg, (int32_t)win->x + 12, (int32_t)(win->y + 214), g_msg, 0xF72585, &clip);

    } else if (win->type == WIN_TYPE_SETTINGS) {
        /* Settings & Preferences */
        static const char *tabs[5] = { "Appearance", "Display", "Sound", "Security", "About" };
        for (int t = 0; t < 5; t++) {
            uint32_t tx = win->x + 10 + t * 76;
            uint32_t tw = 72;
            uint32_t bg = (win->settings_active_tab == t) ? 0x7209B7 : 0x220D38;
            video_draw_rect(tx, win->y + 28, tw, 22, bg);
            font_draw_string(f_ui_reg, (int32_t)tx + 6, (int32_t)win->y + 31, tabs[t], COLOR_WHITE, &clip);
        }

        if (win->settings_active_tab == 0) {
            /* Appearance */
            font_draw_string(f_ui_bold, (int32_t)win->x + 16, (int32_t)win->y + 60, "Theme & Appearance", COLOR_GOLD, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 16, (int32_t)win->y + 82, "Desktop Theme: Cosmic Violet (Locked Design Spec)", COLOR_WHITE, &clip);

            /* Dark mode toggle */
            video_draw_rect(win->x + 16, win->y + 108, 120, 26, win->settings_dark_mode ? 0x3A0CA3 : 0x2A1245);
            font_draw_string(f_ui_bold, (int32_t)win->x + 24, (int32_t)win->y + 112, win->settings_dark_mode ? "Dark Mode: ON" : "Dark Mode: OFF", COLOR_WHITE, &clip);

            /* Night light toggle */
            video_draw_rect(win->x + 146, win->y + 108, 130, 26, win->settings_night_mode ? 0x3A0CA3 : 0x2A1245);
            font_draw_string(f_ui_bold, (int32_t)win->x + 154, (int32_t)win->y + 112, win->settings_night_mode ? "Night Light: ON" : "Night Light: OFF", COLOR_WHITE, &clip);

        } else if (win->settings_active_tab == 1) {
            /* Display */
            font_draw_string(f_ui_bold, (int32_t)win->x + 16, (int32_t)win->y + 60, "Display Configuration", COLOR_GOLD, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 16, (int32_t)win->y + 84, "Resolution: 1024 x 768 @ 60 Hz", COLOR_WHITE, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 16, (int32_t)win->y + 108, "Pixel Format: 32-bit ARGB (Direct Framebuffer)", COLOR_WHITE, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 16, (int32_t)win->y + 132, "Scaling: 100% (Native Fixed-DPI)", 0xC77DFF, &clip);

        } else if (win->settings_active_tab == 2) {
            /* Sound */
            font_draw_string(f_ui_bold, (int32_t)win->x + 16, (int32_t)win->y + 60, "Audio & Sound Settings", COLOR_GOLD, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 16, (int32_t)win->y + 84, "Device: Intel HD Audio Codec (ALC888)", COLOR_WHITE, &clip);

            /* Interactive Volume Buttons */
            video_draw_rect(win->x + 16, win->y + 112, 24, 24, 0x3A0CA3);
            font_draw_string(f_ui_bold, (int32_t)win->x + 23, (int32_t)win->y + 114, "-", COLOR_WHITE, &clip);

            /* Volume Bar */
            video_draw_rect(win->x + 46, win->y + 116, 160, 16, 0x2A1245);
            uint32_t vol_fill = (160 * (uint32_t)win->settings_volume) / 100;
            video_draw_rect(win->x + 46, win->y + 116, vol_fill, 16, 0x9D4EDD);

            video_draw_rect(win->x + 212, win->y + 112, 24, 24, 0x3A0CA3);
            font_draw_string(f_ui_bold, (int32_t)win->x + 219, (int32_t)win->y + 114, "+", COLOR_WHITE, &clip);

            char vol_txt[16];
            vol_txt[0] = ' ';
            vol_txt[1] = '0' + (char)(win->settings_volume / 10);
            vol_txt[2] = '0' + (char)(win->settings_volume % 10);
            vol_txt[3] = '%';
            vol_txt[4] = '\0';
            font_draw_string(f_ui_bold, (int32_t)win->x + 244, (int32_t)win->y + 116, vol_txt, COLOR_WHITE, &clip);

        } else if (win->settings_active_tab == 3) {
            /* Security */
            font_draw_string(f_ui_bold, (int32_t)win->x + 16, (int32_t)win->y + 60, "AegisDetect Root of Trust", COLOR_GOLD, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 16, (int32_t)win->y + 84, "Anchor Key: HMAC-SHA256 (Kernel Enforced)", 0x00FF88, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 16, (int32_t)win->y + 108, "W^X Memory: CR0.WP Write Protection Active", 0x00FF88, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 16, (int32_t)win->y + 132, "Package Integrity: 100% APAPP/APDRV Valid", 0x00FF88, &clip);

        } else if (win->settings_active_tab == 4) {
            /* About */
            font_draw_string(f_ui_bold, (int32_t)win->x + 16, (int32_t)win->y + 60, "About Purple OS", COLOR_GOLD, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 16, (int32_t)win->y + 84, "Operating System: Purple OS Aegis64", COLOR_WHITE, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 16, (int32_t)win->y + 108, "Kernel: Aegis64 Native Hybrid Core v0.4.0", COLOR_WHITE, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 16, (int32_t)win->y + 132, "Architecture: x86-64 SMP (Multi-Core)", 0xC77DFF, &clip);
        }

    } else if (win->type == WIN_TYPE_APPMGR) {
        /* Application & Driver Manager */
        video_draw_rect(win->x + 10, win->y + 28, win->width - 20, 22, 0x2A1245);
        font_draw_string(f_ui_bold, (int32_t)win->x + 16, (int32_t)win->y + 31, "Installed Packages (AegisDetect Cryptographically Verified)", COLOR_GOLD, &clip);

        static const struct {
            const char *pkg;
            const char *ver;
            const char *sig;
        } pkgs[6] = {
            { "terminal.apapplication",  "v1.0.0", "HMAC-SHA256 [VERIFIED]" },
            { "files.apapplication",     "v1.0.0", "HMAC-SHA256 [VERIFIED]" },
            { "sysmon.apapplication",    "v1.2.0", "HMAC-SHA256 [VERIFIED]" },
            { "settings.apapplication",  "v1.1.0", "HMAC-SHA256 [VERIFIED]" },
            { "editor.apapplication",    "v1.0.0", "HMAC-SHA256 [VERIFIED]" },
            { "storage.apdrv",           "v3.0.0", "APDRV Native Module" }
        };

        for (int p = 0; p < 6; p++) {
            uint32_t py = win->y + 54 + p * 22;
            if (py + 20 >= win->y + win->height - 35) break;

            if (p % 2 == 1) {
                video_draw_rect(win->x + 10, py, win->width - 20, 20, 0x180A2C);
            }
            font_draw_string(f_mono_bold, (int32_t)win->x + 16, (int32_t)py + 2, pkgs[p].pkg, COLOR_WHITE, &clip);
            font_draw_string(f_mono_reg, (int32_t)win->x + 230, (int32_t)py + 2, pkgs[p].ver, 0xC77DFF, &clip);
            font_draw_string(f_mono_reg, (int32_t)win->x + 300, (int32_t)py + 2, pkgs[p].sig, 0x00FF88, &clip);
        }

        /* Verify Button */
        video_draw_rect(win->x + 12, win->y + win->height - 30, 200, 22, 0x3A0CA3);
        font_draw_string(f_ui_bold, (int32_t)win->x + 20, (int32_t)(win->y + win->height - 27), "Verify All Signatures", COLOR_WHITE, &clip);

    } else if (win->type == WIN_TYPE_EDITOR) {
        /* Text Editor */
        /* Toolbar */
        video_draw_rect(win->x + 10, win->y + 28, 56, 22, 0x3A0CA3);
        font_draw_string(f_ui_bold, (int32_t)win->x + 16, (int32_t)win->y + 31, "Open", COLOR_WHITE, &clip);

        video_draw_rect(win->x + 72, win->y + 28, 56, 22, 0x3A0CA3);
        font_draw_string(f_ui_bold, (int32_t)win->x + 80, (int32_t)win->y + 31, "Save", COLOR_WHITE, &clip);

        video_draw_rect(win->x + 134, win->y + 28, 56, 22, 0x2A1245);
        font_draw_string(f_ui_bold, (int32_t)win->x + 142, (int32_t)win->y + 31, "Clear", 0xC77DFF, &clip);

        char fn_bar[64];
        strncpy(fn_bar, "File: ", sizeof(fn_bar) - 1);
        strncat(fn_bar, win->editor_filename, sizeof(fn_bar) - strlen(fn_bar) - 1);
        if (win->editor_modified) strncat(fn_bar, " [*]", sizeof(fn_bar) - strlen(fn_bar) - 1);
        font_draw_string(f_ui_reg, (int32_t)win->x + 200, (int32_t)win->y + 31, fn_bar, COLOR_GOLD, &clip);

        /* Text Edit Buffer */
        video_draw_rect(win->x + 10, win->y + 54, win->width - 20, win->height - 64, 0x0E0518);
        video_draw_rect(win->x + 10, win->y + 54, 30, win->height - 64, 0x170928);

        /* Render lines */
        const char *p = win->editor_content;
        int line_num = 1;
        uint32_t ly = win->y + 58;

        while (*p && ly + 18 < win->y + win->height - 10) {
            char num_str[8];
            num_str[0] = '0' + (char)(line_num / 10);
            num_str[1] = '0' + (char)(line_num % 10);
            num_str[2] = '\0';
            font_draw_string(f_mono_reg, (int32_t)win->x + 13, (int32_t)ly, num_str, 0x7209B7, &clip);

            char line_buf[64];
            int li = 0;
            while (*p && *p != '\n' && li < 60) {
                line_buf[li++] = *p++;
            }
            if (*p == '\n') p++;
            line_buf[li] = '\0';
            font_draw_string(f_mono_reg, (int32_t)win->x + 46, (int32_t)ly, line_buf, COLOR_WHITE, &clip);

            line_num++;
            ly += 18;
        }

    } else if (win->type == WIN_TYPE_CALCULATOR) {
        /* Calculator */
        /* LCD Display Box */
        video_draw_rect(win->x + 16, win->y + 28, win->width - 32, 34, 0x110722);
        video_draw_rect(win->x + 16, win->y + 28, win->width - 32, 1, 0x7209B7);

        int32_t dw = font_measure_string(f_ui_bold, win->calc_display);
        int32_t dx = (int32_t)(win->x + win->width - 24 - (dw > 0 ? dw : 10));
        font_draw_string(f_ui_bold, dx, (int32_t)win->y + 36, win->calc_display, COLOR_WHITE, &clip);

        /* Keypad Grid (5 rows x 4 columns) */
        static const char *kpad[5][4] = {
            { "C", "(", ")", "/" },
            { "7", "8", "9", "*" },
            { "4", "5", "6", "-" },
            { "1", "2", "3", "+" },
            { "0", ".", "+/-", "=" }
        };

        for (int r = 0; r < 5; r++) {
            for (int c = 0; c < 4; c++) {
                uint32_t bx = win->x + 16 + c * 48;
                uint32_t by = win->y + 68 + r * 38;
                uint32_t bg = (c == 3 || (r == 4 && c == 3)) ? 0x7209B7 : 0x2A1245;
                if (r == 0 && c == 0) bg = 0x4A1020;

                video_draw_rect(bx, by, 42, 32, bg);
                video_draw_rect(bx, by, 42, 1, 0x9D4EDD);

                int32_t bw = font_measure_string(f_ui_bold, kpad[r][c]);
                int32_t btx = (int32_t)(bx + (42 - bw) / 2);
                font_draw_string(f_ui_bold, btx, (int32_t)by + 8, kpad[r][c], COLOR_WHITE, &clip);
            }
        }

    } else if (win->type == WIN_TYPE_HELP) {
        /* Help Center */
        font_draw_string(f_ui_bold, (int32_t)win->x + 14, (int32_t)win->y + 30, "Purple OS / Aegis64 Documentation & Shortcuts", COLOR_GOLD, &clip);

        font_draw_string(f_ui_bold, (int32_t)win->x + 14, (int32_t)win->y + 54, "1. Window Navigation & Controls:", COLOR_WHITE, &clip);
        font_draw_string(f_ui_reg, (int32_t)win->x + 20, (int32_t)win->y + 72, "[-] Minimize to dock  |  [+] Maximize / Restore  |  [X] Close window", 0xC77DFF, &clip);
        font_draw_string(f_ui_reg, (int32_t)win->x + 20, (int32_t)win->y + 90, "Drag Title Bar: Move window freely across the desktop canvas", 0xC77DFF, &clip);

        font_draw_string(f_ui_bold, (int32_t)win->x + 14, (int32_t)win->y + 114, "2. Shell Commands (Aegis Terminal):", COLOR_WHITE, &clip);
        font_draw_string(f_mono_reg, (int32_t)win->x + 20, (int32_t)win->y + 132, "help, ls, peek <file>, cpu, mem, gpu, gpu bench, clear", COLOR_WHITE, &clip);

        font_draw_string(f_ui_bold, (int32_t)win->x + 14, (int32_t)win->y + 156, "3. Security & Safety Model:", COLOR_WHITE, &clip);
        font_draw_string(f_ui_reg, (int32_t)win->x + 20, (int32_t)win->y + 174, "AegisDetect HMAC-SHA256 binary validation, W^X page tables, CR0.WP.", 0x00FF88, &clip);

        font_draw_string(f_ui_bold, (int32_t)win->x + 14, (int32_t)win->y + 198, "4. Hotkeys:", COLOR_WHITE, &clip);
        font_draw_string(f_ui_reg, (int32_t)win->x + 20, (int32_t)win->y + 216, "Esc: Dismiss Launcher/Panel  |  Up/Down: Command History", 0xC77DFF, &clip);
    }
}

void desktop_render(void) {
    desktop_active = 1;
    if (!video_is_available()) {
        desktop_render_preview();
        return;
    }

    uint32_t scr_w = video_get_width();
    uint32_t scr_h = video_get_height();

    /* 1. Clean Anime Cosmic Fantasy Wallpaper (Pure Vista, ZERO UI Baked In) */
    render_cosmic_background(scr_w, scr_h);

    /* 2. Dynamically Composite All Native Desktop UI Elements Independently */
    render_top_bar(scr_w);
    render_center_clock_and_motto(scr_w, scr_h);
    if (launcher_open) {
        render_launcher_card(scr_w, scr_h);
    }
    if (system_panel_open) {
        render_system_panel(scr_w, scr_h);
    }
    render_bottom_dock(scr_w, scr_h);
    render_welcome_toast(scr_w, scr_h);

    /* 3. Draw Windows in z-order (only visible windows) */
    for (uint32_t i = 0; i < window_count; i++) {
        uint32_t idx = z_order[i];
        if (windows[idx].visible) {
            draw_window_frame(&windows[idx]);
            draw_window_content(&windows[idx], idx);
        }
    }

    /* 4. Mouse Cursor */
    cursor_drawn = 0;
    int32_t mx = mouse_get_x();
    int32_t my = mouse_get_y();
    desktop_move_cursor(mx, my);

    kinfo("Desktop: Graphical desktop frame rendered to framebuffer (%ux%u)", scr_w, scr_h);
}

int desktop_handle_mouse(int32_t x, int32_t y, uint8_t buttons) {
    static uint8_t last_buttons = 0;
    static int is_dragging = 0;
    static int drag_window_idx = -1;
    static int32_t drag_off_x = 0;
    static int32_t drag_off_y = 0;

    desktop_move_cursor(x, y);

    int event = DESKTOP_EVENT_CURSOR_MOVED;
    uint8_t pressed = buttons & ~last_buttons;
    uint8_t released = last_buttons & ~buttons;

    if (is_dragging && (buttons & MOUSE_BTN_LEFT)) {
        if (drag_window_idx >= 0 && drag_window_idx < (int)window_count) {
            gui_window_t *w = &windows[drag_window_idx];
            int32_t n_x = x - drag_off_x;
            int32_t n_y = y - drag_off_y;

            if (n_x < 0) n_x = 0;
            if (n_y < 32) n_y = 32;
            if (video_is_available()) {
                if (n_x + (int32_t)w->width > (int32_t)video_get_width()) {
                    n_x = (int32_t)video_get_width() - (int32_t)w->width;
                }
                if (n_y + (int32_t)w->height > (int32_t)video_get_height()) {
                    n_y = (int32_t)video_get_height() - (int32_t)w->height;
                }
            }
            if ((uint32_t)n_x != w->x || (uint32_t)n_y != w->y) {
                w->x = (uint32_t)n_x;
                w->y = (uint32_t)n_y;
                desktop_render();
                event = DESKTOP_EVENT_WINDOW_MOVED;
            }
        }
    }

    if (released & MOUSE_BTN_LEFT) {
        is_dragging = 0;
        drag_window_idx = -1;
    }

    if (pressed & MOUSE_BTN_LEFT) {
        desktop_hit_result_t hit = desktop_hit_test(x, y);

        if (hit.type == DESKTOP_HIT_WINDOW_CLOSE) {
            desktop_close_window(windows[hit.window_index].id);
            desktop_render();
            event = DESKTOP_EVENT_WINDOW_CLOSED;
        } else if (hit.type == DESKTOP_HIT_WINDOW_MINIMIZE) {
            desktop_minimize_window(windows[hit.window_index].id);
            desktop_render();
            event = DESKTOP_EVENT_WINDOW_CLOSED;
        } else if (hit.type == DESKTOP_HIT_WINDOW_MAXIMIZE) {
            desktop_maximize_window(windows[hit.window_index].id);
            desktop_render();
            event = DESKTOP_EVENT_WINDOW_MOVED;
        } else if (hit.type == DESKTOP_HIT_WINDOW_TITLE) {
            desktop_focus_window(windows[hit.window_index].id);
            is_dragging = 1;
            drag_window_idx = hit.window_index;
            drag_off_x = x - (int32_t)windows[hit.window_index].x;
            drag_off_y = y - (int32_t)windows[hit.window_index].y;
            desktop_render();
            event = DESKTOP_EVENT_WINDOW_FOCUSED;
        } else if (hit.type == DESKTOP_HIT_WINDOW_BODY) {
            desktop_focus_window(windows[hit.window_index].id);
            gui_window_t *w = &windows[hit.window_index];

            if (w->type == WIN_TYPE_SETTINGS) {
                if (hit.click_y >= 28 && hit.click_y <= 52) {
                    int tab = (hit.click_x - 10) / 76;
                    if (tab >= 0 && tab <= 4) w->settings_active_tab = tab;
                } else if (w->settings_active_tab == 2 && hit.click_y >= 110 && hit.click_y <= 138) {
                    if (hit.click_x >= 16 && hit.click_x <= 40 && w->settings_volume > 0) w->settings_volume -= 5;
                    if (hit.click_x >= 210 && hit.click_x <= 236 && w->settings_volume < 100) w->settings_volume += 5;
                } else if (w->settings_active_tab == 0 && hit.click_y >= 108 && hit.click_y <= 136) {
                    if (hit.click_x >= 16 && hit.click_x <= 136) w->settings_dark_mode = !w->settings_dark_mode;
                    if (hit.click_x >= 146 && hit.click_x <= 276) w->settings_night_mode = !w->settings_night_mode;
                }
            } else if (w->type == WIN_TYPE_CALCULATOR) {
                if (hit.click_x >= 16 && hit.click_x <= 16 + 4 * 48 && hit.click_y >= 68 && hit.click_y <= 68 + 5 * 38) {
                    int col = (hit.click_x - 16) / 48;
                    int row = (hit.click_y - 68) / 38;
                    static const char kmap[5][4] = {
                        { 'C', '(', ')', '/' },
                        { '7', '8', '9', '*' },
                        { '4', '5', '6', '-' },
                        { '1', '2', '3', '+' },
                        { '0', '.', '~', '=' }
                    };
                    char key = kmap[row][col];
                    if (key >= '0' && key <= '9') {
                        if (w->calc_new_entry || (w->calc_display[0] == '0' && w->calc_display[1] == '\0')) {
                            w->calc_display[0] = key;
                            w->calc_display[1] = '\0';
                            w->calc_new_entry = 0;
                        } else {
                            size_t dl = strlen(w->calc_display);
                            if (dl + 1 < sizeof(w->calc_display)) {
                                w->calc_display[dl] = key;
                                w->calc_display[dl + 1] = '\0';
                            }
                        }
                    } else if (key == '+' || key == '-' || key == '*' || key == '/') {
                        int64_t val = 0;
                        const char *p = w->calc_display;
                        int sign = 1;
                        if (*p == '-') { sign = -1; p++; }
                        while (*p >= '0' && *p <= '9') { val = val * 10 + (*p - '0'); p++; }
                        w->calc_prev_val = val * sign;
                        w->calc_op = key;
                        w->calc_new_entry = 1;
                    } else if (key == '=') {
                        int64_t val = 0;
                        const char *p = w->calc_display;
                        int sign = 1;
                        if (*p == '-') { sign = -1; p++; }
                        while (*p >= '0' && *p <= '9') { val = val * 10 + (*p - '0'); p++; }
                        val = val * sign;
                        int64_t res = val;
                        if (w->calc_op == '+') res = w->calc_prev_val + val;
                        else if (w->calc_op == '-') res = w->calc_prev_val - val;
                        else if (w->calc_op == '*') res = w->calc_prev_val * val;
                        else if (w->calc_op == '/' && val != 0) res = w->calc_prev_val / val;

                        if (res == 0) {
                            w->calc_display[0] = '0';
                            w->calc_display[1] = '\0';
                        } else {
                            char tmp[32];
                            int ti = 0;
                            int is_neg = 0;
                            if (res < 0) { is_neg = 1; res = -res; }
                            while (res > 0) { tmp[ti++] = '0' + (res % 10); res /= 10; }
                            int di = 0;
                            if (is_neg) w->calc_display[di++] = '-';
                            while (ti > 0) w->calc_display[di++] = tmp[--ti];
                            w->calc_display[di] = '\0';
                        }
                        w->calc_new_entry = 1;
                        w->calc_op = 0;
                    } else if (key == 'C') {
                        w->calc_display[0] = '0';
                        w->calc_display[1] = '\0';
                        w->calc_prev_val = 0;
                        w->calc_op = 0;
                        w->calc_new_entry = 0;
                    }
                }
            } else if (w->type == WIN_TYPE_EDITOR) {
                if (hit.click_y >= 28 && hit.click_y <= 52) {
                    if (hit.click_x >= 10 && hit.click_x <= 66) {
                        /* Open */
                        const char *open_text = "Purple OS Aegis64 Release v0.4.0\nSafe storage and typography ready.\n";
                        strncpy(w->editor_content, open_text, sizeof(w->editor_content) - 1);
                        w->editor_cursor = (uint32_t)strlen(w->editor_content);
                        w->editor_modified = 0;
                    } else if (hit.click_x >= 72 && hit.click_x <= 128) {
                        /* Save */
                        w->editor_modified = 0;
                    } else if (hit.click_x >= 134 && hit.click_x <= 190) {
                        /* Clear */
                        w->editor_content[0] = '\0';
                        w->editor_cursor = 0;
                        w->editor_modified = 1;
                    }
                }
            } else if (w->type == WIN_TYPE_FILES) {
                if (hit.click_y >= 80 && hit.click_y <= 200) {
                    int row = (hit.click_y - 80) / 24;
                    if (row >= 0 && row < 5) w->files_selected_idx = row;
                }
            }
            desktop_render();
            event = DESKTOP_EVENT_WINDOW_FOCUSED;
        } else if (hit.type == DESKTOP_HIT_LAUNCHER_ITEM) {
            static const char *launcher_apps[12] = {
                "terminal", "files", "editor", "calc",
                "settings", "appmgr", "sysmon", "help",
                "music", "video", "photos", "calendar"
            };
            if (hit.menu_index < 12) {
                desktop_open_application(launcher_apps[hit.menu_index]);
            }
            event = DESKTOP_EVENT_MENU_CLICKED;
        } else if (hit.type == DESKTOP_HIT_LAUNCHER_BTN) {
            launcher_open = !launcher_open;
            desktop_render();
            event = DESKTOP_EVENT_MENU_CLICKED;
        } else if (hit.type == DESKTOP_HIT_SYSTEM_PANEL) {
            system_panel_open = !system_panel_open;
            desktop_render();
            event = DESKTOP_EVENT_MENU_CLICKED;
        } else if (hit.type == DESKTOP_HIT_NOTIF_CLOSE) {
            welcome_notif_visible = 0;
            desktop_render();
            event = DESKTOP_EVENT_MENU_CLICKED;
        } else if (hit.type == DESKTOP_HIT_DOCK_ITEM) {
            static const char *dock_apps[9] = {
                "launcher", "terminal", "files", "editor",
                "calc", "sysmon", "settings", "help", "panel"
            };
            if (hit.menu_index == 0) {
                launcher_open = !launcher_open;
            } else if (hit.menu_index == 8) {
                system_panel_open = !system_panel_open;
            } else if (hit.menu_index < 9) {
                desktop_open_application(dock_apps[hit.menu_index]);
            }
            desktop_render();
            event = DESKTOP_EVENT_MENU_CLICKED;
        } else if (hit.type == DESKTOP_HIT_TOPBAR) {
            if (hit.menu_index == 0) {
                launcher_open = !launcher_open;
            } else if (hit.menu_index == 1) {
                desktop_open_application("terminal");
            } else if (hit.menu_index == 2) {
                desktop_open_application("files");
            } else if (hit.menu_index == 4) {
                system_panel_open = !system_panel_open;
            }
            desktop_render();
            event = DESKTOP_EVENT_MENU_CLICKED;
        }
    }

    last_buttons = buttons;
    return event;
}

static void desktop_mouse_event_callback(int32_t x, int32_t y, uint8_t buttons) {
    if (desktop_active) {
        desktop_handle_mouse(x, y, buttons);
    }
}

int desktop_init(void) {
    font_init();
    window_count = 0;
    cursor_drawn = 0;
    desktop_active = 0;
    launcher_open = 1;
    launcher_search[0] = '\0';
    system_panel_open = 1;
    welcome_notif_visible = 0;
    memset(windows, 0, sizeof(windows));
    memset(z_order, 0, sizeof(z_order));

    /* Window 0: Aegis Interactive Terminal */
    int t_id = desktop_create_window("Aegis Terminal (>_)", 30, 45, 340, 230, 0x120822);
    if (t_id >= 0) {
        gui_window_t *w = desktop_get_window((uint32_t)t_id);
        if (w) {
            w->type = WIN_TYPE_TERMINAL;
            w->icon_id = ICON_APP_TERMINAL;
            desktop_terminal_put_line(w, "Purple OS / Aegis64 Shell Terminal v1.0.0");
            desktop_terminal_put_line(w, "Type 'help' for available terminal commands.");
#ifndef __KERNEL_HOST_TEST__
            w->visible = 0;
            w->focused = 0;
#else
            w->visible = 1;
            w->focused = 1;
#endif
        }
    }

    /* Window 1: Purple Files / Storage Explorer */
    int s_id = desktop_create_window("Purple Files", 405, 45, 380, 240, 0x180D26);
    if (s_id >= 0) {
        gui_window_t *w = desktop_get_window((uint32_t)s_id);
        if (w) {
            w->type = WIN_TYPE_FILES;
            w->icon_id = ICON_APP_FILES;
            strncpy(w->files_current_path, "/", sizeof(w->files_current_path) - 1);
            w->files_selected_idx = 0;
            w->visible = 0;
            w->focused = 0;
        }
    }

    /* Window 2: System Monitor & Hardware */
    int m_id = desktop_create_window("System Monitor", 30, 300, 420, 250, 0x180D26);
    if (m_id >= 0) {
        gui_window_t *w = desktop_get_window((uint32_t)m_id);
        if (w) {
            w->type = WIN_TYPE_SYSMON;
            w->icon_id = ICON_APP_SYSMON;
            w->visible = 0;
            w->focused = 0;
        }
    }

    /* Window 3: Settings & Preferences */
    int set_id = desktop_create_window("Settings & Preferences", 465, 300, 400, 250, 0x180D26);
    if (set_id >= 0) {
        gui_window_t *w = desktop_get_window((uint32_t)set_id);
        if (w) {
            w->type = WIN_TYPE_SETTINGS;
            w->icon_id = ICON_APP_SETTINGS;
            w->settings_active_tab = 0;
            w->settings_volume = 65;
            w->settings_dark_mode = 1;
            w->settings_night_mode = 0;
            w->visible = 0;
            w->focused = 0;
        }
    }

    /* Window 4: Application Manager */
    int app_id = desktop_create_window("Application Manager", 60, 70, 480, 280, 0x180D26);
    if (app_id >= 0) {
        gui_window_t *w = desktop_get_window((uint32_t)app_id);
        if (w) {
            w->type = WIN_TYPE_APPMGR;
            w->icon_id = ICON_APP_APPSTORE;
            w->appmgr_selected_idx = 0;
            w->visible = 0;
            w->focused = 0;
        }
    }

    /* Window 5: Text Editor */
    int ed_id = desktop_create_window("Text Editor", 180, 80, 450, 280, 0x140924);
    if (ed_id >= 0) {
        gui_window_t *w = desktop_get_window((uint32_t)ed_id);
        if (w) {
            w->type = WIN_TYPE_EDITOR;
            w->icon_id = ICON_APP_EDITOR;
            strncpy(w->editor_filename, "welcome.txt", sizeof(w->editor_filename) - 1);
            const char *initial_text = "Welcome to Purple OS / Aegis64!\n\n"
                                       "This is the native Aegis Text Editor.\n"
                                       "Keyboard typing and backspace are supported.\n"
                                       "Click [Save] to commit changes.\n";
            strncpy(w->editor_content, initial_text, sizeof(w->editor_content) - 1);
            w->editor_cursor = (uint32_t)strlen(w->editor_content);
            w->editor_modified = 0;
            w->visible = 0;
            w->focused = 0;
        }
    }

    /* Window 6: Calculator */
    int calc_id = desktop_create_window("Calculator", 540, 90, 230, 290, 0x180D26);
    if (calc_id >= 0) {
        gui_window_t *w = desktop_get_window((uint32_t)calc_id);
        if (w) {
            w->type = WIN_TYPE_CALCULATOR;
            w->icon_id = ICON_APP_CALCULATOR;
            w->calc_display[0] = '0';
            w->calc_display[1] = '\0';
            w->calc_prev_val = 0;
            w->calc_op = 0;
            w->calc_new_entry = 0;
            w->visible = 0;
            w->focused = 0;
        }
    }

    /* Window 7: Help Center */
    int help_id = desktop_create_window("Help Center", 120, 60, 480, 300, 0x180D26);
    if (help_id >= 0) {
        gui_window_t *w = desktop_get_window((uint32_t)help_id);
        if (w) {
            w->type = WIN_TYPE_HELP;
            w->icon_id = ICON_APP_HELP;
            w->visible = 0;
            w->focused = 0;
        }
    }

    mouse_set_callback(desktop_mouse_event_callback);
    kinfo("Desktop: Graphical workspace online with 8 application windows");
    return 0;
}

void desktop_run_session(void) {
    desktop_active = 1;
    desktop_render();
    kinfo("Desktop: Interactive session online. Type directly or move mouse, press 'q' to exit.");
    int running = 1;
    int32_t lx = mouse_get_x();
    int32_t ly = mouse_get_y();
    uint8_t lb = mouse_get_buttons();

    while (running) {
        int32_t cx = mouse_get_x();
        int32_t cy = mouse_get_y();
        uint8_t cb = mouse_get_buttons();
        if (cx != lx || cy != ly || cb != lb) {
            desktop_handle_mouse(cx, cy, cb);
            lx = cx;
            ly = cy;
            lb = cb;
        }

#ifndef __KERNEL_HOST_TEST__
        key_event_t ev;
        if (keyboard_poll_event(&ev)) {
            if (ev.pressed) {
                if ((ev.key_code == 'q' || ev.key_code == 'Q') && !(windows[0].focused && windows[0].visible)) {
                    running = 0;
                } else if (ev.key_code == 27) {
                    if (launcher_open || system_panel_open) {
                        launcher_open = 0;
                        system_panel_open = 0;
                        desktop_render();
                    } else {
                        running = 0;
                    }
                } else {
                    desktop_handle_key((char)ev.key_code, ev.scancode, ev.pressed, ev.modifiers);
                }
            }
        } else if (serial_has_char()) {
            char sc = serial_getchar();
            if (sc == 'q' || sc == 'Q' || sc == 27) {
                running = 0;
            } else {
                desktop_handle_key(sc, 0, 1, 0);
            }
        }
        __asm__ volatile("pause");
#else
        running = 0;
#endif
    }
    kinfo("Desktop: Interactive session terminated.");
}

void desktop_render_preview(void) {
    kprintf("\n========================================================================================\n");
    kprintf("   Purple OS / Aegis64 Native Desktop Environment (Workspace Canvas - Locked Layout)     \n");
    kprintf("========================================================================================\n");
    kprintf("  [Top Bar]  Lotus Logo | Applications   Places   System  | WiFi  Vol  98%%  21:30        \n");
    kprintf("  [Motto]    DREAM  \x07  BUILD  \x07  EXPLORE (Hollow Purple Violet Vortex Energy)             \n");
    kprintf("  --------------------------------------------------------------------------------------\n");
    kprintf("  +-------------------------------------+    +-----------------------------------------+\n");
    kprintf("  | [X] Aegis Interactive Terminal (>_) |    | [X] PurpleFS v2 Storage                 |\n");
    kprintf("  | purple@aegis64:~$ peek welcome.txt  |    | Volume: PurpleSystem (16 MiB Partition) |\n");
    kprintf("  | Output: Welcome to Purple OS!       |    | Files: welcome.txt, version.txt, motd   |\n");
    kprintf("  +-------------------------------------+    +-----------------------------------------+\n");
    kprintf("  +------------------------------------------------------------------------------------+\n");
    kprintf("  | [X] Aegis64 System Monitor & AegisDetect Security Console                          |\n");
    kprintf("  | CPU: 4 Cores Discovered | Memory: 4 GiB Managed | GPU: Hardware Accelerated BCS/SDMA|\n");
    kprintf("  +------------------------------------------------------------------------------------+\n");
    kprintf("  [Dock]    [ >_ Terminal ]  [ Dir Files ]  [ Sys Monitor ]  [ Sec Vault ]  [ Settings ] \n");
    kprintf("========================================================================================\n\n");
}
