#include "task.h"
#include "pmm.h"
#include "string.h"
#include "serial.h"
#include "io.h"
#include "cpu.h"
#include "spinlock.h"
#include "smp.h"

static task_t task_table[MAX_TASKS];
static task_t *current_tasks[MAX_CPUS];
static task_t *idle_tasks[MAX_CPUS];
static spinlock_t sched_lock = SPINLOCK_INIT;
static uint32_t next_task_id = 1;
static uint64_t system_ticks = 0;

static uint32_t get_slice_for_priority(task_priority_t prio) {
    switch (prio) {
        case PRIORITY_IDLE:   return 1;
        case PRIORITY_LOW:    return 2;
        case PRIORITY_NORMAL: return 5;
        case PRIORITY_HIGH:   return 10;
        default:              return 5;
    }
}

task_t *task_get_current(void) {
    uint32_t cpu_id = cpu_get_current_id();
    if (cpu_id >= MAX_CPUS) cpu_id = 0;
    return current_tasks[cpu_id];
}

static void idle_task_func(void *arg) {
    (void)arg;
    while (1) {
        hlt();
    }
}

void task_set_affinity(task_t *task, uint64_t cpu_mask) {
    if (!task || cpu_mask == 0) return;
    uint64_t rflags;
    spinlock_acquire_irqsave(&sched_lock, &rflags);
    task->cpu_affinity = cpu_mask;
    spinlock_release_irqrestore(&sched_lock, rflags);
    smp_broadcast_resched();
}

uint64_t task_get_affinity(const task_t *task) {
    if (!task) return 0;
    return task->cpu_affinity;
}

task_t *task_create(const char *name, void (*entry_point)(void *), void *arg, task_priority_t priority) {
    uint64_t rflags;
    spinlock_acquire_irqsave(&sched_lock, &rflags);

    task_t *t = NULL;
    for (int i = 0; i < MAX_TASKS; i++) {
        if (task_table[i].state == TASK_TERMINATED || task_table[i].id == 0) {
            t = &task_table[i];
            break;
        }
    }

    if (!t) {
        kerror("Scheduler: Maximum task limit reached (%d)", MAX_TASKS);
        spinlock_release_irqrestore(&sched_lock, rflags);
        return NULL;
    }

    /* Allocate 16 KiB stack = 4 physical pages */
    void *stack = pmm_alloc_frames(TASK_STACK_SIZE / PMM_PAGE_SIZE);
    if (!stack) {
        kerror("Scheduler: Failed to allocate stack for task %s", name);
        spinlock_release_irqrestore(&sched_lock, rflags);
        return NULL;
    }

    t->id = next_task_id++;
    strncpy(t->name, name, sizeof(t->name) - 1);
    t->name[sizeof(t->name) - 1] = '\0';
    t->priority = priority;
    t->time_slice_ticks = get_slice_for_priority(priority);
    t->ticks_remaining = t->time_slice_ticks;
    t->sleep_until_ticks = 0;
    t->stack_base = stack;
    t->cpu_affinity = ~0ULL; // Any CPU by default
    t->last_cpu = cpu_get_current_id();

    /* Setup initial stack for switch_context and task_trampoline */
    uint64_t *sp = (uint64_t *)((uintptr_t)stack + TASK_STACK_SIZE);

    /* Trampoline expects:
       [sp+0]: function pointer
       [sp+8]: argument
    */
    *(--sp) = (uint64_t)arg;
    *(--sp) = (uint64_t)entry_point;

    /* switch_context will pop:
       rip (task_trampoline), rflags, rbx, rbp, r12, r13, r14, r15
    */
    *(--sp) = (uint64_t)task_trampoline; // RIP
    *(--sp) = 0x202;                     // RFLAGS (IF enabled)
    *(--sp) = 0;                         // RBX
    *(--sp) = 0;                         // RBP
    *(--sp) = 0;                         // R12
    *(--sp) = 0;                         // R13
    *(--sp) = 0;                         // R14
    *(--sp) = 0;                         // R15

    t->rsp = (uint64_t)sp;
    t->state = TASK_READY;

    kinfo("Scheduler: Created task %u ('%s') at stack %p, priority %u",
          t->id, t->name, stack, (uint32_t)priority);

    spinlock_release_irqrestore(&sched_lock, rflags);
    smp_broadcast_resched();
    return t;
}

void task_exit(void) {
    uint32_t cpu_id = cpu_get_current_id();
    if (cpu_id >= MAX_CPUS) cpu_id = 0;

    uint64_t rflags;
    spinlock_acquire_irqsave(&sched_lock, &rflags);
    task_t *cur = current_tasks[cpu_id];
    if (!cur) {
        spinlock_release_irqrestore(&sched_lock, rflags);
        hlt();
    }

    kinfo("Scheduler: Task %u ('%s') exiting on CPU %u", cur->id, cur->name, cpu_id);
    cur->state = TASK_TERMINATED;

    /* Free stack if not idle task */
    if (cur != idle_tasks[cpu_id] && cur->stack_base) {
        pmm_free_frames(cur->stack_base, TASK_STACK_SIZE / PMM_PAGE_SIZE);
        cur->stack_base = NULL;
    }

    spinlock_release_irqrestore(&sched_lock, rflags);
    schedule();

    /* Should never reach here */
    while (1) {
        hlt();
    }
}

void task_sleep(uint32_t ticks) {
    uint32_t cpu_id = cpu_get_current_id();
    if (cpu_id >= MAX_CPUS) cpu_id = 0;

    uint64_t rflags;
    spinlock_acquire_irqsave(&sched_lock, &rflags);
    task_t *cur = current_tasks[cpu_id];
    if (!cur) {
        spinlock_release_irqrestore(&sched_lock, rflags);
        return;
    }
    cur->sleep_until_ticks = system_ticks + ticks;
    cur->state = TASK_SLEEPING;
    spinlock_release_irqrestore(&sched_lock, rflags);
    schedule();
}

void task_yield(void) {
    uint32_t cpu_id = cpu_get_current_id();
    if (cpu_id >= MAX_CPUS) cpu_id = 0;

    uint64_t rflags;
    spinlock_acquire_irqsave(&sched_lock, &rflags);
    task_t *cur = current_tasks[cpu_id];
    if (cur) {
        cur->ticks_remaining = 0;
    }
    spinlock_release_irqrestore(&sched_lock, rflags);
    schedule();
}

static task_t *pick_next_task(uint32_t cpu_id) {
    /* 1. Wake up sleeping tasks whose timer expired */
    for (int i = 0; i < MAX_TASKS; i++) {
        if (task_table[i].id != 0 && task_table[i].state == TASK_SLEEPING) {
            if (system_ticks >= task_table[i].sleep_until_ticks) {
                task_table[i].state = TASK_READY;
            }
        }
    }

    /* 2. Find highest priority ready task eligible for this CPU */
    for (int prio = PRIORITY_HIGH; prio >= PRIORITY_LOW; prio--) {
        int start_idx = (current_tasks[cpu_id]) ? (int)(current_tasks[cpu_id] - task_table + 1) : 0;
        for (int i = 0; i < MAX_TASKS; i++) {
            int idx = (start_idx + i) % MAX_TASKS;
            if (task_table[idx].id != 0 &&
                task_table[idx].state == TASK_READY &&
                task_table[idx].priority == (task_priority_t)prio &&
                (task_table[idx].cpu_affinity & (1ULL << cpu_id))) {
                return &task_table[idx];
            }
        }
    }

    /* If current task is ready and still running on this CPU, it can continue */
    if (current_tasks[cpu_id] && current_tasks[cpu_id]->state == TASK_RUNNING &&
        current_tasks[cpu_id] != idle_tasks[cpu_id]) {
        return current_tasks[cpu_id];
    }

    /* Otherwise, pick this CPU's idle task */
    return idle_tasks[cpu_id];
}

void schedule(void) {
    uint32_t cpu_id = cpu_get_current_id();
    if (cpu_id >= MAX_CPUS) cpu_id = 0;

    uint64_t rflags;
    spinlock_acquire_irqsave(&sched_lock, &rflags);

    task_t *prev = current_tasks[cpu_id];
    task_t *next = pick_next_task(cpu_id);

    if (!next) {
        next = idle_tasks[cpu_id];
    }

    if (prev == next && prev && prev->state == TASK_RUNNING) {
        spinlock_release_irqrestore(&sched_lock, rflags);
        return;
    }

    if (prev && prev->state == TASK_RUNNING) {
        prev->state = TASK_READY;
    }

    if (next) {
        next->state = TASK_RUNNING;
        next->ticks_remaining = next->time_slice_ticks;
        next->last_cpu = cpu_id;
    }
    current_tasks[cpu_id] = next;

    spinlock_release_irqrestore(&sched_lock, rflags);

    if (prev) {
        switch_context(&prev->rsp, next->rsp);
    } else {
        uint64_t dummy = 0;
        switch_context(&dummy, next->rsp);
    }
}

void task_timer_tick(void) {
    uint32_t cpu_id = cpu_get_current_id();
    if (cpu_id >= MAX_CPUS) cpu_id = 0;

    cpu_t *c = cpu_get(cpu_id);
    if (c) c->lapic_timer_ticks++;

    if (cpu_id == 0) {
        system_ticks++;
    }

    task_t *cur = current_tasks[cpu_id];
    if (!cur) return;

    if (cur->ticks_remaining > 0) {
        cur->ticks_remaining--;
    }

    if (cur->ticks_remaining == 0) {
        schedule();
    }
}

void scheduler_init(void) {
    memset(task_table, 0, sizeof(task_table));
    memset(current_tasks, 0, sizeof(current_tasks));
    memset(idle_tasks, 0, sizeof(idle_tasks));
    system_ticks = 0;
    next_task_id = 1;

    /* Initialize boot/main kernel context as Task 1 on CPU 0 */
    task_t *main_task = &task_table[0];
    main_task->id = next_task_id++;
    strncpy(main_task->name, "kernel_main", sizeof(main_task->name));
    main_task->priority = PRIORITY_NORMAL;
    main_task->time_slice_ticks = get_slice_for_priority(PRIORITY_NORMAL);
    main_task->ticks_remaining = main_task->time_slice_ticks;
    main_task->state = TASK_RUNNING;
    main_task->stack_base = NULL; // Uses kernel boot stack
    main_task->cpu_affinity = 1ULL << 0; // Bound to BSP
    main_task->last_cpu = 0;
    current_tasks[0] = main_task;

    /* Create idle task for CPU 0 */
    idle_tasks[0] = task_create("idle-0", idle_task_func, NULL, PRIORITY_IDLE);
    if (idle_tasks[0]) {
        task_set_affinity(idle_tasks[0], 1ULL << 0);
    }

    kinfo("Scheduler (AegisSched_64 / PurpleSched) Initialized");
}

void scheduler_init_ap(uint32_t cpu_id) {
    if (cpu_id >= MAX_CPUS) return;

    char name[16];
    name[0] = 'i'; name[1] = 'd'; name[2] = 'l'; name[3] = 'e'; name[4] = '-';
    if (cpu_id < 10) {
        name[5] = '0' + cpu_id;
        name[6] = '\0';
    } else {
        name[5] = '0' + (cpu_id / 10);
        name[6] = '0' + (cpu_id % 10);
        name[7] = '\0';
    }

    idle_tasks[cpu_id] = task_create(name, idle_task_func, NULL, PRIORITY_IDLE);
    if (idle_tasks[cpu_id]) {
        task_set_affinity(idle_tasks[cpu_id], 1ULL << cpu_id);
        current_tasks[cpu_id] = idle_tasks[cpu_id];
    }
    kinfo("Scheduler: Initialized AP core CPU #%u with %s", cpu_id, name);
}

void task_list_print(void) {
    kprintf("\n=== AegisSched_64 Active Tasks ===\n");
    kprintf("%-4s %-16s %-10s %-8s %-6s %-10s %-6s\n",
            "ID", "NAME", "STATE", "PRIORITY", "TICKS", "AFFINITY", "CPU#");
    for (int i = 0; i < MAX_TASKS; i++) {
        if (task_table[i].id != 0 && task_table[i].state != TASK_TERMINATED) {
            const char *state_str = "UNKNOWN";
            switch (task_table[i].state) {
                case TASK_READY:      state_str = "READY"; break;
                case TASK_RUNNING:    state_str = "RUNNING"; break;
                case TASK_SLEEPING:   state_str = "SLEEPING"; break;
                case TASK_BLOCKED:    state_str = "BLOCKED"; break;
                case TASK_TERMINATED: state_str = "TERMINATED"; break;
            }
            const char *prio_str = "NORMAL";
            switch (task_table[i].priority) {
                case PRIORITY_IDLE:   prio_str = "IDLE"; break;
                case PRIORITY_LOW:    prio_str = "LOW"; break;
                case PRIORITY_NORMAL: prio_str = "NORMAL"; break;
                case PRIORITY_HIGH:   prio_str = "HIGH"; break;
            }
            kprintf("%-4u %-16s %-10s %-8s %-6u 0x%-8x CPU#%u\n",
                    task_table[i].id, task_table[i].name, state_str, prio_str,
                    task_table[i].ticks_remaining, (uint32_t)task_table[i].cpu_affinity,
                    task_table[i].last_cpu);
        }
    }
}

int task_kill(uint32_t task_id) {
    if (task_id == 1) {
        kerror("Scheduler: Cannot terminate critical kernel task");
        return -1;
    }
    for (int c = 0; c < MAX_CPUS; c++) {
        if (idle_tasks[c] && idle_tasks[c]->id == task_id) {
            kerror("Scheduler: Cannot terminate idle task");
            return -1;
        }
    }
    uint64_t rflags;
    spinlock_acquire_irqsave(&sched_lock, &rflags);
    for (int i = 0; i < MAX_TASKS; i++) {
        if (task_table[i].id == task_id) {
            task_table[i].state = TASK_TERMINATED;
            if (task_table[i].stack_base) {
                pmm_free_frames(task_table[i].stack_base, TASK_STACK_SIZE / PMM_PAGE_SIZE);
                task_table[i].stack_base = NULL;
            }
            kinfo("Scheduler: Terminated task %u", task_id);
            spinlock_release_irqrestore(&sched_lock, rflags);
            return 0;
        }
    }
    spinlock_release_irqrestore(&sched_lock, rflags);
    kerror("Scheduler: Task ID %u not found", task_id);
    return -1;
}
