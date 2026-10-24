#ifndef __KERNEL_HOST_TEST__
#define __KERNEL_HOST_TEST__
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "icon_data.h"
#include "video.h"
#include "multiboot.h"

#define FB_W 640
#define FB_H 480
static uint32_t fake_fb[FB_W * FB_H];

static void setup_test_fb(void) {
    memset(fake_fb, 0, sizeof(fake_fb));
    struct multiboot_info mbi;
    memset(&mbi, 0, sizeof(mbi));
    mbi.flags = MULTIBOOT_INFO_FRAMEBUFFER_INFO;
    mbi.framebuffer_addr = (uint64_t)(uintptr_t)fake_fb;
    mbi.framebuffer_width = FB_W;
    mbi.framebuffer_height = FB_H;
    mbi.framebuffer_pitch = FB_W * 4;
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

static void test_icon_count_and_descriptors(void) {
    printf("[TEST  1] Running test_icon_count_and_descriptors           ... ");
    assert(ICON_TOTAL_COUNT == 100);

    for (uint32_t i = 0; i < ICON_TOTAL_COUNT; i++) {
        const icon_descriptor_t *desc = icon_get((icon_id_t)i);
        assert(desc != NULL);
        assert((uint32_t)desc->id == i);
        assert(desc->name != NULL && strlen(desc->name) > 0);
        assert(desc->category != NULL && strlen(desc->category) > 0);
        assert(desc->description != NULL && strlen(desc->description) > 0);
        assert(desc->data_32x32 != NULL);
    }

    /* Out of bounds returns fallback descriptor */
    const icon_descriptor_t *oob = icon_get((icon_id_t)9999);
    assert(oob != NULL);
    assert(oob->id == ICON_APP_WELCOME);

    printf("PASSED\n");
}

static void test_icon_name_lookup(void) {
    printf("[TEST  2] Running test_icon_name_lookup                     ... ");
    const icon_descriptor_t *term = icon_find_by_name("Terminal");
    assert(term != NULL);
    assert(term->id == ICON_APP_TERMINAL);
    assert(strcmp(term->category, "Core App") == 0);

    const icon_descriptor_t *settings = icon_find_by_name("Settings");
    assert(settings != NULL);
    assert(settings->id == ICON_APP_SETTINGS);

    const icon_descriptor_t *files = icon_find_by_name("Files");
    assert(files != NULL);
    assert(files->id == ICON_APP_FILES);

    const icon_descriptor_t *missing = icon_find_by_name("NonExistentApp123");
    assert(missing == NULL);

    printf("PASSED\n");
}

static void test_icon_drawing_and_alpha(void) {
    printf("[TEST  3] Running test_icon_drawing_and_alpha               ... ");
    setup_test_fb();

    /* Clear framebuffer to dark indigo */
    video_clear(0x130A24);

    /* Draw terminal icon at (50, 50) */
    icon_draw(ICON_APP_TERMINAL, 50, 50);

    /* Check that pixels in 32x32 area were modified */
    int modified_pixels = 0;
    for (int y = 50; y < 50 + 32; y++) {
        for (int x = 50; x < 50 + 32; x++) {
            if (video_get_pixel(x, y) != 0x130A24) {
                modified_pixels++;
            }
        }
    }
    assert(modified_pixels > 100);

    /* Pixels outside the 32x32 box must be unchanged */
    assert(video_get_pixel(40, 50) == 0x130A24);
    assert(video_get_pixel(50, 40) == 0x130A24);
    assert(video_get_pixel(90, 50) == 0x130A24);

    printf("PASSED\n");
}

static void test_icon_scaling(void) {
    printf("[TEST  4] Running test_icon_scaling                         ... ");
    setup_test_fb();
    video_clear(0x000000);

    /* Draw scaled icon 2x at (100, 100) -> 64x64 area */
    icon_draw_scale(ICON_APP_SETTINGS, 100, 100, 2);

    int modified_64 = 0;
    for (int y = 100; y < 100 + 64; y++) {
        for (int x = 100; x < 100 + 64; x++) {
            if (video_get_pixel(x, y) != 0x000000) {
                modified_64++;
            }
        }
    }
    assert(modified_64 > 200);

    /* Outside 64x64 bounding box must be untouched */
    assert(video_get_pixel(90, 100) == 0x000000);
    assert(video_get_pixel(170, 100) == 0x000000);

    printf("PASSED\n");
}

static void test_icon_bounds_safety(void) {
    printf("[TEST  5] Running test_icon_bounds_safety                   ... ");
    setup_test_fb();

    /* Partially off-screen top-left */
    icon_draw(ICON_APP_WELCOME, -10, -10);

    /* Partially off-screen bottom-right */
    icon_draw(ICON_APP_FILES, FB_W - 10, FB_H - 10);

    /* Completely off-screen */
    icon_draw(ICON_APP_SYSMON, -100, -100);
    icon_draw(ICON_APP_CALCULATOR, FB_W + 50, FB_H + 50);

    /* Invalid scale */
    icon_draw_scale(ICON_APP_TERMINAL, 10, 10, -1);
    icon_draw_scale(ICON_APP_TERMINAL, 10, 10, 0);

    printf("PASSED\n");
}

int main(void) {
    printf("=====================================================\n");
    printf(" Running Purple OS Master Icon Library Unit Tests    \n");
    printf("=====================================================\n");

    test_icon_count_and_descriptors();
    test_icon_name_lookup();
    test_icon_drawing_and_alpha();
    test_icon_scaling();
    test_icon_bounds_safety();

    printf("=====================================================\n");
    printf(" Icon Library Tests: 5/5 PASSED                      \n");
    printf("=====================================================\n");
    return 0;
}
