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
#include "icon_data.h"

#define FB_W 1024
#define FB_H 768
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

static void test_window_minimize_and_maximize(void) {
    printf("[TEST  1] Running test_window_minimize_and_maximize         ... ");
    setup_test_fb();
    desktop_init();

    /* Win 1 (Terminal) is id 1 */
    gui_window_t *w = desktop_get_window(1);
    assert(w != NULL);
    w->visible = 1;
    uint32_t orig_x = w->x;
    uint32_t orig_y = w->y;
    uint32_t orig_w = w->width;
    uint32_t orig_h = w->height;

    /* Maximize window */
    assert(desktop_maximize_window(1) == 0);
    assert(w->is_maximized == 1);
    assert(w->x == 10);
    assert(w->y == 36);
    assert(w->width == FB_W - 20);

    /* Restore window */
    assert(desktop_maximize_window(1) == 0);
    assert(w->is_maximized == 0);
    assert(w->x == orig_x);
    assert(w->y == orig_y);
    assert(w->width == orig_w);
    assert(w->height == orig_h);

    /* Minimize window */
    assert(desktop_minimize_window(1) == 0);
    assert(w->visible == 0);
    assert(w->focused == 0);

    printf("PASSED\n");
}

static void test_application_opening(void) {
    printf("[TEST  2] Running test_application_opening                  ... ");
    setup_test_fb();
    desktop_init();

    /* Open Calculator (win index 6, id 7) */
    int calc_id = desktop_open_application("calc");
    assert(calc_id > 0);
    gui_window_t *w_calc = desktop_get_window((uint32_t)calc_id);
    assert(w_calc != NULL);
    assert(w_calc->visible == 1);
    assert(w_calc->focused == 1);
    assert(w_calc->type == WIN_TYPE_CALCULATOR);

    /* Open Text Editor (win index 5, id 6) */
    int ed_id = desktop_open_application("editor");
    assert(ed_id > 0);
    gui_window_t *w_ed = desktop_get_window((uint32_t)ed_id);
    assert(w_ed != NULL);
    assert(w_ed->visible == 1);
    assert(w_ed->focused == 1);
    assert(w_ed->type == WIN_TYPE_EDITOR);

    /* Open Settings */
    int set_id = desktop_open_application("settings");
    assert(set_id > 0);
    gui_window_t *w_set = desktop_get_window((uint32_t)set_id);
    assert(w_set != NULL);
    assert(w_set->visible == 1);
    assert(w_set->type == WIN_TYPE_SETTINGS);

    printf("PASSED\n");
}

static void test_calculator_widget(void) {
    printf("[TEST  3] Running test_calculator_widget                    ... ");
    setup_test_fb();
    desktop_init();

    int calc_id = desktop_open_application("calc");
    gui_window_t *w = desktop_get_window((uint32_t)calc_id);
    assert(w != NULL);
    assert(strcmp(w->calc_display, "0") == 0);

    /* Simulate clicking '7' button (row 1, col 0): bx = w->x + 16 + 0*48, by = w->y + 68 + 1*38 */
    int32_t btn7_x = (int32_t)(w->x + 16 + 10);
    int32_t btn7_y = (int32_t)(w->y + 68 + 38 + 10);
    desktop_handle_mouse(btn7_x, btn7_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(btn7_x, btn7_y, 0); // Release
    assert(strcmp(w->calc_display, "7") == 0);

    /* Simulate clicking '+' button (row 3, col 3): bx = w->x + 16 + 3*48, by = w->y + 68 + 3*38 */
    int32_t btn_plus_x = (int32_t)(w->x + 16 + 3 * 48 + 10);
    int32_t btn_plus_y = (int32_t)(w->y + 68 + 3 * 38 + 10);
    desktop_handle_mouse(btn_plus_x, btn_plus_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(btn_plus_x, btn_plus_y, 0);
    assert(w->calc_op == '+');
    assert(w->calc_prev_val == 7);

    /* Simulate clicking '5' button (row 2, col 1): bx = w->x + 16 + 1*48, by = w->y + 68 + 2*38 */
    int32_t btn5_x = (int32_t)(w->x + 16 + 1 * 48 + 10);
    int32_t btn5_y = (int32_t)(w->y + 68 + 2 * 38 + 10);
    desktop_handle_mouse(btn5_x, btn5_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(btn5_x, btn5_y, 0);
    assert(strcmp(w->calc_display, "5") == 0);

    /* Simulate clicking '=' button (row 4, col 3): bx = w->x + 16 + 3*48, by = w->y + 68 + 4*38 */
    int32_t btn_eq_x = (int32_t)(w->x + 16 + 3 * 48 + 10);
    int32_t btn_eq_y = (int32_t)(w->y + 68 + 4 * 38 + 10);
    desktop_handle_mouse(btn_eq_x, btn_eq_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(btn_eq_x, btn_eq_y, 0);
    /* 7 + 5 = 12 */
    assert(strcmp(w->calc_display, "12") == 0);

    /* Simulate clicking 'C' button (row 0, col 0) */
    int32_t btn_c_x = (int32_t)(w->x + 16 + 10);
    int32_t btn_c_y = (int32_t)(w->y + 68 + 10);
    desktop_handle_mouse(btn_c_x, btn_c_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(btn_c_x, btn_c_y, 0);
    assert(strcmp(w->calc_display, "0") == 0);

    printf("PASSED\n");
}

static void test_settings_tabs_and_controls(void) {
    printf("[TEST  4] Running test_settings_tabs_and_controls           ... ");
    setup_test_fb();
    desktop_init();

    int set_id = desktop_open_application("settings");
    gui_window_t *w = desktop_get_window((uint32_t)set_id);
    assert(w != NULL);
    assert(w->settings_active_tab == 0);

    /* Click Tab 2 (Sound): tx = w->x + 10 + 2*76 + 10 = w->x + 172, ty = w->y + 35 */
    int32_t tab2_x = (int32_t)(w->x + 10 + 2 * 76 + 10);
    int32_t tab2_y = (int32_t)(w->y + 35);
    desktop_handle_mouse(tab2_x, tab2_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(tab2_x, tab2_y, 0);
    assert(w->settings_active_tab == 2);

    /* In Sound Tab, click [+] button: x = w->x + 215, y = w->y + 120 */
    int orig_vol = w->settings_volume;
    int32_t plus_x = (int32_t)(w->x + 215);
    int32_t plus_y = (int32_t)(w->y + 120);
    desktop_handle_mouse(plus_x, plus_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(plus_x, plus_y, 0);
    assert(w->settings_volume == orig_vol + 5);

    /* Click [-] button: x = w->x + 20, y = w->y + 120 */
    int32_t minus_x = (int32_t)(w->x + 20);
    int32_t minus_y = (int32_t)(w->y + 120);
    desktop_handle_mouse(minus_x, minus_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(minus_x, minus_y, 0);
    assert(w->settings_volume == orig_vol);

    /* Click Tab 0 (Appearance) */
    int32_t tab0_x = (int32_t)(w->x + 20);
    int32_t tab0_y = (int32_t)(w->y + 35);
    desktop_handle_mouse(tab0_x, tab0_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(tab0_x, tab0_y, 0);
    assert(w->settings_active_tab == 0);

    /* Click Dark Mode button: x = w->x + 30, y = w->y + 115 */
    int orig_dark = w->settings_dark_mode;
    int32_t dark_x = (int32_t)(w->x + 30);
    int32_t dark_y = (int32_t)(w->y + 115);
    desktop_handle_mouse(dark_x, dark_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(dark_x, dark_y, 0);
    assert(w->settings_dark_mode != orig_dark);

    printf("PASSED\n");
}

static void test_text_editor_typing_and_toolbar(void) {
    printf("[TEST  5] Running test_text_editor_typing_and_toolbar       ... ");
    setup_test_fb();
    desktop_init();

    int ed_id = desktop_open_application("editor");
    gui_window_t *w = desktop_get_window((uint32_t)ed_id);
    assert(w != NULL);

    /* Click [Clear] button: x = w->x + 145, y = w->y + 35 */
    int32_t clear_x = (int32_t)(w->x + 145);
    int32_t clear_y = (int32_t)(w->y + 35);
    desktop_handle_mouse(clear_x, clear_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(clear_x, clear_y, 0);
    assert(strlen(w->editor_content) == 0);

    /* Type "Aegis64" via desktop_handle_key */
    const char *text = "Aegis64";
    for (size_t i = 0; i < strlen(text); i++) {
        desktop_handle_key(text[i], 0, 1, 0);
    }
    assert(strcmp(w->editor_content, "Aegis64") == 0);
    assert(w->editor_modified == 1);

    /* Backspace last character */
    desktop_handle_key('\b', 0, 1, 0);
    assert(strcmp(w->editor_content, "Aegis6") == 0);

    /* Click [Save] button: x = w->x + 90, y = w->y + 35 */
    int32_t save_x = (int32_t)(w->x + 90);
    int32_t save_y = (int32_t)(w->y + 35);
    desktop_handle_mouse(save_x, save_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(save_x, save_y, 0);
    assert(w->editor_modified == 0);

    printf("PASSED\n");
}

static void test_files_explorer_selection(void) {
    printf("[TEST  6] Running test_files_explorer_selection             ... ");
    setup_test_fb();
    desktop_init();

    int files_id = desktop_open_application("files");
    gui_window_t *w = desktop_get_window((uint32_t)files_id);
    assert(w != NULL);
    assert(w->files_selected_idx == 0);

    /* Click Row 3 (apps/): x = w->x + 50, y = w->y + 80 + 3*24 + 10 */
    int32_t row3_x = (int32_t)(w->x + 50);
    int32_t row3_y = (int32_t)(w->y + 80 + 3 * 24 + 10);
    desktop_handle_mouse(row3_x, row3_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(row3_x, row3_y, 0);
    assert(w->files_selected_idx == 3);

    printf("PASSED\n");
}

static void test_desktop_rendering_stability(void) {
    printf("[TEST  7] Running test_desktop_rendering_stability          ... ");
    setup_test_fb();
    desktop_init();

    /* Open multiple application windows */
    desktop_open_application("calc");
    desktop_open_application("editor");
    desktop_open_application("settings");
    desktop_open_application("files");
    desktop_open_application("sysmon");

    /* Render complete desktop environment */
    desktop_render();

    /* Verify non-empty framebuffer */
    int non_zero = 0;
    for (int i = 0; i < FB_W * FB_H; i++) {
        if (fake_fb[i] != 0) non_zero++;
    }
    assert(non_zero > (FB_W * FB_H) / 2);

    printf("PASSED\n");
}

static void test_vlc_media_player(void) {
    printf("[TEST  8] Running test_vlc_media_player                     ... ");
    setup_test_fb();
    desktop_init();

    int vlc_id = desktop_open_application("vlc");
    assert(vlc_id > 0);
    gui_window_t *w = desktop_get_window((uint32_t)vlc_id);
    assert(w != NULL);
    assert(w->type == WIN_TYPE_VLC);
    assert(w->vlc_state == 1); // Playing by default

    /* Calculate play/pause button coordinates */
    uint32_t vp_h = (w->height > 160) ? (w->height - 150) : 100;
    uint32_t sb_y = 52 + vp_h;
    uint32_t c_y = sb_y + 14;

    /* 1. Click Play/Pause button (x = w->x + 60, y = w->y + c_y + 10) */
    int32_t play_x = (int32_t)(w->x + 60);
    int32_t play_y = (int32_t)(w->y + c_y + 10);
    desktop_handle_mouse(play_x, play_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(play_x, play_y, 0);
    assert(w->vlc_state == 2); // Paused

    desktop_handle_mouse(play_x, play_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(play_x, play_y, 0);
    assert(w->vlc_state == 1); // Resumed Playing

    /* 2. Spacebar hotkey toggle */
    desktop_handle_key(' ', 0x39, 1, 0);
    assert(w->vlc_state == 2); // Paused via Space

    /* 3. Next Track Hotkey ']' */
    assert(w->vlc_playlist_idx == 0);
    desktop_handle_key(']', 0x1B, 1, 0);
    assert(w->vlc_playlist_idx == 1);
    assert(strcmp(w->vlc_track, "02_cosmic_pulse.wav") == 0);

    /* 4. Previous Track Hotkey '[' */
    desktop_handle_key('[', 0x1A, 1, 0);
    assert(w->vlc_playlist_idx == 0);
    assert(strcmp(w->vlc_track, "01_astral_theme.wav") == 0);

    /* 5. Click Stop button (x = w->x + 100, y = w->y + c_y + 10) */
    int32_t stop_x = (int32_t)(w->x + 100);
    desktop_handle_mouse(stop_x, play_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(stop_x, play_y, 0);
    assert(w->vlc_state == 0); // Stopped
    assert(w->vlc_elapsed_sec == 0);

    printf("PASSED\n");
}

static void test_purple_viewer(void) {
    printf("[TEST  9] Running test_purple_viewer                        ... ");
    setup_test_fb();
    desktop_init();

    int view_id = desktop_open_application("viewer");
    assert(view_id > 0);
    gui_window_t *w = desktop_get_window((uint32_t)view_id);
    assert(w != NULL);
    assert(w->type == WIN_TYPE_VIEWER);
    assert(w->viewer_page_curr == 1);

    /* 1. Click Next Page '>' button: x = w->x + w->width - 76, y = w->y + 40 */
    int32_t next_x = (int32_t)(w->x + w->width - 76);
    int32_t btn_y = (int32_t)(w->y + 40);
    desktop_handle_mouse(next_x, btn_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(next_x, btn_y, 0);
    assert(w->viewer_page_curr == 2);

    /* 2. Click Prev Page '<' button: x = w->x + w->width - 144, y = w->y + 40 */
    int32_t prev_x = (int32_t)(w->x + w->width - 144);
    desktop_handle_mouse(prev_x, btn_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(prev_x, btn_y, 0);
    assert(w->viewer_page_curr == 1);

    /* 3. Click Zoom button: x = w->x + w->width - 30, y = w->y + 40 */
    int32_t zoom_x = (int32_t)(w->x + w->width - 30);
    desktop_handle_mouse(zoom_x, btn_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(zoom_x, btn_y, 0);
    assert(w->viewer_zoom == 125);

    printf("PASSED\n");
}

static void test_partition_manager(void) {
    printf("[TEST 10] Running test_partition_manager                    ... ");
    setup_test_fb();
    desktop_init();

    int disk_id = desktop_open_application("diskmgr");
    assert(disk_id > 0);
    gui_window_t *w = desktop_get_window((uint32_t)disk_id);
    assert(w != NULL);
    assert(w->type == WIN_TYPE_DISKMGR);
    assert(w->diskmgr_confirm_action == 0);

    /* 1. Click "Format Volume" button: x = w->x + 30, y = w->y + w->height - 28 */
    int32_t fmt_x = (int32_t)(w->x + 30);
    int32_t fmt_y = (int32_t)(w->y + w->height - 28);
    desktop_handle_mouse(fmt_x, fmt_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(fmt_x, fmt_y, 0);
    assert(w->diskmgr_confirm_action == 1); // Confirmation modal active!

    /* 2. Click [CANCEL] on confirmation modal: x = w->x + 140, y = w->y + 168 */
    int32_t cancel_x = (int32_t)(w->x + 140);
    int32_t modal_y = (int32_t)(w->y + 168);
    desktop_handle_mouse(cancel_x, modal_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(cancel_x, modal_y, 0);
    assert(w->diskmgr_confirm_action == 0); // Cancelled

    /* 3. Click "Format Volume" again, then click [CONFIRM]: x = w->x + 50, y = w->y + 168 */
    desktop_handle_mouse(fmt_x, fmt_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(fmt_x, fmt_y, 0);
    assert(w->diskmgr_confirm_action == 1);

    int32_t conf_x = (int32_t)(w->x + 50);
    desktop_handle_mouse(conf_x, modal_y, MOUSE_BTN_LEFT);
    desktop_handle_mouse(conf_x, modal_y, 0);
    assert(w->diskmgr_confirm_action == 0); // Format committed safely

    printf("PASSED\n");
}

static void test_terminal_scrolling_and_tab_complete(void) {
    printf("[TEST 11] Running test_terminal_scrolling_and_tab_complete  ... ");
    setup_test_fb();
    desktop_init();

    int term_id = desktop_open_application("terminal");
    assert(term_id > 0);
    gui_window_t *w = desktop_get_window((uint32_t)term_id);
    assert(w != NULL);
    assert(w->type == WIN_TYPE_TERMINAL);

    /* 1. Type partial command "hel" and press TAB for auto-completion */
    w->input_line[0] = 'h';
    w->input_line[1] = 'e';
    w->input_line[2] = 'l';
    w->input_line[3] = '\0';
    w->input_cursor = 3;

    desktop_handle_key('\t', 0x0F, 1, 0);
    assert(strcmp(w->input_line, "help") == 0);

    /* 2. Execute command */
    desktop_handle_key('\n', 0x1C, 1, 0);
    assert(strlen(w->input_line) == 0);
    assert(w->output_count > 2);

    /* 3. Add lines to test Page Up / Page Down scrolling */
    for (int i = 0; i < 20; i++) {
        desktop_terminal_put_line(w, "Purple OS Test Line Output");
    }
    assert(w->output_count > 20);
    assert(w->term_scroll_offset == 0);

    /* Page Up: scancode 0x49 */
    desktop_handle_key(0, 0x49, 1, 0);
    assert(w->term_scroll_offset == 4);

    /* Page Down: scancode 0x51 */
    desktop_handle_key(0, 0x51, 1, 0);
    assert(w->term_scroll_offset == 0);

    printf("PASSED\n");
}

int main(void) {
    printf("=====================================================\n");
    printf(" Running Purple OS GUI Widgets & Apps Unit Tests     \n");
    printf("=====================================================\n");

    test_window_minimize_and_maximize();
    test_application_opening();
    test_calculator_widget();
    test_settings_tabs_and_controls();
    test_text_editor_typing_and_toolbar();
    test_files_explorer_selection();
    test_desktop_rendering_stability();
    test_vlc_media_player();
    test_purple_viewer();
    test_partition_manager();
    test_terminal_scrolling_and_tab_complete();

    printf("=====================================================\n");
    printf(" GUI Widgets & Application Tests: 11/11 passed       \n");
    printf("=====================================================\n");
    return 0;
}
