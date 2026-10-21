#ifndef __KERNEL_HOST_TEST__
#define __KERNEL_HOST_TEST__
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "video.h"
#include "font8x16.h"
#include "multiboot.h"

#define FB_WIDTH  80
#define FB_HEIGHT 40
#define FB_PITCH  (FB_WIDTH * 4)

static uint32_t fake_fb[FB_WIDTH * FB_HEIGHT];

static void setup_test_framebuffer(struct multiboot_info *mbi) {
    memset(fake_fb, 0, sizeof(fake_fb));
    memset(mbi, 0, sizeof(*mbi));
    mbi->flags = MULTIBOOT_INFO_FRAMEBUFFER_INFO;
    mbi->framebuffer_addr = (uint64_t)(uintptr_t)fake_fb;
    mbi->framebuffer_width = FB_WIDTH;
    mbi->framebuffer_height = FB_HEIGHT;
    mbi->framebuffer_pitch = FB_PITCH;
    mbi->framebuffer_bpp = 32;
    mbi->framebuffer_type = MULTIBOOT_FRAMEBUFFER_TYPE_RGB;
    mbi->framebuffer_red_field_position = 16;
    mbi->framebuffer_red_mask_size = 8;
    mbi->framebuffer_green_field_position = 8;
    mbi->framebuffer_green_mask_size = 8;
    mbi->framebuffer_blue_field_position = 0;
    mbi->framebuffer_blue_mask_size = 8;
}

static void test_video_init_multiboot(void) {
    printf("[TEST  1] Running test_video_init_multiboot                ... ");
    struct multiboot_info mbi;
    setup_test_framebuffer(&mbi);

    video_init(&mbi);
    assert(video_is_available() == 1);
    assert(video_get_width() == FB_WIDTH);
    assert(video_get_height() == FB_HEIGHT);

    /* Test invalid framebuffer flags */
    mbi.flags = 0;
    video_init(&mbi);
    assert(video_is_available() == 0);

    /* Test unsupported bpp */
    setup_test_framebuffer(&mbi);
    mbi.framebuffer_bpp = 16;
    video_init(&mbi);
    assert(video_is_available() == 0);

    /* Re-init valid for subsequent tests */
    setup_test_framebuffer(&mbi);
    video_init(&mbi);
    assert(video_is_available() == 1);

    printf("PASSED\n");
}

static void test_video_color_masks(void) {
    printf("[TEST  2] Running test_video_color_masks                   ... ");
    struct multiboot_info mbi;
    setup_test_framebuffer(&mbi);
    video_init(&mbi);

    uint32_t col = video_color(0x12, 0x34, 0x56);
    assert(col == 0x00123456);

    uint32_t red = video_color(0xFF, 0x00, 0x00);
    assert(red == 0x00FF0000);

    uint32_t green = video_color(0x00, 0xFF, 0x00);
    assert(green == 0x0000FF00);

    uint32_t blue = video_color(0x00, 0x00, 0xFF);
    assert(blue == 0x000000FF);

    printf("PASSED\n");
}

static void test_video_draw_pixel_and_rect(void) {
    printf("[TEST  3] Running test_video_draw_pixel_and_rect           ... ");
    struct multiboot_info mbi;
    setup_test_framebuffer(&mbi);
    video_init(&mbi);

    video_clear(0x000000);
    for (int i = 0; i < FB_WIDTH * FB_HEIGHT; i++) {
        assert(fake_fb[i] == 0x000000);
    }

    video_draw_pixel(5, 5, 0x123456);
    assert(fake_fb[5 * FB_WIDTH + 5] == 0x123456);

    /* Out of bounds pixel drawing should be clipped safely */
    video_draw_pixel(FB_WIDTH + 10, 5, 0xFFFFFF);
    video_draw_pixel(5, FB_HEIGHT + 10, 0xFFFFFF);

    /* Draw rectangle */
    video_draw_rect(10, 10, 20, 10, 0xABCDEF);
    for (uint32_t y = 10; y < 20; y++) {
        for (uint32_t x = 10; x < 30; x++) {
            assert(fake_fb[y * FB_WIDTH + x] == 0xABCDEF);
        }
    }
    /* Adjacent pixel should not be overwritten */
    assert(fake_fb[10 * FB_WIDTH + 9] == 0x000000);
    assert(fake_fb[10 * FB_WIDTH + 30] == 0x000000);

    printf("PASSED\n");
}

static void test_video_font_glyphs(void) {
    printf("[TEST  4] Running test_video_font_glyphs                   ... ");
    struct multiboot_info mbi;
    setup_test_framebuffer(&mbi);
    video_init(&mbi);

    uint32_t fg = 0xFFFFFF;
    uint32_t bg = 0x000000;
    video_set_colors(fg, bg);
    video_clear(bg);

    /* Verify character 'A' (ASCII 65) */
    video_set_cursor(0, 0);
    video_putc('A');

    /* Compare fake_fb pixels (0..7 x 0..15) with font8x16[65] */
    const uint8_t *glyph_A = font8x16[(uint8_t)'A'];
    for (int r = 0; r < 16; r++) {
        uint8_t row_bits = glyph_A[r];
        for (int c = 0; c < 8; c++) {
            uint32_t pixel = fake_fb[r * FB_WIDTH + c];
            uint32_t expected = (row_bits & (0x80 >> c)) ? fg : bg;
            assert(pixel == expected);
        }
    }

    /* Verify character ' ' (ASCII 32) is completely blank (all bg) */
    video_set_cursor(1, 0);
    video_putc(' ');
    for (int r = 0; r < 16; r++) {
        for (int c = 0; c < 8; c++) {
            uint32_t pixel = fake_fb[r * FB_WIDTH + (8 + c)];
            assert(pixel == bg);
        }
    }

    /* Verify character '0' (ASCII 48) */
    video_set_cursor(2, 0);
    video_putc('0');
    const uint8_t *glyph_0 = font8x16[(uint8_t)'0'];
    for (int r = 0; r < 16; r++) {
        uint8_t row_bits = glyph_0[r];
        for (int c = 0; c < 8; c++) {
            uint32_t pixel = fake_fb[r * FB_WIDTH + (16 + c)];
            uint32_t expected = (row_bits & (0x80 >> c)) ? fg : bg;
            assert(pixel == expected);
        }
    }

    printf("PASSED\n");
}

static void test_video_putc_formatting_and_scroll(void) {
    printf("[TEST  5] Running test_video_putc_formatting_and_scroll     ... ");
    struct multiboot_info mbi;
    setup_test_framebuffer(&mbi);
    video_init(&mbi);

    uint32_t fg = 0x00FF00;
    uint32_t bg = 0x000000;
    video_set_colors(fg, bg);
    video_clear(bg);

    /* Text output with \n */
    video_puts("Hi\n");

    /* First row should have 'H' at x=0..7 and 'i' at x=8..15 */
    const uint8_t *glyph_H = font8x16[(uint8_t)'H'];
    for (int r = 0; r < 16; r++) {
        uint8_t row_bits = glyph_H[r];
        for (int c = 0; c < 8; c++) {
            uint32_t pixel = fake_fb[r * FB_WIDTH + c];
            uint32_t expected = (row_bits & (0x80 >> c)) ? fg : bg;
            assert(pixel == expected);
        }
    }

    /* Backspace test */
    video_set_cursor(1, 0);
    video_putc('\b');
    /* Pixel at x=0..7 should now be erased to bg */
    for (int r = 0; r < 16; r++) {
        for (int c = 0; c < 8; c++) {
            assert(fake_fb[r * FB_WIDTH + c] == bg);
        }
    }

    /* Test scrolling by writing enough newlines to exceed FB_HEIGHT (40 px / 16 = 2 rows) */
    video_clear(bg);
    video_puts("Row0\nRow1\nRow2\n");
    /* After scrolling, Row1 should be at row 0 (y=0..15) and Row2 at row 1 (y=16..31) */
    const uint8_t *glyph_R = font8x16[(uint8_t)'R'];
    for (int r = 0; r < 16; r++) {
        uint8_t row_bits = glyph_R[r];
        for (int c = 0; c < 8; c++) {
            uint32_t pixel = fake_fb[r * FB_WIDTH + c];
            uint32_t expected = (row_bits & (0x80 >> c)) ? fg : bg;
            assert(pixel == expected);
        }
    }

    printf("PASSED\n");
}

static void test_video_extended_ascii(void) {
    printf("[TEST  6] Running test_video_extended_ascii                ... ");
    struct multiboot_info mbi;
    setup_test_framebuffer(&mbi);
    video_init(&mbi);

    uint32_t fg = 0xFFD166;
    uint32_t bg = 0x1E102E;
    video_set_colors(fg, bg);
    video_clear(bg);

    /* Render extended characters without faulting */
    for (int ch = 128; ch < 256; ch++) {
        video_set_cursor(0, 0);
        video_putc((char)ch);
        /* Check that font8x16[ch] matches rendered output */
        const uint8_t *glyph = font8x16[ch];
        for (int r = 0; r < 16; r++) {
            uint8_t row_bits = glyph[r];
            for (int c = 0; c < 8; c++) {
                uint32_t pixel = fake_fb[r * FB_WIDTH + c];
                uint32_t expected = (row_bits & (0x80 >> c)) ? fg : bg;
                assert(pixel == expected);
            }
        }
    }

    printf("PASSED\n");
}

int main(void) {
    printf("=== Aegis64 Video & Font Driver Host Test Suite ===\n");
    test_video_init_multiboot();
    test_video_color_masks();
    test_video_draw_pixel_and_rect();
    test_video_font_glyphs();
    test_video_putc_formatting_and_scroll();
    test_video_extended_ascii();
    printf("=== All Video Driver Host Tests Passed Successfully! ===\n");
    return 0;
}
