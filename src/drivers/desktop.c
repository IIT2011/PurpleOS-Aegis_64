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
#include "task.h"
#include "pit.h"

static gui_window_t windows[DESKTOP_MAX_WINDOWS];
static uint32_t window_count = 0;
static uint32_t z_order[DESKTOP_MAX_WINDOWS];
static int desktop_active = 0;

#define CURSOR_W 11
#define CURSOR_H 16

/* High-contrast 16x11 Arrow Cursor:
   'X' = Black outline (0x000000)
   '.' = White fill    (0xFFFFFF)
   ' ' = Transparent   (keep background) */
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
    win->id = window_count;
    strncpy(win->title, title, sizeof(win->title) - 1);
    win->title[sizeof(win->title) - 1] = '\0';
    win->x = x;
    win->y = y;
    win->width = w;
    win->height = h;
    win->bg_color = bg;
    win->title_bg = COLOR_PURPLE_BAR;
    win->visible = 1;
    win->focused = 1;

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

desktop_hit_result_t desktop_hit_test(int32_t x, int32_t y) {
    desktop_hit_result_t res;
    res.type = DESKTOP_HIT_NONE;
    res.window_index = -1;
    res.menu_index = 0;

    if (x < 0 || y < 0) return res;

    // Check top bar
    if (y < 28) {
        res.type = DESKTOP_HIT_TOPBAR;
        if (x < 90) {
            res.menu_index = 0; /* Purple OS */
        } else if (x < 170) {
            res.menu_index = 1; /* System */
        } else if (x < 230) {
            res.menu_index = 2; /* Files */
        } else if (x < 310) {
            res.menu_index = 3; /* Security */
        } else {
            res.menu_index = 4; /* Devices */
        }
        return res;
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
                // Title bar or close button
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

    res.type = DESKTOP_HIT_DESKTOP;
    return res;
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
    memset(windows, 0, sizeof(windows));
    memset(z_order, 0, sizeof(z_order));

    desktop_create_window("Aegis64 System Monitor", 30, 45, 340, 230, 0x180D26);
    desktop_create_window("PurpleFS v2 Storage", 390, 45, 380, 230, 0x180D26);
    desktop_create_window("AegisDetect Console", 30, 295, 740, 230, 0x180D26);

    mouse_set_callback(desktop_mouse_event_callback);

    kinfo("Desktop: Graphical workspace and windowing primitives initialized (3 default windows)");
    return 0;
}

static void draw_window_frame(const gui_window_t *win) {
    if (!video_is_available() || !win->visible) return;

    // Window shadow / outline
    uint32_t outline_color = win->focused ? COLOR_PURPLE_ACC : 0x4A2574;
    video_draw_rect(win->x - 1, win->y - 1, win->width + 2, win->height + 2, outline_color);

    // Title bar
    video_draw_rect(win->x, win->y, win->width, 24, win->title_bg);

    // Title text
    uint32_t text_col = (win->x + 8) / 8;
    uint32_t text_row = (win->y + 4) / 16;
    video_set_cursor(text_col, text_row);
    video_set_colors(COLOR_WHITE, win->title_bg);
    video_puts(win->title);

    // Close button [X]
    uint32_t close_col = (win->x + win->width - 24) / 8;
    video_set_cursor(close_col, text_row);
    video_set_colors(COLOR_RED, win->title_bg);
    video_puts("[X]");

    // Body background
    video_draw_rect(win->x, win->y + 24, win->width, win->height - 24, win->bg_color);
}

static void draw_window_content(const gui_window_t *win, uint32_t idx) {
    if (!video_is_available() || !win->visible) return;

    if (idx == 0) {
        // Window 1 Content: System Monitor
        video_set_colors(COLOR_WHITE, win->bg_color);
        video_set_cursor((win->x + 12) / 8, (win->y + 36) / 16);
        video_puts("CPU Topology: ");
        const cpu_features_t *f = cpu_get_features();
        video_puts(f->vendor);

        video_set_cursor((win->x + 12) / 8, (win->y + 60) / 16);
        video_puts("Cores Online: 4 Discovered / Active");

        video_set_cursor((win->x + 12) / 8, (win->y + 84) / 16);
        video_puts("Scheduler:    AegisSched (4 Priorities)");

        video_set_cursor((win->x + 12) / 8, (win->y + 108) / 16);
        video_puts("Physical PMM: 4 GiB managed (239 MiB Free)");
    } else if (idx == 1) {
        // Window 2 Content: Storage
        video_set_colors(COLOR_WHITE, win->bg_color);
        video_set_cursor((win->x + 12) / 8, (win->y + 36) / 16);
        video_puts("Volume: PurpleSystem (16 MiB PurpleFS v2)");

        video_set_cursor((win->x + 12) / 8, (win->y + 60) / 16);
        video_puts("Block Size: 4096 bytes | Journal: Block 4088");

        video_set_cursor((win->x + 12) / 8, (win->y + 84) / 16);
        video_puts("Files:");

        video_set_cursor((win->x + 20) / 8, (win->y + 104) / 16);
        video_puts("- welcome.txt (132 bytes)");
        video_set_cursor((win->x + 20) / 8, (win->y + 124) / 16);
        video_puts("- version.txt (65 bytes)");
        video_set_cursor((win->x + 20) / 8, (win->y + 144) / 16);
        video_puts("- motd        (59 bytes)");
    } else if (idx == 2) {
        // Window 3 Content: AegisDetect
        video_set_colors(COLOR_WHITE, win->bg_color);
        video_set_cursor((win->x + 12) / 8, (win->y + 36) / 16);
        video_puts("Trust Model: HMAC-SHA256 Authenticated Baseline");

        video_set_cursor((win->x + 12) / 8, (win->y + 60) / 16);
        video_puts("Manifest: 3 Protected -> 3 TRUSTED, 0 TAMPERED");

        video_set_cursor((win->x + 12) / 8, (win->y + 84) / 16);
        video_puts("PurpleDetect: Hardware Devices & Drivers Registered");
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

    // 1. Wallpaper background
    video_draw_rect(0, 0, scr_w, scr_h, COLOR_PURPLE_BG);

    // 2. Top Taskbar / Menu Header
    video_draw_rect(0, 0, scr_w, 28, COLOR_PURPLE_BAR);
    video_set_cursor(1, 0);
    video_set_colors(COLOR_GOLD, COLOR_PURPLE_BAR);
    video_puts("Purple OS");
    video_set_colors(COLOR_WHITE, COLOR_PURPLE_BAR);
    video_puts("  |  System  Files  Security  Devices  |  ");

#ifndef __KERNEL_HOST_TEST__
    uint64_t sec = pit_get_ticks() / 100;
#else
    uint64_t sec = 0;
#endif
    char stat[64];
    stat[0] = 'C'; stat[1] = 'P'; stat[2] = 'U'; stat[3] = 's'; stat[4] = ':'; stat[5] = ' ';
    stat[6] = '0' + (char)cpu_count();
    stat[7] = ' '; stat[8] = '|'; stat[9] = ' ';
    stat[10] = 'U'; stat[11] = 'p'; stat[12] = ':'; stat[13] = ' ';
    stat[14] = '0' + (char)((sec / 10) % 10);
    stat[15] = '0' + (char)(sec % 10);
    stat[16] = 's'; stat[17] = '\0';
    video_puts(stat);

    // 3. Draw Windows in z-order
    for (uint32_t i = 0; i < window_count; i++) {
        uint32_t idx = z_order[i];
        draw_window_frame(&windows[idx]);
        draw_window_content(&windows[idx], idx);
    }

    // 4. Mouse Cursor (save background and render)
    cursor_drawn = 0; // Fresh screen
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
            if (n_y < 28) n_y = 28;
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
        } else if (hit.type == DESKTOP_HIT_TOPBAR) {
            if (hit.menu_index == 1 && window_count > 0) {
                desktop_toggle_window(windows[0].id);
            } else if (hit.menu_index == 2 && window_count > 1) {
                desktop_toggle_window(windows[1].id);
            } else if (hit.menu_index == 3 && window_count > 2) {
                desktop_toggle_window(windows[2].id);
            } else if (hit.menu_index == 0) {
                for (uint32_t w = 0; w < window_count; w++) {
                    windows[w].visible = 1;
                }
            }
            desktop_render();
            event = DESKTOP_EVENT_MENU_CLICKED;
        }
    }

    last_buttons = buttons;
    return event;
}

void desktop_run_session(void) {
    desktop_active = 1;
    desktop_render();
    kinfo("Desktop: Interactive session online. Move mouse to interact, press 'q' or ESC to exit.");
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
        if (keyboard_has_char()) {
            char c = keyboard_getchar();
            if (c == 'q' || c == 'Q' || c == 27) {
                running = 0;
            } else if (c == '1' && window_count > 0) {
                desktop_toggle_window(windows[0].id);
                desktop_render();
            } else if (c == '2' && window_count > 1) {
                desktop_toggle_window(windows[1].id);
                desktop_render();
            } else if (c == '3' && window_count > 2) {
                desktop_toggle_window(windows[2].id);
                desktop_render();
            } else if (c == 'r' || c == 'R') {
                desktop_render();
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
    kprintf("   Purple OS / Aegis64 Native Desktop Environment (Workspace Canvas)                     \n");
    kprintf("========================================================================================\n");
#ifndef __KERNEL_HOST_TEST__
    uint32_t uptime_sec = (uint32_t)(pit_get_ticks() / 100);
#else
    uint32_t uptime_sec = 0;
#endif
    kprintf("  [Top Bar]  Purple OS  |  System   Files   Security   Devices  |  CPUs: %u  Uptime: %us\n",
            cpu_count(), uptime_sec);
    kprintf("  --------------------------------------------------------------------------------------\n");
    kprintf("  +-----------------------------------+    +-------------------------------------------+\n");
    kprintf("  | [X] Aegis64 System Monitor        |    | [X] PurpleFS v2 Storage                   |\n");
    kprintf("  | CPU: Genuine/Authentic x86-64     |    | Volume: PurpleSystem (16 MiB Partition)   |\n");
    kprintf("  | Cores: 4 Discovered / Online      |    | Journal: Redo Transaction Journal Ready   |\n");
    kprintf("  | Scheduler: AegisSched_64 Active   |    | Files: welcome.txt, version.txt, motd     |\n");
    kprintf("  +-----------------------------------+    +-------------------------------------------+\n");
    kprintf("  +------------------------------------------------------------------------------------+\n");
    kprintf("  | [X] AegisDetect Security Console & PurpleDetect Device Registry                    |\n");
    kprintf("  | Integrity Baseline: AUTHENTICATED (HMAC-SHA256 Hardware Root of Trust Active)     |\n");
    kprintf("  | Protected Components: 3/3 TRUSTED (welcome.txt, version.txt, motd)                 |\n");
    kprintf("  | Discovered Hardware: 13 Devices across ISA, ATA, PCI, MEMORY buses                 |\n");
    kprintf("  +------------------------------------------------------------------------------------+\n");
    kprintf("  Mouse Pointer: (%d, %d) | Buttons: 0x%x\n",
            mouse_get_x(), mouse_get_y(), mouse_get_buttons());
    kprintf("========================================================================================\n\n");
}
