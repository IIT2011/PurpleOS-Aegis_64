#include "types.h"
#include "multiboot.h"
#include "serial.h"
#include "video.h"
#include "pmm.h"
#include "vmm.h"
#include "idt.h"
#include "pic.h"
#include "pit.h"
#include "keyboard.h"
#include "task.h"
#include "shell.h"
#include "kernel_storage.h"
#include "cpu.h"
#include "acpi.h"
#include "apic.h"
#include "smp.h"
#include "security/aegis_detect.h"
#include "purple_detect.h"
#include "pci.h"
#include "mouse.h"
#include "desktop.h"
#include "ksym.h"
#include "netdev.h"
#include "apdrv.h"
#include "apapp.h"
#include "io.h"

extern char _kernel_start[];
extern char _kernel_end[];

extern uint64_t boot_pd_3[];
static void check_pd3(const char *where) {
    if (boot_pd_3[503] != 0xfee00083) {
        kerror("MAIN: boot_pd_3[503] changed at %s! val=0x%llx", where, (unsigned long long)boot_pd_3[503]);
    }
}

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
    extern uint64_t boot_pd_3[];
    kinfo("MAIN: boot_pd_3[503]=0x%llx", (unsigned long long)boot_pd_3[503]);

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

    /* Step 0: CPU Feature Discovery */
    cpu_init_bsp();
    cpu_print_info();

    /* Step 1: Physical Memory Manager */
    pmm_init(mbi, (uintptr_t)_kernel_start, (uintptr_t)_kernel_end);
    check_pd3("after pmm_init");

    /* Step 2: Virtual Memory Manager */
    vmm_init();
    check_pd3("after vmm_init");

    /* Step 3: Interrupts and Drivers */
    idt_init();
    pic_init();
    pit_init(100);       // 100 Hz = 10 ms per tick
    keyboard_init();
    mouse_init();
    check_pd3("after idt/pic/pit/kbd/mouse");

    /* Step 3.5: ACPI Platform Discovery & Local APIC */
    int acpi_res = acpi_init(mbi);
    uint64_t lapic_phys = 0;
    if (acpi_res == ACPI_OK) {
        const acpi_madt_info_t *madt = acpi_get_madt_info();
        if (madt && madt->lapic_phys_addr != 0) {
            lapic_phys = madt->lapic_phys_addr;
        }
    }

    if (apic_is_supported()) {
        if (apic_init(lapic_phys) == 0) {
            apic_timer_init(100);
        }
    }
    check_pd3("after apic_init");

    /* Step 4: AegisSched_64 */
    scheduler_init();
    check_pd3("after scheduler_init");

    /* Step 4.5: Symmetric Multiprocessing (SMP AP Bootstrap) */
    smp_init();
    check_pd3("after smp_init");

    /* Step 5: Storage Discovery and PurpleFS v2 Mount */
    storage_init();
    check_pd3("after storage_init");

    /* Step 5.5: AegisDetect Boot Integrity Verification */
    aegis_detect_init(NULL, 0);
    aegis_detect_load_default_manifest();
    if (storage_is_fs_mounted()) {
        aegis_detect_verify_storage();
    }
    check_pd3("after aegis_detect");

    /* Step 5.6: PurpleDetect Hardware Discovery & Daemon */
    purple_detect_init();
    purple_detect_scan_all();
    task_create("purple_detect", purple_detect_daemon, NULL, PRIORITY_LOW);
    check_pd3("after purple_detect");

    /* Step 5.6.5: Kernel Symbols, Native Drivers & App Bundles */
    ksym_init();
    netdev_init();
    apdrv_subsystem_init();
    apapp_subsystem_init();
    check_pd3("after apdrv/apapp_init");

    /* Step 5.7: Desktop GUI Subsystem */
    desktop_init();
    check_pd3("after desktop_init");

    /* Enable Interrupts */
    sti();
    kinfo("Interrupts enabled, entering AegisShell...");
    check_pd3("after sti");

    /* Step 6: Command Shell */
    shell_run();

    /* Halt if shell ever exits */
    while (1) {
        hlt();
    }
}
