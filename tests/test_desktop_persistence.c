#ifndef __KERNEL_HOST_TEST__
#define __KERNEL_HOST_TEST__
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "desktop.h"
#include "video.h"
#include "multiboot.h"
#include "mouse.h"

#define CANARY_GUARD 4096
#define FB_W 1024
#define FB_H 768

/* Padded buffer with canaries to verify zero out-of-bounds writes */
static uint32_t fb_guard_before[CANARY_GUARD];
static uint32_t test_fb[FB_W * FB_H];
static uint32_t fb_guard_after[CANARY_GUARD];

static void setup_test_fb(void) {
    for (int i = 0; i < CANARY_GUARD; i++) {
        fb_guard_before[i] = 0xDEADBEEF;
        fb_guard_after[i]  = 0xBEEFCAFE;
    }
    memset(test_fb, 0, sizeof(test_fb));

    struct multiboot_info mbi;
    memset(&mbi, 0, sizeof(mbi));
    mbi.flags = MULTIBOOT_INFO_FRAMEBUFFER_INFO;
    mbi.framebuffer_addr = (uint64_t)(uintptr_t)test_fb;
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

static void check_canaries(void) {
    for (int i = 0; i < CANARY_GUARD; i++) {
        assert(fb_guard_before[i] == 0xDEADBEEF);
        assert(fb_guard_after[i]  == 0xBEEFCAFE);
    }
}

/* 1. Clearing terminal contents does not clear the desktop background */
static void test_terminal_clear_preserves_desktop_background(void) {
    printf("[TEST  1] Running test_terminal_clear_preserves_desktop_background ... ");
    setup_test_fb();
    assert(desktop_init() == 0);

    /* Sample wallpaper pixels outside any window area */
    uint32_t bg_sample_x = 920;
    uint32_t bg_sample_y = 180;
    uint32_t bg_before = video_get_pixel(bg_sample_x, bg_sample_y);
    assert(bg_before != 0); /* Background must be rendered */

    /* Put terminal output and then clear */
    desktop_terminal_write("AegisShell test command 1\n");
    desktop_terminal_write("AegisShell test command 2\n");
    desktop_terminal_write("AegisShell test command 3\n");

    /* Invoke terminal clear */
    desktop_terminal_clear();

    /* Wallpaper must be 100% preserved */
    uint32_t bg_after = video_get_pixel(bg_sample_x, bg_sample_y);
    assert(bg_before == bg_after);

    /* Background must NOT be wiped with flat COLOR_PURPLE_BG */
    assert(bg_after != COLOR_PURPLE_BG);

    check_canaries();
    printf("PASSED\n");
}

/* 2. Clearing a terminal does not erase another window */
static void test_terminal_clear_preserves_other_windows(void) {
    printf("[TEST  2] Running test_terminal_clear_preserves_other_windows       ... ");
    setup_test_fb();
    desktop_init();

    /* Window 1 is Purple Files (x=405, y=45, w=380, h=240) */
    gui_window_t *w_files = desktop_get_window(2);
    assert(w_files != NULL);
    assert(w_files->visible == 1);

    /* Sample pixels from the Files window (title bar and body) */
    uint32_t files_title_pixel = video_get_pixel(w_files->x + 20, w_files->y + 10);
    uint32_t files_body_pixel  = video_get_pixel(w_files->x + 50, w_files->y + 50);

    /* Produce terminal activity and clear */
    desktop_terminal_write("Running ls -la / on Aegis Terminal\n");
    desktop_terminal_write("Kernel: Aegis64 Long Mode x86-64\n");
    desktop_terminal_clear();

    /* Files window pixels must be completely identical and preserved */
    uint32_t files_title_after = video_get_pixel(w_files->x + 20, w_files->y + 10);
    uint32_t files_body_after  = video_get_pixel(w_files->x + 50, w_files->y + 50);
    assert(files_title_pixel == files_title_after);
    assert(files_body_pixel  == files_body_after);

    check_canaries();
    printf("PASSED\n");
}

/* 3. The dock and wallpaper survive terminal redraws */
static void test_dock_and_wallpaper_survive_terminal_redraws(void) {
    printf("[TEST  3] Running test_dock_and_wallpaper_survive_terminal_redraws ... ");
    setup_test_fb();
    desktop_init();

    /* Top panel pixel */
    uint32_t topbar_pixel = video_get_pixel(50, 15);
    /* Bottom dock pixel (dock is at y = FB_H - 70) */
    uint32_t dock_pixel = video_get_pixel(FB_W / 2, FB_H - 40);

    assert(topbar_pixel != 0);
    assert(dock_pixel != 0);

    /* Run 5 cycles of terminal output and clearing */
    for (int i = 0; i < 5; i++) {
        char buf[64];
        snprintf(buf, sizeof(buf), "Batch cycle %d test log message\n", i);
        desktop_terminal_write(buf);
        desktop_terminal_clear();
    }

    /* Topbar and dock must remain intact */
    assert(video_get_pixel(50, 15) == topbar_pixel);
    assert(video_get_pixel(FB_W / 2, FB_H - 40) == dock_pixel);

    check_canaries();
    printf("PASSED\n");
}

/* 4. Moving a window restores the previous area correctly */
static void test_moving_window_restores_previous_area(void) {
    printf("[TEST  4] Running test_moving_window_restores_previous_area         ... ");
    setup_test_fb();
    desktop_init();

    /* Create an isolated test window at (600, 200, 200, 150) */
    int win_id = desktop_create_window("MoveTest", 600, 200, 200, 150, 0x551122);
    assert(win_id > 0);
    gui_window_t *w = desktop_get_window((uint32_t)win_id);
    assert(w != NULL);

    /* First, hide window to capture pristine background at (650, 250) */
    w->visible = 0;
    desktop_composite();
    uint32_t pristine_bg = video_get_pixel(650, 250);

    /* Show window: the window covers (650, 250) */
    w->visible = 1;
    desktop_render();
    uint32_t covered_pixel = video_get_pixel(650, 250);
    assert(covered_pixel != pristine_bg);

    /* Move window to (150, 400) */
    w->x = 150;
    w->y = 400;
    desktop_composite();

    /* Area at (650, 250) where window used to be MUST be restored to pristine background */
    uint32_t restored_pixel = video_get_pixel(650, 250);
    assert(restored_pixel == pristine_bg);

    /* And new location (200, 450) must now display the window */
    uint32_t new_loc_pixel = video_get_pixel(200, 450);
    assert(new_loc_pixel == covered_pixel);

    check_canaries();
    printf("PASSED\n");
}

/* 5. Overlapping windows composite in the correct order */
static void test_overlapping_windows_composite_order(void) {
    printf("[TEST  5] Running test_overlapping_windows_composite_order         ... ");
    setup_test_fb();
    desktop_init();

    /* Create Window A (Red body) and Window B (Green body) overlapping at (500, 150) */
    int idA = desktop_create_window("WindowA_Red", 450, 100, 200, 200, 0xEE0000);
    int idB = desktop_create_window("WindowB_Green", 500, 150, 200, 200, 0x00EE00);
    assert(idA > 0 && idB > 0);

    /* Overlap test point at (550, 200) inside both windows' bodies */
    /* Since idB was created second, it is brought to front (top of z-order) */
    desktop_render();
    uint32_t pixel_b_on_top = video_get_pixel(550, 200);
    assert((pixel_b_on_top & 0x00FF00) > 0x80); /* Window B (Green) is on top */

    /* Now focus Window A -> Window A must be brought to front */
    desktop_focus_window((uint32_t)idA);
    uint32_t pixel_a_on_top = video_get_pixel(550, 200);
    assert((pixel_a_on_top & 0xFF0000) > 0x80); /* Window A (Red) is now on top */

    check_canaries();
    printf("PASSED\n");
}

/* 6. Repainting does not write outside framebuffer or surface bounds */
static void test_clipping_boundary_and_canaries(void) {
    printf("[TEST  6] Running test_clipping_boundary_and_canaries              ... ");
    setup_test_fb();
    desktop_init();

    /* Create window positioned at screen edges */
    int id_edge = desktop_create_window("EdgeWindow", FB_W - 50, FB_H - 50, 200, 200, 0x334455);
    assert(id_edge > 0);

    desktop_render();
    desktop_composite();

    /* Verify no memory corruption outside framebuffer */
    check_canaries();
    printf("PASSED\n");
}

int main(void) {
    printf("\n=====================================================\n");
    printf("  Running Purple Desktop Persistence & Compositor Tests\n");
    printf("=====================================================\n");

    test_terminal_clear_preserves_desktop_background();
    test_terminal_clear_preserves_other_windows();
    test_dock_and_wallpaper_survive_terminal_redraws();
    test_moving_window_restores_previous_area();
    test_overlapping_windows_composite_order();
    test_clipping_boundary_and_canaries();

    printf("=====================================================\n");
    printf("  Desktop Persistence Tests: ALL 6 TESTS PASSED!     \n");
    printf("=====================================================\n\n");
    return 0;
}
