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

uint32_t video_rgb_to_hw(uint32_t rgb) {
    if (r_pos == 16 && g_pos == 8 && b_pos == 0 && r_size == 8 && g_size == 8 && b_size == 8) {
        return rgb;
    }
    uint8_t r = (rgb >> 16) & 0xFF;
    uint8_t g = (rgb >> 8) & 0xFF;
    uint8_t b = rgb & 0xFF;
    uint32_t r_val = (r >> (8 - r_size)) << r_pos;
    uint32_t g_val = (g >> (8 - g_size)) << g_pos;
    uint32_t b_val = (b >> (8 - b_size)) << b_pos;
    return r_val | g_val | b_val;
}

uint32_t video_hw_to_rgb(uint32_t hw) {
    if (r_pos == 16 && g_pos == 8 && b_pos == 0 && r_size == 8 && g_size == 8 && b_size == 8) {
        return hw;
    }
    uint8_t r = ((hw >> r_pos) << (8 - r_size)) & 0xFF;
    uint8_t g = ((hw >> g_pos) << (8 - g_size)) & 0xFF;
    uint8_t b = ((hw >> b_pos) << (8 - b_size)) & 0xFF;
    return ((uint32_t)r << 16) | ((uint32_t)g << 8) | (uint32_t)b;
}

void video_set_channel_masks(uint8_t rpos, uint8_t rsz, uint8_t gpos, uint8_t gsz, uint8_t bpos, uint8_t bsz) {
    r_pos = rpos; r_size = rsz;
    g_pos = gpos; g_size = gsz;
    b_pos = bpos; b_size = bsz;
    kinfo("Video: Dynamic channel masks set to R:[pos=%u, sz=%u] G:[pos=%u, sz=%u] B:[pos=%u, sz=%u]",
          r_pos, r_size, g_pos, g_size, b_pos, b_size);
}

void video_get_channel_masks(uint8_t *rpos, uint8_t *gpos, uint8_t *bpos) {
    if (rpos) *rpos = r_pos;
    if (gpos) *gpos = g_pos;
    if (bpos) *bpos = b_pos;
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

#ifndef __KERNEL_HOST_TEST__
#include "io.h"
#include "pci.h"
#include "vmm.h"

#define VBE_DISPI_IOPORT_INDEX 0x01CE
#define VBE_DISPI_IOPORT_DATA  0x01CF

#define VBE_DISPI_INDEX_ID     0
#define VBE_DISPI_INDEX_XRES   1
#define VBE_DISPI_INDEX_YRES   2
#define VBE_DISPI_INDEX_BPP    3
#define VBE_DISPI_INDEX_ENABLE 4

#define VBE_DISPI_DISABLED     0x00
#define VBE_DISPI_ENABLED      0x01
#define VBE_DISPI_LFB_ENABLED  0x40

int video_init_bochs_vbe(uint32_t width, uint32_t height) {
    if (fb_available && fb_width == width && fb_height == height) return 0;

    outw(VBE_DISPI_IOPORT_INDEX, VBE_DISPI_INDEX_ID);
    uint16_t vbe_id = inw(VBE_DISPI_IOPORT_DATA);
    if (vbe_id < 0xB0C0 || vbe_id > 0xB0C6) {
        return -1;
    }

    const pci_device_t *vga = pci_find_device(0x1234, 0x1111);
    if (!vga) {
        return -2;
    }

    /* Enable I/O and Memory Space decoding on PCI VGA controller */
    pci_enable_io(vga->bus, vga->dev, vga->func);

    uint64_t lfb_phys = pci_get_bar_address(vga, 0);
    if (lfb_phys == 0) {
        return -2;
    }

    /* Ensure linear framebuffer is mapped in page tables */
    page_table_t *pml4 = vmm_get_kernel_pml4();
    if (pml4) {
        uint32_t fb_size_pages = (width * height * 4 + 4095) / 4096;
        for (uint32_t p = 0; p < fb_size_pages; p++) {
            uint64_t addr = lfb_phys + ((uint64_t)p * 4096ULL);
            vmm_map_page(pml4, addr, addr, PAGE_PRESENT | PAGE_WRITABLE);
        }
    }

    outw(VBE_DISPI_IOPORT_INDEX, VBE_DISPI_INDEX_ENABLE);
    outw(VBE_DISPI_IOPORT_DATA, VBE_DISPI_DISABLED);

    outw(VBE_DISPI_IOPORT_INDEX, VBE_DISPI_INDEX_XRES);
    outw(VBE_DISPI_IOPORT_DATA, (uint16_t)width);

    outw(VBE_DISPI_IOPORT_INDEX, VBE_DISPI_INDEX_YRES);
    outw(VBE_DISPI_IOPORT_DATA, (uint16_t)height);

    outw(VBE_DISPI_IOPORT_INDEX, VBE_DISPI_INDEX_BPP);
    outw(VBE_DISPI_IOPORT_DATA, 32);

    outw(VBE_DISPI_IOPORT_INDEX, VBE_DISPI_INDEX_ENABLE);
    outw(VBE_DISPI_IOPORT_DATA, VBE_DISPI_ENABLED | VBE_DISPI_LFB_ENABLED);

    fb_ptr = (uint32_t *)(uintptr_t)lfb_phys;
    fb_width = width;
    fb_height = height;
    fb_pitch = width * 4;
    fb_bpp = 32;
    fb_type = MULTIBOOT_FRAMEBUFFER_TYPE_RGB;
    r_pos = 16; r_size = 8;
    g_pos = 8;  g_size = 8;
    b_pos = 0;  b_size = 8;
    fb_available = 1;

    kinfo("Video: Activated Bochs VBE %ux%u@32bpp linear framebuffer at %p",
          width, height, (void *)fb_ptr);
    video_clear(current_bg);
    return 0;
}

int video_set_mode(uint32_t width, uint32_t height, uint32_t bpp) {
    (void)bpp;
    fb_available = 0;
    return video_init_bochs_vbe(width, height);
}
#else
int video_init_bochs_vbe(uint32_t width, uint32_t height) {
    (void)width;
    (void)height;
    return 0;
}

int video_set_mode(uint32_t width, uint32_t height, uint32_t bpp) {
    (void)width;
    (void)height;
    (void)bpp;
    return 0;
}
#endif

void video_draw_pixel(uint32_t x, uint32_t y, uint32_t color) {
    if (!fb_available || x >= fb_width || y >= fb_height) return;
    uint32_t *dest = (uint32_t *)((uint8_t *)fb_ptr + y * fb_pitch) + x;
    *dest = video_rgb_to_hw(color);
}

uint32_t video_get_pixel(uint32_t x, uint32_t y) {
    if (!fb_available || x >= fb_width || y >= fb_height) return 0;
    const uint32_t *src = (const uint32_t *)((const uint8_t *)fb_ptr + y * fb_pitch) + x;
    return video_hw_to_rgb(*src);
}

void video_draw_rect(uint32_t x, uint32_t y, uint32_t w, uint32_t h, uint32_t color) {
    if (!fb_available) return;
    if (x >= fb_width || y >= fb_height) return;
    if (x + w > fb_width) w = fb_width - x;
    if (y + h > fb_height) h = fb_height - y;

    uint32_t hw_color = video_rgb_to_hw(color);
    for (uint32_t r = 0; r < h; r++) {
        uint32_t *row = (uint32_t *)((uint8_t *)fb_ptr + (y + r) * fb_pitch) + x;
        for (uint32_t c = 0; c < w; c++) {
            row[c] = hw_color;
        }
    }
}

int video_test_colors(void) {
    if (!fb_available) return -1;
    static const uint32_t test_colors[8] = {
        0xFF0000, /* RED */
        0x00FF00, /* GREEN */
        0x0000FF, /* BLUE */
        0xFFFF00, /* YELLOW */
        0x00FFFF, /* CYAN */
        0xFF00FF, /* MAGENTA */
        0xFFFFFF, /* WHITE */
        0x000000  /* BLACK */
    };
    uint32_t bar_w = fb_width / 8;
    uint32_t bar_h = 32;
    for (int i = 0; i < 8; i++) {
        video_draw_rect(i * bar_w, 0, bar_w, bar_h, test_colors[i]);
    }
    for (int i = 0; i < 8; i++) {
        uint32_t px = i * bar_w + bar_w / 2;
        uint32_t py = bar_h / 2;
        uint32_t readback = video_get_pixel(px, py);
        if ((readback & 0xFFFFFF) != test_colors[i]) {
            return -2;
        }
    }
    return 0;
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
    const uint8_t *bitmap = font8x16[ch];
    uint32_t hw_fg = video_rgb_to_hw(fg);
    uint32_t hw_bg = video_rgb_to_hw(bg);

    for (uint32_t row = 0; row < 16; row++) {
        if (y + row >= fb_height) break;
        uint8_t bits = bitmap[row];
        uint32_t *dest = (uint32_t *)((uint8_t *)fb_ptr + (y + row) * fb_pitch) + x;
        for (uint32_t col = 0; col < 8; col++) {
            if (x + col < fb_width) {
                dest[col] = (bits & (0x80 >> col)) ? hw_fg : hw_bg;
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
