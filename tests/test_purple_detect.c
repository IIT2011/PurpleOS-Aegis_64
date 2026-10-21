#define __KERNEL_HOST_TEST__
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "purple_detect.h"

static int event_added_count = 0;
static int event_removed_count = 0;
static int event_changed_count = 0;
static uint32_t last_event_dev_id = 0;

static void test_event_handler(const purple_device_t *dev, uint32_t event) {
    if (!dev) return;
    last_event_dev_id = dev->id;
    if (event == PURPLE_EVENT_DEVICE_ADDED) event_added_count++;
    else if (event == PURPLE_EVENT_DEVICE_REMOVED) event_removed_count++;
    else if (event == PURPLE_EVENT_STATE_CHANGED) event_changed_count++;
}

static void test_purple_detect_init(void) {
    printf("[TEST  1] Running test_purple_detect_init                  ... ");
    assert(purple_detect_init() == 0);
    assert(purple_detect_count() == 0);
    assert(purple_detect_get(1) == NULL);
    assert(purple_detect_find_by_name("nonexistent") == NULL);
    printf("PASSED\n");
}

static void test_purple_detect_registration(void) {
    printf("[TEST  2] Running test_purple_detect_registration          ... ");
    purple_detect_init();
    purple_detect_set_event_handler(test_event_handler);
    event_added_count = 0;

    int id1 = purple_detect_register(PURPLE_CAT_STORAGE, "sda", "Generic", "TestDisk", "ATA", 0x1F0, 14,
                                     PURPLE_STATE_ACTIVE, PURPLE_DEV_FLAG_REMOVABLE);
    assert(id1 == 1);
    assert(purple_detect_count() == 1);
    assert(event_added_count == 1);
    assert(last_event_dev_id == 1);

    const purple_device_t *d = purple_detect_get(1);
    assert(d != NULL);
    assert(d->id == 1);
    assert(d->category == PURPLE_CAT_STORAGE);
    assert(d->state == PURPLE_STATE_ACTIVE);
    assert(strcmp(d->name, "sda") == 0);
    assert(strcmp(d->vendor, "Generic") == 0);
    assert(strcmp(d->model, "TestDisk") == 0);
    assert(strcmp(d->bus, "ATA") == 0);
    assert(d->address == 0x1F0);
    assert(d->irq == 14);
    assert(d->flags & PURPLE_DEV_FLAG_REMOVABLE);

    int id2 = purple_detect_register(PURPLE_CAT_INPUT, "kbd", "IBM", "PS2Kbd", "ISA", 0x60, 1,
                                     PURPLE_STATE_ACTIVE, PURPLE_DEV_FLAG_BUILTIN);
    assert(id2 == 2);
    assert(purple_detect_count() == 2);
    assert(event_added_count == 2);
    assert(last_event_dev_id == 2);

    printf("PASSED\n");
}

static void test_purple_detect_queries(void) {
    printf("[TEST  3] Running test_purple_detect_queries               ... ");
    purple_detect_init();

    purple_detect_register(PURPLE_CAT_STORAGE, "disk0", "VendorA", "Model1", "ATA", 0x100, 14,
                           PURPLE_STATE_ACTIVE, 0);
    purple_detect_register(PURPLE_CAT_NETWORK, "eth0", "Intel", "e1000", "PCI", 0x200, 11,
                           PURPLE_STATE_ACTIVE, 0);
    purple_detect_register(PURPLE_CAT_STORAGE, "disk1", "VendorB", "Model2", "ATA", 0x110, 15,
                           PURPLE_STATE_ACTIVE, 0);

    assert(purple_detect_count() == 3);

    const purple_device_t *by_name = purple_detect_find_by_name("eth0");
    assert(by_name != NULL);
    assert(by_name->category == PURPLE_CAT_NETWORK);
    assert(strcmp(by_name->vendor, "Intel") == 0);

    const purple_device_t *s0 = purple_detect_find_by_category(PURPLE_CAT_STORAGE, 0);
    assert(s0 != NULL);
    assert(strcmp(s0->name, "disk0") == 0);

    const purple_device_t *s1 = purple_detect_find_by_category(PURPLE_CAT_STORAGE, 1);
    assert(s1 != NULL);
    assert(strcmp(s1->name, "disk1") == 0);

    const purple_device_t *s2 = purple_detect_find_by_category(PURPLE_CAT_STORAGE, 2);
    assert(s2 == NULL); // Only 2 storage devices

    printf("PASSED\n");
}

static void test_purple_detect_state_transitions(void) {
    printf("[TEST  4] Running test_purple_detect_state_transitions     ... ");
    purple_detect_init();
    purple_detect_set_event_handler(test_event_handler);
    event_changed_count = 0;

    int id = purple_detect_register(PURPLE_CAT_DISPLAY, "vga", "Bochs", "VGA", "PCI", 0x300, 0,
                                    PURPLE_STATE_DISCOVERED, 0);
    assert(id == 1);

    assert(purple_detect_update_state(1, PURPLE_STATE_ACTIVE) == 0);
    assert(event_changed_count == 1);
    assert(purple_detect_get(1)->state == PURPLE_STATE_ACTIVE);

    assert(purple_detect_update_state(1, PURPLE_STATE_FAILED) == 0);
    assert(event_changed_count == 2);
    assert(purple_detect_get(1)->state == PURPLE_STATE_FAILED);

    // Invalid ID
    assert(purple_detect_update_state(999, PURPLE_STATE_ACTIVE) != 0);

    printf("PASSED\n");
}

static void test_purple_detect_unregistration(void) {
    printf("[TEST  5] Running test_purple_detect_unregistration        ... ");
    purple_detect_init();
    purple_detect_set_event_handler(test_event_handler);
    event_removed_count = 0;

    int id = purple_detect_register(PURPLE_CAT_SERIAL, "COM1", "Standard", "UART", "ISA", 0x3F8, 4,
                                    PURPLE_STATE_ACTIVE, PURPLE_DEV_FLAG_BUILTIN);
    assert(id == 1);

    assert(purple_detect_unregister(1) == 0);
    assert(event_removed_count == 1);
    assert(purple_detect_get(1)->state == PURPLE_STATE_REMOVED);

    // Unregister non-existent
    assert(purple_detect_unregister(99) != 0);

    printf("PASSED\n");
}

static void test_purple_detect_table_capacity(void) {
    printf("[TEST  6] Running test_purple_detect_table_capacity        ... ");
    purple_detect_init();

    for (int i = 0; i < PURPLE_MAX_DEVICES; i++) {
        char name[16];
        snprintf(name, sizeof(name), "dev%d", i);
        int res = purple_detect_register(PURPLE_CAT_VIRTUAL, name, "Test", "Test", "MEM", i, 0,
                                         PURPLE_STATE_ACTIVE, 0);
        assert(res > 0);
    }
    assert(purple_detect_count() == PURPLE_MAX_DEVICES);

    // Next registration must fail cleanly
    int overflow_res = purple_detect_register(PURPLE_CAT_VIRTUAL, "overflow", "Test", "Test", "MEM", 999, 0,
                                              PURPLE_STATE_ACTIVE, 0);
    assert(overflow_res == -1);
    assert(purple_detect_count() == PURPLE_MAX_DEVICES);

    printf("PASSED\n");
}

static void test_purple_detect_string_conversions(void) {
    printf("[TEST  7] Running test_purple_detect_string_conversions    ... ");
    assert(strcmp(purple_category_to_str(PURPLE_CAT_STORAGE), "STORAGE") == 0);
    assert(strcmp(purple_category_to_str(PURPLE_CAT_INPUT), "INPUT") == 0);
    assert(strcmp(purple_category_to_str(PURPLE_CAT_NETWORK), "NETWORK") == 0);
    assert(strcmp(purple_category_to_str(PURPLE_CAT_DISPLAY), "DISPLAY") == 0);
    assert(strcmp(purple_category_to_str(PURPLE_CAT_AUDIO), "AUDIO") == 0);
    assert(strcmp(purple_category_to_str(PURPLE_CAT_BRIDGE), "BRIDGE") == 0);

    assert(strcmp(purple_state_to_str(PURPLE_STATE_ACTIVE), "ACTIVE") == 0);
    assert(strcmp(purple_state_to_str(PURPLE_STATE_DISCOVERED), "DISCOVERED") == 0);
    assert(strcmp(purple_state_to_str(PURPLE_STATE_SUPPORTED), "SUPPORTED") == 0);
    assert(strcmp(purple_state_to_str(PURPLE_STATE_REMOVED), "REMOVED") == 0);
    assert(strcmp(purple_state_to_str(PURPLE_STATE_FAILED), "FAILED") == 0);
    printf("PASSED\n");
}

int main(void) {
    printf("=====================================================\n");
    printf(" Running PurpleDetect Hardware Subsystem Unit Tests  \n");
    printf("=====================================================\n");

    test_purple_detect_init();
    test_purple_detect_registration();
    test_purple_detect_queries();
    test_purple_detect_state_transitions();
    test_purple_detect_unregistration();
    test_purple_detect_table_capacity();
    test_purple_detect_string_conversions();

    printf("=====================================================\n");
    printf(" PurpleDetect Tests: 7/7 passed                      \n");
    printf("=====================================================\n");
    return 0;
}
