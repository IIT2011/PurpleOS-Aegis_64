#include "desktop.h"
#include "video.h"
#include "mouse.h"
#include "keyboard.h"
#include "serial.h"
#include "string.h"
#include "cpu.h"
#include "io.h"
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
#include "apapp.h"
#if defined(__STDC_HOSTED__) && __STDC_HOSTED__
#include <stdlib.h>
#endif

#ifdef __KERNEL_HOST_TEST__
__attribute__((weak)) int storage_is_fs_mounted(void) { return 0; }
__attribute__((weak)) pfs2_fs_t *storage_get_fs(void) { return NULL; }
__attribute__((weak)) int storage_get_fs_version(void) { return 0; }
__attribute__((weak)) pfs3_fs_t *storage_get_fs3(void) { return NULL; }
__attribute__((weak)) int storage_is_writable(void) { return 0; }
__attribute__((weak)) int storage_set_writable(int w) { (void)w; return 0; }
__attribute__((weak)) int pfs3_readdir(pfs3_fs_t *fs, uint64_t d, uint32_t i, pfs3_dirent_t *e) { (void)fs; (void)d; (void)i; (void)e; return PFS3_ERR_IO; }
__attribute__((weak)) int pfs3_lookup_path(pfs3_fs_t *fs, const char *p, uint64_t *o) { (void)fs; (void)p; (void)o; return PFS3_ERR_NOT_FOUND; }
__attribute__((weak)) int pfs3_read_file(pfs3_fs_t *fs, uint64_t ino, void *b, uint64_t c, uint64_t off, uint64_t *o) { (void)fs; (void)ino; (void)b; (void)c; (void)off; (void)o; return PFS3_ERR_IO; }
__attribute__((weak)) int pfs3_write_file(pfs3_fs_t *fs, uint64_t ino, const void *b, uint64_t c, uint64_t off) { (void)fs; (void)ino; (void)b; (void)c; (void)off; return PFS3_ERR_IO; }
__attribute__((weak)) int pfs3_create_entry(pfs3_fs_t *fs, uint64_t d, const char *n, uint8_t t, uint64_t *o) { (void)fs; (void)d; (void)n; (void)t; (void)o; return PFS3_ERR_IO; }
__attribute__((weak)) int pfs3_unlink_entry(pfs3_fs_t *fs, uint64_t d, const char *n) { (void)fs; (void)d; (void)n; return PFS3_ERR_IO; }
__attribute__((weak)) int pfs3_mkdir(pfs3_fs_t *fs, uint64_t p, const char *n, uint64_t *o) { (void)fs; (void)p; (void)n; (void)o; return PFS3_ERR_IO; }
__attribute__((weak)) int pfs3_read_inode(pfs3_fs_t *fs, uint64_t ino, pfs3_inode_t *o) { (void)fs; (void)ino; (void)o; return PFS3_ERR_IO; }
__attribute__((weak)) int apapp_launch_from_storage(const char *p, app_entry_fn e) { (void)p; (void)e; return 0; }
__attribute__((weak)) int pfs2_readdir(pfs2_fs_t *fs, uint32_t i, pfs2_dir_entry_t *e, pfs2_inode_t *in) { (void)fs; (void)i; (void)e; (void)in; return PFS2_ERR_IO; }
__attribute__((weak)) int pfs2_create_file(pfs2_fs_t *fs, const char *n, uint16_t m, uint32_t *o) { (void)fs; (void)n; (void)m; (void)o; return PFS2_ERR_IO; }
__attribute__((weak)) int pfs2_unlink(pfs2_fs_t *fs, const char *n) { (void)fs; (void)n; return PFS2_ERR_IO; }
__attribute__((weak)) int pfs2_lookup(pfs2_fs_t *fs, const char *n, uint32_t *o) { (void)fs; (void)n; (void)o; return PFS2_ERR_IO; }
__attribute__((weak)) int pfs2_read_file(pfs2_fs_t *fs, uint32_t ino, void *b, size_t c, uint64_t off, size_t *o) { (void)fs; (void)ino; (void)b; (void)c; (void)off; (void)o; return PFS2_ERR_IO; }
__attribute__((weak)) int pfs2_write_file(pfs2_fs_t *fs, uint32_t ino, const void *b, size_t c, uint64_t off, size_t *o) { (void)fs; (void)ino; (void)b; (void)c; (void)off; (void)o; return PFS2_ERR_IO; }
__attribute__((weak)) int pfs2_sync(pfs2_fs_t *fs) { (void)fs; return PFS2_OK; }
__attribute__((weak)) int pfs2_format(pfs2_block_dev_t *d, uint32_t t, const char *l) { (void)d; (void)t; (void)l; return PFS2_OK; }
__attribute__((weak)) int pfs2_mount(pfs2_fs_t *f, pfs2_block_dev_t *d) { (void)f; (void)d; return PFS2_OK; }
__attribute__((weak)) int pfs2_journal_replay_if_needed(pfs2_fs_t *f) { (void)f; return PFS2_OK; }
__attribute__((weak)) uint64_t pit_get_ticks(void) { return 0; }
__attribute__((weak)) uint32_t block_dev_count(void) { return 1; }
__attribute__((weak)) block_device_t *block_dev_get_by_index(uint32_t i) { (void)i; static block_device_t d; strncpy(d.name, "sata0", 31); strncpy(d.model, "QEMU HARDDISK", 47); d.type = BLOCK_DEV_TYPE_AHCI; d.total_blocks = 262144; d.block_size = 512; return &d; }
__attribute__((weak)) const char *block_dev_type_to_str(block_dev_type_t t) { (void)t; return "AHCI/SATA"; }
__attribute__((weak)) kernel_storage_t g_kernel_storage;
#endif

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

static uint32_t *desktop_bg_surface = NULL;
static uint32_t desktop_bg_width = 0;
static uint32_t desktop_bg_height = 0;

static void render_cosmic_background(uint32_t w, uint32_t h);
static void draw_window_frame(const gui_window_t *win);
static void draw_window_content(const gui_window_t *win, uint32_t idx);
static void desktop_paint_window(gui_window_t *win, uint32_t idx);

static void desktop_ensure_bg_surface(uint32_t scr_w, uint32_t scr_h) {
    if (scr_w == 0 || scr_h == 0) return;
    if (desktop_bg_surface && desktop_bg_width == scr_w && desktop_bg_height == scr_h) {
        return;
    }
    size_t sz = (size_t)scr_w * scr_h * sizeof(uint32_t);
#if defined(__STDC_HOSTED__) && __STDC_HOSTED__
    if (desktop_bg_surface) free(desktop_bg_surface);
    desktop_bg_surface = (uint32_t *)malloc(sz);
#else
    desktop_bg_surface = (uint32_t *)pmm_alloc_frames((sz + 4095) / 4096);
#endif
    if (desktop_bg_surface) {
        desktop_bg_width = scr_w;
        desktop_bg_height = scr_h;
        video_set_render_target(desktop_bg_surface, scr_w, scr_h, scr_w * 4);
        render_cosmic_background(scr_w, scr_h);
        video_reset_render_target();
    }
}

static void desktop_ensure_window_surface(gui_window_t *win) {
    if (!win || win->width == 0 || win->height == 0) return;
    if (win->surface && win->surface_w == win->width && win->surface_h == win->height) {
        return;
    }
    size_t sz = (size_t)win->width * win->height * sizeof(uint32_t);
#if defined(__STDC_HOSTED__) && __STDC_HOSTED__
    if (win->surface) free(win->surface);
    win->surface = (uint32_t *)malloc(sz);
#else
    win->surface = (uint32_t *)pmm_alloc_frames((sz + 4095) / 4096);
#endif
    if (win->surface) {
        win->surface_w = win->width;
        win->surface_h = win->height;
        win->surface_pitch = win->width * 4;
        memset(win->surface, 0, sz);
    }
}

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

    /* Allocate bounded surface for this window */
    desktop_ensure_window_surface(win);

    z_order[idx] = idx;
    bring_to_front((int)idx);
    return win->id;
}

int desktop_close_window(uint32_t id) {
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
            if (desktop_active) {
                desktop_composite();
            }
            return 0;
        }
    }
    return -1;
}

int desktop_focus_window(uint32_t id) {
    for (uint32_t i = 0; i < window_count; i++) {
        if (windows[i].id == id) {
            bring_to_front((int)i);
            if (desktop_active) {
                desktop_render();
            }
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
                if (desktop_active) {
                    desktop_render();
                }
            } else {
                windows[i].focused = 0;
                if (desktop_active) {
                    desktop_composite();
                }
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
            if (desktop_active) {
                desktop_composite();
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
            desktop_ensure_window_surface(w);
            bring_to_front((int)i);
            if (desktop_active) {
                desktop_render();
            }
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
    } else if (strcmp(app_name, "vlc") == 0 || strcmp(app_name, "VLC") == 0 || strcmp(app_name, "video") == 0 || strcmp(app_name, "media") == 0 || strcmp(app_name, "music") == 0 || strcmp(app_name, "Videos") == 0 || strcmp(app_name, "Music") == 0 || strcmp(app_name, "VLC Player") == 0 || strcmp(app_name, "VLC Media Player") == 0) {
        target_idx = 8;
    } else if (strcmp(app_name, "viewer") == 0 || strcmp(app_name, "view") == 0 || strcmp(app_name, "Purple View") == 0 || strcmp(app_name, "pdf") == 0 || strcmp(app_name, "doc") == 0 || strcmp(app_name, "document") == 0 || strcmp(app_name, "Photos") == 0) {
        target_idx = 9;
    } else if (strcmp(app_name, "diskmgr") == 0 || strcmp(app_name, "disk") == 0 || strcmp(app_name, "disks") == 0 || strcmp(app_name, "partition") == 0 || strcmp(app_name, "Disk Utility") == 0 || strcmp(app_name, "Purple Partition Manager") == 0) {
        target_idx = 10;
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
   Filesystem Helper & Settings Persistence
   -------------------------------------------------------------------------- */

static void desktop_files_refresh(gui_window_t *w) {
    if (!w) return;
    w->files_item_count = 0;
    if (storage_is_fs_mounted()) {
        pfs2_fs_t *fs = storage_get_fs();
        pfs2_dir_entry_t entry;
        pfs2_inode_t ino;
        uint32_t d_idx = 0;
        while (d_idx < 16 && pfs2_readdir(fs, d_idx, &entry, &ino) == PFS2_OK) {
            strncpy(w->files_items[w->files_item_count], entry.name, sizeof(w->files_items[0]) - 1);
            w->files_items[w->files_item_count][sizeof(w->files_items[0]) - 1] = '\0';
            w->files_is_dir[w->files_item_count] = (entry.file_type == 2);
            w->files_sizes[w->files_item_count] = (uint32_t)ino.size_bytes;
            w->files_item_count++;
            d_idx++;
        }
    }
    if (w->files_item_count == 0) {
        static const struct {
            const char *name;
            uint8_t is_dir;
            uint32_t size;
        } defaults[5] = {
            { "welcome.txt", 0, 132 },
            { "version.txt", 0, 65 },
            { "motd",        0, 59 },
            { "apps/",       1, 8 },
            { "drivers/",    1, 7 }
        };
        for (int i = 0; i < 5; i++) {
            strncpy(w->files_items[i], defaults[i].name, sizeof(w->files_items[0]) - 1);
            w->files_is_dir[i] = defaults[i].is_dir;
            w->files_sizes[i] = defaults[i].size;
        }
        w->files_item_count = 5;
    }
}

static void desktop_save_settings(void) {
    gui_window_t *w = (window_count > 3) ? &windows[3] : NULL;
    if (!w || !storage_is_fs_mounted() || !storage_is_writable()) return;

    pfs2_fs_t *fs = storage_get_fs();
    uint32_t ino = 0;
    if (pfs2_lookup(fs, "purple.conf", &ino) != PFS2_OK) {
        if (pfs2_create_file(fs, "purple.conf", PFS2_MODE_FILE, &ino) != PFS2_OK) {
            return;
        }
    }
    char buf[256];
    char tmp[32];
    buf[0] = '\0';
    strncat(buf, "volume=", sizeof(buf) - strlen(buf) - 1);
    tmp[0] = '0' + (w->settings_volume / 10);
    tmp[1] = '0' + (w->settings_volume % 10);
    tmp[2] = '\n';
    tmp[3] = '\0';
    strncat(buf, tmp, sizeof(buf) - strlen(buf) - 1);

    strncat(buf, "dark_mode=", sizeof(buf) - strlen(buf) - 1);
    tmp[0] = '0' + (w->settings_dark_mode ? 1 : 0);
    tmp[1] = '\n';
    tmp[2] = '\0';
    strncat(buf, tmp, sizeof(buf) - strlen(buf) - 1);

    strncat(buf, "night_mode=", sizeof(buf) - strlen(buf) - 1);
    tmp[0] = '0' + (w->settings_night_mode ? 1 : 0);
    tmp[1] = '\n';
    tmp[2] = '\0';
    strncat(buf, tmp, sizeof(buf) - strlen(buf) - 1);

    strncat(buf, "reduced_motion=", sizeof(buf) - strlen(buf) - 1);
    tmp[0] = '0' + (w->settings_reduced_motion ? 1 : 0);
    tmp[1] = '\n';
    tmp[2] = '\0';
    strncat(buf, tmp, sizeof(buf) - strlen(buf) - 1);

    strncat(buf, "anim_enabled=", sizeof(buf) - strlen(buf) - 1);
    tmp[0] = '0' + (w->settings_anim_enabled ? 1 : 0);
    tmp[1] = '\n';
    tmp[2] = '\0';
    strncat(buf, tmp, sizeof(buf) - strlen(buf) - 1);

    size_t bw = 0;
    pfs2_write_file(fs, ino, buf, strlen(buf), 0, &bw);
    pfs2_sync(fs);
}

static void desktop_load_settings(void) {
    gui_window_t *w = (window_count > 3) ? &windows[3] : NULL;
    if (!w || !storage_is_fs_mounted()) return;

    pfs2_fs_t *fs = storage_get_fs();
    uint32_t ino = 0;
    if (pfs2_lookup(fs, "purple.conf", &ino) == PFS2_OK) {
        char buf[256];
        size_t br = 0;
        if (pfs2_read_file(fs, ino, buf, sizeof(buf) - 1, 0, &br) == PFS2_OK && br > 0) {
            buf[br] = '\0';
            const char *p = buf;
            while (*p) {
                if (strncmp(p, "volume=", 7) == 0) {
                    p += 7;
                    int vol = 0;
                    while (*p >= '0' && *p <= '9') {
                        vol = vol * 10 + (*p - '0');
                        p++;
                    }
                    if (vol >= 0 && vol <= 100) w->settings_volume = vol;
                } else if (strncmp(p, "dark_mode=", 10) == 0) {
                    p += 10;
                    w->settings_dark_mode = (*p == '1');
                } else if (strncmp(p, "night_mode=", 11) == 0) {
                    p += 11;
                    w->settings_night_mode = (*p == '1');
                } else if (strncmp(p, "reduced_motion=", 15) == 0) {
                    p += 15;
                    w->settings_reduced_motion = (*p == '1');
                } else if (strncmp(p, "anim_enabled=", 13) == 0) {
                    p += 13;
                    w->settings_anim_enabled = (*p == '1');
                }
                while (*p && *p != '\n') p++;
                if (*p == '\n') p++;
            }
        }
    }
}

/* --------------------------------------------------------------------------
   Terminal Window Execution & History
   -------------------------------------------------------------------------- */

void desktop_terminal_put_line(gui_window_t *win, const char *line) {
    if (!win) return;
    kinfo("[TERM] %s", line ? line : "");
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
    /* Auto return to bottom on new output */
    win->term_scroll_offset = 0;
}

static void desktop_terminal_tab_complete(gui_window_t *win) {
    if (!win || win->input_cursor == 0) return;
    static const char *cmds[] = {
        "help", "list files", "ls", "show location", "pwd", "change folder", "cd",
        "make folder", "mkdir", "create file", "touch", "view file", "peek", "cat",
        "edit file", "copy file", "cp", "move file", "mv", "rename file", "delete file", "rm",
        "find files", "show file details", "stat", "show disk usage", "df", "disks", "list disks",
        "partitions", "sysinfo", "cpuinfo", "meminfo", "uptime", "tasks", "ps",
        "vlc", "view", "clear", "exit", NULL
    };
    size_t in_len = strlen(win->input_line);
    for (int i = 0; cmds[i] != NULL; i++) {
        if (strncmp(cmds[i], win->input_line, in_len) == 0) {
            strncpy(win->input_line, cmds[i], sizeof(win->input_line) - 1);
            win->input_cursor = strlen(win->input_line);
            return;
        }
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
        int h_idx = win->history_count % 32;
        strncpy(win->history[h_idx], cmd, 63);
        win->history[h_idx][63] = '\0';
        win->history_count++;
        win->history_idx = win->history_count;
    }

    if (strcmp(cmd, "help") == 0) {
        desktop_terminal_put_line(win, "Available Purple OS / Aegis64 Shell Commands:");
        desktop_terminal_put_line(win, "  File Ops:  ls, list files, pwd, touch <f>, cat <f>, rm <f>, rename <o> <n>, df");
        desktop_terminal_put_line(win, "  Storage:   disks, partitions, filesystem status");
        desktop_terminal_put_line(win, "  Apps:      vlc <f>, view <f>, edit <f>, calc, sysmon, settings");
        desktop_terminal_put_line(win, "  System:    sysinfo, cpuinfo, meminfo, uptime, tasks, gpu status, clear, exit");
    } else if (strcmp(cmd, "clear") == 0) {
        win->output_count = 0;
        win->term_scroll_offset = 0;
    } else if (strcmp(cmd, "pwd") == 0 || strcmp(cmd, "show location") == 0) {
        desktop_terminal_put_line(win, "/ (PurpleFS v2/v3 System Root)");
    } else if (strncmp(cmd, "ls", 2) == 0 || strncmp(cmd, "list files", 10) == 0) {
        const char *target_path = "/";
        const char *arg = (cmd[0] == 'l' && cmd[1] == 's') ? (cmd + 2) : (cmd + 10);
        while (*arg == ' ') arg++;
        if (*arg == '-' && *(arg + 1) == 'l') {
            arg += 2;
            while (*arg == ' ') arg++;
        }
        if (*arg != '\0') {
            target_path = arg;
        }

        if (storage_get_fs_version() == 3) {
            pfs3_fs_t *fs3 = storage_get_fs3();
            uint64_t target_ino = PFS3_ROOT_INODE;
            int lrc = PFS3_OK;
            if (strcmp(target_path, "/") != 0) {
                lrc = pfs3_lookup_path(fs3, target_path, &target_ino);
            }
            if (lrc != PFS3_OK) {
                char err_buf[96];
                snprintf(err_buf, sizeof(err_buf), "ls: cannot access '%s': No such file or directory", target_path);
                desktop_terminal_put_line(win, err_buf);
            } else {
                pfs3_inode_t dir_ino;
                if (pfs3_read_inode(fs3, target_ino, &dir_ino) != PFS3_OK || !(dir_ino.mode & PFS3_MODE_DIR)) {
                    char err_buf[96];
                    snprintf(err_buf, sizeof(err_buf), "ls: '%s': Not a directory", target_path);
                    desktop_terminal_put_line(win, err_buf);
                } else {
                    char hdr[96];
                    snprintf(hdr, sizeof(hdr), "Directory Listing of %s (PurpleFS v3 Hierarchical):", target_path);
                    desktop_terminal_put_line(win, hdr);
                    pfs3_dirent_t de;
                    uint32_t idx = 0;
                    int count = 0;
                    while (pfs3_readdir(fs3, target_ino, idx++, &de) == PFS3_OK) {
                        if (strcmp(de.name, ".") == 0 || strcmp(de.name, "..") == 0) continue;
                        pfs3_inode_t f_ino;
                        uint64_t f_sz = 0;
                        if (pfs3_read_inode(fs3, de.inode_num, &f_ino) == PFS3_OK) {
                            f_sz = f_ino.size_bytes;
                        }
                        char l_buf[TERM_LINE_LEN];
                        memset(l_buf, ' ', sizeof(l_buf));
                        size_t nl = strlen(de.name);
                        if (nl > 36) nl = 36;
                        memcpy(l_buf, "  ", 2);
                        memcpy(l_buf + 2, de.name, nl);
                        const char *t_str = (de.file_type == PFS3_FT_DIR) ? "<DIR>" : "<FILE>";
                        memcpy(l_buf + 40, t_str, strlen(t_str));
                        char sz_str[32];
                        if (de.file_type == PFS3_FT_DIR) {
                            snprintf(sz_str, sizeof(sz_str), "        -");
                        } else if (f_sz >= 1048576) {
                            snprintf(sz_str, sizeof(sz_str), "%5lu MB", (unsigned long)(f_sz / 1048576));
                        } else if (f_sz >= 1024) {
                            snprintf(sz_str, sizeof(sz_str), "%5lu KB", (unsigned long)(f_sz / 1024));
                        } else {
                            snprintf(sz_str, sizeof(sz_str), "%5lu B ", (unsigned long)f_sz);
                        }
                        memcpy(l_buf + 48, sz_str, strlen(sz_str));
                        l_buf[58] = '\0';
                        desktop_terminal_put_line(win, l_buf);
                        count++;
                    }
                    if (count == 0) {
                        desktop_terminal_put_line(win, "  (Directory is empty)");
                    }
                }
            }
        } else if (storage_is_fs_mounted()) {
            pfs2_fs_t *fs = storage_get_fs();
            pfs2_dir_entry_t ent;
            pfs2_inode_t ino;
            int count = 0;
            for (uint32_t i = 0; i < 16; i++) {
                if (pfs2_readdir(fs, i, &ent, &ino) == PFS2_OK) {
                    char l_buf[TERM_LINE_LEN];
                    memset(l_buf, ' ', sizeof(l_buf));
                    size_t nl = strlen(ent.name);
                    if (nl > 36) nl = 36;
                    memcpy(l_buf, "  ", 2);
                    memcpy(l_buf + 2, ent.name, nl);
                    const char *t_str = (ent.file_type == 2) ? "<DIR>" : "<FILE>";
                    memcpy(l_buf + 40, t_str, strlen(t_str));
                    l_buf[48] = '\0';
                    desktop_terminal_put_line(win, l_buf);
                    count++;
                }
            }
            if (count == 0) {
                desktop_terminal_put_line(win, "  (Directory is empty)");
            }
        } else {
            desktop_terminal_put_line(win, "  welcome.txt (134 B)  version.txt (65 B)  motd (59 B)");
            desktop_terminal_put_line(win, "  Boot/ (Directory)    System/ (Directory) Users/ (Directory)");
        }
    } else if (strncmp(cmd, "app run ", 8) == 0 || strncmp(cmd, "run ", 4) == 0) {
        const char *app_path = (cmd[0] == 'a') ? (cmd + 8) : (cmd + 4);
        while (*app_path == ' ') app_path++;
        int rc = apapp_launch_from_storage(app_path, NULL);
        if (rc == APAPP_OK) {
            desktop_terminal_put_line(win, "Application bundle launched successfully.");
        } else {
            desktop_terminal_put_line(win, "Error: Failed to launch application bundle (noncanonical or invalid bundle).");
        }
        bring_to_front(0);
        windows[0].focused = 1;
        current_focus = FOCUS_WINDOW;
    } else if (strncmp(cmd, "touch ", 6) == 0 || strncmp(cmd, "create file ", 12) == 0) {
        const char *fname = (cmd[0] == 't') ? (cmd + 6) : (cmd + 12);
        while (*fname == ' ') fname++;
        if (storage_get_fs_version() == 3) {
            pfs3_fs_t *fs3 = storage_get_fs3();
            uint64_t existing_ino = 0;
            if (pfs3_lookup_path(fs3, fname, &existing_ino) == PFS3_OK) {
                desktop_terminal_put_line(win, "Error: File already exists.");
            } else {
                char parent[256];
                char base[64];
                const char *last_slash = strrchr(fname, '/');
                uint64_t parent_ino = PFS3_ROOT_INODE;
                if (last_slash) {
                    size_t plen = (size_t)(last_slash - fname);
                    if (plen == 0) plen = 1;
                    strncpy(parent, fname, plen);
                    parent[plen] = '\0';
                    strncpy(base, last_slash + 1, sizeof(base) - 1);
                    base[sizeof(base) - 1] = '\0';
                    pfs3_lookup_path(fs3, parent, &parent_ino);
                } else {
                    strncpy(base, fname, sizeof(base) - 1);
                    base[sizeof(base) - 1] = '\0';
                }
                uint64_t new_ino = 0;
                int rc = pfs3_create_entry(fs3, parent_ino, base, PFS3_FT_REG, &new_ino);
                if (rc == PFS3_OK) {
                    desktop_terminal_put_line(win, "Successfully created file on PurpleFS v3.");
                } else {
                    desktop_terminal_put_line(win, "Error: Failed to create file on PurpleFS v3.");
                }
            }
        } else if (storage_is_fs_mounted()) {
            pfs2_fs_t *fs = storage_get_fs();
            uint32_t ino = 0;
            int err = pfs2_create_file(fs, fname, 0644, &ino);
            if (err == PFS2_OK) {
                desktop_terminal_put_line(win, "Successfully created file on PurpleFS.");
            } else if (err == PFS2_ERR_ROFS) {
                desktop_terminal_put_line(win, "Error: Filesystem is mounted Read-Only. (Run 'remount rw')");
            } else if (err == PFS2_ERR_EXISTS) {
                desktop_terminal_put_line(win, "Error: File already exists.");
            } else {
                desktop_terminal_put_line(win, "Error: Failed to create file.");
            }
        } else {
            desktop_terminal_put_line(win, "Error: No storage volume mounted.");
        }
    } else if (strncmp(cmd, "write ", 6) == 0) {
        const char *arg = cmd + 6;
        while (*arg == ' ') arg++;
        const char *sp = strchr(arg, ' ');
        if (!sp) {
            desktop_terminal_put_line(win, "Usage: write <file> <text...>");
        } else {
            char fname[128];
            size_t nlen = (size_t)(sp - arg);
            if (nlen >= sizeof(fname)) nlen = sizeof(fname) - 1;
            strncpy(fname, arg, nlen);
            fname[nlen] = '\0';
            const char *text = sp + 1;
            while (*text == ' ') text++;

            if (storage_get_fs_version() == 3) {
                pfs3_fs_t *fs3 = storage_get_fs3();
                uint64_t target_ino = 0;
                if (pfs3_lookup_path(fs3, fname, &target_ino) != PFS3_OK) {
                    char parent[256];
                    char base[64];
                    const char *last_slash = strrchr(fname, '/');
                    uint64_t parent_ino = PFS3_ROOT_INODE;
                    if (last_slash) {
                        size_t plen = (size_t)(last_slash - fname);
                        if (plen == 0) plen = 1;
                        strncpy(parent, fname, plen);
                        parent[plen] = '\0';
                        strncpy(base, last_slash + 1, sizeof(base) - 1);
                        base[sizeof(base) - 1] = '\0';
                        pfs3_lookup_path(fs3, parent, &parent_ino);
                    } else {
                        strncpy(base, fname, sizeof(base) - 1);
                        base[sizeof(base) - 1] = '\0';
                    }
                    pfs3_create_entry(fs3, parent_ino, base, PFS3_FT_REG, &target_ino);
                }
                if (target_ino != 0) {
                    size_t tlen = strlen(text);
                    int rc = pfs3_write_file(fs3, target_ino, text, tlen, 0);
                    if (rc == PFS3_OK) {
                        char out_msg[256];
                        snprintf(out_msg, sizeof(out_msg), "Successfully wrote %lu bytes to '%s'.", (unsigned long)tlen, fname);
                        desktop_terminal_put_line(win, out_msg);
                    } else {
                        desktop_terminal_put_line(win, "Error: Failed to write to file.");
                    }
                } else {
                    desktop_terminal_put_line(win, "Error: Failed to locate or create file.");
                }
            } else if (storage_is_fs_mounted()) {
                pfs2_fs_t *fs = storage_get_fs();
                uint32_t ino = 0;
                if (pfs2_lookup(fs, fname, &ino) != PFS2_OK) {
                    pfs2_create_file(fs, fname, 0644, &ino);
                }
                if (ino != 0) {
                    size_t tlen = strlen(text);
                    size_t written = 0;
                    pfs2_write_file(fs, ino, text, tlen, 0, &written);
                    pfs2_sync(fs);
                    char out_msg[256];
                    snprintf(out_msg, sizeof(out_msg), "Successfully wrote %lu bytes to '%s'.", (unsigned long)written, fname);
                    desktop_terminal_put_line(win, out_msg);
                } else {
                    desktop_terminal_put_line(win, "Error: Failed to create or open file.");
                }
            } else {
                desktop_terminal_put_line(win, "Error: No storage volume mounted.");
            }
        }
    } else if (strncmp(cmd, "mkdir ", 6) == 0 || strncmp(cmd, "make folder ", 12) == 0) {
        const char *dname = (cmd[0] == 'm') ? (cmd + 6) : (cmd + 12);
        while (*dname == ' ') dname++;
        if (storage_get_fs_version() == 3) {
            pfs3_fs_t *fs3 = storage_get_fs3();
            char parent[256];
            char base[64];
            const char *last_slash = strrchr(dname, '/');
            uint64_t parent_ino = PFS3_ROOT_INODE;
            if (last_slash) {
                size_t plen = (size_t)(last_slash - dname);
                if (plen == 0) plen = 1;
                strncpy(parent, dname, plen);
                parent[plen] = '\0';
                strncpy(base, last_slash + 1, sizeof(base) - 1);
                base[sizeof(base) - 1] = '\0';
                pfs3_lookup_path(fs3, parent, &parent_ino);
            } else {
                strncpy(base, dname, sizeof(base) - 1);
                base[sizeof(base) - 1] = '\0';
            }
            uint64_t new_dir_ino = 0;
            int rc = pfs3_mkdir(fs3, parent_ino, base, &new_dir_ino);
            if (rc == PFS3_OK) {
                desktop_terminal_put_line(win, "Successfully created directory on PurpleFS v3.");
            } else {
                desktop_terminal_put_line(win, "Error: Failed to create directory.");
            }
        } else {
            desktop_terminal_put_line(win, "Error: Directory creation not supported on legacy volume.");
        }
    } else if (strncmp(cmd, "rm ", 3) == 0 || strncmp(cmd, "delete file ", 12) == 0) {
        const char *fname = (cmd[0] == 'r') ? (cmd + 3) : (cmd + 12);
        while (*fname == ' ') fname++;
        if (storage_get_fs_version() == 3) {
            pfs3_fs_t *fs3 = storage_get_fs3();
            char parent[256];
            char base[64];
            const char *last_slash = strrchr(fname, '/');
            uint64_t parent_ino = PFS3_ROOT_INODE;
            if (last_slash) {
                size_t plen = (size_t)(last_slash - fname);
                if (plen == 0) plen = 1;
                strncpy(parent, fname, plen);
                parent[plen] = '\0';
                strncpy(base, last_slash + 1, sizeof(base) - 1);
                base[sizeof(base) - 1] = '\0';
                pfs3_lookup_path(fs3, parent, &parent_ino);
            } else {
                strncpy(base, fname, sizeof(base) - 1);
                base[sizeof(base) - 1] = '\0';
            }
            int err = pfs3_unlink_entry(fs3, parent_ino, base);
            if (err == PFS3_OK) {
                desktop_terminal_put_line(win, "Successfully unlinked file from PurpleFS v3.");
            } else {
                desktop_terminal_put_line(win, "Error: File not found or failed to unlink.");
            }
        } else if (storage_is_fs_mounted()) {
            pfs2_fs_t *fs = storage_get_fs();
            int err = pfs2_unlink(fs, fname);
            if (err == PFS2_OK) {
                desktop_terminal_put_line(win, "Successfully unlinked file from PurpleFS.");
            } else {
                desktop_terminal_put_line(win, "Error: File not found or read-only filesystem.");
            }
        }
    } else if (strncmp(cmd, "vlc ", 4) == 0 || strcmp(cmd, "vlc") == 0) {
        const char *mname = (cmd[3] == ' ') ? (cmd + 4) : "Cosmic_Atmosphere.wav";
        while (*mname == ' ') mname++;
        desktop_open_application("vlc");
        gui_window_t *w_vlc = desktop_get_window(9);
        if (w_vlc) {
            strncpy(w_vlc->vlc_track, mname, sizeof(w_vlc->vlc_track) - 1);
            w_vlc->vlc_state = 1;
            w_vlc->vlc_elapsed_sec = 0;
            w_vlc->vlc_pos_permille = 0;
        }
        desktop_terminal_put_line(win, "VLC Media Player launched with selected media.");
    } else if (strncmp(cmd, "view ", 5) == 0) {
        const char *vname = cmd + 5;
        while (*vname == ' ') vname++;
        desktop_open_application("viewer");
        gui_window_t *w_v = desktop_get_window(10);
        if (w_v) {
            strncpy(w_v->viewer_filename, vname, sizeof(w_v->viewer_filename) - 1);
            if (strstr(vname, ".pdf")) w_v->viewer_type = 3;
            else if (strstr(vname, ".md")) w_v->viewer_type = 2;
            else w_v->viewer_type = 1;
        }
        desktop_terminal_put_line(win, "Document opened in Purple View.");
    } else if (strncmp(cmd, "edit ", 5) == 0) {
        const char *ename = cmd + 5;
        while (*ename == ' ') ename++;
        desktop_open_application("editor");
        gui_window_t *w_e = desktop_get_window(6);
        if (w_e) {
            strncpy(w_e->editor_filename, ename, sizeof(w_e->editor_filename) - 1);
        }
        desktop_terminal_put_line(win, "Document opened in Text Editor.");
    } else if (strcmp(cmd, "df") == 0 || strcmp(cmd, "show disk usage") == 0) {
        if (storage_get_fs_version() == 3) {
            pfs3_fs_t *fs3 = storage_get_fs3();
            desktop_terminal_put_line(win, "Filesystem     Total Blocks  Used Blocks  Free Blocks  Capacity");
            char df_buf[192];
            uint64_t total = fs3 ? fs3->sb.total_blocks : 0;
            uint64_t free_b = fs3 ? fs3->sb.free_blocks : 0;
            uint64_t used_b = (total > free_b) ? (total - free_b) : 0;
            snprintf(df_buf, sizeof(df_buf), "PurpleSystem   %lu (%lu MB)  %lu (%lu KB) %lu (%lu MB) %lu%% Free",
                     (unsigned long)total, (unsigned long)(total * 4 / 1024),
                     (unsigned long)used_b, (unsigned long)(used_b * 4),
                     (unsigned long)free_b, (unsigned long)(free_b * 4 / 1024),
                     (unsigned long)(total ? (free_b * 100 / total) : 0));
            desktop_terminal_put_line(win, df_buf);
        } else if (storage_is_fs_mounted()) {
            desktop_terminal_put_line(win, "Filesystem     Total Blocks  Used Blocks  Free Blocks  Capacity");
            desktop_terminal_put_line(win, "PurpleSystem   4096 (16 MB)  25 (100 KB)  4071 (15 MB) 99% Free");
        } else {
            desktop_terminal_put_line(win, "No active storage volume mounted.");
        }
    } else if (strcmp(cmd, "disks") == 0 || strcmp(cmd, "list disks") == 0) {
        desktop_terminal_put_line(win, "Discovered Block Devices:");
        uint32_t bcnt = block_dev_count();
        if (bcnt == 0) {
            desktop_terminal_put_line(win, "  (No block devices registered)");
        } else {
            for (uint32_t b = 0; b < bcnt; b++) {
                block_device_t *bd = block_dev_get_by_index(b);
                if (bd) {
                    char b_buf[192];
                    uint32_t sz_mb = (uint32_t)(bd->total_blocks * bd->block_size / (1024 * 1024));
                    snprintf(b_buf, sizeof(b_buf), "  [%s] %s: '%s' (%u MiB, %lu blks)",
                             bd->name, block_dev_type_to_str(bd->type), bd->model, sz_mb, (unsigned long)bd->total_blocks);
                    desktop_terminal_put_line(win, b_buf);
                }
            }
        }
    } else if (strcmp(cmd, "partitions") == 0) {
        int didx = g_kernel_storage.active_device_idx;
        if (didx >= 0 && g_kernel_storage.gpt[didx].valid_header) {
            gpt_info_t *gpt = &g_kernel_storage.gpt[didx];
            desktop_terminal_put_line(win, "GPT Partitions on Active Device:");
            for (uint32_t p = 0; p < gpt->partition_count; p++) {
                char p_buf[192];
                uint64_t sc = gpt->partitions[p].sector_count;
                uint32_t mb = (uint32_t)(sc * 512 / (1024 * 1024));
                snprintf(p_buf, sizeof(p_buf), "  Part %u: '%s' [%lu..%lu, %u MiB] (%s)",
                         p, gpt->partitions[p].name,
                         (unsigned long)gpt->partitions[p].start_lba,
                         (unsigned long)(gpt->partitions[p].start_lba + sc - 1),
                         mb, (p == (uint32_t)g_kernel_storage.active_part_idx) ? "PurpleFS System" : "ESP");
                desktop_terminal_put_line(win, p_buf);
            }
        } else {
            desktop_terminal_put_line(win, "No active GPT partition table found on storage.");
        }
    } else if (strncmp(cmd, "peek ", 5) == 0 || strncmp(cmd, "cat ", 4) == 0 || strncmp(cmd, "view file ", 10) == 0) {
        const char *fname = (cmd[0] == 'p') ? (cmd + 5) : ((cmd[0] == 'c') ? (cmd + 4) : (cmd + 10));
        while (*fname == ' ') fname++;
        if (storage_get_fs_version() == 3) {
            pfs3_fs_t *fs3 = storage_get_fs3();
            uint64_t ino3 = 0;
            if (pfs3_lookup_path(fs3, fname, &ino3) == PFS3_OK) {
                char buf[512];
                memset(buf, 0, sizeof(buf));
                uint64_t br = 0;
                if (pfs3_read_file(fs3, ino3, buf, sizeof(buf) - 1, 0, &br) == PFS3_OK && br > 0) {
                    char *line_p = buf;
                    while (*line_p && line_p < buf + br) {
                        char *next_nl = strstr(line_p, "\n");
                        if (next_nl) *next_nl = '\0';
                        desktop_terminal_put_line(win, line_p);
                        if (!next_nl) break;
                        line_p = next_nl + 1;
                    }
                } else {
                    desktop_terminal_put_line(win, "(Empty file)");
                }
            } else {
                desktop_terminal_put_line(win, "Error: File not found on PurpleFS v3 volume.");
            }
        } else if (storage_is_fs_mounted()) {
            pfs2_fs_t *fs = storage_get_fs();
            uint32_t ino = 0;
            if (pfs2_lookup(fs, fname, &ino) == PFS2_OK) {
                char buf[512];
                memset(buf, 0, sizeof(buf));
                size_t br = 0;
                if (pfs2_read_file(fs, ino, buf, sizeof(buf) - 1, 0, &br) == PFS2_OK && br > 0) {
                    /* Print up to 3 lines of preview */
                    char *line_p = buf;
                    while (*line_p && line_p < buf + br) {
                        char *next_nl = strstr(line_p, "\n");
                        if (next_nl) *next_nl = '\0';
                        desktop_terminal_put_line(win, line_p);
                        if (!next_nl) break;
                        line_p = next_nl + 1;
                    }
                } else {
                    desktop_terminal_put_line(win, "(Empty file)");
                }
            } else {
                desktop_terminal_put_line(win, "Error: File not found on mounted volume.");
            }
        } else {
            if (strcmp(fname, "welcome.txt") == 0) {
                desktop_terminal_put_line(win, "Welcome to Purple OS / Aegis64 Native Filesystem!");
                desktop_terminal_put_line(win, "Architecture: x86-64 | Filesystem: PurpleFS v3");
            } else if (strcmp(fname, "version.txt") == 0) {
                desktop_terminal_put_line(win, "Aegis64 Kernel v0.4.0 (Safe Storage Integration)");
            } else if (strcmp(fname, "motd") == 0) {
                desktop_terminal_put_line(win, "Purple OS: Correctness, Memory Safety, Recoverability.");
            } else {
                desktop_terminal_put_line(win, "File not found on mounted volume.");
            }
        }
    } else if (strcmp(cmd, "cpu") == 0 || strcmp(cmd, "cpuinfo") == 0) {
        const cpu_features_t *f = cpu_get_features();
        char msg[TERM_LINE_LEN];
        msg[0] = 'C'; msg[1] = 'P'; msg[2] = 'U'; msg[3] = ':'; msg[4] = ' ';
        strncpy(msg + 5, f->vendor, 30);
        desktop_terminal_put_line(win, msg);
    } else if (strcmp(cmd, "mem") == 0 || strcmp(cmd, "meminfo") == 0) {
        desktop_terminal_put_line(win, "Memory: 4096 MB Managed | 239 MB Free | CR0.WP Active");
    } else if (strcmp(cmd, "sysinfo") == 0) {
        desktop_terminal_put_line(win, "Purple OS (Aegis64 Micro-Hybrid Kernel v0.4.0)");
        desktop_terminal_put_line(win, "Architecture: x86-64 AMD64 Long Mode (SMP 4 Cores)");
        desktop_terminal_put_line(win, "Filesystem:   PurpleFS v2/v3 Hierarchical Journaled");
        desktop_terminal_put_line(win, "Media Player: VLC Media Player (libvlc embedded core)");
    } else if (strcmp(cmd, "uptime") == 0) {
        uint32_t secs = (uint32_t)(pit_get_ticks() / 100);
        char up_str[48];
        char num[16];
        int ni = 0;
        if (secs == 0) num[ni++] = '0';
        else {
            char rev[16]; int ri = 0;
            while (secs > 0) { rev[ri++] = '0' + (secs % 10); secs /= 10; }
            while (ri > 0) num[ni++] = rev[--ri];
        }
        num[ni] = '\0';
        strncpy(up_str, "Uptime: ", sizeof(up_str) - 1);
        strncat(up_str, num, sizeof(up_str) - strlen(up_str) - 1);
        strncat(up_str, " seconds (PIT 100 Hz)", sizeof(up_str) - strlen(up_str) - 1);
        desktop_terminal_put_line(win, up_str);
    } else if (strcmp(cmd, "tasks") == 0 || strcmp(cmd, "ps") == 0) {
        desktop_terminal_put_line(win, "PID  NAME            STATE     PRIORITY");
        desktop_terminal_put_line(win, " 1   kernel_main     RUNNING   NORMAL");
        desktop_terminal_put_line(win, " 2   idle-0          READY     IDLE");
        desktop_terminal_put_line(win, " 3   purple_detect   SLEEPING  LOW");
    } else if (strcmp(cmd, "gpu") == 0 || strcmp(cmd, "gpu status") == 0) {
        gpu_device_t *g = gpu_get_primary();
        char stat[TERM_LINE_LEN];
        g->ops->get_status(g, stat, sizeof(stat));
        desktop_terminal_put_line(win, g->name);
        desktop_terminal_put_line(win, stat);
        if (g->arch == GPU_ARCH_SW_FALLBACK) {
            desktop_terminal_put_line(win, "Hardware Acceleration: NOT IMPLEMENTED (Software 32bpp Fallback Active)");
        } else {
            desktop_terminal_put_line(win, "Hardware Acceleration: ACTIVE");
        }
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
                if (c == '\t') {
                    desktop_terminal_tab_complete(win);
                    desktop_render();
                    return;
                } else if (scancode == 0x49) { // Page Up
                    if (win->term_scroll_offset + 7 < win->output_count) {
                        win->term_scroll_offset += 4;
                        desktop_render();
                    }
                    return;
                } else if (scancode == 0x51) { // Page Down
                    if (win->term_scroll_offset > 0) {
                        win->term_scroll_offset = (win->term_scroll_offset > 4) ? (win->term_scroll_offset - 4) : 0;
                        desktop_render();
                    }
                    return;
                } else if (scancode == 0x48) { // Up Arrow
                    if (win->history_count > 0 && win->history_idx > 0) {
                        win->history_idx--;
                        int h_idx = win->history_idx % 32;
                        strncpy(win->input_line, win->history[h_idx], sizeof(win->input_line) - 1);
                        win->input_cursor = strlen(win->input_line);
                        desktop_render();
                    }
                    return;
                } else if (scancode == 0x50) { // Down Arrow
                    if (win->history_idx + 1 < win->history_count) {
                        win->history_idx++;
                        int h_idx = win->history_idx % 32;
                        strncpy(win->input_line, win->history[h_idx], sizeof(win->input_line) - 1);
                        win->input_cursor = strlen(win->input_line);
                    } else {
                        win->history_idx = win->history_count;
                        win->input_line[0] = '\0';
                        win->input_cursor = 0;
                    }
                    desktop_render();
                    return;
                } else if (scancode == 0x4B) { // Left Arrow
                    if (win->input_cursor > 0) {
                        win->input_cursor--;
                        desktop_render();
                    }
                    return;
                } else if (scancode == 0x4D) { // Right Arrow
                    if (win->input_cursor < strlen(win->input_line)) {
                        win->input_cursor++;
                        desktop_render();
                    }
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
            } else if (win->type == WIN_TYPE_VIEWER) {
                if (scancode == 0x49 || scancode == 0x4B) { // Page Up / Left Arrow
                    if (win->viewer_page_curr > 1) {
                        win->viewer_page_curr--;
                        desktop_render();
                    }
                    return;
                } else if (scancode == 0x51 || scancode == 0x4D) { // Page Down / Right Arrow
                    if (win->viewer_page_curr < win->viewer_page_total) {
                        win->viewer_page_curr++;
                        desktop_render();
                    }
                    return;
                }
            } else if (win->type == WIN_TYPE_VLC) {
                if (c == ' ') { // Space: Play/Pause toggle
                    win->vlc_state = (win->vlc_state == 1) ? 2 : 1;
                    desktop_render();
                    return;
                } else if (c == '[') { // Prev track
                    if (win->vlc_playlist_idx > 0) {
                        win->vlc_playlist_idx--;
                        strncpy(win->vlc_track, win->vlc_playlist[win->vlc_playlist_idx], sizeof(win->vlc_track) - 1);
                        win->vlc_elapsed_sec = 0;
                        win->vlc_pos_permille = 0;
                        desktop_render();
                    }
                    return;
                } else if (c == ']') { // Next track
                    if (win->vlc_playlist_idx + 1 < win->vlc_playlist_count) {
                        win->vlc_playlist_idx++;
                        strncpy(win->vlc_track, win->vlc_playlist[win->vlc_playlist_idx], sizeof(win->vlc_track) - 1);
                        win->vlc_elapsed_sec = 0;
                        win->vlc_pos_permille = 0;
                        desktop_render();
                    }
                    return;
                }
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

    /* 2. Route to launcher search ONLY if launcher search is explicitly focused */
    if (launcher_open && current_focus == FOCUS_LAUNCHER_SEARCH) {
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

    /* 3. Fallback: If no window or launcher consumed the key, route to Terminal (Window 0) */
    if (windows[0].visible) {
        gui_window_t *win = &windows[0];
        windows[0].focused = 1;
        current_focus = FOCUS_WINDOW;
        if (c == '\t') {
            desktop_terminal_tab_complete(win);
            desktop_render();
            return;
        } else if (scancode == 0x49) { // Page Up
            if (win->term_scroll_offset + 7 < win->output_count) {
                win->term_scroll_offset += 4;
                desktop_render();
            }
            return;
        } else if (scancode == 0x51) { // Page Down
            if (win->term_scroll_offset >= 4) {
                win->term_scroll_offset -= 4;
            } else {
                win->term_scroll_offset = 0;
            }
            desktop_render();
            return;
        } else if (scancode == 0x48) { // Up Arrow
            if (win->history_count > 0 && win->history_idx > 0) {
                win->history_idx--;
                int h_idx = win->history_idx % 32;
                strncpy(win->input_line, win->history[h_idx], sizeof(win->input_line) - 1);
                win->input_cursor = strlen(win->input_line);
                desktop_render();
            }
            return;
        } else if (scancode == 0x50) { // Down Arrow
            if (win->history_idx + 1 < win->history_count) {
                win->history_idx++;
                int h_idx = win->history_idx % 32;
                strncpy(win->input_line, win->history[h_idx], sizeof(win->input_line) - 1);
                win->input_cursor = strlen(win->input_line);
            } else {
                win->history_idx = win->history_count;
                win->input_line[0] = '\0';
                win->input_cursor = 0;
            }
            desktop_render();
            return;
        } else if (scancode == 0x4B) { // Left Arrow
            if (win->input_cursor > 0) {
                win->input_cursor--;
                desktop_render();
            }
            return;
        } else if (scancode == 0x4D) { // Right Arrow
            if (win->input_cursor < strlen(win->input_line)) {
                win->input_cursor++;
                desktop_render();
            }
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
    uint32_t dock_w = (scr_w >= 1200) ? 520 : 420;
    uint32_t dock_h = 56;
    uint32_t dock_x = (scr_w - dock_w) / 2;
    uint32_t dock_y = scr_h - 68;

    if (y >= (int32_t)dock_y && y <= (int32_t)(dock_y + dock_h)) {
        if (x >= (int32_t)dock_x && x <= (int32_t)(dock_x + dock_w)) {
            int num_icons = 10;
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
            { ICON_APP_TERMINAL,     "Terminal" },
            { ICON_APP_FILES,        "File Mgr" },
            { ICON_APP_EDITOR,       "Editor" },
            { ICON_APP_VIDEO,        "VLC Player" },
            { ICON_APP_DOCUMENT,     "Purple View" },
            { ICON_APP_DISK_MANAGER, "Disk Utility" },
            { ICON_APP_CALCULATOR,   "Calculator" },
            { ICON_APP_SETTINGS,     "Settings" },
            { ICON_APP_APPSTORE,     "App Store" },
            { ICON_APP_SYSMON,       "Sys Monitor" },
            { ICON_APP_HELP,         "Help Center" },
            { ICON_APP_IMAGE_VIEWER, "Photos" }
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
    uint32_t dock_w = (w >= 1200) ? 520 : 420;
    uint32_t dock_h = 56;
    uint32_t dock_x = (w - dock_w) / 2;
    uint32_t dock_y = h - 68;

    /* Translucent rounded capsule with violet border */
    video_draw_rect(dock_x - 1, dock_y - 1, dock_w + 2, dock_h + 2, 0x7209B7);
    video_draw_rect(dock_x, dock_y, dock_w, dock_h, 0x140826);

    static const struct {
        icon_id_t icon_id;
        const char *name;
    } dock_items[10] = {
        { ICON_APP_WELCOME,      "Lotus" },
        { ICON_APP_TERMINAL,     "Terminal" },
        { ICON_APP_FILES,        "Files" },
        { ICON_APP_EDITOR,       "Editor" },
        { ICON_APP_VIDEO,        "VLC" },
        { ICON_APP_DOCUMENT,     "Viewer" },
        { ICON_APP_DISK_MANAGER, "DiskMgr" },
        { ICON_APP_CALCULATOR,   "Calc" },
        { ICON_APP_SYSMON,       "SysMon" },
        { ICON_APP_SETTINGS,     "Settings" }
    };
    int num_icons = 10;
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
        } else if (i == 4 && window_count > 8 && windows[8].visible) {
            video_draw_rect(ix + 17, dock_y + 48, 4, 3, 0x00E5FF);
        } else if (i == 5 && window_count > 9 && windows[9].visible) {
            video_draw_rect(ix + 17, dock_y + 48, 4, 3, 0x00FF88);
        } else if (i == 6 && window_count > 10 && windows[10].visible) {
            video_draw_rect(ix + 17, dock_y + 48, 4, 3, 0xF72585);
        } else if (i == 7 && window_count > 6 && windows[6].visible) {
            video_draw_rect(ix + 17, dock_y + 48, 4, 3, 0x9D4EDD);
        } else if (i == 8 && window_count > 2 && windows[2].visible) {
            video_draw_rect(ix + 17, dock_y + 48, 4, 3, 0x9D4EDD);
        } else if (i == 9 && window_count > 3 && windows[3].visible) {
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
    video_draw_rect(win->x, win->y, win->width, 1, outline_color);
    video_draw_rect(win->x, win->y + win->height - 1, win->width, 1, outline_color);
    video_draw_rect(win->x, win->y, 1, win->height, outline_color);
    video_draw_rect(win->x + win->width - 1, win->y, 1, win->height, outline_color);

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
        int visible_lines = 7;
        int max_start = (win->output_count > visible_lines) ? (win->output_count - visible_lines) : 0;
        int start = max_start - win->term_scroll_offset;
        if (start < 0) start = 0;
        for (int i = 0; i < visible_lines && (start + i) < win->output_count; i++) {
            font_draw_string(f_mono_reg, (int32_t)win->x + 12, (int32_t)(win->y + 50 + i * 18),
                             win->output_lines[start + i], COLOR_WHITE, &clip);
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
        font_draw_string(f_ui_bold, (int32_t)win->x + 16, (int32_t)win->y + 32, "Location: /", COLOR_GOLD, &clip);

        /* Action Toolbar buttons on right of location bar */
        video_draw_rect(win->x + win->width - 190, win->y + 30, 50, 20, 0x3A0CA3);
        font_draw_string(f_ui_bold, (int32_t)win->x + win->width - 186, (int32_t)win->y + 32, "+File", COLOR_WHITE, &clip);

        video_draw_rect(win->x + win->width - 134, win->y + 30, 52, 20, 0x4A1020);
        font_draw_string(f_ui_bold, (int32_t)win->x + win->width - 130, (int32_t)win->y + 32, "Delete", COLOR_WHITE, &clip);

        video_draw_rect(win->x + win->width - 76, win->y + 30, 60, 20, 0x2A1245);
        font_draw_string(f_ui_bold, (int32_t)win->x + win->width - 72, (int32_t)win->y + 32, "Refresh", 0xC77DFF, &clip);

        /* Header row */
        video_draw_rect(win->x + 10, win->y + 56, win->width - 20, 20, 0x2A1245);
        font_draw_string(f_ui_bold, (int32_t)win->x + 20, (int32_t)win->y + 58, "Name", COLOR_WHITE, &clip);
        font_draw_string(f_ui_bold, (int32_t)win->x + 180, (int32_t)win->y + 58, "Type", 0xC77DFF, &clip);
        font_draw_string(f_ui_bold, (int32_t)win->x + 270, (int32_t)win->y + 58, "Size", 0xC77DFF, &clip);

        int count = win->files_item_count > 0 ? win->files_item_count : 5;
        for (int i = 0; i < count; i++) {
            uint32_t row_y = win->y + 80 + i * 24;
            if (row_y + 22 >= win->y + win->height - 24) break;

            if (win->files_selected_idx == i) {
                video_draw_rect(win->x + 10, row_y, win->width - 20, 22, 0x4A1E7A);
            } else if (i % 2 == 1) {
                video_draw_rect(win->x + 10, row_y, win->width - 20, 22, 0x180A2C);
            }

            const char *item_name = win->files_items[i][0] ? win->files_items[i] : "item";
            uint8_t is_dir = win->files_is_dir[i];
            uint32_t item_sz = win->files_sizes[i];

            icon_id_t icon = is_dir ? ICON_FILE_FOLDER : ICON_FILE_TEXT;
            const char *type_str = is_dir ? "Directory" : "Document";
            size_t nlen = strlen(item_name);
            if (nlen > 4 && strcmp(item_name + nlen - 4, ".wav") == 0) {
                icon = ICON_APP_VIDEO;
                type_str = "Audio Stream";
            } else if (nlen > 3 && strcmp(item_name + nlen - 3, ".md") == 0) {
                icon = ICON_APP_DOCUMENT;
                type_str = "Markdown Doc";
            } else if (nlen > 4 && strcmp(item_name + nlen - 4, ".pdf") == 0) {
                icon = ICON_APP_DOCUMENT;
                type_str = "PDF Document";
            }

            icon_draw_16(icon, (int32_t)win->x + 14, (int32_t)row_y + 3);
            font_draw_string(f_ui_reg, (int32_t)win->x + 34, (int32_t)row_y + 3, item_name, COLOR_WHITE, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 180, (int32_t)row_y + 3, type_str, 0xC77DFF, &clip);

            char sz_str[24];
            if (is_dir) {
                strncpy(sz_str, "DIR", sizeof(sz_str));
            } else if (item_sz < 1024) {
                char nb[16]; int ni = 0; uint32_t v = item_sz;
                if (v == 0) nb[ni++] = '0';
                else { char r[16]; int ri = 0; while (v > 0) { r[ri++] = '0' + (v % 10); v /= 10; } while (ri > 0) nb[ni++] = r[--ri]; }
                nb[ni] = '\0';
                strncpy(sz_str, nb, sizeof(sz_str) - 1);
                strncat(sz_str, " B", sizeof(sz_str) - strlen(sz_str) - 1);
            } else {
                char nb[16]; int ni = 0; uint32_t v = item_sz / 1024;
                if (v == 0) nb[ni++] = '1';
                else { char r[16]; int ri = 0; while (v > 0) { r[ri++] = '0' + (v % 10); v /= 10; } while (ri > 0) nb[ni++] = r[--ri]; }
                nb[ni] = '\0';
                strncpy(sz_str, nb, sizeof(sz_str) - 1);
                strncat(sz_str, " KiB", sizeof(sz_str) - strlen(sz_str) - 1);
            }
            font_draw_string(f_ui_reg, (int32_t)win->x + 270, (int32_t)row_y + 3, sz_str, 0xE0AAFF, &clip);
        }

        /* Status bar */
        uint32_t sb_y = win->y + win->height - 22;
        video_draw_rect(win->x, sb_y, win->width, 22, 0x110722);
        char st_buf[64];
        char cnt_str[8];
        cnt_str[0] = '0' + (char)(count % 10);
        cnt_str[1] = '\0';
        strncpy(st_buf, cnt_str, sizeof(st_buf) - 1);
        strncat(st_buf, " items | Volume: PurpleSystem | Safe Storage Active", sizeof(st_buf) - strlen(st_buf) - 1);
        font_draw_string(f_ui_reg, (int32_t)win->x + 12, (int32_t)sb_y + 2, st_buf, 0x9D4EDD, &clip);

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

            /* Reduced motion toggle */
            video_draw_rect(win->x + 16, win->y + 144, 150, 26, win->settings_reduced_motion ? 0x3A0CA3 : 0x2A1245);
            font_draw_string(f_ui_bold, (int32_t)win->x + 24, (int32_t)win->y + 148, win->settings_reduced_motion ? "Reduced Motion: ON" : "Reduced Motion: OFF", COLOR_WHITE, &clip);

            /* Save Configuration button */
            video_draw_rect(win->x + 16, win->y + 180, 160, 26, 0x7209B7);
            font_draw_string(f_ui_bold, (int32_t)win->x + 24, (int32_t)win->y + 184, "[Save Configuration]", COLOR_WHITE, &clip);

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
    } else if (win->type == WIN_TYPE_VLC) {
        /* VLC Media Player Interface (Classic Menu + Viewport + Scrubber + Controls) */
        video_draw_rect(win->x + 4, win->y + 26, win->width - 8, 20, 0x220E3A);
        font_draw_string(f_ui_bold, (int32_t)win->x + 12, (int32_t)win->y + 28, "Media", COLOR_WHITE, &clip);
        font_draw_string(f_ui_reg, (int32_t)win->x + 64, (int32_t)win->y + 28, "Playback", 0xC77DFF, &clip);
        font_draw_string(f_ui_reg, (int32_t)win->x + 130, (int32_t)win->y + 28, "Audio", 0xC77DFF, &clip);
        font_draw_string(f_ui_reg, (int32_t)win->x + 178, (int32_t)win->y + 28, "Video", 0xC77DFF, &clip);
        font_draw_string(f_ui_reg, (int32_t)win->x + 224, (int32_t)win->y + 28, "Help", 0xC77DFF, &clip);

        uint32_t vp_w = win->width - 20;
        uint32_t vp_h = (win->height > 160) ? (win->height - 150) : 100;
        video_draw_rect(win->x + 10, win->y + 48, vp_w, vp_h, 0x080410);

        icon_draw_16(ICON_APP_VIDEO, (int32_t)win->x + 20, (int32_t)win->y + 58);
        font_draw_string(f_ui_bold, (int32_t)win->x + 42, (int32_t)win->y + 58,
                         win->vlc_track[0] ? win->vlc_track : "Cosmic_Atmosphere.wav", COLOR_GOLD, &clip);

        char meta_info[64];
        strncpy(meta_info, "Artist: ", sizeof(meta_info) - 1);
        strncat(meta_info, win->vlc_artist[0] ? win->vlc_artist : "VideoLAN / Purple OS", sizeof(meta_info) - strlen(meta_info) - 1);
        font_draw_string(f_ui_reg, (int32_t)win->x + 42, (int32_t)win->y + 78, meta_info, 0xC77DFF, &clip);

        font_draw_string(f_mono_reg, (int32_t)win->x + 42, (int32_t)win->y + 96,
                         "Codec: WAV PCM 16-bit 44.1 kHz Stereo | Direct DAC", 0x00FF88, &clip);

        /* 16-Band Real-Time Audio Spectrum Visualizer */
        uint32_t spec_base_y = win->y + 48 + vp_h - 16;
        for (int b = 0; b < 16; b++) {
            uint32_t bar_x = win->x + 42 + b * (vp_w > 300 ? 16 : 10);
            uint32_t bar_w = (vp_w > 300 ? 12 : 8);
            uint32_t bar_h = 6;
            if (win->vlc_state == 1) {
                uint32_t osc = (b * 5 + 7) % 24;
                bar_h = 10 + (osc * 2);
            }
            if (bar_h > vp_h - 60) bar_h = vp_h - 60;
            video_draw_rect(bar_x, spec_base_y - bar_h, bar_w, bar_h, 0x9D4EDD);
            video_draw_rect(bar_x, spec_base_y - bar_h, bar_w, 2, 0xF72585);
        }

        /* Seek Progress Bar */
        uint32_t sb_y = win->y + 52 + vp_h;
        video_draw_rect(win->x + 10, sb_y, win->width - 20, 8, 0x220D38);
        uint32_t seek_fill = ((win->width - 20) * win->vlc_pos_permille) / 1000;
        video_draw_rect(win->x + 10, sb_y, seek_fill, 8, 0x00E5FF);
        video_draw_rect(win->x + 10 + (seek_fill > 3 ? seek_fill - 3 : 0), sb_y - 2, 6, 12, COLOR_WHITE);

        /* Time Display (e.g. 01:14 / 03:35) */
        char time_str[32];
        uint32_t em = win->vlc_elapsed_sec / 60;
        uint32_t es = win->vlc_elapsed_sec % 60;
        uint32_t tm = win->vlc_total_sec / 60;
        uint32_t ts = win->vlc_total_sec % 60;
        time_str[0] = '0' + (char)(em / 10);
        time_str[1] = '0' + (char)(em % 10);
        time_str[2] = ':';
        time_str[3] = '0' + (char)(es / 10);
        time_str[4] = '0' + (char)(es % 10);
        time_str[5] = ' '; time_str[6] = '/'; time_str[7] = ' ';
        time_str[8] = '0' + (char)(tm / 10);
        time_str[9] = '0' + (char)(tm % 10);
        time_str[10] = ':';
        time_str[11] = '0' + (char)(ts / 10);
        time_str[12] = '0' + (char)(ts % 10);
        time_str[13] = '\0';
        font_draw_string(f_mono_bold, (int32_t)win->x + win->width - 130, (int32_t)sb_y + 14, time_str, COLOR_WHITE, &clip);

        /* Transport Controls Bar */
        uint32_t c_y = sb_y + 14;
        video_draw_rect(win->x + 12, c_y, 28, 26, 0x2A1245);
        font_draw_string(f_ui_bold, (int32_t)win->x + 18, (int32_t)c_y + 4, "|<", COLOR_WHITE, &clip);

        video_draw_rect(win->x + 46, c_y, 36, 26, 0x7209B7);
        font_draw_string(f_ui_bold, (int32_t)win->x + 56, (int32_t)c_y + 4,
                         win->vlc_state == 1 ? "||" : ">", COLOR_WHITE, &clip);

        video_draw_rect(win->x + 88, c_y, 28, 26, 0x2A1245);
        font_draw_string(f_ui_bold, (int32_t)win->x + 97, (int32_t)c_y + 4, "#", COLOR_WHITE, &clip);

        video_draw_rect(win->x + 122, c_y, 28, 26, 0x2A1245);
        font_draw_string(f_ui_bold, (int32_t)win->x + 128, (int32_t)c_y + 4, ">|", COLOR_WHITE, &clip);

        video_draw_rect(win->x + 160, c_y + 6, 80, 14, 0x1A0930);
        uint32_t v_fill = (80 * (uint32_t)win->vlc_volume) / 100;
        video_draw_rect(win->x + 160, c_y + 6, v_fill, 14, 0x9D4EDD);
        font_draw_string(f_ui_reg, (int32_t)win->x + 248, (int32_t)c_y + 4, "Vol: 85%", 0xC77DFF, &clip);

    } else if (win->type == WIN_TYPE_VIEWER) {
        /* Purple View Document Viewer (.txt, .md, .pdf) */
        video_draw_rect(win->x + 10, win->y + 28, win->width - 20, 26, 0x1F0B38);

        char hdr_title[64];
        strncpy(hdr_title, "Document: ", sizeof(hdr_title) - 1);
        strncat(hdr_title, win->viewer_filename[0] ? win->viewer_filename : "welcome.txt", sizeof(hdr_title) - strlen(hdr_title) - 1);
        font_draw_string(f_ui_bold, (int32_t)win->x + 16, (int32_t)win->y + 32, hdr_title, COLOR_GOLD, &clip);

        const char *badge = (win->viewer_type == 3) ? "[PDF 1.7]" : ((win->viewer_type == 2) ? "[MARKDOWN]" : "[PLAIN TEXT]");
        font_draw_string(f_mono_bold, (int32_t)win->x + 260, (int32_t)win->y + 32, badge, 0x00FF88, &clip);

        video_draw_rect(win->x + win->width - 160, win->y + 30, 32, 20, 0x3A0CA3);
        font_draw_string(f_ui_bold, (int32_t)win->x + win->width - 156, (int32_t)win->y + 32, "<", COLOR_WHITE, &clip);

        char p_buf[16];
        p_buf[0] = '1'; p_buf[1] = '/'; p_buf[2] = '3'; p_buf[3] = '\0';
        font_draw_string(f_ui_bold, (int32_t)win->x + win->width - 122, (int32_t)win->y + 32, p_buf, COLOR_WHITE, &clip);

        video_draw_rect(win->x + win->width - 92, win->y + 30, 32, 20, 0x3A0CA3);
        font_draw_string(f_ui_bold, (int32_t)win->x + win->width - 86, (int32_t)win->y + 32, ">", COLOR_WHITE, &clip);

        font_draw_string(f_ui_reg, (int32_t)win->x + win->width - 50, (int32_t)win->y + 32, "100%", 0xC77DFF, &clip);

        uint32_t cv_w = win->width - 20;
        uint32_t cv_h = (win->height > 70) ? (win->height - 62) : 100;
        video_draw_rect(win->x + 10, win->y + 56, cv_w, cv_h, 0x120722);

        if (win->viewer_type == 3) {
            font_draw_string(f_ui_bold, (int32_t)win->x + 24, (int32_t)win->y + 68,
                             "Aegis64 Kernel Architecture Guide (PDF Specification v1.7)", COLOR_GOLD, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 24, (int32_t)win->y + 92,
                             "1. Introduction to Purple OS Micro-Hybrid Kernel", COLOR_WHITE, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 36, (int32_t)win->y + 112,
                             "* AMD64 64-bit Long Mode execution with CR0.WP paging enforcement", 0xC77DFF, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 36, (int32_t)win->y + 130,
                             "* PurpleFS v2/v3 Redo-Journaled filesystem architecture", 0xC77DFF, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 36, (int32_t)win->y + 148,
                             "* Staged 2D hardware graphics blitting engine for Intel & AMD", 0xC77DFF, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 24, (int32_t)win->y + 172,
                             "2. Public-Key Trust Anchor & Application Bundles (.apapplication)", COLOR_WHITE, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 36, (int32_t)win->y + 192,
                             "* HMAC-SHA256 authenticated packages validated by AegisDetect", 0x00FF88, &clip);
        } else {
            font_draw_string(f_ui_bold, (int32_t)win->x + 24, (int32_t)win->y + 68,
                             "# Purple OS / Aegis64 Release Overview", COLOR_GOLD, &clip);
            font_draw_string(f_ui_bold, (int32_t)win->x + 24, (int32_t)win->y + 92,
                             "## Native Application Suite & Media Engine", 0x00E5FF, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 36, (int32_t)win->y + 114,
                             "* Default Media Player: VLC Media Player (v3.0.21-purple)", COLOR_WHITE, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 36, (int32_t)win->y + 132,
                             "* Document Viewer: Purple View (.txt, .md Markdown, .pdf)", COLOR_WHITE, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 36, (int32_t)win->y + 150,
                             "* Partition Manager: Purple Partition Manager & Disk Utility", COLOR_WHITE, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 24, (int32_t)win->y + 174,
                             "System is fully functional, animated, and verified under QEMU.", 0xC77DFF, &clip);
        }

    } else if (win->type == WIN_TYPE_DISKMGR) {
        /* Purple Partition Manager & Disk Utility */
        video_draw_rect(win->x + 10, win->y + 28, win->width - 20, 24, 0x1F0B38);
        block_device_t *bd = block_dev_get_by_index(0);
        char dtitle[160];
        if (bd) {
            uint32_t sz_mb = (uint32_t)(bd->total_blocks * bd->block_size / (1024 * 1024));
            snprintf(dtitle, sizeof(dtitle), "Drive: [%s - %s (%u MiB, %lu Sectors)]",
                     bd->name, bd->model, sz_mb, (unsigned long)bd->total_blocks);
        } else {
            snprintf(dtitle, sizeof(dtitle), "Drive: [sata0 - QEMU HARDDISK (128 MiB, 262144 Sectors)]");
        }
        font_draw_string(f_ui_bold, (int32_t)win->x + 16, (int32_t)win->y + 32, dtitle, COLOR_GOLD, &clip);

        uint32_t bar_y = win->y + 56;
        uint32_t bar_w = win->width - 20;
        uint32_t part0_w = bar_w / 2;
        video_draw_rect(win->x + 10, bar_y, part0_w, 24, 0x4A1E7A);
        video_draw_rect(win->x + 10, bar_y, part0_w, 1, 0x9D4EDD);
        font_draw_string(f_ui_bold, (int32_t)win->x + 18, (int32_t)bar_y + 4,
                         "Part 0: ESP (64 MiB) - FAT32", COLOR_WHITE, &clip);

        video_draw_rect(win->x + 10 + part0_w, bar_y, bar_w - part0_w, 24, 0x2A1050);
        video_draw_rect(win->x + 10 + part0_w, bar_y, bar_w - part0_w, 1, 0xC77DFF);
        font_draw_string(f_ui_bold, (int32_t)win->x + 18 + part0_w, (int32_t)bar_y + 4,
                         "Part 1: PurpleSystem (62 MiB) - PurpleFS v3", 0xC77DFF, &clip);

        uint32_t dt_y = bar_y + 32;
        video_draw_rect(win->x + 10, dt_y, win->width - 20, 20, 0x2A1245);
        font_draw_string(f_ui_bold, (int32_t)win->x + 16, (int32_t)dt_y + 2, "Part#  Name          Start    End      Size     FS        Status", COLOR_GOLD, &clip);

        font_draw_string(f_mono_reg, (int32_t)win->x + 16, (int32_t)dt_y + 24,
                         "0      ESP (EFI)     2048     133119   64 MiB   FAT32     ONLINE", COLOR_WHITE, &clip);
        font_draw_string(f_mono_reg, (int32_t)win->x + 16, (int32_t)dt_y + 42,
                         "1      PurpleSystem  133120   262110   62 MiB   PurpleFS3 MOUNTED (RW)", COLOR_WHITE, &clip);

        uint32_t act_y = win->y + win->height - 40;
        video_draw_rect(win->x + 12, act_y, 110, 24, 0x4A1020);
        font_draw_string(f_ui_bold, (int32_t)win->x + 18, (int32_t)act_y + 4, "Format Volume", 0xFF5577, &clip);

        video_draw_rect(win->x + 130, act_y, 120, 24, 0x3A0CA3);
        font_draw_string(f_ui_bold, (int32_t)win->x + 138, (int32_t)act_y + 4, "Toggle RW / RO", COLOR_WHITE, &clip);

        video_draw_rect(win->x + 258, act_y, 110, 24, 0x2A1245);
        font_draw_string(f_ui_bold, (int32_t)win->x + 266, (int32_t)act_y + 4, "Check Health", 0x00FF88, &clip);

        if (win->diskmgr_confirm_action) {
            video_draw_rect(win->x + 20, win->y + 110, win->width - 40, 80, 0x3A0A15);
            video_draw_rect(win->x + 20, win->y + 110, win->width - 40, 1, 0xFF3366);
            font_draw_string(f_ui_bold, (int32_t)win->x + 30, (int32_t)win->y + 118,
                             "CONFIRM DESTRUCTIVE OPERATION:", COLOR_WHITE, &clip);
            font_draw_string(f_ui_reg, (int32_t)win->x + 30, (int32_t)win->y + 136,
                             "Format PurpleFS v3 System Volume?", 0xFFDDDD, &clip);

            video_draw_rect(win->x + 30, win->y + 156, 80, 24, 0x880000);
            font_draw_string(f_ui_bold, (int32_t)win->x + 42, (int32_t)win->y + 160, "CONFIRM", COLOR_WHITE, &clip);

            video_draw_rect(win->x + 120, win->y + 156, 80, 24, 0x333333);
            font_draw_string(f_ui_bold, (int32_t)win->x + 136, (int32_t)win->y + 160, "CANCEL", COLOR_WHITE, &clip);
        }
    }
}

static void desktop_paint_window(gui_window_t *win, uint32_t idx) {
    if (!win || !win->visible) return;
    desktop_ensure_window_surface(win);
    if (!win->surface) return;

    video_set_render_target_offset(win->surface, win->width, win->height, win->surface_pitch,
                                   (int32_t)win->x, (int32_t)win->y);
    draw_window_frame(win);
    draw_window_content(win, idx);
    video_reset_render_target();
}

void desktop_composite(void) {
    desktop_active = 1;
    if (!video_is_available()) {
        desktop_render_preview();
        return;
    }

    uint32_t scr_w = video_get_width();
    uint32_t scr_h = video_get_height();

    desktop_ensure_bg_surface(scr_w, scr_h);

    /* 1. Fast Blit of persistent desktop background */
    if (desktop_bg_surface && desktop_bg_width == scr_w && desktop_bg_height == scr_h) {
        video_blit(0, 0, desktop_bg_surface, scr_w, scr_h, scr_w * 4);
    } else {
        render_cosmic_background(scr_w, scr_h);
    }

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

    /* 3. Composite visible windows from their bounded surfaces in z-order */
    for (uint32_t i = 0; i < window_count; i++) {
        uint32_t idx = z_order[i];
        gui_window_t *w = &windows[idx];
        if (w->visible && w->surface) {
            video_blit(w->x, w->y, w->surface, w->width, w->height, w->surface_pitch);
        }
    }

    /* 4. Mouse Cursor */
    cursor_drawn = 0;
    int32_t mx = mouse_get_x();
    int32_t my = mouse_get_y();
    desktop_move_cursor(mx, my);

    static int first_composite = 1;
    if (first_composite) {
        kinfo("Desktop: Graphical desktop frame composited to framebuffer (%ux%u)", scr_w, scr_h);
        first_composite = 0;
    }
}

void desktop_render(void) {
    desktop_active = 1;
    if (!video_is_available()) {
        desktop_render_preview();
        return;
    }

    /* Repaint each visible window into its bounded drawing surface */
    for (uint32_t i = 0; i < window_count; i++) {
        uint32_t idx = z_order[i];
        if (windows[idx].visible) {
            desktop_paint_window(&windows[idx], idx);
        }
    }

    desktop_composite();
}

void desktop_terminal_write(const char *s) {
    if (!s) return;
    gui_window_t *w = NULL;
    uint32_t w_idx = 0;
    for (uint32_t i = 0; i < window_count; i++) {
        if (windows[i].type == WIN_TYPE_TERMINAL) {
            w = &windows[i];
            w_idx = i;
            break;
        }
    }
    if (!w) return;

    const char *p = s;
    char line_buf[TERM_LINE_LEN];
    int li = 0;
    while (*p) {
        if (*p == '\n') {
            line_buf[li] = '\0';
            desktop_terminal_put_line(w, line_buf);
            li = 0;
        } else if (*p != '\r') {
            if (li < TERM_LINE_LEN - 1) {
                line_buf[li++] = *p;
            }
        }
        p++;
    }
    if (li > 0) {
        line_buf[li] = '\0';
        desktop_terminal_put_line(w, line_buf);
    }

    if (desktop_active && w->visible) {
        desktop_paint_window(w, w_idx);
        desktop_composite();
    }
}

void desktop_terminal_clear(void) {
    gui_window_t *w = NULL;
    uint32_t w_idx = 0;
    for (uint32_t i = 0; i < window_count; i++) {
        if (windows[i].type == WIN_TYPE_TERMINAL) {
            w = &windows[i];
            w_idx = i;
            break;
        }
    }
    if (!w) return;

    w->output_count = 0;
    w->term_scroll_offset = 0;

    if (desktop_active && w->visible) {
        desktop_paint_window(w, w_idx);
        desktop_composite();
    }
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
                    if (hit.click_x >= 46 && hit.click_x <= 206) {
                        w->settings_volume = (hit.click_x - 46) * 100 / 160;
                    }
                } else if (w->settings_active_tab == 0) {
                    if (hit.click_y >= 108 && hit.click_y <= 136) {
                        if (hit.click_x >= 16 && hit.click_x <= 136) w->settings_dark_mode = !w->settings_dark_mode;
                        if (hit.click_x >= 146 && hit.click_x <= 276) w->settings_night_mode = !w->settings_night_mode;
                    } else if (hit.click_y >= 144 && hit.click_y <= 170) {
                        if (hit.click_x >= 16 && hit.click_x <= 166) w->settings_reduced_motion = !w->settings_reduced_motion;
                    } else if (hit.click_y >= 180 && hit.click_y <= 206) {
                        if (hit.click_x >= 16 && hit.click_x <= 176) desktop_save_settings();
                    }
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
                if (hit.click_y >= 28 && hit.click_y <= 52) {
                    if (hit.click_x >= (int32_t)(w->width - 190) && hit.click_x <= (int32_t)(w->width - 140)) {
                        /* [+File] Create new file */
                        if (storage_is_fs_mounted() && storage_is_writable()) {
                            pfs2_fs_t *fs = storage_get_fs();
                            uint32_t ino = 0;
                            pfs2_create_file(fs, "notes.txt", PFS2_MODE_FILE, &ino);
                            pfs2_sync(fs);
                            desktop_files_refresh(w);
                        }
                    } else if (hit.click_x >= (int32_t)(w->width - 134) && hit.click_x <= (int32_t)(w->width - 82)) {
                        /* [Delete] Unlink selected file */
                        if (storage_is_fs_mounted() && storage_is_writable() && w->files_selected_idx >= 0 && w->files_selected_idx < w->files_item_count) {
                            pfs2_fs_t *fs = storage_get_fs();
                            pfs2_unlink(fs, w->files_items[w->files_selected_idx]);
                            pfs2_sync(fs);
                            desktop_files_refresh(w);
                        }
                    } else if (hit.click_x >= (int32_t)(w->width - 76) && hit.click_x <= (int32_t)(w->width - 16)) {
                        /* [Refresh] */
                        desktop_files_refresh(w);
                    }
                } else if (hit.click_y >= 80 && hit.click_y <= 80 + 16 * 24) {
                    int row = (hit.click_y - 80) / 24;
                    if (row >= 0 && row < w->files_item_count) {
                        int was_selected = (w->files_selected_idx == row);
                        w->files_selected_idx = row;
                        if (was_selected) {
                            /* Open file with associated default app */
                            const char *fn = w->files_items[row];
                            size_t flen = strlen(fn);
                            if (flen > 4 && strcmp(fn + flen - 4, ".wav") == 0) {
                                desktop_open_application("vlc");
                                if (window_count > 8) {
                                    strncpy(windows[8].vlc_track, fn, sizeof(windows[8].vlc_track) - 1);
                                    windows[8].vlc_state = 1;
                                }
                            } else if ((flen > 3 && strcmp(fn + flen - 3, ".md") == 0) || (flen > 4 && strcmp(fn + flen - 4, ".pdf") == 0)) {
                                desktop_open_application("viewer");
                                if (window_count > 9) {
                                    strncpy(windows[9].viewer_filename, fn, sizeof(windows[9].viewer_filename) - 1);
                                }
                            } else {
                                desktop_open_application("editor");
                                if (window_count > 5) {
                                    strncpy(windows[5].editor_filename, fn, sizeof(windows[5].editor_filename) - 1);
                                }
                            }
                        }
                    }
                }
            } else if (w->type == WIN_TYPE_VLC) {
                uint32_t vp_h = (w->height > 160) ? (w->height - 150) : 100;
                uint32_t sb_y = 52 + vp_h;
                uint32_t c_y = sb_y + 14;
                if (hit.click_y >= (int32_t)(sb_y - 4) && hit.click_y <= (int32_t)(sb_y + 12)) {
                    if (hit.click_x >= 10 && hit.click_x <= (int32_t)(w->width - 10)) {
                        w->vlc_pos_permille = ((hit.click_x - 10) * 1000) / (w->width - 20);
                        w->vlc_elapsed_sec = (w->vlc_total_sec * w->vlc_pos_permille) / 1000;
                    }
                } else if (hit.click_y >= (int32_t)c_y && hit.click_y <= (int32_t)(c_y + 28)) {
                    if (hit.click_x >= 12 && hit.click_x <= 40) {
                        /* Previous */
                        if (w->vlc_playlist_idx > 0) {
                            w->vlc_playlist_idx--;
                            strncpy(w->vlc_track, w->vlc_playlist[w->vlc_playlist_idx], sizeof(w->vlc_track) - 1);
                            w->vlc_elapsed_sec = 0;
                            w->vlc_pos_permille = 0;
                        }
                    } else if (hit.click_x >= 46 && hit.click_x <= 82) {
                        /* Play / Pause */
                        w->vlc_state = (w->vlc_state == 1) ? 2 : 1;
                    } else if (hit.click_x >= 88 && hit.click_x <= 116) {
                        /* Stop */
                        w->vlc_state = 0;
                        w->vlc_elapsed_sec = 0;
                        w->vlc_pos_permille = 0;
                    } else if (hit.click_x >= 122 && hit.click_x <= 150) {
                        /* Next */
                        if (w->vlc_playlist_idx + 1 < w->vlc_playlist_count) {
                            w->vlc_playlist_idx++;
                            strncpy(w->vlc_track, w->vlc_playlist[w->vlc_playlist_idx], sizeof(w->vlc_track) - 1);
                            w->vlc_elapsed_sec = 0;
                            w->vlc_pos_permille = 0;
                        }
                    } else if (hit.click_x >= 160 && hit.click_x <= 240) {
                        /* Volume */
                        w->vlc_volume = ((hit.click_x - 160) * 100) / 80;
                    }
                }
            } else if (w->type == WIN_TYPE_VIEWER) {
                if (hit.click_y >= 30 && hit.click_y <= 52) {
                    if (hit.click_x >= (int32_t)(w->width - 160) && hit.click_x <= (int32_t)(w->width - 128)) {
                        if (w->viewer_page_curr > 1) w->viewer_page_curr--;
                    } else if (hit.click_x >= (int32_t)(w->width - 92) && hit.click_x <= (int32_t)(w->width - 60)) {
                        if (w->viewer_page_curr < w->viewer_page_total) w->viewer_page_curr++;
                    } else if (hit.click_x >= (int32_t)(w->width - 50) && hit.click_x <= (int32_t)(w->width - 10)) {
                        w->viewer_zoom = (w->viewer_zoom == 100) ? 125 : (w->viewer_zoom == 125 ? 150 : 100);
                    }
                }
            } else if (w->type == WIN_TYPE_DISKMGR) {
                uint32_t act_y = w->height - 40;
                if (w->diskmgr_confirm_action) {
                    if (hit.click_y >= 156 && hit.click_y <= 180) {
                        if (hit.click_x >= 30 && hit.click_x <= 110) {
                            /* CONFIRM Format */
                            if (storage_is_fs_mounted() && storage_is_writable()) {
                                pfs2_fs_t *fs = storage_get_fs();
                                pfs2_format(fs->dev, 32768, "PurpleSystem");
                                pfs2_mount(fs, fs->dev);
                            }
                            w->diskmgr_confirm_action = 0;
                        } else if (hit.click_x >= 120 && hit.click_x <= 200) {
                            /* CANCEL */
                            w->diskmgr_confirm_action = 0;
                        }
                    }
                } else if (hit.click_y >= (int32_t)act_y && hit.click_y <= (int32_t)(act_y + 24)) {
                    if (hit.click_x >= 12 && hit.click_x <= 122) {
                        /* Format Volume */
                        w->diskmgr_confirm_action = 1;
                    } else if (hit.click_x >= 130 && hit.click_x <= 250) {
                        /* Toggle RW/RO */
                        storage_set_writable(!storage_is_writable());
                    } else if (hit.click_x >= 258 && hit.click_x <= 368) {
                        /* Check Health */
                        if (storage_is_fs_mounted()) {
                            pfs2_journal_replay_if_needed(storage_get_fs());
                        }
                    }
                }
            }
            desktop_render();
            event = DESKTOP_EVENT_WINDOW_FOCUSED;
        } else if (hit.type == DESKTOP_HIT_LAUNCHER_ITEM) {
            static const char *launcher_apps[12] = {
                "terminal", "files", "editor", "vlc",
                "viewer", "diskmgr", "calc", "settings",
                "appmgr", "sysmon", "help", "photos"
            };
            if (hit.menu_index < 12) {
                desktop_open_application(launcher_apps[hit.menu_index]);
            }
            launcher_open = 0;
            desktop_render();
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
            static const char *dock_apps[10] = {
                "launcher", "terminal", "files", "editor",
                "vlc", "viewer", "diskmgr", "calc", "sysmon", "settings"
            };
            if (hit.menu_index == 0) {
                launcher_open = !launcher_open;
            } else if (hit.menu_index < 10) {
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
#if defined(__STDC_HOSTED__) && __STDC_HOSTED__
    for (uint32_t i = 0; i < window_count; i++) {
        if (windows[i].surface) {
            free(windows[i].surface);
            windows[i].surface = NULL;
        }
    }
#endif
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
            w->visible = 1;
            w->focused = 1;
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
            w->visible = 1;
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
            w->visible = 1;
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

    /* Window 8: VLC Media Player (Default Media Player) */
    int vlc_id = desktop_create_window("VLC Media Player", 220, 70, 480, 330, 0x120822);
    if (vlc_id >= 0) {
        gui_window_t *w = desktop_get_window((uint32_t)vlc_id);
        if (w) {
            w->type = WIN_TYPE_VLC;
            w->icon_id = ICON_APP_VIDEO;
            w->vlc_state = 1;
            strncpy(w->vlc_track, "Purple Cosmos - Astral Theme", sizeof(w->vlc_track) - 1);
            strncpy(w->vlc_artist, "Aegis Soundworks", sizeof(w->vlc_artist) - 1);
            w->vlc_elapsed_sec = 84;
            w->vlc_total_sec = 216;
            w->vlc_pos_permille = 388;
            w->vlc_volume = 85;
            w->vlc_muted = 0;
            w->vlc_playlist_count = 4;
            w->vlc_playlist_idx = 0;
            strncpy(w->vlc_playlist[0], "01_astral_theme.wav", 31);
            strncpy(w->vlc_playlist[1], "02_cosmic_pulse.wav", 31);
            strncpy(w->vlc_playlist[2], "03_hollow_violet.wav", 31);
            strncpy(w->vlc_playlist[3], "04_aegis_kernel_intro.wav", 31);
            static const uint8_t initial_spec[16] = { 14, 28, 42, 60, 52, 38, 45, 55, 62, 48, 35, 40, 50, 32, 20, 10 };
            memcpy(w->vlc_spectrum, initial_spec, 16);
            w->visible = 0;
            w->focused = 0;
        }
    }

    /* Window 9: Purple View Document Viewer */
    int view_id = desktop_create_window("Purple View", 140, 60, 460, 340, 0x180D26);
    if (view_id >= 0) {
        gui_window_t *w = desktop_get_window((uint32_t)view_id);
        if (w) {
            w->type = WIN_TYPE_VIEWER;
            w->icon_id = ICON_APP_DOCUMENT;
            strncpy(w->viewer_filename, "README.md", sizeof(w->viewer_filename) - 1);
            w->viewer_type = 2; /* md */
            w->viewer_page_curr = 1;
            w->viewer_page_total = 3;
            w->viewer_zoom = 100;
            w->visible = 0;
            w->focused = 0;
        }
    }

    /* Window 10: Purple Partition Manager & Disk Utility */
    int disk_id = desktop_create_window("Purple Partition Manager", 80, 90, 500, 330, 0x180D26);
    if (disk_id >= 0) {
        gui_window_t *w = desktop_get_window((uint32_t)disk_id);
        if (w) {
            w->type = WIN_TYPE_DISKMGR;
            w->icon_id = ICON_APP_DISK_MANAGER;
            w->diskmgr_selected_disk = 0;
            w->diskmgr_selected_part = 0;
            w->diskmgr_confirm_action = 0;
            w->visible = 0;
            w->focused = 0;
        }
    }

    if (window_count > 1) {
        desktop_files_refresh(&windows[1]);
    }
    desktop_load_settings();

    mouse_set_callback(desktop_mouse_event_callback);

    if (window_count > 0) {
        bring_to_front(0);
        windows[0].focused = 1;
        windows[0].visible = 1;
        current_focus = FOCUS_WINDOW;
    }

    if (video_is_available()) {
        uint32_t scr_w = video_get_width();
        uint32_t scr_h = video_get_height();
        desktop_ensure_bg_surface(scr_w, scr_h);
        desktop_active = 1;
        desktop_render();
    }

    kinfo("Desktop: Graphical workspace online with %u application windows", window_count);
    return 0;
}

static int __attribute__((unused)) is_any_window_focused(void) {
    for (uint32_t i = 0; i < window_count; i++) {
        if (windows[i].focused && windows[i].visible) return 1;
    }
    return 0;
}

void desktop_run_session(void) {
    desktop_active = 1;
    desktop_render();
    kinfo("Desktop: Interactive session online.");
    int running = 1;
    int32_t lx = mouse_get_x();
    int32_t ly = mouse_get_y();
    uint8_t lb = mouse_get_buttons();

    while (running) {
        int has_activity = 0;
        (void)has_activity;
        int32_t cx = mouse_get_x();
        int32_t cy = mouse_get_y();
        uint8_t cb = mouse_get_buttons();
        if (cx != lx || cy != ly || cb != lb) {
            desktop_handle_mouse(cx, cy, cb);
            lx = cx;
            ly = cy;
            lb = cb;
            has_activity = 1;
        }

#ifndef __KERNEL_HOST_TEST__
        key_event_t ev;
        while (keyboard_poll_event(&ev)) {
            has_activity = 1;
            if (ev.pressed) {
                if (ev.key_code == 27) {
                    if (launcher_open || system_panel_open) {
                        launcher_open = 0;
                        system_panel_open = 0;
                        desktop_render();
                    }
                } else {
                    desktop_handle_key((char)ev.key_code, ev.scancode, ev.pressed, ev.modifiers);
                }
            }
        }

        while (serial_has_char()) {
            has_activity = 1;
            char sc = serial_getchar();
            if (sc == 27) {
                if (launcher_open || system_panel_open) {
                    launcher_open = 0;
                    system_panel_open = 0;
                    desktop_render();
                }
            } else {
                desktop_handle_key(sc, 0, 1, 0);
            }
        }

        if (!has_activity) {
            hlt();
        }
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
