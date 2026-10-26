#ifndef PURPLE_ABI_H
#define PURPLE_ABI_H

#include "types.h"

#ifdef __cplusplus
extern "C" {
#endif

/* =========================================================================
 * PURPLE OS APPLICATION ABI SPECIFICATION (v1.0)
 * =========================================================================
 * Kernel: Aegis64 (x86-64)
 * Target Architecture: System V AMD64 ABI (freestanding native execution)
 * Surface Pixel Format: 32-bit ARGB8888 (Canonical 0x00RRGGBB)
 * ========================================================================= */

#define PURPLE_ABI_VERSION_MAJOR 1
#define PURPLE_ABI_VERSION_MINOR 0

typedef uint32_t purple_color_t;
typedef uint32_t purple_handle_t;

/* Standard Canonical Colors */
#define PURPLE_COLOR_BLACK       0x00000000
#define PURPLE_COLOR_WHITE       0x00FFFFFF
#define PURPLE_COLOR_RED         0x00EF476F
#define PURPLE_COLOR_GREEN       0x0006D6A0
#define PURPLE_COLOR_BLUE        0x00118AB2
#define PURPLE_COLOR_YELLOW      0x00FFD166
#define PURPLE_COLOR_PURPLE      0x009D4EDD
#define PURPLE_COLOR_BG_DEEP     0x00130A24
#define PURPLE_COLOR_BAR         0x001F103A

/* Rectangle structure */
typedef struct {
    int32_t  x;
    int32_t  y;
    uint32_t w;
    uint32_t h;
} purple_rect_t;

/* Surface descriptor for graphics presentation */
typedef struct {
    uint32_t  width;
    uint32_t  height;
    uint32_t  pitch;          /* Bytes per row */
    uint32_t *pixels;         /* Linear 32bpp RGB pixel buffer */
    uint32_t  flags;          /* 0x1 = Backbuffered, 0x2 = Direct FB */
} purple_surface_t;

/* Window handle opaque structure */
typedef struct purple_window_s purple_window_t;

/* Event subsystem */
typedef enum {
    PURPLE_EVENT_NONE = 0,
    PURPLE_EVENT_KEY_DOWN,
    PURPLE_EVENT_KEY_UP,
    PURPLE_EVENT_MOUSE_MOVE,
    PURPLE_EVENT_MOUSE_DOWN,
    PURPLE_EVENT_MOUSE_UP,
    PURPLE_EVENT_WINDOW_CLOSE,
    PURPLE_EVENT_WINDOW_RESIZE,
    PURPLE_EVENT_WINDOW_FOCUS
} purple_event_type_t;

typedef struct {
    purple_event_type_t type;
    uint32_t window_id;
    union {
        struct {
            char     character;
            uint8_t  scancode;
            uint8_t  modifiers; /* Bit 0: Shift, Bit 1: Ctrl, Bit 2: Alt */
        } key;
        struct {
            int32_t  x;
            int32_t  y;
            uint8_t  buttons;   /* Bit 0: Left, Bit 1: Right, Bit 2: Middle */
        } mouse;
        struct {
            uint32_t new_width;
            uint32_t new_height;
        } resize;
    };
} purple_event_t;

/* File access flags */
#define PURPLE_FILE_READ   0x01
#define PURPLE_FILE_WRITE  0x02
#define PURPLE_FILE_CREATE 0x04

typedef struct {
    uint32_t inode_num;
    uint64_t size_bytes;
    uint32_t flags;
    uint32_t modified_time;
    char     name[64];
} purple_stat_t;

typedef struct {
    char     name[64];
    uint32_t size_bytes;
    uint8_t  is_dir;
} purple_dirent_t;

typedef struct purple_file_s purple_file_t;
typedef struct purple_dir_s purple_dir_t;

/* Thread and synchronization types */
typedef uint32_t purple_thread_t;
typedef struct {
    volatile uint32_t locked;
    uint32_t          owner_tid;
} purple_mutex_t;

/* Real-time wall clock */
typedef struct {
    uint16_t year;
    uint8_t  month;
    uint8_t  day;
    uint8_t  hour;
    uint8_t  minute;
    uint8_t  second;
} purple_time_t;

/* =========================================================================
 * 1. GRAPHICS & WINDOW MANAGEMENT ABI
 * ========================================================================= */

purple_window_t *purple_window_create(const char *title, uint32_t x, uint32_t y, uint32_t w, uint32_t h);
int              purple_window_destroy(purple_window_t *win);
int              purple_window_get_surface(purple_window_t *win, purple_surface_t *out_surface);
int              purple_window_present(purple_window_t *win);
int              purple_window_set_title(purple_window_t *win, const char *title);
int              purple_window_get_id(const purple_window_t *win);

/* Surface primitive rendering */
void purple_surface_draw_pixel(purple_surface_t *surf, uint32_t x, uint32_t y, purple_color_t color);
purple_color_t purple_surface_get_pixel(const purple_surface_t *surf, uint32_t x, uint32_t y);
void purple_surface_fill_rect(purple_surface_t *surf, uint32_t x, uint32_t y, uint32_t w, uint32_t h, purple_color_t color);
void purple_surface_draw_text(purple_surface_t *surf, uint32_t x, uint32_t y, const char *text, purple_color_t fg, purple_color_t bg);
void purple_surface_blit(purple_surface_t *dst, uint32_t dx, uint32_t dy,
                         const purple_surface_t *src, uint32_t sx, uint32_t sy,
                         uint32_t w, uint32_t h);

/* =========================================================================
 * 2. INPUT & EVENT DISPATCH ABI
 * ========================================================================= */

int  purple_poll_event(purple_event_t *out_event);
int  purple_wait_event(purple_event_t *out_event, uint32_t timeout_ms);
void purple_push_event(const purple_event_t *event);

/* =========================================================================
 * 3. STORAGE & PURPLEFS FILESYSTEM ABI
 * ========================================================================= */

purple_file_t *purple_file_open(const char *path, uint32_t flags);
int            purple_file_read(purple_file_t *file, void *buf, size_t count);
int            purple_file_write(purple_file_t *file, const void *buf, size_t count);
int            purple_file_seek(purple_file_t *file, int64_t offset, int whence);
int            purple_file_close(purple_file_t *file);
int            purple_file_stat(const char *path, purple_stat_t *st);
int            purple_file_exists(const char *path);

/* Directory traversal */
purple_dir_t  *purple_dir_open(const char *path);
int            purple_dir_read(purple_dir_t *dir, purple_dirent_t *out_ent);
int            purple_dir_close(purple_dir_t *dir);

/* =========================================================================
 * 4. MEMORY MANAGEMENT ABI
 * ========================================================================= */

void *purple_malloc(size_t bytes);
void  purple_free(void *ptr);
void *purple_mmap(void *hint, size_t len, int prot, int flags);
int   purple_munmap(void *addr, size_t len);

/* =========================================================================
 * 5. MULTITHREADING & SYNCHRONIZATION ABI
 * ========================================================================= */

purple_thread_t purple_thread_create(const char *name, void (*entry)(void *), void *arg);
void            purple_thread_yield(void);
void            purple_thread_exit(int code);
int             purple_mutex_init(purple_mutex_t *mutex);
int             purple_mutex_lock(purple_mutex_t *mutex);
int             purple_mutex_unlock(purple_mutex_t *mutex);

/* =========================================================================
 * 6. TIMERS, TIME & CLOCKS ABI
 * ========================================================================= */

uint64_t purple_get_ticks(void);
uint64_t purple_get_time_ms(void);
void     purple_sleep_ms(uint32_t ms);
int      purple_get_wall_clock(purple_time_t *tm);

/* =========================================================================
 * 7. AUDIO HARDWARE PLAYBACK ABI
 * ========================================================================= */

int  purple_audio_is_available(void);
int  purple_audio_play_pcm(const void *pcm, size_t bytes, uint32_t sample_rate, uint8_t channels);
void purple_audio_set_volume(uint8_t volume);

/* =========================================================================
 * 8. APPLICATION LIFECYCLE ABI
 * ========================================================================= */

void purple_app_exit(int code);

#ifdef __cplusplus
}
#endif

#endif /* PURPLE_ABI_H */
