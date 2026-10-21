#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>

#include "task.h"
#include "pmm.h"

static int tests_run = 0;
static int tests_passed = 0;

#define RUN_TEST(fn) do { \
    tests_run++; \
    printf("[TEST %d] Running %s... ", tests_run, #fn); \
    fflush(stdout); \
    if (fn()) { \
        printf("PASSED\n"); \
        tests_passed++; \
    } else { \
        printf("FAILED\n"); \
    } \
    fflush(stdout); \
} while (0)

static void setup_test_env(void) {
    mmap((void *)0x1000000, 48 * 1024 * 1024, PROT_READ|PROT_WRITE,
         MAP_PRIVATE|MAP_ANONYMOUS|MAP_FIXED, -1, 0);

    void *mem = mmap(NULL, 8192, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANONYMOUS|MAP_32BIT, -1, 0);
    struct multiboot_info *mbi = (struct multiboot_info *)mem;
    struct multiboot_mmap_entry *mmap_entries = (struct multiboot_mmap_entry *)((uint8_t *)mem + sizeof(struct multiboot_info));

    mbi->flags = MULTIBOOT_INFO_MEM_MAP;

    mmap_entries[0].size = sizeof(struct multiboot_mmap_entry) - sizeof(uint32_t);
    mmap_entries[0].addr = 0x0;
    mmap_entries[0].len  = 0x1000000;
    mmap_entries[0].type = MULTIBOOT_MEMORY_RESERVED;

    mmap_entries[1].size = sizeof(struct multiboot_mmap_entry) - sizeof(uint32_t);
    mmap_entries[1].addr = 0x1000000;
    mmap_entries[1].len  = 0x3000000; // 48 MiB
    mmap_entries[1].type = MULTIBOOT_MEMORY_AVAILABLE;

    mbi->mmap_addr = (uint32_t)(uintptr_t)mmap_entries;
    mbi->mmap_length = sizeof(struct multiboot_mmap_entry) * 2;

    pmm_init(mbi, 0x100000, 0x200000);
    scheduler_init();
}

static void dummy_task_entry(void *arg) {
    (void)arg;
}

static int test_scheduler_init_state(void) {
    task_t *cur = task_get_current();
    if (!cur) return 0;
    if (cur->id != 1) return 0;
    if (cur->state != TASK_RUNNING) return 0;
    if (cur->priority != PRIORITY_NORMAL) return 0;
    if (strcmp(cur->name, "kernel_main") != 0) return 0;
    return 1;
}

static int test_scheduler_task_create(void) {
    task_t *t = task_create("Worker1", dummy_task_entry, (void *)0x1234, PRIORITY_HIGH);
    if (!t) return 0;
    if (t->state != TASK_READY) return 0;
    if (t->priority != PRIORITY_HIGH) return 0;
    if (t->time_slice_ticks != 10) return 0; // HIGH = 10 ticks
    if (strcmp(t->name, "Worker1") != 0) return 0;

    return 1;
}

static int test_scheduler_priority_slices(void) {
    task_t *t_idle = task_create("IdleTest", dummy_task_entry, NULL, PRIORITY_IDLE);
    task_t *t_low  = task_create("LowTest", dummy_task_entry, NULL, PRIORITY_LOW);
    task_t *t_norm = task_create("NormTest", dummy_task_entry, NULL, PRIORITY_NORMAL);
    task_t *t_high = task_create("HighTest", dummy_task_entry, NULL, PRIORITY_HIGH);

    if (!t_idle || !t_low || !t_norm || !t_high) return 0;

    if (t_idle->time_slice_ticks != 1) return 0;
    if (t_low->time_slice_ticks  != 2) return 0;
    if (t_norm->time_slice_ticks != 5) return 0;
    if (t_high->time_slice_ticks != 10) return 0;

    return 1;
}

static int test_scheduler_kill(void) {
    task_t *t = task_create("KillMe", dummy_task_entry, NULL, PRIORITY_NORMAL);
    if (!t) return 0;
    uint32_t tid = t->id;

    int res = task_kill(tid);
    if (res != 0) return 0;
    if (t->state != TASK_TERMINATED) return 0;

    /* Killing task 1 (kernel_main) must fail */
    res = task_kill(1);
    if (res == 0) return 0;

    return 1;
}

int main(void) {
    printf("========================================\n");
    printf(" Running AegisSched_64 Unit Tests\n");
    printf("========================================\n");

    setup_test_env();

    RUN_TEST(test_scheduler_init_state);
    RUN_TEST(test_scheduler_task_create);
    RUN_TEST(test_scheduler_priority_slices);
    RUN_TEST(test_scheduler_kill);

    printf("========================================\n");
    printf(" Scheduler Tests: %d/%d passed\n", tests_passed, tests_run);
    printf("========================================\n");

    return (tests_passed == tests_run) ? 0 : 1;
}
