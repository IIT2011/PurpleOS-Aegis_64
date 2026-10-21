#ifndef VIDEO_H
#define VIDEO_H

#include "types.h"
#include "multiboot.h"

/* Standard Purple OS Palette */
#define COLOR_PURPLE_BG    0x1E102E   /* Deep purple */
#define COLOR_PURPLE_BAR   0x3C1B63   /* Header bar */
#define COLOR_PURPLE_ACC   0x9D4EDD   /* Vibrant purple accent */
#define COLOR_WHITE        0xF8F8FF   /* Off-white text */
#define COLOR_GRAY         0x888899   /* Muted text */
#define COLOR_GREEN        0x06D6A0   /* Status OK */
#define COLOR_RED          0xEF476F   /* Status ERROR */
#define COLOR_GOLD         0xFFD166   /* Status WARN */

void video_init(struct multiboot_info *mbi);
int video_init_bochs_vbe(uint32_t width, uint32_t height);
int video_set_mode(uint32_t width, uint32_t height, uint32_t bpp);
int video_is_available(void);
uint32_t video_color(uint8_t r, uint8_t g, uint8_t b);
void video_clear(uint32_t color);
void video_draw_pixel(uint32_t x, uint32_t y, uint32_t color);
void video_draw_rect(uint32_t x, uint32_t y, uint32_t w, uint32_t h, uint32_t color);
void video_putc(char c);
void video_puts(const char *s);
void video_set_cursor(uint32_t col, uint32_t row);
void video_set_colors(uint32_t fg, uint32_t bg);
uint32_t video_get_width(void);
uint32_t video_get_height(void);

#endif /* VIDEO_H */
