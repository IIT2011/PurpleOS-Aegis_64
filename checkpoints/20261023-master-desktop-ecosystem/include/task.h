#ifndef TASK_H
#define TASK_H

#include "types.h"

#define TASK_STACK_SIZE    (16 * 1024) /* 16 KiB task stack */
#define MAX_TASKS          64

typedef enum {
    PRIORITY_IDLE   = 0,
    PRIORITY_LOW    = 1,
    PRIORITY_NORMAL = 2,
    PRIORITY_HIGH   = 3
} task_priority_t;

typedef enum {
    TASK_READY,
    TASK_RUNNING,
    TASK_SLEEPING,
    TASK_BLOCKED,
    TASK_TERMINATED
} task_state_t;

typedef struct task_context {
    uint64_t r15;
    uint64_t r14;
    uint64_t r13;
    uint64_t r12;
    uint64_t rbp;
    uint64_t rbx;
    uint64_t rflags;
    uint64_t rip;
} task_context_t;

typedef struct task {
    uint32_t id;
    char name[32];
    task_state_t state;
    task_priority_t priority;
    uint32_t time_slice_ticks;
    uint32_t ticks_remaining;
    uint64_t sleep_until_ticks;
    uint64_t rsp;
    void *stack_base;
    uint64_t cpu_affinity;
    uint32_t last_cpu;
    struct task *next;
} task_t;

void scheduler_init(void);
void scheduler_init_ap(uint32_t cpu_id);
task_t *task_create(const char *name, void (*entry_point)(void *), void *arg, task_priority_t priority);
task_t *task_create_affined(const char *name, void (*entry_point)(void *), void *arg, task_priority_t priority, uint64_t affinity);
void task_exit(void);
void task_sleep(uint32_t ticks);
void task_yield(void);
void task_set_affinity(task_t *task, uint64_t cpu_mask);
uint64_t task_get_affinity(const task_t *task);
void schedule(void);
void task_timer_tick(void);

task_t *task_get_current(void);
void task_list_print(void);
int task_kill(uint32_t task_id);

/* Assembly context switch and trampoline */
extern void switch_context(uint64_t *old_rsp, uint64_t new_rsp);
extern void task_trampoline(void);

#endif /* TASK_H */
