#ifndef __KERNEL_HOST_TEST__
#define __KERNEL_HOST_TEST__
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "font.h"
#include "video.h"
#include "multiboot.h"

#define TEST_FB_W 400
#define TEST_FB_H 300
static uint32_t test_fb[TEST_FB_W * TEST_FB_H];

static void setup_test_fb(void) {
    memset(test_fb, 0, sizeof(test_fb));
    struct multiboot_info mbi;
    memset(&mbi, 0, sizeof(mbi));
    mbi.flags = MULTIBOOT_INFO_FRAMEBUFFER_INFO;
    mbi.framebuffer_addr = (uint64_t)(uintptr_t)test_fb;
    mbi.framebuffer_width = TEST_FB_W;
    mbi.framebuffer_height = TEST_FB_H;
    mbi.framebuffer_pitch = TEST_FB_W * 4;
    mbi.framebuffer_bpp = 32;
    mbi.framebuffer_type = MULTIBOOT_FRAMEBUFFER_TYPE_RGB;
    mbi.framebuffer_red_field_position = 16;
    mbi.framebuffer_red_mask_size = 8;
    mbi.framebuffer_green_field_position = 8;
    mbi.framebuffer_green_mask_size = 8;
    mbi.framebuffer_blue_field_position = 0;
    mbi.framebuffer_blue_mask_size = 8;

    video_init(&mbi);
}

static void test_font_initialization_and_metrics(void) {
    printf("[TEST  1] Running test_font_initialization_and_metrics     ... ");
    font_init();

    for (int i = 0; i < FONT_ID_COUNT; i++) {
        const font_t *f = font_get((font_id_t)i);
        assert(f != NULL);
        assert(f->name != NULL);
        assert(f->family != NULL);
        assert(f->style != NULL);
        assert(f->size_pt > 0);
        assert(f->ascent > 0);
        assert(f->glyph_count > 40);
        assert(f->glyphs != NULL);
        assert(f->ascii_map != NULL);
    }

    /* Verify specific font properties */
    const font_t *ui_reg = font_get(FONT_UI_REGULAR);
    assert(strcmp(ui_reg->family, "Noto Sans") == 0);
    assert(ui_reg->size_pt == 14);
    assert(ui_reg->is_monospace == 0);

    const font_t *mono_reg = font_get(FONT_MONO_REG);
    assert(strcmp(mono_reg->family, "Noto Sans Mono") == 0);
    assert(mono_reg->is_monospace == 1);

    const font_t *clock_hero = font_get(FONT_CLOCK_HERO);
    assert(clock_hero->size_pt == 48);

    printf("PASSED\n");
}

static void test_ascii_lookup_and_monospace_uniformity(void) {
    printf("[TEST  2] Running test_ascii_lookup_and_monospace          ... ");
    const font_t *mono = font_get(FONT_MONO_REG);
    assert(mono != NULL);

    int16_t expected_advance = -1;

    for (int c = 33; c <= 126; c++) {
        const font_glyph_t *g = font_lookup_glyph(mono, (uint32_t)c);
        assert(g != NULL);
        assert(g->codepoint == (uint32_t)c);
        if (expected_advance == -1) {
            expected_advance = g->advance_x;
            assert(expected_advance > 0);
        } else {
            /* Monospace rule: EVERY printable character MUST have identical advance */
            assert(g->advance_x == expected_advance);
        }
    }

    /* Check space character */
    const font_glyph_t *sp = font_lookup_glyph(mono, ' ');
    assert(sp != NULL);
    assert(sp->advance_x == expected_advance);

    printf("PASSED\n");
}

static void test_utf8_decoding_and_malformed_handling(void) {
    printf("[TEST  3] Running test_utf8_decoding_and_malformed_handling... ");
    uint32_t cp = 0;

    /* 1-byte ASCII */
    const char *s1 = "A";
    const char *p1 = s1;
    assert(font_decode_utf8(&p1, &cp) == 1);
    assert(cp == 'A');
    assert(*p1 == '\0');

    /* 2-byte UTF-8: 'é' (U+00E9 = 0xC3 0xA9) */
    const char *s2 = "\xC3\xA9";
    const char *p2 = s2;
    assert(font_decode_utf8(&p2, &cp) == 2);
    assert(cp == 0x00E9);
    assert(*p2 == '\0');

    /* 3-byte UTF-8: '•' (Bullet U+2022 = 0xE2 0x80 0xA2) */
    const char *s3 = "\xE2\x80\xA2";
    const char *p3 = s3;
    assert(font_decode_utf8(&p3, &cp) == 3);
    assert(cp == 0x2022);
    assert(*p3 == '\0');

    /* 3-byte UTF-8: '★' (Star U+2605 = 0xE2 0x98 0x85) */
    const char *s4 = "\xE2\x98\x85";
    const char *p4 = s4;
    assert(font_decode_utf8(&p4, &cp) == 3);
    assert(cp == 0x2605);
    assert(*p4 == '\0');

    /* Malformed sequence: truncated 2-byte prefix 0xC3 without continuation */
    const char *s_bad1 = "\xC3xyz";
    const char *p_bad1 = s_bad1;
    assert(font_decode_utf8(&p_bad1, &cp) == 1);
    assert(cp == '?'); /* Safe fallback character */
    assert(p_bad1 == s_bad1 + 1);

    /* Malformed sequence: truncated 3-byte prefix */
    const char *s_bad2 = "\xE2\x80\x20";
    const char *p_bad2 = s_bad2;
    assert(font_decode_utf8(&p_bad2, &cp) == 1);
    assert(cp == '?');

    /* Null / empty string */
    const char *s_empty = "";
    const char *p_empty = s_empty;
    assert(font_decode_utf8(&p_empty, &cp) == 0);
    assert(cp == 0);

    printf("PASSED\n");
}

static void test_fallback_replacement_glyph(void) {
    printf("[TEST  4] Running test_fallback_replacement_glyph          ... ");
    const font_t *f = font_get(FONT_UI_REGULAR);

    /* Unmapped exotic codepoint */
    uint32_t exotic = 0x1F600; /* Grinning Face Emoji */
    const font_glyph_t *g = font_lookup_glyph(f, exotic);
    assert(g != NULL);
    /* Must return fallback glyph (default_glyph_idx) without crashing */
    assert(g == &f->glyphs[f->default_glyph_idx]);
    assert(g->advance_x > 0);

    printf("PASSED\n");
}

static void test_string_measurement_accuracy(void) {
    printf("[TEST  5] Running test_string_measurement_accuracy         ... ");
    const font_t *f = font_get(FONT_UI_REGULAR);

    assert(font_measure_string(f, "") == 0);
    assert(font_measure_string(f, NULL) == 0);

    const font_glyph_t *gA = font_lookup_glyph(f, 'A');
    const font_glyph_t *gB = font_lookup_glyph(f, 'B');
    const font_glyph_t *gC = font_lookup_glyph(f, 'C');
    assert(gA && gB && gC);

    int32_t expected = gA->advance_x + gB->advance_x + gC->advance_x;
    int32_t actual = font_measure_string(f, "ABC");
    assert(expected == actual);

    /* Test font_measure_string_n */
    assert(font_measure_string_n(f, "ABC", 2) == gA->advance_x + gB->advance_x);
    assert(font_measure_string_n(f, "ABC", 1) == gA->advance_x);
    assert(font_measure_string_n(f, "ABC", 0) == 0);

    printf("PASSED\n");
}

static void test_clipping_boundary_safety(void) {
    printf("[TEST  6] Running test_clipping_boundary_safety            ... ");
    setup_test_fb();
    const font_t *f = font_get(FONT_UI_REGULAR);

    /* Background fill in dark purple 0x120824 */
    video_clear(0x120824);

    /* Define a clip window: x: 50..100, y: 50..80 */
    font_rect_t clip = { 50, 50, 50, 30 };

    /* Draw string starting at (30, 45) spanning across clip boundary */
    font_draw_string(f, 30, 45, "Long Text String Across Boundary", 0xFFFFFF, &clip);

    /* Verify that NO pixels outside clip rect were altered */
    for (int y = 0; y < 50; y++) {
        for (int x = 0; x < TEST_FB_W; x++) {
            assert(video_get_pixel(x, y) == 0x120824);
        }
    }
    for (int y = 80; y < TEST_FB_H; y++) {
        for (int x = 0; x < TEST_FB_W; x++) {
            assert(video_get_pixel(x, y) == 0x120824);
        }
    }
    for (int y = 50; y < 80; y++) {
        for (int x = 0; x < 50; x++) {
            assert(video_get_pixel(x, y) == 0x120824);
        }
        for (int x = 100; x < TEST_FB_W; x++) {
            assert(video_get_pixel(x, y) == 0x120824);
        }
    }

    printf("PASSED\n");
}

static void test_alpha_blending_correctness(void) {
    printf("[TEST  7] Running test_alpha_blending_correctness          ... ");
    setup_test_fb();
    const font_t *f = font_get(FONT_UI_BOLD);

    /* Background: pure black 0x000000 */
    video_clear(0x000000);

    /* Draw white character 'A' (color 0xFFFFFF) at (10, 10) */
    font_draw_char(f, 10, 10, 'A', 0xFFFFFF, NULL);

    /* Find anti-aliased edge pixels (pixels with 0 < brightness < 0xFFFFFF) */
    int aa_pixel_found = 0;
    int solid_pixel_found = 0;

    for (int y = 10; y < 30; y++) {
        for (int x = 10; x < 30; x++) {
            uint32_t px = video_get_pixel(x, y);
            if (px > 0 && px < 0xFFFFFF) {
                aa_pixel_found++;
                /* Verify grayscale channel equality */
                uint32_t r = (px >> 16) & 0xFF;
                uint32_t g = (px >> 8) & 0xFF;
                uint32_t b = px & 0xFF;
                assert(r == g && g == b);
            } else if (px == 0xFFFFFF) {
                solid_pixel_found++;
            }
        }
    }

    assert(aa_pixel_found > 0);
    assert(solid_pixel_found > 0);
    printf("PASSED (found %d anti-aliased edge pixels)\n", aa_pixel_found);
}

static void test_terminal_grid_alignment(void) {
    printf("[TEST  8] Running test_terminal_grid_alignment             ... ");
    const font_t *mono = font_get(FONT_MONO_REG);

    /* Different lines of equal character count must have IDENTICAL pixel widths */
    const char *line1 = "purple@aegis64:~$ ls -la /purplefs";
    const char *line2 = "-rw-r--r-- 1 root root  132 Oct 22";
    const char *line3 = "==================================";

    size_t len1 = strlen(line1);
    size_t len2 = strlen(line2);
    size_t len3 = strlen(line3);
    assert(len1 == len2 && len2 == len3);

    int32_t w1 = font_measure_string(mono, line1);
    int32_t w2 = font_measure_string(mono, line2);
    int32_t w3 = font_measure_string(mono, line3);

    assert(w1 == w2);
    assert(w2 == w3);

    printf("PASSED\n");
}

int main(void) {
    printf("\n=====================================================\n");
    printf(" Running Aegis64 Noto Typography Subsystem Unit Tests\n");
    printf("=====================================================\n");

    test_font_initialization_and_metrics();
    test_ascii_lookup_and_monospace_uniformity();
    test_utf8_decoding_and_malformed_handling();
    test_fallback_replacement_glyph();
    test_string_measurement_accuracy();
    test_clipping_boundary_safety();
    test_alpha_blending_correctness();
    test_terminal_grid_alignment();

    printf("=====================================================\n");
    printf(" Typography Tests: ALL 8 SUITES PASSED               \n");
    printf("=====================================================\n\n");
    return 0;
}
