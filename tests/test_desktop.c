#ifndef __KERNEL_HOST_TEST__
#define __KERNEL_HOST_TEST__
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "desktop.h"
#include "mouse.h"

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

    mouse_get_state(&st);
    int32_t x = mouse_get_x();
    int32_t y = mouse_get_y();
    uint8_t btns = mouse_get_buttons();

    assert(x == st.x);
    assert(y == st.y);
    assert(btns == st.buttons);

    printf("PASSED\n");
}

int main(void) {
    printf("=====================================================\n");
    printf(" Running Desktop GUI & Window Subsystem Unit Tests   \n");
    printf("=====================================================\n");

    test_desktop_init_and_windows();
    test_desktop_window_capacity();
    test_mouse_queries_and_state();

    printf("=====================================================\n");
    printf(" Desktop & Mouse Tests: 3/3 passed                   \n");
    printf("=====================================================\n");
    return 0;
}
