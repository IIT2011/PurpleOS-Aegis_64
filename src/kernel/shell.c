#include "shell.h"
#include "serial.h"
#include "video.h"
#include "keyboard.h"
#include "string.h"
#include "pmm.h"
#include "vmm.h"
#include "task.h"
#include "pit.h"
#include "io.h"

#define SHELL_INPUT_BUFFER_LEN 256
#define MAX_ARGS 16

static char shell_buf[SHELL_INPUT_BUFFER_LEN];
static size_t shell_buf_len = 0;

static void shell_print(const char *s) {
    serial_write(s);
    if (video_is_available()) {
        video_puts(s);
    }
}

static void shell_prompt(void) {
    if (video_is_available()) {
        video_set_colors(COLOR_PURPLE_ACC, COLOR_PURPLE_BG);
        video_puts("aegis64");
        video_set_colors(COLOR_GOLD, COLOR_PURPLE_BG);
        video_puts("@");
        video_set_colors(COLOR_WHITE, COLOR_PURPLE_BG);
        video_puts("purple:~$ ");
    }
    serial_write("aegis64@purple:~$ ");
}

static void reboot_system(void) {
    shell_print("Rebooting Aegis64 system via 8042 keyboard controller...\n");
    uint8_t good = 0x02;
    while (good & 0x02) {
        good = inb(0x64);
    }
    outb(0x64, 0xFE);
    hlt();
}

static void test_task_worker(void *arg) {
    const char *name = (const char *)arg;
    for (int i = 0; i < 3; i++) {
        kinfo("TaskWorker [%s]: iteration %d/3", name, i + 1);
        task_sleep(10); // sleep 100 ms
    }
    kinfo("TaskWorker [%s]: finished execution", name);
}

static void execute_command(int argc, char **argv) {
    if (argc == 0) return;

    if (strcmp(argv[0], "help") == 0) {
        shell_print("\n=== AegisShell / Purple Shell Commands ===\n");
        shell_print("  help           - Show available shell commands\n");
        shell_print("  sysinfo        - Display OS version and architectural parameters\n");
        shell_print("  cpuinfo        - Display CPU long-mode features\n");
        shell_print("  mem            - Display physical and virtual memory statistics\n");
        shell_print("  uptime         - Display system uptime and PIT timer ticks\n");
        shell_print("  tasks / ps     - Display task table and AegisSched_64 states\n");
        shell_print("  task spawn <n> - Spawn a test background task with given name\n");
        shell_print("  task kill <id> - Terminate a running task by ID\n");
        shell_print("  task test      - Run automated multi-task preemption test\n");
        shell_print("  clear          - Clear terminal screen\n");
        shell_print("  echo <text>    - Display text arguments\n");
        shell_print("  reboot/restart - Perform ACPI/controller system reboot\n");
        shell_print("  panic          - Trigger intentional kernel panic for diagnostics\n\n");
    } else if (strcmp(argv[0], "sysinfo") == 0) {
        shell_print("\nPurple OS (Aegis64 Kernel)\n");
        shell_print("  Architecture: x86-64 (AMD64 Long Mode)\n");
        shell_print("  Scheduler:    AegisSched_64 / PurpleSched (Round-Robin, 4-tier)\n");
        shell_print("  Filesystem:   PurpleFS v2 (Native persistent journaled FS)\n");
        shell_print("  Driver Model: APDRV v3 (.apdrv Ring 0 packages)\n");
        shell_print("  App Format:   .apapplications (Native bundle format)\n");
        shell_print("  Trust Model:  AegisDetect Hardware Baseline\n\n");
    } else if (strcmp(argv[0], "cpuinfo") == 0) {
        shell_print("\nCPU Configuration:\n");
        shell_print("  Mode: 64-bit Long Mode enabled\n");
        shell_print("  Paging: 4-Level Paging (PML4, PDPT, PD, PT)\n");
        shell_print("  Protection: CR0.WP enforced, EFER.NXE enabled (W^X)\n\n");
    } else if (strcmp(argv[0], "mem") == 0) {
        size_t total = pmm_get_total_frames();
        size_t free_f = pmm_get_free_frames();
        size_t alloc_f = pmm_get_allocated_frames();
        size_t res_f = pmm_get_reserved_frames();

        kprintf("\nMemory Accounting:\n");
        kprintf("  Managed Space: 0 - 4 GiB (%u frames, 4096 bytes/frame)\n", (uint32_t)total);
        kprintf("  Free Frames:   %u (%u MiB)\n", (uint32_t)free_f, (uint32_t)(free_f * 4 / 1024));
        kprintf("  Allocated:     %u (%u MiB)\n", (uint32_t)alloc_f, (uint32_t)(alloc_f * 4 / 1024));
        kprintf("  Reserved:      %u (%u MiB)\n\n", (uint32_t)res_f, (uint32_t)(res_f * 4 / 1024));
    } else if (strcmp(argv[0], "uptime") == 0) {
        uint64_t ticks = pit_get_ticks();
        uint64_t sec = ticks / 100;
        kprintf("Uptime: %u.%02u seconds (%u ticks @ 100 Hz)\n",
                (uint32_t)sec, (uint32_t)(ticks % 100), (uint32_t)ticks);
    } else if (strcmp(argv[0], "tasks") == 0 || strcmp(argv[0], "ps") == 0) {
        task_list_print();
    } else if (strcmp(argv[0], "task") == 0) {
        if (argc >= 3 && strcmp(argv[1], "spawn") == 0) {
            task_t *t = task_create(argv[2], test_task_worker, argv[2], PRIORITY_NORMAL);
            if (t) {
                kprintf("Spawned background task '%s' (ID %u)\n", argv[2], t->id);
            }
        } else if (argc >= 3 && strcmp(argv[1], "kill") == 0) {
            uint32_t tid = 0;
            for (char *c = argv[2]; *c >= '0' && *c <= '9'; c++) {
                tid = tid * 10 + (*c - '0');
            }
            task_kill(tid);
        } else if (argc >= 2 && strcmp(argv[1], "test") == 0) {
            shell_print("Spawning automated scheduler test tasks (Task_A, Task_B, Task_C)...\n");
            task_create("Task_A", test_task_worker, "Task_A", PRIORITY_HIGH);
            task_create("Task_B", test_task_worker, "Task_B", PRIORITY_NORMAL);
            task_create("Task_C", test_task_worker, "Task_C", PRIORITY_LOW);
        } else {
            shell_print("Usage: task spawn <name> | task kill <id> | task test\n");
        }
    } else if (strcmp(argv[0], "clear") == 0) {
        if (video_is_available()) {
            video_clear(COLOR_PURPLE_BG);
        }
    } else if (strcmp(argv[0], "echo") == 0) {
        for (int i = 1; i < argc; i++) {
            shell_print(argv[i]);
            if (i + 1 < argc) shell_print(" ");
        }
        shell_print("\n");
    } else if (strcmp(argv[0], "panic") == 0) {
        kerror("Intentional kernel panic requested from AegisShell!");
        while (1) {
            cli();
            hlt();
        }
    } else if (strcmp(argv[0], "reboot") == 0 || strcmp(argv[0], "restart") == 0) {
        reboot_system();
    } else {
        shell_print("Unknown command: '");
        shell_print(argv[0]);
        shell_print("'. Type 'help' for available commands.\n");
    }
}

static void parse_and_execute(char *cmd) {
    char *argv[MAX_ARGS];
    int argc = 0;
    char *p = cmd;

    while (*p && argc < MAX_ARGS) {
        // Skip leading whitespace
        while (*p == ' ' || *p == '\t') p++;
        if (*p == '\0') break;

        if (*p == '"') {
            p++; // skip leading quote
            argv[argc++] = p;
            while (*p && *p != '"') p++;
            if (*p == '"') {
                *p = '\0';
                p++;
            }
        } else {
            argv[argc++] = p;
            while (*p && *p != ' ' && *p != '\t') p++;
            if (*p) {
                *p = '\0';
                p++;
            }
        }
    }

    execute_command(argc, argv);
}

void shell_init(void) {
    shell_buf_len = 0;
    shell_print("\n");
    shell_print("=======================================================================\n");
    shell_print("  Welcome to Purple OS / Aegis64 (Native 64-bit Kernel)\n");
    shell_print("  Command Environment: AegisShell 1.0\n");
    shell_print("  Type 'help' for available commands, 'tasks' to view scheduler.\n");
    shell_print("=======================================================================\n\n");
    shell_prompt();
}

void shell_step(void) {
    char c = 0;
    int got_char = 0;

    if (keyboard_has_char()) {
        c = keyboard_getchar();
        got_char = 1;
    } else if (serial_has_char()) {
        c = serial_getchar();
        got_char = 1;
    }

    if (got_char) {
        if (c == '\n' || c == '\r') {
            shell_print("\n");
            shell_buf[shell_buf_len] = '\0';
            parse_and_execute(shell_buf);
            shell_buf_len = 0;
            shell_prompt();
        } else if (c == '\b' || c == 127) {
            if (shell_buf_len > 0) {
                shell_buf_len--;
                shell_buf[shell_buf_len] = '\0';
                serial_write("\b \b");
                if (video_is_available()) {
                    video_putc('\b');
                }
            }
        } else if (c >= 32 && c <= 126) {
            if (shell_buf_len < SHELL_INPUT_BUFFER_LEN - 1) {
                shell_buf[shell_buf_len++] = c;
                shell_buf[shell_buf_len] = '\0';
                serial_putchar(c);
                if (video_is_available()) {
                    video_putc(c);
                }
            }
        }
    }
}

void shell_run(void) {
    shell_init();
    while (1) {
        shell_step();
        task_yield();
    }
}
