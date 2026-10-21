#include "video.h"
#include "font8x16.h"
#include "string.h"
#include "serial.h"

static uint32_t *fb_ptr = NULL;
static uint32_t fb_width = 0;
static uint32_t fb_height = 0;
static uint32_t fb_pitch = 0;
static uint8_t  fb_bpp = 0;
static uint8_t  fb_type = 0;

static uint8_t r_pos = 16, r_size = 8;
static uint8_t g_pos = 8,  g_size = 8;
static uint8_t b_pos = 0,  b_size = 8;

static uint32_t cursor_x = 0;
static uint32_t cursor_y = 0;
static uint32_t current_fg = COLOR_WHITE;
static uint32_t current_bg = COLOR_PURPLE_BG;

static int fb_available = 0;

int video_is_available(void) {
    return fb_available;
}

uint32_t video_get_width(void) {
    return fb_width;
}

uint32_t video_get_height(void) {
    return fb_height;
}

uint32_t video_color(uint8_t r, uint8_t g, uint8_t b) {
    uint32_t r_val = (r >> (8 - r_size)) << r_pos;
    uint32_t g_val = (g >> (8 - g_size)) << g_pos;
    uint32_t b_val = (b >> (8 - b_size)) << b_pos;
    return r_val | g_val | b_val;
}

void video_init(struct multiboot_info *mbi) {
    if (!mbi || !(mbi->flags & MULTIBOOT_INFO_FRAMEBUFFER_INFO)) {
        kinfo("Video: Framebuffer information not available from bootloader");
        fb_available = 0;
        return;
    }

    fb_ptr = (uint32_t *)(uintptr_t)mbi->framebuffer_addr;
    fb_width = mbi->framebuffer_width;
    fb_height = mbi->framebuffer_height;
    fb_pitch = mbi->framebuffer_pitch;
    fb_bpp = mbi->framebuffer_bpp;
    fb_type = mbi->framebuffer_type;

    if (fb_type == MULTIBOOT_FRAMEBUFFER_TYPE_RGB && fb_bpp == 32) {
        /* Check if bootloader provided valid color bit masks */
        if (mbi->framebuffer_red_mask_size != 0) {
            r_pos = mbi->framebuffer_red_field_position;
            r_size = mbi->framebuffer_red_mask_size;
            g_pos = mbi->framebuffer_green_field_position;
            g_size = mbi->framebuffer_green_mask_size;
            b_pos = mbi->framebuffer_blue_field_position;
            b_size = mbi->framebuffer_blue_mask_size;
        } else {
            /* Historical fallback: When red_mask_size is 0, default to standard 32-bit RGBX */
            r_pos = 16; r_size = 8;
            g_pos = 8;  g_size = 8;
            b_pos = 0;  b_size = 8;
        }
        fb_available = 1;
        kinfo("Video: Initialized %ux%u@%u bpp (pitch %u bytes) at %p",
              fb_width, fb_height, fb_bpp, fb_pitch, (void *)fb_ptr);
        kinfo("Video: Color channels R:[pos=%u, sz=%u] G:[pos=%u, sz=%u] B:[pos=%u, sz=%u]",
              r_pos, r_size, g_pos, g_size, b_pos, b_size);
    } else {
        kinfo("Video: Unsupported framebuffer format (type %u, bpp %u)", fb_type, fb_bpp);
        fb_available = 0;
        return;
    }

    cursor_x = 0;
    cursor_y = 0;
    video_clear(current_bg);
}

void video_draw_pixel(uint32_t x, uint32_t y, uint32_t color) {
    if (!fb_available || x >= fb_width || y >= fb_height) return;
    uint32_t *dest = (uint32_t *)((uint8_t *)fb_ptr + y * fb_pitch) + x;
    *dest = color;
}

void video_draw_rect(uint32_t x, uint32_t y, uint32_t w, uint32_t h, uint32_t color) {
    if (!fb_available) return;
    if (x >= fb_width || y >= fb_height) return;
    if (x + w > fb_width) w = fb_width - x;
    if (y + h > fb_height) h = fb_height - y;

    for (uint32_t r = 0; r < h; r++) {
        uint32_t *row = (uint32_t *)((uint8_t *)fb_ptr + (y + r) * fb_pitch) + x;
        for (uint32_t c = 0; c < w; c++) {
            row[c] = color;
        }
    }
}

void video_clear(uint32_t color) {
    if (!fb_available) return;
    video_draw_rect(0, 0, fb_width, fb_height, color);
    cursor_x = 0;
    cursor_y = 0;
}

void video_set_cursor(uint32_t col, uint32_t row) {
    cursor_x = col * 8;
    cursor_y = row * 16;
}

void video_set_colors(uint32_t fg, uint32_t bg) {
    current_fg = fg;
    current_bg = bg;
}

static void video_scroll(void) {
    if (!fb_available) return;
    uint32_t row_bytes = fb_pitch;
    uint32_t copy_height = fb_height - 16;

    for (uint32_t y = 0; y < copy_height; y++) {
        uint8_t *dst = (uint8_t *)fb_ptr + y * row_bytes;
        const uint8_t *src = (const uint8_t *)fb_ptr + (y + 16) * row_bytes;
        memcpy(dst, src, row_bytes);
    }
    /* Clear bottom row */
    video_draw_rect(0, fb_height - 16, fb_width, 16, current_bg);
    cursor_y = fb_height - 16;
}

static void video_draw_glyph(uint32_t x, uint32_t y, char c, uint32_t fg, uint32_t bg) {
    uint8_t ch = (uint8_t)c;
    if (ch >= 128) ch = '?';
    const uint8_t *bitmap = font8x16[ch];

    for (uint32_t row = 0; row < 16; row++) {
        uint8_t bits = bitmap[row];
        uint32_t *dest = (uint32_t *)((uint8_t *)fb_ptr + (y + row) * fb_pitch) + x;
        for (uint32_t col = 0; col < 8; col++) {
            if (x + col < fb_width && y + row < fb_height) {
                dest[col] = (bits & (0x80 >> col)) ? fg : bg;
            }
        }
    }
}

void video_putc(char c) {
    if (!fb_available) return;

    if (c == '\n') {
        cursor_x = 0;
        cursor_y += 16;
        if (cursor_y + 16 > fb_height) {
            video_scroll();
        }
        return;
    } else if (c == '\r') {
        cursor_x = 0;
        return;
    } else if (c == '\t') {
        cursor_x = (cursor_x + 32) & ~31;
        if (cursor_x + 8 > fb_width) {
            video_putc('\n');
        }
        return;
    } else if (c == '\b') {
        if (cursor_x >= 8) {
            cursor_x -= 8;
            video_draw_glyph(cursor_x, cursor_y, ' ', current_fg, current_bg);
        }
        return;
    }

    if (cursor_x + 8 > fb_width) {
        cursor_x = 0;
        cursor_y += 16;
        if (cursor_y + 16 > fb_height) {
            video_scroll();
        }
    }

    video_draw_glyph(cursor_x, cursor_y, c, current_fg, current_bg);
    cursor_x += 8;
}

void video_puts(const char *s) {
    if (!s) return;
    while (*s) {
        video_putc(*s++);
    }
}
