#ifndef __KERNEL_HOST_TEST__
#define __KERNEL_HOST_TEST__
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "desktop.h"
#include "mouse.h"
#include "keyboard.h"
#include "video.h"
#include "multiboot.h"

#define FB_W 800
#define FB_H 600
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

static void test_desktop_init_and_windows(void) {
    printf("[TEST  1] Running test_desktop_init_and_windows            ... ");
    assert(desktop_init() == 0);

    int w1 = desktop_create_window("Terminal", 100, 100, 400, 300, 0x000000);
    assert(w1 > 0);

    int w2 = desktop_create_window("Editor", 200, 200, 500, 400, 0x111111);
    assert(w2 > 0);

    printf("PASSED\n");
}

static void test_desktop_window_capacity(void) {
    printf("[TEST  2] Running test_desktop_window_capacity             ... ");
    desktop_init(); // 3 default windows

    for (int i = 3; i < DESKTOP_MAX_WINDOWS; i++) {
        char title[16];
        snprintf(title, sizeof(title), "Win%d", i);
        int id = desktop_create_window(title, 10 * i, 10 * i, 200, 150, 0x222222);
        assert(id > 0);
    }

    // Overflow window creation must fail gracefully
    int overflow = desktop_create_window("Overflow", 0, 0, 100, 100, 0);
    assert(overflow == -1);

    printf("PASSED\n");
}

static void test_mouse_queries_and_state(void) {
    printf("[TEST  3] Running test_mouse_queries_and_state             ... ");
    mouse_state_t st;
    memset(&st, 0, sizeof(st));

    mouse_set_position(320, 240);
    mouse_get_state(&st);
    int32_t x = mouse_get_x();
    int32_t y = mouse_get_y();
    uint8_t btns = mouse_get_buttons();

    assert(x == 320);
    assert(y == 240);
    assert(x == st.x);
    assert(y == st.y);
    assert(btns == st.buttons);

    printf("PASSED\n");
}

static void test_desktop_hit_testing(void) {
    printf("[TEST  4] Running test_desktop_hit_testing                 ... ");
    setup_test_fb();
    desktop_init();

    /* Window 0 is at (30, 45, width=340, height=230) */
    gui_window_t *w0 = desktop_get_window(1);
    assert(w0 != NULL);
    assert(w0->x == 30 && w0->y == 45);

    /* 1. Hit title bar of Window 0 */
    desktop_hit_result_t hit = desktop_hit_test(50, 50);
    assert(hit.type == DESKTOP_HIT_WINDOW_TITLE);
    assert(hit.window_index == 0);

    /* 2. Hit close button [X] of Window 0 (at x = 30 + 340 - 24 = 346, y = 50) */
    hit = desktop_hit_test(30 + 340 - 15, 50);
    assert(hit.type == DESKTOP_HIT_WINDOW_CLOSE);
    assert(hit.window_index == 0);

    /* 3. Hit body of Window 0 */
    hit = desktop_hit_test(100, 150);
    assert(hit.type == DESKTOP_HIT_WINDOW_BODY);
    assert(hit.window_index == 0);

    /* 4. Hit top bar */
    hit = desktop_hit_test(30, 10);
    assert(hit.type == DESKTOP_HIT_TOPBAR);
    assert(hit.menu_index == 0); /* "Purple OS" */

    hit = desktop_hit_test(130, 10);
    assert(hit.type == DESKTOP_HIT_TOPBAR);
    assert(hit.menu_index == 1); /* "System" */

    /* 5. Hit background desktop area (empty space) */
    hit = desktop_hit_test(750, 550);
    assert(hit.type == DESKTOP_HIT_DESKTOP);

    printf("PASSED\n");
}

static void test_desktop_window_closing_and_toggling(void) {
    printf("[TEST  5] Running test_desktop_window_closing_and_toggling ... ");
    desktop_init();

    gui_window_t *w0 = desktop_get_window(1);
    assert(w0 != NULL);
    assert(w0->visible == 1);

    /* Close window */
    assert(desktop_close_window(1) == 0);
    assert(w0->visible == 0);

    /* Toggle window back on */
    int vis = desktop_toggle_window(1);
    assert(vis == 1);
    assert(w0->visible == 1);

    /* Focus window */
    assert(desktop_focus_window(1) == 0);
    assert(w0->focused == 1);

    printf("PASSED\n");
}

static void test_desktop_mouse_dispatch_and_close_click(void) {
    printf("[TEST  6] Running test_desktop_mouse_dispatch_and_close    ... ");
    setup_test_fb();
    desktop_init();
    desktop_render();

    gui_window_t *w0 = desktop_get_window(1);
    assert(w0->visible == 1);

    /* Click on [X] close button of Window 0 */
    int32_t close_x = (int32_t)(w0->x + w0->width - 15);
    int32_t close_y = (int32_t)(w0->y + 10);

    int ev = desktop_handle_mouse(close_x, close_y, MOUSE_BTN_LEFT);
    assert(ev == DESKTOP_EVENT_WINDOW_CLOSED);
    assert(w0->visible == 0);

    /* Release button */
    desktop_handle_mouse(close_x, close_y, 0);

    printf("PASSED\n");
}

static void test_desktop_cursor_background_preservation(void) {
    printf("[TEST  7] Running test_desktop_cursor_preservation         ... ");
    setup_test_fb();
    desktop_init();
    desktop_render();

    /* Save pristine background at test area (100, 100) */
    uint32_t orig_pixels[16][16];
    for (uint32_t r = 0; r < 16; r++) {
        for (uint32_t c = 0; c < 16; c++) {
            orig_pixels[r][c] = video_get_pixel(100 + c, 100 + r);
        }
    }

    /* Move cursor to (100, 100) -> draws cursor and saves background */
    desktop_move_cursor(100, 100);

    /* Move cursor away to (400, 400) -> must restore pristine background at (100, 100) */
    desktop_move_cursor(400, 400);

    /* Verify 100% restoration with ZERO trails */
    for (uint32_t r = 0; r < 16; r++) {
        for (uint32_t c = 0; c < 16; c++) {
            uint32_t current = video_get_pixel(100 + c, 100 + r);
            assert(current == orig_pixels[r][c]);
        }
    }

    printf("PASSED\n");
}

static void test_mouse_packet_processing_and_overflow(void) {
    printf("[TEST  8] Running test_mouse_packet_processing_and_overflow... ");
    setup_test_fb();
    mouse_init();
    mouse_set_position(300, 200);

    /* 1. Valid positive delta: dx=+15, dy=+10 */
    /* Byte 0: 0x08 (bit 3=1, no overflow, no sign) */
    /* Byte 1: 0x0F (+15) */
    /* Byte 2: 0x0A (+10) */
    mouse_process_byte(0x08);
    mouse_process_byte(0x0F);
    mouse_process_byte(0x0A);

    assert(mouse_get_x() == 315);
    assert(mouse_get_y() == 190); // Inverted Y screen space: 200 - 10 = 190

    /* 2. Negative delta: dx=-10, dy=-5 */
    /* Byte 0: 0x08 | 0x10 (X sign) | 0x20 (Y sign) = 0x38 */
    /* Byte 1: 0xF6 (-10 in two's complement) */
    /* Byte 2: 0xFB (-5 in two's complement) */
    mouse_process_byte(0x38);
    mouse_process_byte(0xF6);
    mouse_process_byte(0xFB);

    assert(mouse_get_x() == 305);
    assert(mouse_get_y() == 195); // 190 - (-5) = 195

    /* 3. Overflow packet: bit 6 or 7 set -> MUST be discarded */
    mouse_process_byte(0x48); // X overflow bit set
    mouse_process_byte(0xFF);
    mouse_process_byte(0xFF);

    // Position must remain unchanged!
    assert(mouse_get_x() == 305);
    assert(mouse_get_y() == 195);

    /* 4. Boundary clamping */
    mouse_set_position(10, 10);
    /* Large negative delta */
    mouse_process_byte(0x38);
    mouse_process_byte(0x80); // -128
    mouse_process_byte(0x7F); // +127
    assert(mouse_get_x() == 0); // clamped to 0

    printf("PASSED\n");
}

int main(void) {
    printf("=====================================================\n");
    printf(" Running Desktop GUI & Window Subsystem Unit Tests   \n");
    printf("=====================================================\n");

    test_desktop_init_and_windows();
    test_desktop_window_capacity();
    test_mouse_queries_and_state();
    test_desktop_hit_testing();
    test_desktop_window_closing_and_toggling();
    test_desktop_mouse_dispatch_and_close_click();
    test_desktop_cursor_background_preservation();
    test_mouse_packet_processing_and_overflow();

    printf("=====================================================\n");
    printf(" Desktop & Mouse Tests: 8/8 passed                   \n");
    printf("=====================================================\n");
    return 0;
}
