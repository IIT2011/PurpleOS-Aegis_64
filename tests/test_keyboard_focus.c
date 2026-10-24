#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "keyboard.h"

static int test_events_received = 0;
static key_event_t last_event;

static void test_key_callback(const key_event_t *ev) {
    test_events_received++;
    last_event = *ev;
}

static void test_basic_letters(void) {
    keyboard_init();
    
    // Press 'a' (scancode 0x1E)
    keyboard_process_scancode(0x1E);
    assert(keyboard_has_char());
    char c = keyboard_getchar();
    assert(c == 'a');

    // Release 'a' (scancode 0x9E)
    keyboard_process_scancode(0x9E);
    assert(!keyboard_has_char());

    printf("[PASS] test_basic_letters\n");
}

static void test_shift_and_caps(void) {
    keyboard_init();

    // Press Left Shift (0x2A)
    keyboard_process_scancode(0x2A);
    assert(keyboard_get_modifiers() & KBD_MOD_SHIFT);

    // Press 'a' (0x1E) -> 'A'
    keyboard_process_scancode(0x1E);
    assert(keyboard_has_char());
    char c = keyboard_getchar();
    assert(c == 'A');

    // Release Left Shift (0xAA)
    keyboard_process_scancode(0xAA);
    assert(!(keyboard_get_modifiers() & KBD_MOD_SHIFT));

    // Press 'a' (0x1E) -> 'a'
    keyboard_process_scancode(0x1E);
    c = keyboard_getchar();
    assert(c == 'a');

    // Caps Lock toggle (0x3A)
    keyboard_process_scancode(0x3A);
    assert(keyboard_get_modifiers() & KBD_MOD_CAPS);

    // Press 'b' (0x30) -> 'B'
    keyboard_process_scancode(0x30);
    c = keyboard_getchar();
    assert(c == 'B');

    // Caps Lock toggle off (0x3A)
    keyboard_process_scancode(0x3A);
    assert(!(keyboard_get_modifiers() & KBD_MOD_CAPS));

    printf("[PASS] test_shift_and_caps\n");
}

static void test_extended_arrows_and_keys(void) {
    keyboard_init();
    keyboard_set_callback(test_key_callback);
    test_events_received = 0;

    // Up Arrow: 0xE0 0x48
    keyboard_process_scancode(0xE0);
    keyboard_process_scancode(0x48);
    assert(test_events_received == 1);
    assert(last_event.key_code == KEY_UP);
    assert(last_event.pressed == 1);

    // Down Arrow: 0xE0 0x50
    keyboard_process_scancode(0xE0);
    keyboard_process_scancode(0x50);
    assert(test_events_received == 2);
    assert(last_event.key_code == KEY_DOWN);
    assert(last_event.pressed == 1);

    // Left Arrow: 0xE0 0x4B
    keyboard_process_scancode(0xE0);
    keyboard_process_scancode(0x4B);
    assert(last_event.key_code == KEY_LEFT);

    // Right Arrow: 0xE0 0x4D
    keyboard_process_scancode(0xE0);
    keyboard_process_scancode(0x4D);
    assert(last_event.key_code == KEY_RIGHT);

    // Delete: 0xE0 0x53
    keyboard_process_scancode(0xE0);
    keyboard_process_scancode(0x53);
    assert(last_event.key_code == KEY_DELETE);

    // Up Arrow Release: 0xE0 0xC8
    keyboard_process_scancode(0xE0);
    keyboard_process_scancode(0xC8);
    assert(last_event.key_code == KEY_UP);
    assert(last_event.pressed == 0);

    printf("[PASS] test_extended_arrows_and_keys\n");
}

static void test_event_queue_polling(void) {
    keyboard_init();

    // Type "PURPLE" with Shift held
    keyboard_process_scancode(0x2A); // Shift press
    keyboard_process_scancode(0x19); // 'P'
    keyboard_process_scancode(0x99); // 'P' release
    keyboard_process_scancode(0x16); // 'U'
    keyboard_process_scancode(0x96); // 'U' release
    keyboard_process_scancode(0xAA); // Shift release

    key_event_t ev;
    int count = 0;
    while (keyboard_poll_event(&ev)) {
        count++;
    }
    assert(count == 6); // 1 shift-down + 2 P (down,up) + 2 U (down,up) + 1 shift-up
    printf("[PASS] test_event_queue_polling\n");
}

int main(void) {
    printf("=====================================================\n");
    printf(" Running Keyboard & Input Decoding Unit Tests        \n");
    printf("=====================================================\n");

    test_basic_letters();
    test_shift_and_caps();
    test_extended_arrows_and_keys();
    test_event_queue_polling();

    printf("=====================================================\n");
    printf(" Keyboard & Input Tests: ALL PASSED                  \n");
    printf("=====================================================\n");
    return 0;
}
