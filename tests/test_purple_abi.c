#ifndef __KERNEL_HOST_TEST__
#define __KERNEL_HOST_TEST__
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "purple_abi.h"

static void test_abi_window_and_surface(void) {
    printf("[TEST  1] Running test_abi_window_and_surface              ... ");

    purple_window_t *win = purple_window_create("Native VLC Window", 50, 50, 320, 240);
    assert(win != NULL);
    assert(purple_window_get_id(win) > 0);

    purple_surface_t surf;
    int rc = purple_window_get_surface(win, &surf);
    assert(rc == 0);
    assert(surf.width == 320);
    assert(surf.height == 240);
    assert(surf.pitch == 320 * 4);
    assert(surf.pixels != NULL);

    /* Draw pixels and verify */
    purple_surface_draw_pixel(&surf, 10, 10, PURPLE_COLOR_RED);
    assert(purple_surface_get_pixel(&surf, 10, 10) == PURPLE_COLOR_RED);

    purple_surface_draw_pixel(&surf, 20, 20, PURPLE_COLOR_GREEN);
    assert(purple_surface_get_pixel(&surf, 20, 20) == PURPLE_COLOR_GREEN);

    purple_surface_draw_pixel(&surf, 30, 30, PURPLE_COLOR_BLUE);
    assert(purple_surface_get_pixel(&surf, 30, 30) == PURPLE_COLOR_BLUE);

    /* Fill rectangle */
    purple_surface_fill_rect(&surf, 50, 50, 100, 50, PURPLE_COLOR_PURPLE);
    for (uint32_t y = 50; y < 100; y++) {
        for (uint32_t x = 50; x < 150; x++) {
            assert(purple_surface_get_pixel(&surf, x, y) == PURPLE_COLOR_PURPLE);
        }
    }

    /* Draw text */
    purple_surface_draw_text(&surf, 10, 120, "VLC Media Player Native", PURPLE_COLOR_WHITE, PURPLE_COLOR_BLACK);
    /* At least some pixels should be white */
    int white_count = 0;
    for (uint32_t y = 120; y < 136; y++) {
        for (uint32_t x = 10; x < 10 + 23 * 8; x++) {
            if (purple_surface_get_pixel(&surf, x, y) == PURPLE_COLOR_WHITE) {
                white_count++;
            }
        }
    }
    assert(white_count > 50);

    /* Blit sub-surface */
    purple_surface_t small_surf;
    uint32_t small_pix[16 * 16];
    for (int i = 0; i < 16 * 16; i++) small_pix[i] = PURPLE_COLOR_YELLOW;
    small_surf.width = 16;
    small_surf.height = 16;
    small_surf.pitch = 16 * 4;
    small_surf.pixels = small_pix;
    small_surf.flags = 0;

    purple_surface_blit(&surf, 200, 150, &small_surf, 0, 0, 16, 16);
    assert(purple_surface_get_pixel(&surf, 200, 150) == PURPLE_COLOR_YELLOW);
    assert(purple_surface_get_pixel(&surf, 215, 165) == PURPLE_COLOR_YELLOW);

    /* Present window */
    assert(purple_window_present(win) == 0);

    /* Change title */
    assert(purple_window_set_title(win, "VLC - Playing Video") == 0);

    /* Destroy window */
    assert(purple_window_destroy(win) == 0);

    printf("PASSED\n");
}

static void test_abi_event_queue(void) {
    printf("[TEST  2] Running test_abi_event_queue                     ... ");

    purple_event_t ev1;
    ev1.type = PURPLE_EVENT_KEY_DOWN;
    ev1.window_id = 1;
    ev1.key.character = ' ';
    ev1.key.scancode = 0x39;
    ev1.key.modifiers = 0;
    purple_push_event(&ev1);

    purple_event_t ev2;
    ev2.type = PURPLE_EVENT_MOUSE_DOWN;
    ev2.window_id = 1;
    ev2.mouse.x = 100;
    ev2.mouse.y = 80;
    ev2.mouse.buttons = 1;
    purple_push_event(&ev2);

    purple_event_t out;
    assert(purple_poll_event(&out) == 1);
    assert(out.type == PURPLE_EVENT_KEY_DOWN);
    assert(out.key.character == ' ');

    assert(purple_poll_event(&out) == 1);
    assert(out.type == PURPLE_EVENT_MOUSE_DOWN);
    assert(out.mouse.x == 100);
    assert(out.mouse.y == 80);

    /* Queue should now be empty */
    assert(purple_poll_event(&out) == 0);

    printf("PASSED\n");
}

static void test_abi_memory(void) {
    printf("[TEST  3] Running test_abi_memory                          ... ");

    void *ptr = purple_malloc(1024);
    assert(ptr != NULL);
    memset(ptr, 0x5A, 1024);
    uint8_t *b = (uint8_t *)ptr;
    assert(b[0] == 0x5A && b[1023] == 0x5A);
    purple_free(ptr);

    void *m = purple_mmap(NULL, 4096, 0, 0);
    assert(m != NULL);
    memset(m, 0xEE, 4096);
    assert(((uint8_t *)m)[2000] == 0xEE);
    assert(purple_munmap(m, 4096) == 0);

    printf("PASSED\n");
}

static void test_abi_storage_and_files(void) {
    printf("[TEST  4] Running test_abi_storage_and_files               ... ");

    purple_file_t *f = purple_file_open("video.mp4", PURPLE_FILE_READ | PURPLE_FILE_WRITE);
    assert(f != NULL);

    char buf[128];
    int rd = purple_file_read(f, buf, sizeof(buf));
    assert(rd == sizeof(buf));

    int wr = purple_file_write(f, "TEST_DATA", 9);
    assert(wr == 9);

    assert(purple_file_seek(f, 0, 0) == 0);

    assert(purple_file_close(f) == 0);

    purple_stat_t st;
    assert(purple_file_stat("video.mp4", &st) == 0);
    assert(st.size_bytes > 0);

    /* Directory reading */
    purple_dir_t *d = purple_dir_open("/");
    assert(d != NULL);
    purple_dirent_t ent;
    int count = 0;
    while (purple_dir_read(d, &ent) == 1) {
        assert(strlen(ent.name) > 0);
        count++;
    }
    assert(count >= 1);
    assert(purple_dir_close(d) == 0);

    printf("PASSED\n");
}

static void test_abi_threading_and_sync(void) {
    printf("[TEST  5] Running test_abi_threading_and_sync              ... ");

    purple_mutex_t mtx;
    assert(purple_mutex_init(&mtx) == 0);
    assert(purple_mutex_lock(&mtx) == 0);
    assert(mtx.locked != 0);
    assert(purple_mutex_unlock(&mtx) == 0);
    assert(mtx.locked == 0);

    purple_thread_t tid = purple_thread_create("vlc_decode_worker", NULL, NULL);
    assert(tid > 0);
    purple_thread_yield();

    printf("PASSED\n");
}

static void test_abi_timers_and_audio(void) {
    printf("[TEST  6] Running test_abi_timers_and_audio                ... ");

    uint64_t ticks = purple_get_ticks();
    assert(ticks > 0);
    uint64_t ms = purple_get_time_ms();
    assert(ms > 0);

    purple_time_t tm;
    assert(purple_get_wall_clock(&tm) == 0);
    assert(tm.year >= 2026);

    assert(purple_audio_is_available() == 1);
    uint8_t pcm[512];
    memset(pcm, 0, sizeof(pcm));
    assert(purple_audio_play_pcm(pcm, sizeof(pcm), 44100, 2) == 0);
    purple_audio_set_volume(85);

    printf("PASSED\n");
}

int main(void) {
    printf("=====================================================\n");
    printf("  Running Purple OS Application ABI Host Test Suite  \n");
    printf("=====================================================\n");
    test_abi_window_and_surface();
    test_abi_event_queue();
    test_abi_memory();
    test_abi_storage_and_files();
    test_abi_threading_and_sync();
    test_abi_timers_and_audio();
    printf("=====================================================\n");
    printf("  Purple ABI Tests: ALL 6 TEST SUITES PASSED!        \n");
    printf("=====================================================\n");
    return 0;
}
