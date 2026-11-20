#ifndef PURPLE_GUI_H
#define PURPLE_GUI_H
#include <stddef.h>
#include <stdint.h>

#define PURPLE_GUI_ABI_VERSION 1
#define PURPLE_GUI_CONSOLE 0x01
#define PURPLE_GUI_WINDOWS 0x02
#define PURPLE_GUI_SHELL 0x04
#define PURPLE_GUI_SHELL_OUTPUT_SIZE 4096
#define PURPLE_GUI_SHELL_PROMPT_SIZE 96

enum purple_gui_call {
    PURPLE_GUI_VERSION = 0x1100,
    PURPLE_GUI_STARTUP = 0x1101,
    PURPLE_GUI_CREATE = 0x1102,
    PURPLE_GUI_CLOSE = 0x1103,
    PURPLE_GUI_PRESENT = 0x1104,
    PURPLE_GUI_POLL = 0x1105,
    PURPLE_GUI_SIZE = 0x1106,
    PURPLE_GUI_LOG = 0x1107,
    PURPLE_GUI_MAP = 0x1108,
    PURPLE_GUI_UNMAP = 0x1109,
    PURPLE_GUI_SHELL_COMMAND = 0x110a,
    PURPLE_GUI_SHELL_PROMPT = 0x110b,
    PURPLE_GUI_WINDOW_GLASS = 0x110c
};

enum purple_gui_event_kind {
    PURPLE_GUI_POINTER = 1,
    PURPLE_GUI_KEY = 2,
    PURPLE_GUI_FOCUS = 3,
    PURPLE_GUI_CLOSE_REQUEST = 4,
    PURPLE_GUI_RESIZE = 5,
    PURPLE_GUI_OVERFLOW = 6
};

struct purple_gui_startup {
    uint64_t version;
    uint64_t window;
    uint64_t width;
    uint64_t height;
    const void *resources;
    uint64_t resources_size;
    const void *metadata;
    uint64_t metadata_size;
};

struct purple_gui_event {
    uint32_t kind;
    int32_t x;
    int32_t y;
    uint32_t detail;
};

static inline int64_t purple_gui_syscall(uint64_t number, uint64_t a, uint64_t b, uint64_t c)
{
    int64_t result;
    __asm__ volatile("syscall" : "=a"(result)
                     : "a"(number), "D"(a), "S"(b), "d"(c)
                     : "rcx", "r11", "memory", "cc");
    return result;
}

static inline const struct purple_gui_startup *purple_gui_get_startup(void)
{
    int64_t result = purple_gui_syscall(PURPLE_GUI_STARTUP, 0, 0, 0);
    return result > 0 ? (const struct purple_gui_startup *)(uintptr_t)result : NULL;
}

static inline int64_t purple_gui_create(const char *title, uint64_t width, uint64_t height)
{
    return purple_gui_syscall(PURPLE_GUI_CREATE, (uintptr_t)title, width, height);
}

static inline int64_t purple_gui_present(uint64_t window, const uint32_t *pixels, size_t bytes)
{
    return purple_gui_syscall(PURPLE_GUI_PRESENT, window, (uintptr_t)pixels, bytes);
}

/* Enable live blurred acrylic and ARGB8888 presentation for an owned window.
 * Call before presenting; alpha zero shows the compositor backdrop. */
static inline int64_t purple_gui_set_glass(uint64_t window, int enabled)
{
    return purple_gui_syscall(PURPLE_GUI_WINDOW_GLASS, window, (uint64_t)!!enabled, 0);
}

static inline int64_t purple_gui_poll(uint64_t window, struct purple_gui_event *event)
{
    return purple_gui_syscall(PURPLE_GUI_POLL, window, (uintptr_t)event, 0);
}

static inline void purple_gui_yield(void)
{
    (void)purple_gui_syscall(24, 0, 0, 0);
}

#endif
