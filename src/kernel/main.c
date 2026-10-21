#include "types.h"
#include "multiboot.h"
#include "serial.h"
#include "video.h"
#include "pmm.h"
#include "vmm.h"
#include "idt.h"
#include "pit.h"
#include "keyboard.h"
#include "task.h"
#include "shell.h"
#include "io.h"

extern char _kernel_start[];
extern char _kernel_end[];

uintptr_t __stack_chk_guard = 0x595e9fbd94fda766ULL;

void __stack_chk_fail(void) {
    kerror("Kernel Panic: Stack smashing detected!");
    while (1) {
        cli();
        hlt();
    }
}

void kernel_main_64(struct multiboot_info *mbi, uint32_t magic) {
    serial_init();

    kinfo("=================================================");
    kinfo("    Aegis64 Kernel (Purple OS) Initializing      ");
    kinfo("=================================================");
    kinfo("Kernel binary loaded: %p to %p", (void *)_kernel_start, (void *)_kernel_end);

    if (magic == MULTIBOOT_BOOTLOADER_MAGIC) {
        kinfo("Multiboot 1 bootloader detected (magic: 0x%x)", magic);
        if (mbi && (mbi->flags & MULTIBOOT_INFO_BOOT_LOADER_NAME)) {
            kinfo("Bootloader name: %s", (const char *)(uintptr_t)mbi->boot_loader_name);
        }
    } else {
        kwarn("Bootloader magic mismatch (got: 0x%x, expected 0x%x)",
              magic, MULTIBOOT_BOOTLOADER_MAGIC);
    }

    /* Initialize Video / Framebuffer */
    video_init(mbi);
    if (video_is_available()) {
        video_draw_rect(0, 0, video_get_width(), 32, COLOR_PURPLE_BAR);
        video_set_cursor(2, 0);
        video_set_colors(COLOR_WHITE, COLOR_PURPLE_BAR);
        video_puts("Purple OS / Aegis64 Native Console [x86-64]");
        video_set_cursor(0, 3);
        video_set_colors(COLOR_WHITE, COLOR_PURPLE_BG);
    }

    /* Step 1: Physical Memory Manager */
    pmm_init(mbi, (uintptr_t)_kernel_start, (uintptr_t)_kernel_end);

    /* Step 2: Virtual Memory Manager */
    vmm_init();

    /* Step 3: Interrupts and Drivers */
    idt_init();
    pit_init(100);       // 100 Hz = 10 ms per tick
    keyboard_init();

    /* Step 4: AegisSched_64 */
    scheduler_init();

    /* Enable Interrupts */
    sti();
    kinfo("Interrupts enabled, entering AegisShell...");

    /* Step 5: Command Shell */
    shell_run();

    /* Halt if shell ever exits */
    while (1) {
        hlt();
    }
}
