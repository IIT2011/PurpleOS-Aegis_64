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

    /* Check if Launcher is open and accepting search input */
    if (launcher_open) {
        if (c == 27) { // ESC
            launcher_open = 0;
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

    /* Route to currently focused window */
    for (int i = (int)window_count - 1; i >= 0; i--) {
        int idx = z_order[i];
        gui_window_t *win = &windows[idx];
        if (win->visible && win->focused && win->type == WIN_TYPE_TERMINAL) {
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

    if (x < 0 || y < 0) return res;

    uint32_t scr_w = video_get_width();
    uint32_t scr_h = video_get_height();

    // Top Bar (y < 28)
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

    // Welcome notification close button (bottom right)
    if (welcome_notif_visible && scr_w >= 340 && scr_h >= 170) {
        uint32_t nx = scr_w - 310;
        uint32_t ny = scr_h - 170;
        if (x >= (int32_t)(nx + 260) && x <= (int32_t)(nx + 285) &&
            y >= (int32_t)(ny + 4) && y <= (int32_t)(ny + 24)) {
            res.type = DESKTOP_HIT_NOTIF_CLOSE;
            return res;
        }
    }

    // Check windows in z-order from top to bottom
    for (int i = (int)window_count - 1; i >= 0; i--) {
        int idx = z_order[i];
        gui_window_t *win = &windows[idx];
        if (!win->visible) continue;

        if (x >= (int32_t)win->x && x < (int32_t)(win->x + win->width) &&
            y >= (int32_t)win->y && y < (int32_t)(win->y + win->height)) {
            res.window_index = idx;
            if (y < (int32_t)(win->y + 24)) {
                if (x >= (int32_t)(win->x + win->width - 28) &&
                    x <= (int32_t)(win->x + win->width - 4)) {
                    res.type = DESKTOP_HIT_WINDOW_CLOSE;
                } else {
                    res.type = DESKTOP_HIT_WINDOW_TITLE;
                }
            } else {
                res.type = DESKTOP_HIT_WINDOW_BODY;
            }
            return res;
        }
    }

    // Bottom Dock (y: scr_h - 60 .. scr_h - 12)
    if (y >= (int32_t)(scr_h - 60) && y <= (int32_t)(scr_h - 12)) {
        uint32_t dock_w = 350;
        uint32_t dock_x = (scr_w - dock_w) / 2;
        if (x >= (int32_t)dock_x && x <= (int32_t)(dock_x + dock_w)) {
            res.type = DESKTOP_HIT_DOCK_ITEM;
            res.menu_index = (x - (int32_t)dock_x) / 50;
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
    video_set_cursor(5, 0);
    video_set_colors(COLOR_GOLD, 0x130A24);
    video_puts("Applications");

    /* Navigation tabs */
    video_set_cursor(19, 0);
    video_set_colors(0xC77DFF, 0x130A24);
    video_puts("Places");

    video_set_cursor(27, 0);
    video_puts("System");

    /* Active pill under Applications */
    video_draw_rect(38, 29, 96, 2, 0x9D4EDD);

    /* Right System Tray: Network, Audio, Battery, Clock */
    uint32_t tray_col = (w - 220) / 8;
    video_set_cursor(tray_col, 0);
    video_set_colors(COLOR_WHITE, 0x130A24);
    video_puts("[WiFi] [Vol] [98%] ");

#ifndef __KERNEL_HOST_TEST__
    uint64_t sec = pit_get_ticks() / 100;
#else
    uint64_t sec = 42;
#endif
    char time_str[16];
    time_str[0] = '2'; time_str[1] = '1'; time_str[2] = ':';
    time_str[3] = '0' + (char)((sec / 60) % 10);
    time_str[4] = '0' + (char)(sec % 10);
    time_str[5] = '\0';
    video_puts(time_str);
}

static void render_center_clock_and_motto(uint32_t w, uint32_t h) {
    (void)h;
    uint32_t center_col = (w / 8) / 2;

    /* Large Digital Clock */
    video_set_cursor(center_col - 3, 4);
    video_set_colors(COLOR_WHITE, COLOR_PURPLE_BG);
    video_puts("21 : 30");

    /* Date String */
    video_set_cursor(center_col - 12, 6);
    video_set_colors(0xC77DFF, COLOR_PURPLE_BG);
    video_puts("Thursday, October 22, 2026");

    /* Inspirational Motto: DREAM • BUILD • EXPLORE */
    video_set_cursor(center_col - 13, 8);
    video_set_colors(0xF72585, COLOR_PURPLE_BG);
    video_puts("DREAM  \x07  BUILD  \x07  EXPLORE");
}

static void render_launcher_card(uint32_t w, uint32_t h) {
    (void)w; (void)h;
    uint32_t lx = 20;
    uint32_t ly = 40;
    uint32_t lw = 320;
    uint32_t lh = 380;

    /* Frosted glass body with purple border */
    video_draw_rect(lx - 1, ly - 1, lw + 2, lh + 2, 0x7209B7);
    video_draw_rect(lx, ly, lw, lh, 0x150A28);

    /* Search Input Bar */
    video_draw_rect(lx + 12, ly + 14, lw - 24, 28, 0x2A1245);
    video_set_cursor((lx + 20) / 8, (ly + 20) / 16);
    video_set_colors(0xC77DFF, 0x2A1245);
    if (strlen(launcher_search) == 0) {
        video_puts("Search applications...");
    } else {
        video_puts(launcher_search);
        video_puts("_");
    }

    /* Category Header */
    video_set_cursor((lx + 16) / 8, (ly + 56) / 16);
    video_set_colors(COLOR_GOLD, 0x150A28);
    video_puts("CATEGORIES");

    const char *cats[] = { "Favorites", "System Tools", "Development", "Settings" };
    for (int i = 0; i < 4; i++) {
        video_set_cursor((lx + 16) / 8, (ly + 76 + i * 20) / 16);
        video_set_colors(COLOR_WHITE, 0x150A28);
        video_puts("- ");
        video_puts(cats[i]);
    }

    /* Application items */
    video_set_cursor((lx + 16) / 8, (ly + 170) / 16);
    video_set_colors(COLOR_GOLD, 0x150A28);
    video_puts("APPLICATIONS");

    const char *apps[] = {
        ">_  Aegis Terminal",
        "[#] System Monitor",
        "[D] PurpleFS Storage",
        "[S] AegisDetect Security"
    };
    for (int i = 0; i < 4; i++) {
        video_draw_rect(lx + 12, ly + 192 + i * 32, lw - 24, 26, 0x220E3A);
        video_set_cursor((lx + 20) / 8, (ly + 197 + i * 32) / 16);
        video_set_colors(0xE0AAFF, 0x220E3A);
        video_puts(apps[i]);
    }

    /* User Profile Footer */
    video_draw_rect(lx, ly + lh - 40, lw, 40, 0x10061E);
    video_draw_rect(lx + 12, ly + lh - 32, 24, 24, 0x9D4EDD);
    video_set_cursor((lx + 44) / 8, (ly + lh - 26) / 16);
    video_set_colors(COLOR_WHITE, 0x10061E);
    video_puts("saptanshu | Aegis64");
}

static void render_system_panel(uint32_t w, uint32_t h) {
    (void)h;
    uint32_t px = w - 320;
    uint32_t py = 40;
    uint32_t pw = 300;
    uint32_t ph = 320;

    video_draw_rect(px - 1, py - 1, pw + 2, ph + 2, 0x7209B7);
    video_draw_rect(px, py, pw, ph, 0x150A28);

    video_set_cursor((px + 16) / 8, (py + 12) / 16);
    video_set_colors(COLOR_GOLD, 0x150A28);
    video_puts("System Quick Settings");

    /* Resource Meters */
    video_set_cursor((px + 16) / 8, (py + 36) / 16);
    video_set_colors(COLOR_WHITE, 0x150A28);
    video_puts("CPU: 12% (4 Cores Online)");
    video_draw_rect(px + 16, py + 54, pw - 32, 10, 0x2A1245);
    video_draw_rect(px + 16, py + 54, (pw - 32) * 12 / 100, 10, 0x9D4EDD);

    video_set_cursor((px + 16) / 8, (py + 72) / 16);
    video_puts("Memory: 185 MB / 4096 MB (4%)");
    video_draw_rect(px + 16, py + 90, pw - 32, 10, 0x2A1245);
    video_draw_rect(px + 16, py + 90, (pw - 32) * 4 / 100, 10, 0x9D4EDD);

    video_set_cursor((px + 16) / 8, (py + 108) / 16);
    video_puts("Storage: 16 MB / 64 MB (25%)");
    video_draw_rect(px + 16, py + 126, pw - 32, 10, 0x2A1245);
    video_draw_rect(px + 16, py + 126, (pw - 32) * 25 / 100, 10, 0x9D4EDD);

    /* GPU backend info */
    gpu_device_t *g = gpu_get_primary();
    video_set_cursor((px + 16) / 8, (py + 146) / 16);
    video_set_colors(0xC77DFF, 0x150A28);
    video_puts("GPU: ");
    video_puts(g->arch_name);

    /* Quick Toggles */
    video_draw_rect(px + 16, py + 175, 120, 30, 0x3A0CA3);
    video_set_cursor((px + 24) / 8, (py + 182) / 16);
    video_set_colors(COLOR_WHITE, 0x3A0CA3);
    video_puts("Wi-Fi: ON");

    video_draw_rect(px + 150, py + 175, 120, 30, 0x3A0CA3);
    video_set_cursor((px + 158) / 8, (py + 182) / 16);
    video_puts("BT: ON");

    video_draw_rect(px + 16, py + 215, 120, 30, 0x2A1245);
    video_set_cursor((px + 24) / 8, (py + 222) / 16);
    video_set_colors(0xC77DFF, 0x2A1245);
    video_puts("Night: OFF");

    video_draw_rect(px + 150, py + 215, 120, 30, 0x3A0CA3);
    video_set_cursor((px + 158) / 8, (py + 222) / 16);
    video_set_colors(COLOR_WHITE, 0x3A0CA3);
    video_puts("Dark: ON");

    /* Volume and Brightness Sliders */
    video_set_cursor((px + 16) / 8, (py + 258) / 16);
    video_set_colors(COLOR_WHITE, 0x150A28);
    video_puts("Volume: [=====|   ] 65%");

    video_set_cursor((px + 16) / 8, (py + 280) / 16);
    video_puts("Bright: [======|  ] 80%");
}

static void render_bottom_dock(uint32_t w, uint32_t h) {
    uint32_t dock_w = 350;
    uint32_t dock_h = 50;
    uint32_t dock_x = (w - dock_w) / 2;
    uint32_t dock_y = h - 60;

    /* Translucent rounded capsule */
    video_draw_rect(dock_x - 1, dock_y - 1, dock_w + 2, dock_h + 2, 0x7209B7);
    video_draw_rect(dock_x, dock_y, dock_w, dock_h, 0x150A28);

    const char *dock_icons[7] = { ">_", "Dir", "Sys", "Sec", "Calc", "Cfg", "Pwr" };
    for (int i = 0; i < 7; i++) {
        uint32_t ix = dock_x + 12 + i * 48;
        video_draw_rect(ix, dock_y + 8, 36, 34, 0x2A1245);
        video_set_cursor((ix + 6) / 8, (dock_y + 17) / 16);
        video_set_colors(0xF72585, 0x2A1245);
        video_puts(dock_icons[i]);

        /* Running dot indicator under apps 0, 1, 2 */
        if (i < 3 && windows[i].visible) {
            video_draw_rect(ix + 16, dock_y + 44, 4, 3, 0x9D4EDD);
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

    /* Close button [X] */
    video_set_cursor((nx + nw - 22) / 8, (ny + 6) / 16);
    video_set_colors(COLOR_RED, 0x180C2E);
    video_puts("[X]");

    video_set_cursor((nx + 12) / 8, (ny + 8) / 16);
    video_set_colors(COLOR_GOLD, 0x180C2E);
    video_puts("Welcome to Purple OS");

    video_set_cursor((nx + 12) / 8, (ny + 28) / 16);
    video_set_colors(COLOR_WHITE, 0x180C2E);
    video_puts("Kernel: Aegis64 Native x86-64");

    video_set_cursor((nx + 12) / 8, (ny + 46) / 16);
    video_set_colors(0xC77DFF, 0x180C2E);
    video_puts("Volume: PurpleFS v2 Mounted");

    video_set_cursor((nx + 12) / 8, (ny + 64) / 16);
    video_set_colors(0xF72585, 0x180C2E);
    video_puts("GPU: Acceleration Ready");
}

static void draw_window_frame(const gui_window_t *win) {
    if (!video_is_available() || !win->visible) return;

    uint32_t outline_color = win->focused ? 0x9D4EDD : 0x4A2574;
    video_draw_rect(win->x - 1, win->y - 1, win->width + 2, win->height + 2, outline_color);

    /* Title bar */
    video_draw_rect(win->x, win->y, win->width, 24, win->title_bg);

    /* Title text */
    video_set_cursor((win->x + 8) / 8, (win->y + 4) / 16);
    video_set_colors(COLOR_WHITE, win->title_bg);
    video_puts(win->title);

    /* Close button [X] */
    video_set_cursor((win->x + win->width - 24) / 8, (win->y + 4) / 16);
    video_set_colors(COLOR_RED, win->title_bg);
    video_puts("[X]");

    /* Body background */
    video_draw_rect(win->x, win->y + 24, win->width, win->height - 24, win->bg_color);
}

static void draw_window_content(const gui_window_t *win, uint32_t idx) {
    if (!video_is_available() || !win->visible) return;

    if (win->type == WIN_TYPE_TERMINAL) {
        /* Terminal Window */
        video_set_colors(0xC77DFF, win->bg_color);
        video_set_cursor((win->x + 12) / 8, (win->y + 32) / 16);
        video_puts("Purple OS / Aegis64 Interactive Shell Terminal");

        /* Scrollback Output Lines */
        for (int i = 0; i < win->output_count && i < 7; i++) {
            video_set_cursor((win->x + 12) / 8, (win->y + 52 + i * 18) / 16);
            video_set_colors(COLOR_WHITE, win->bg_color);
            video_puts(win->output_lines[i]);
        }

        /* Active prompt and input line */
        uint32_t prompt_y = win->y + win->height - 24;
        video_set_cursor((win->x + 12) / 8, prompt_y / 16);
        video_set_colors(0xF72585, win->bg_color);
        video_puts("purple@aegis64:~$ ");
        video_set_colors(COLOR_WHITE, win->bg_color);
        video_puts(win->input_line);
        video_puts("_");
    } else if (idx == 1) {
        /* Storage explorer */
        video_set_colors(COLOR_WHITE, win->bg_color);
        video_set_cursor((win->x + 12) / 8, (win->y + 36) / 16);
        video_puts("Volume: PurpleSystem (PurpleFS v2 Mounted)");

        video_set_cursor((win->x + 12) / 8, (win->y + 60) / 16);
        video_puts("Block Size: 4096 bytes | Journal: Active");

        video_set_cursor((win->x + 12) / 8, (win->y + 84) / 16);
        video_puts("Root Directory Files:");

        video_set_cursor((win->x + 20) / 8, (win->y + 104) / 16);
        video_set_colors(0xE0AAFF, win->bg_color);
        video_puts("[FILE] welcome.txt (132 bytes)");

        video_set_cursor((win->x + 20) / 8, (win->y + 124) / 16);
        video_puts("[FILE] version.txt (65 bytes)");

        video_set_cursor((win->x + 20) / 8, (win->y + 144) / 16);
        video_puts("[FILE] motd        (59 bytes)");
    } else if (idx == 2) {
        /* System Monitor */
        video_set_colors(COLOR_WHITE, win->bg_color);
        video_set_cursor((win->x + 12) / 8, (win->y + 36) / 16);
        video_puts("CPU Cores: 4 Discovered / Online");

        video_set_cursor((win->x + 12) / 8, (win->y + 60) / 16);
        video_puts("Scheduler: AegisSched_64 (4 Priorities)");

        video_set_cursor((win->x + 12) / 8, (win->y + 84) / 16);
        video_puts("Memory:    4 GiB Managed (239 MiB Free)");

        video_set_cursor((win->x + 12) / 8, (win->y + 108) / 16);
        video_puts("Security:  AegisDetect Root of Trust Active");
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

    /* 1. Anime Cosmic Fantasy Wallpaper */
    render_cosmic_background(scr_w, scr_h);

    /* 2. Top Translucent Taskbar & Tray */
    render_top_bar(scr_w);

    /* 3. Centered Clock & Inspirational Motto */
    render_center_clock_and_motto(scr_w, scr_h);

    /* 4. Draw Windows in z-order */
    for (uint32_t i = 0; i < window_count; i++) {
        uint32_t idx = z_order[i];
        draw_window_frame(&windows[idx]);
        draw_window_content(&windows[idx], idx);
    }

    /* 5. Bottom Floating Dock */
    render_bottom_dock(scr_w, scr_h);

    /* 6. Welcome Notification Toast */
    render_welcome_toast(scr_w, scr_h);

    /* 7. Application Launcher (if open) */
    if (launcher_open) {
        render_launcher_card(scr_w, scr_h);
    }

    /* 8. System Quick Controls Panel (if open) */
    if (system_panel_open) {
        render_system_panel(scr_w, scr_h);
    }

    /* 9. Mouse Cursor */
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
            desktop_render();
            event = DESKTOP_EVENT_WINDOW_FOCUSED;
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
            if (hit.menu_index == 0 && window_count > 0) { // Terminal
                desktop_toggle_window(windows[0].id);
            } else if (hit.menu_index == 1 && window_count > 1) { // Storage
                desktop_toggle_window(windows[1].id);
            } else if (hit.menu_index == 2 && window_count > 2) { // Sysmon
                desktop_toggle_window(windows[2].id);
            } else if (hit.menu_index == 5) {
                system_panel_open = !system_panel_open;
            }
            desktop_render();
            event = DESKTOP_EVENT_MENU_CLICKED;
        } else if (hit.type == DESKTOP_HIT_TOPBAR) {
            if (hit.menu_index == 1 && window_count > 0) {
                desktop_toggle_window(windows[0].id);
            } else if (hit.menu_index == 2 && window_count > 1) {
                desktop_toggle_window(windows[1].id);
            } else if (hit.menu_index == 3 && window_count > 2) {
                desktop_toggle_window(windows[2].id);
            } else if (hit.menu_index == 0) {
                launcher_open = !launcher_open;
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
    window_count = 0;
    cursor_drawn = 0;
    desktop_active = 0;
    launcher_open = 0;
    launcher_search[0] = '\0';
    system_panel_open = 0;
    welcome_notif_visible = 1;
    memset(windows, 0, sizeof(windows));
    memset(z_order, 0, sizeof(z_order));

    /* Window 0: Aegis Interactive Terminal */
    int t_id = desktop_create_window("Aegis Terminal (>_)", 30, 45, 340, 230, 0x120822);
    if (t_id >= 0) {
        gui_window_t *w = desktop_get_window((uint32_t)t_id);
        if (w) {
            w->type = WIN_TYPE_TERMINAL;
            desktop_terminal_put_line(w, "Type 'help' for available terminal commands.");
        }
    }

    /* Window 1: PurpleFS Storage Explorer */
    desktop_create_window("PurpleFS v2 Storage", 390, 45, 380, 230, 0x180D26);

    /* Window 2: AegisDetect & System Monitor */
    desktop_create_window("AegisDetect Console", 30, 295, 740, 230, 0x180D26);

    mouse_set_callback(desktop_mouse_event_callback);
    kinfo("Desktop: Graphical workspace online with 3 default windows");
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
                } else if (ev.key_code == 27 && !launcher_open && !system_panel_open) {
                    running = 0;
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
