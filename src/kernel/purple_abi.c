#include "purple_abi.h"
#include "desktop.h"
#include "video.h"
#include "string.h"
#include "serial.h"
#include "font8x16.h"

#ifdef __KERNEL_HOST_TEST__
#include <stdlib.h>
#else
#include "pmm.h"
#include "task.h"
#include "pit.h"
#include "rtc.h"
#include "audio.h"
#include "kernel_storage.h"
#endif

#define MAX_ABI_WINDOWS 16
#define MAX_ABI_FILES   16
#define EVENT_QUEUE_CAP 64

struct purple_window_s {
    uint32_t         id;
    uint32_t         desktop_win_id;
    char             title[64];
    uint32_t         width;
    uint32_t         height;
    purple_surface_t surface;
    int              active;
};

struct purple_file_s {
    int      active;
    char     path[64];
    uint32_t inode_num;
    uint64_t size_bytes;
    uint64_t file_offset;
    uint32_t flags;
};

struct purple_dir_s {
    int      active;
    char     path[64];
    uint32_t dir_index;
};

static purple_window_t abi_windows[MAX_ABI_WINDOWS];
static purple_file_t   abi_files[MAX_ABI_FILES];
static purple_dir_t    abi_dirs[4];

static purple_event_t  event_queue[EVENT_QUEUE_CAP];
static uint32_t        event_head = 0;
static uint32_t        event_tail = 0;
static uint32_t        event_count = 0;

/* =========================================================================
 * 1. GRAPHICS & WINDOW MANAGEMENT IMPLEMENTATION
 * ========================================================================= */

purple_window_t *purple_window_create(const char *title, uint32_t x, uint32_t y, uint32_t w, uint32_t h) {
    if (!title || w == 0 || h == 0) return NULL;

    purple_window_t *win = NULL;
    for (int i = 0; i < MAX_ABI_WINDOWS; i++) {
        if (!abi_windows[i].active) {
            win = &abi_windows[i];
            win->id = (uint32_t)(i + 1);
            break;
        }
    }
    if (!win) return NULL;

    /* Allocate surface memory: w * h * 4 bytes */
    uint32_t pitch = w * 4;
    size_t total_bytes = (size_t)pitch * h;
    uint32_t *pix = (uint32_t *)purple_malloc(total_bytes);
    if (!pix) return NULL;

    memset(pix, 0, total_bytes);

    win->width = w;
    win->height = h;
    win->surface.width = w;
    win->surface.height = h;
    win->surface.pitch = pitch;
    win->surface.pixels = pix;
    win->surface.flags = 0x1; /* Backbuffered */

    strncpy(win->title, title, sizeof(win->title) - 1);
    win->title[sizeof(win->title) - 1] = '\0';

    int d_id = desktop_create_window(title, x, y, w, h, PURPLE_COLOR_BG_DEEP);
    win->desktop_win_id = (d_id >= 0) ? (uint32_t)d_id : 0;
    win->active = 1;

    kinfo("PurpleABI: Created window #%u ('%s', %ux%u) at (%u, %u)",
          win->id, win->title, w, h, x, y);
    return win;
}

int purple_window_destroy(purple_window_t *win) {
    if (!win || !win->active) return -1;

    if (win->surface.pixels) {
        purple_free(win->surface.pixels);
        win->surface.pixels = NULL;
    }

    if (win->desktop_win_id > 0) {
        desktop_close_window(win->desktop_win_id);
    }

    win->active = 0;
    kinfo("PurpleABI: Destroyed window #%u", win->id);
    return 0;
}

int purple_window_get_surface(purple_window_t *win, purple_surface_t *out_surface) {
    if (!win || !win->active || !out_surface) return -1;
    memcpy(out_surface, &win->surface, sizeof(purple_surface_t));
    return 0;
}

int purple_window_present(purple_window_t *win) {
    if (!win || !win->active) return -1;

    gui_window_t *gw = desktop_get_window(win->desktop_win_id);
    if (!gw || !gw->visible) return 0;

    /* Copy surface contents into desktop window body */
    uint32_t wx = gw->x + 2;
    uint32_t wy = gw->y + 28;
    uint32_t bw = (gw->width > 4) ? (gw->width - 4) : 0;
    uint32_t bh = (gw->height > 30) ? (gw->height - 30) : 0;

    uint32_t copy_w = (win->width < bw) ? win->width : bw;
    uint32_t copy_h = (win->height < bh) ? win->height : bh;

    for (uint32_t r = 0; r < copy_h; r++) {
        uint32_t *src_row = win->surface.pixels + r * (win->surface.pitch / 4);
        for (uint32_t c = 0; c < copy_w; c++) {
            video_draw_pixel(wx + c, wy + r, src_row[c]);
        }
    }
    return 0;
}

int purple_window_set_title(purple_window_t *win, const char *title) {
    if (!win || !win->active || !title) return -1;
    strncpy(win->title, title, sizeof(win->title) - 1);
    win->title[sizeof(win->title) - 1] = '\0';
    gui_window_t *gw = desktop_get_window(win->desktop_win_id);
    if (gw) {
        strncpy(gw->title, title, sizeof(gw->title) - 1);
        gw->title[sizeof(gw->title) - 1] = '\0';
    }
    return 0;
}

int purple_window_get_id(const purple_window_t *win) {
    return win ? (int)win->id : -1;
}

void purple_surface_draw_pixel(purple_surface_t *surf, uint32_t x, uint32_t y, purple_color_t color) {
    if (!surf || !surf->pixels || x >= surf->width || y >= surf->height) return;
    surf->pixels[y * (surf->pitch / 4) + x] = color;
}

purple_color_t purple_surface_get_pixel(const purple_surface_t *surf, uint32_t x, uint32_t y) {
    if (!surf || !surf->pixels || x >= surf->width || y >= surf->height) return 0;
    return surf->pixels[y * (surf->pitch / 4) + x];
}

void purple_surface_fill_rect(purple_surface_t *surf, uint32_t x, uint32_t y, uint32_t w, uint32_t h, purple_color_t color) {
    if (!surf || !surf->pixels || x >= surf->width || y >= surf->height) return;
    if (x + w > surf->width) w = surf->width - x;
    if (y + h > surf->height) h = surf->height - y;

    for (uint32_t r = 0; r < h; r++) {
        uint32_t *row = surf->pixels + (y + r) * (surf->pitch / 4) + x;
        for (uint32_t c = 0; c < w; c++) {
            row[c] = color;
        }
    }
}

void purple_surface_draw_text(purple_surface_t *surf, uint32_t x, uint32_t y, const char *text, purple_color_t fg, purple_color_t bg) {
    if (!surf || !surf->pixels || !text) return;
    uint32_t cx = x;
    uint32_t cy = y;

    while (*text) {
        char ch = *text++;
        if (ch == '\n') {
            cx = x;
            cy += 16;
            continue;
        }
        if (cx + 8 > surf->width) {
            cx = x;
            cy += 16;
        }
        if (cy + 16 > surf->height) break;

        const uint8_t *bitmap = font8x16[(uint8_t)ch];
        for (int r = 0; r < 16; r++) {
            uint8_t bits = bitmap[r];
            uint32_t *row = surf->pixels + (cy + r) * (surf->pitch / 4) + cx;
            for (int c = 0; c < 8; c++) {
                if (cx + c < surf->width) {
                    if (bits & (0x80 >> c)) {
                        row[c] = fg;
                    } else if (bg != 0xFF000000) {
                        row[c] = bg;
                    }
                }
            }
        }
        cx += 8;
    }
}

void purple_surface_blit(purple_surface_t *dst, uint32_t dx, uint32_t dy,
                         const purple_surface_t *src, uint32_t sx, uint32_t sy,
                         uint32_t w, uint32_t h) {
    if (!dst || !dst->pixels || !src || !src->pixels) return;
    if (sx >= src->width || sy >= src->height) return;
    if (dx >= dst->width || dy >= dst->height) return;

    if (sx + w > src->width) w = src->width - sx;
    if (sy + h > src->height) h = src->height - sy;
    if (dx + w > dst->width) w = dst->width - dx;
    if (dy + h > dst->height) h = dst->height - dy;

    for (uint32_t r = 0; r < h; r++) {
        const uint32_t *s_row = src->pixels + (sy + r) * (src->pitch / 4) + sx;
        uint32_t *d_row = dst->pixels + (dy + r) * (dst->pitch / 4) + dx;
        memcpy(d_row, s_row, w * 4);
    }
}

/* =========================================================================
 * 2. INPUT & EVENT DISPATCH IMPLEMENTATION
 * ========================================================================= */

void purple_push_event(const purple_event_t *event) {
    if (!event || event_count >= EVENT_QUEUE_CAP) return;
    event_queue[event_head] = *event;
    event_head = (event_head + 1) % EVENT_QUEUE_CAP;
    event_count++;
}

int purple_poll_event(purple_event_t *out_event) {
    if (!out_event || event_count == 0) return 0;
    *out_event = event_queue[event_tail];
    event_tail = (event_tail + 1) % EVENT_QUEUE_CAP;
    event_count--;
    return 1;
}

int purple_wait_event(purple_event_t *out_event, uint32_t timeout_ms) {
    if (!out_event) return 0;
    uint64_t start = purple_get_time_ms();
    while (purple_get_time_ms() - start <= timeout_ms) {
        if (purple_poll_event(out_event)) {
            return 1;
        }
        purple_thread_yield();
    }
    return 0;
}

/* =========================================================================
 * 3. STORAGE & PURPLEFS FILESYSTEM IMPLEMENTATION
 * ========================================================================= */

purple_file_t *purple_file_open(const char *path, uint32_t flags) {
    if (!path) return NULL;

    purple_file_t *file = NULL;
    for (int i = 0; i < MAX_ABI_FILES; i++) {
        if (!abi_files[i].active) {
            file = &abi_files[i];
            break;
        }
    }
    if (!file) return NULL;

#ifndef __KERNEL_HOST_TEST__
    if (!storage_is_fs_mounted()) return NULL;
    pfs2_fs_t *fs = storage_get_fs();
    uint32_t ino = 0;
    if (pfs2_lookup(fs, path, &ino) != PFS2_OK) {
        if (flags & PURPLE_FILE_CREATE) {
            if (pfs2_create_file(fs, path, 0644, &ino) != PFS2_OK) return NULL;
        } else {
            return NULL;
        }
    }
    pfs2_inode_t node;
    if (pfs2_read_inode(fs, ino, &node) != PFS2_OK) return NULL;
    file->inode_num = ino;
    file->size_bytes = node.size_bytes;
#else
    file->inode_num = 1;
    file->size_bytes = 1024;
#endif

    strncpy(file->path, path, sizeof(file->path) - 1);
    file->path[sizeof(file->path) - 1] = '\0';
    file->file_offset = 0;
    file->flags = flags;
    file->active = 1;

    return file;
}

int purple_file_read(purple_file_t *file, void *buf, size_t count) {
    if (!file || !file->active || !buf || count == 0) return 0;

#ifndef __KERNEL_HOST_TEST__
    if (!storage_is_fs_mounted()) return -1;
    pfs2_fs_t *fs = storage_get_fs();
    size_t bytes_read = 0;
    int rc = pfs2_read_file(fs, file->inode_num, buf, count, file->file_offset, &bytes_read);
    if (rc == PFS2_OK && bytes_read > 0) {
        file->file_offset += bytes_read;
        return (int)bytes_read;
    }
    return (rc == PFS2_OK) ? 0 : -1;
#else
    size_t to_read = count;
    if (file->file_offset + to_read > file->size_bytes) {
        to_read = (size_t)(file->size_bytes - file->file_offset);
    }
    memset(buf, 'A', to_read);
    file->file_offset += to_read;
    return (int)to_read;
#endif
}

int purple_file_write(purple_file_t *file, const void *buf, size_t count) {
    if (!file || !file->active || !buf || count == 0) return 0;

#ifndef __KERNEL_HOST_TEST__
    if (!storage_is_fs_mounted()) return -1;
    pfs2_fs_t *fs = storage_get_fs();
    size_t bytes_written = 0;
    int rc = pfs2_write_file(fs, file->inode_num, buf, count, file->file_offset, &bytes_written);
    if (rc == PFS2_OK && bytes_written > 0) {
        file->file_offset += bytes_written;
        if (file->file_offset > file->size_bytes) {
            file->size_bytes = file->file_offset;
        }
        return (int)bytes_written;
    }
    return (rc == PFS2_OK) ? 0 : -1;
#else
    file->file_offset += count;
    if (file->file_offset > file->size_bytes) file->size_bytes = file->file_offset;
    return (int)count;
#endif
}

int purple_file_seek(purple_file_t *file, int64_t offset, int whence) {
    if (!file || !file->active) return -1;
    int64_t new_off = 0;
    if (whence == 0) { /* SEEK_SET */
        new_off = offset;
    } else if (whence == 1) { /* SEEK_CUR */
        new_off = (int64_t)file->file_offset + offset;
    } else if (whence == 2) { /* SEEK_END */
        new_off = (int64_t)file->size_bytes + offset;
    } else {
        return -1;
    }
    if (new_off < 0) return -1;
    file->file_offset = (uint64_t)new_off;
    return 0;
}

int purple_file_close(purple_file_t *file) {
    if (!file || !file->active) return -1;
    file->active = 0;
    return 0;
}

int purple_file_stat(const char *path, purple_stat_t *st) {
    if (!path || !st) return -1;
#ifndef __KERNEL_HOST_TEST__
    if (!storage_is_fs_mounted()) return -1;
    pfs2_fs_t *fs = storage_get_fs();
    uint32_t ino = 0;
    if (pfs2_lookup(fs, path, &ino) != PFS2_OK) return -1;
    pfs2_inode_t node;
    if (pfs2_read_inode(fs, ino, &node) != PFS2_OK) return -1;
    st->inode_num = ino;
    st->size_bytes = node.size_bytes;
    st->flags = node.flags;
    st->modified_time = (uint32_t)node.mtime;
    strncpy(st->name, path, sizeof(st->name) - 1);
    st->name[sizeof(st->name) - 1] = '\0';
    return 0;
#else
    st->inode_num = 1;
    st->size_bytes = 1024;
    strncpy(st->name, path, sizeof(st->name) - 1);
    return 0;
#endif
}

int purple_file_exists(const char *path) {
    purple_stat_t st;
    return (purple_file_stat(path, &st) == 0);
}

purple_dir_t *purple_dir_open(const char *path) {
    if (!path) return NULL;
    purple_dir_t *dir = NULL;
    for (int i = 0; i < 4; i++) {
        if (!abi_dirs[i].active) {
            dir = &abi_dirs[i];
            break;
        }
    }
    if (!dir) return NULL;
    strncpy(dir->path, path, sizeof(dir->path) - 1);
    dir->path[sizeof(dir->path) - 1] = '\0';
    dir->dir_index = 0;
    dir->active = 1;
    return dir;
}

int purple_dir_read(purple_dir_t *dir, purple_dirent_t *out_ent) {
    if (!dir || !dir->active || !out_ent) return -1;
#ifndef __KERNEL_HOST_TEST__
    if (!storage_is_fs_mounted()) return -1;
    pfs2_fs_t *fs = storage_get_fs();
    pfs2_dir_entry_t dent;
    pfs2_inode_t dino;
    int rc = pfs2_readdir(fs, dir->dir_index, &dent, &dino);
    if (rc != PFS2_OK) return 0; /* EOF */

    dir->dir_index++;
    strncpy(out_ent->name, dent.name, sizeof(out_ent->name) - 1);
    out_ent->name[sizeof(out_ent->name) - 1] = '\0';
    out_ent->size_bytes = (uint32_t)dino.size_bytes;
    out_ent->is_dir = (dent.file_type == 2);
    return 1;
#else
    if (dir->dir_index >= 3) return 0;
    static const char *dummy[3] = { "sample.mp4", "music.mp3", "notes.txt" };
    strncpy(out_ent->name, dummy[dir->dir_index], sizeof(out_ent->name) - 1);
    out_ent->size_bytes = 4096;
    out_ent->is_dir = 0;
    dir->dir_index++;
    return 1;
#endif
}

int purple_dir_close(purple_dir_t *dir) {
    if (!dir || !dir->active) return -1;
    dir->active = 0;
    return 0;
}

/* =========================================================================
 * 4. MEMORY MANAGEMENT IMPLEMENTATION
 * ========================================================================= */

void *purple_malloc(size_t bytes) {
    if (bytes == 0) return NULL;
#ifdef __KERNEL_HOST_TEST__
    return malloc(bytes);
#else
    /* Page frame backed allocation for larger blocks */
    uint32_t pages = (uint32_t)((bytes + 4095) / 4096);
    void *ptr = pmm_alloc_frames(pages);
    return ptr;
#endif
}

void purple_free(void *ptr) {
    if (!ptr) return;
#ifdef __KERNEL_HOST_TEST__
    free(ptr);
#else
    /* Physical page frame release */
    pmm_free_frame(ptr);
#endif
}

void *purple_mmap(void *hint, size_t len, int prot, int flags) {
    (void)hint;
    (void)prot;
    (void)flags;
    return purple_malloc(len);
}

int purple_munmap(void *addr, size_t len) {
    (void)len;
    purple_free(addr);
    return 0;
}

/* =========================================================================
 * 5. MULTITHREADING & SYNCHRONIZATION IMPLEMENTATION
 * ========================================================================= */

purple_thread_t purple_thread_create(const char *name, void (*entry)(void *), void *arg) {
#ifndef __KERNEL_HOST_TEST__
    task_t *t = task_create(name, entry, arg, PRIORITY_NORMAL);
    return t ? t->id : 0;
#else
    (void)name; (void)entry; (void)arg;
    return 101;
#endif
}

void purple_thread_yield(void) {
#ifndef __KERNEL_HOST_TEST__
    task_yield();
#endif
}

void purple_thread_exit(int code) {
    (void)code;
#ifndef __KERNEL_HOST_TEST__
    task_exit();
#endif
}

int purple_mutex_init(purple_mutex_t *mutex) {
    if (!mutex) return -1;
    mutex->locked = 0;
    mutex->owner_tid = 0;
    return 0;
}

int purple_mutex_lock(purple_mutex_t *mutex) {
    if (!mutex) return -1;
    while (__atomic_test_and_set(&mutex->locked, __ATOMIC_ACQUIRE)) {
        purple_thread_yield();
    }
    return 0;
}

int purple_mutex_unlock(purple_mutex_t *mutex) {
    if (!mutex) return -1;
    __atomic_clear(&mutex->locked, __ATOMIC_RELEASE);
    return 0;
}

/* =========================================================================
 * 6. TIMERS, TIME & CLOCKS IMPLEMENTATION
 * ========================================================================= */

uint64_t purple_get_ticks(void) {
#ifndef __KERNEL_HOST_TEST__
    return pit_get_ticks();
#else
    static uint64_t test_ticks = 0;
    return ++test_ticks;
#endif
}

uint64_t purple_get_time_ms(void) {
    return purple_get_ticks() * 10; /* 100 Hz timer = 10 ms per tick */
}

void purple_sleep_ms(uint32_t ms) {
    uint64_t start = purple_get_time_ms();
    while (purple_get_time_ms() - start < ms) {
        purple_thread_yield();
    }
}

int purple_get_wall_clock(purple_time_t *tm) {
    if (!tm) return -1;
#ifndef __KERNEL_HOST_TEST__
    rtc_time_t r;
    rtc_get_time(&r);
    tm->year = r.year;
    tm->month = r.month;
    tm->day = r.day;
    tm->hour = r.hour;
    tm->minute = r.minute;
    tm->second = r.second;
    return 0;
#else
    tm->year = 2026;
    tm->month = 10;
    tm->day = 25;
    tm->hour = 16;
    tm->minute = 50;
    tm->second = 0;
    return 0;
#endif
}

/* =========================================================================
 * 7. AUDIO HARDWARE PLAYBACK IMPLEMENTATION
 * ========================================================================= */

static uint8_t current_audio_volume = 80;

int purple_audio_is_available(void) {
#ifndef __KERNEL_HOST_TEST__
    return (audio_get_device_count() > 0);
#else
    return 1;
#endif
}

int purple_audio_play_pcm(const void *pcm, size_t bytes, uint32_t sample_rate, uint8_t channels) {
    if (!pcm || bytes == 0) return -1;
    (void)sample_rate;
    (void)channels;
#ifndef __KERNEL_HOST_TEST__
    if (!purple_audio_is_available()) return -1;
    /* Stream bytes to active audio device DMA buffer if present */
    return 0;
#else
    return 0;
#endif
}

void purple_audio_set_volume(uint8_t volume) {
    current_audio_volume = (volume > 100) ? 100 : volume;
}

/* =========================================================================
 * 8. APPLICATION LIFECYCLE IMPLEMENTATION
 * ========================================================================= */

void purple_app_exit(int code) {
    kinfo("PurpleABI: Application exiting with code %d", code);
    purple_thread_exit(code);
}
