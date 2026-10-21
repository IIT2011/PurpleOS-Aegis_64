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
#include "kernel_storage.h"
#include "cpu.h"
#include "acpi.h"
#include "apic.h"
#include "smp.h"
#include "security/aegis_detect.h"
#include "purple_detect.h"
#include "pci.h"

#define SHELL_INPUT_BUFFER_LEN 256
#define MAX_ARGS 16

static char shell_buf[SHELL_INPUT_BUFFER_LEN];
static size_t shell_buf_len = 0;

static void smp_worker_func(void *arg) {
    uint32_t target_cpu = (uint32_t)(uintptr_t)arg;
    for (int i = 0; i < 3; i++) {
        uint32_t my_cpu = cpu_get_current_id();
        kinfo("SMP Worker (bound to CPU %u) executing on actual CPU %u [pass %d/3]",
              target_cpu, my_cpu, i + 1);
        task_sleep(5);
    }
    kinfo("SMP Worker for CPU %u complete", target_cpu);
}

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

static void cmd_list_files(void) {
    if (!storage_is_fs_mounted()) {
        shell_print("Error: No PurpleFS volume mounted. Check 'disk status'.\n");
        return;
    }
    pfs2_fs_t *fs = storage_get_fs();
    pfs2_inode_t root_ino;
    if (pfs2_read_inode(fs, PFS2_ROOT_INODE, &root_ino) != PFS2_OK) {
        shell_print("Error: Failed to read root directory inode.\n");
        return;
    }

    shell_print("\nDirectory Listing of / (PurpleFS v2 Root):\n");
    shell_print("  INODE   TYPE    SIZE (BYTES)  NAME\n");
    shell_print("  -----------------------------------------\n");

    uint32_t dir_block = root_ino.direct_blocks[0];
    uint8_t dir_buf[PFS2_BLOCK_SIZE];
    if (fs->dev->read_block(fs->dev, dir_block, dir_buf) != 0) {
        shell_print("Error: Failed to read directory data block.\n");
        return;
    }

    uint32_t count = 0;
    pfs2_dir_entry_t *de = (pfs2_dir_entry_t *)dir_buf;
    uint32_t max_entries = PFS2_BLOCK_SIZE / sizeof(pfs2_dir_entry_t);

    for (uint32_t i = 0; i < max_entries; i++) {
        if (de[i].inode != 0) {
            count++;
            pfs2_inode_t file_ino;
            uint64_t size = 0;
            if (pfs2_read_inode(fs, de[i].inode, &file_ino) == PFS2_OK) {
                size = file_ino.size_bytes;
            }
            kprintf("  %5u   %s   %10u   %s\n",
                    de[i].inode,
                    (de[i].file_type == 2 ? "<DIR> " : "<FILE>"),
                    (uint32_t)size,
                    de[i].name);
        }
    }
    shell_print("  -----------------------------------------\n");
    kprintf("  Total: %u file(s)\n\n", count);
}

static void cmd_peek_file(const char *name) {
    if (!name || name[0] == '\0') {
        shell_print("Usage: peek <filename>\n");
        return;
    }
    if (!storage_is_fs_mounted()) {
        shell_print("Error: No PurpleFS volume mounted.\n");
        return;
    }
    pfs2_fs_t *fs = storage_get_fs();
    uint32_t ino = 0;
    if (pfs2_lookup(fs, name, &ino) != PFS2_OK) {
        kprintf("Error: File '%s' not found.\n", name);
        return;
    }

    pfs2_inode_t file_ino;
    if (pfs2_read_inode(fs, ino, &file_ino) != PFS2_OK) {
        kprintf("Error: Failed to read inode for '%s'.\n", name);
        return;
    }

    size_t to_read = (file_ino.size_bytes > 512) ? 512 : (size_t)file_ino.size_bytes;
    char buf[513];
    memset(buf, 0, sizeof(buf));
    size_t bytes_read = 0;
    if (to_read > 0) {
        int rc = pfs2_read_file(fs, ino, buf, to_read, 0, &bytes_read);
        if (rc != PFS2_OK) {
            kprintf("Error: Read failed on file '%s' (code %d).\n", name, rc);
            return;
        }
    }
    buf[bytes_read] = '\0';

    kprintf("\n=== File: %s (%u bytes", name, (uint32_t)file_ino.size_bytes);
    if (file_ino.size_bytes > 512) {
        shell_print(", preview limited to 512 bytes");
    }
    shell_print(") ===\n");
    shell_print(buf);
    shell_print("\n=== End of File Preview ===\n\n");
}

static void cmd_touch(const char *name) {
    if (!storage_is_fs_mounted()) {
        shell_print("Error: No PurpleFS volume mounted. Check 'disk status'.\n");
        return;
    }
    if (!storage_is_writable()) {
        shell_print("Error: Filesystem mounted Read-Only. Remount with 'remount rw'.\n");
        return;
    }
    pfs2_fs_t *fs = storage_get_fs();
    uint32_t ino = 0;
    if (pfs2_lookup(fs, name, &ino) == PFS2_OK) {
        kprintf("File '%s' already exists (Inode %u).\n", name, ino);
        return;
    }
    int rc = pfs2_create_file(fs, name, 0644, &ino);
    if (rc != PFS2_OK) {
        kprintf("Error: Failed to create file '%s' (code %d).\n", name, rc);
        return;
    }
    pfs2_sync(fs);
    kprintf("Successfully created file '%s' (Inode %u).\n", name, ino);
}

static void cmd_write_file(const char *name, const char *text) {
    if (!storage_is_fs_mounted()) {
        shell_print("Error: No PurpleFS volume mounted. Check 'disk status'.\n");
        return;
    }
    if (!storage_is_writable()) {
        shell_print("Error: Filesystem mounted Read-Only. Remount with 'remount rw'.\n");
        return;
    }
    pfs2_fs_t *fs = storage_get_fs();
    uint32_t ino = 0;
    if (pfs2_lookup(fs, name, &ino) != PFS2_OK) {
        int rc = pfs2_create_file(fs, name, 0644, &ino);
        if (rc != PFS2_OK) {
            kprintf("Error: Failed to create file '%s' (code %d).\n", name, rc);
            return;
        }
    }
    size_t len = strlen(text);
    size_t written = 0;
    int rc = pfs2_write_file(fs, ino, text, len, 0, &written);
    if (rc != PFS2_OK) {
        kprintf("Error: Failed to write to file '%s' (code %d).\n", name, rc);
        return;
    }
    pfs2_sync(fs);
    kprintf("Successfully wrote %u bytes to '%s' (Inode %u).\n", (uint32_t)written, name, ino);
}

static void cmd_disk_status(void) {
    shell_print("\n=== ATA Storage Devices & GPT Partition Status ===\n");
    const char *names[4] = { "Primary Master", "Primary Slave", "Secondary Master", "Secondary Slave" };

    int found_any = 0;
    for (int i = 0; i < 4; i++) {
        ata_device_t *dev = &g_kernel_storage.devices[i];
        if (dev->is_present) {
            found_any++;
            uint32_t size_mib = (uint32_t)(dev->total_sectors * 512 / (1024 * 1024));
            kprintf("[%d] Channel: %-16s Model: %-24s Sectors: %u (%u MiB)\n",
                    i, names[i], dev->model, (uint32_t)dev->total_sectors, size_mib);

            gpt_info_t *gpt = &g_kernel_storage.gpt[i];
            if (gpt->valid_header && gpt->partition_count > 0) {
                kprintf("    GPT: Valid (Protective MBR: %s, Partitions: %u)\n",
                        gpt->valid_protective_mbr ? "Yes" : "No", gpt->partition_count);
                for (uint32_t p = 0; p < gpt->partition_count; p++) {
                    const gpt_partition_t *part = &gpt->partitions[p];
                    uint32_t part_mib = (uint32_t)(part->sector_count * 512 / (1024 * 1024));
                    kprintf("    -> Part %u: %-16s LBA: %u..%u (%u sectors, %u MiB) %s\n",
                            p, part->name, (uint32_t)part->start_lba, (uint32_t)part->end_lba,
                            (uint32_t)part->sector_count, part_mib,
                            part->is_purplefs ? "[PurpleFS v2]" : "");
                }
            } else {
                shell_print("    GPT: None / Unpartitioned\n");
            }
        }
    }
    if (!found_any) {
        shell_print("No ATA storage devices detected.\n");
    }
    shell_print("\n");
}

static void cmd_filesystem_status(void) {
    shell_print("\n=== PurpleFS v2 Filesystem Status ===\n");
    if (!storage_is_fs_mounted()) {
        shell_print("  Mount State:   UNMOUNTED (No valid PurpleFS volume)\n\n");
        return;
    }
    pfs2_fs_t *fs = storage_get_fs();
    uint32_t total_kib = fs->sb.total_blocks * (fs->sb.block_size / 1024);
    uint32_t free_kib  = fs->sb.free_blocks * (fs->sb.block_size / 1024);
    uint32_t used_kib  = total_kib - free_kib;

    kprintf("  Mount State:   MOUNTED (%s)\n",
            storage_is_writable() ? "Read-Write Kernel Mode" : "Read-Only Kernel Mode");
    kprintf("  Volume Label:  %s\n", fs->sb.volume_label);
    kprintf("  Block Size:    %u bytes\n", fs->sb.block_size);
    kprintf("  Total Blocks:  %u (%u KiB / %u MiB)\n", fs->sb.total_blocks, total_kib, total_kib / 1024);
    kprintf("  Used Blocks:   %u (%u KiB / %u MiB)\n", fs->sb.total_blocks - fs->sb.free_blocks, used_kib, used_kib / 1024);
    kprintf("  Free Blocks:   %u (%u KiB / %u MiB)\n", fs->sb.free_blocks, free_kib, free_kib / 1024);
    kprintf("  Total Inodes:  %u\n", fs->sb.total_inodes);
    kprintf("  Free Inodes:   %u\n", fs->sb.free_inodes);
    kprintf("  Journal Start: Block %u (8 blocks reserved)\n\n", fs->sb.journal_start_block);
}

static void execute_command(int argc, char **argv) {
    if (argc == 0) return;

    if (strcmp(argv[0], "help") == 0) {
        shell_print("\n=== AegisShell / Purple Shell Commands ===\n");
        shell_print("  help              - Show available shell commands\n");
        shell_print("  sysinfo           - Display OS version and architectural parameters\n");
        shell_print("  cpuinfo           - Display CPU long-mode features and topology\n");
        shell_print("  smp               - Display multi-core topology, online cores, and ticks\n");
        shell_print("  smptest           - Run concurrent multi-core task affinity benchmark\n");
        shell_print("  acpi              - Display detected ACPI tables and MADT configuration\n");
        shell_print("  apic              - Report Local APIC ID, version, and timer status\n");
        shell_print("  mem               - Display physical and virtual memory statistics\n");
        shell_print("  uptime            - Display system uptime and PIT timer ticks\n");
        shell_print("  tasks / ps        - Display task table and AegisSched_64 states\n");
        shell_print("  task spawn <n>    - Spawn a test background task with given name\n");
        shell_print("  task kill <id>    - Terminate a running task by ID\n");
        shell_print("  task test         - Run automated multi-task preemption test\n");
        shell_print("  list files / ls   - List files in root directory of mounted PurpleFS\n");
        shell_print("  peek <name> / cat - Display bounded file content preview\n");
        shell_print("  touch <name>      - Create empty file on mounted writable PurpleFS\n");
        shell_print("  write <name> <txt>- Write text to file on mounted writable PurpleFS\n");
        shell_print("  remount <ro|rw>   - Toggle PurpleFS volume read-only or read-write\n");
        shell_print("  sync              - Commit journal transactions and flush cache to disk\n");
        shell_print("  disk status       - Report detected ATA storage and GPT partitions\n");
        shell_print("  filesystem status - Report PurpleFS mount state and space metrics\n");
        shell_print("  devices           - Report hardware devices discovered by PurpleDetect\n");
        shell_print("  lspci             - Display PCI bus device inventory\n");
        shell_print("  aegisdetect [verify] - Report or run AegisDetect integrity verification\n");
        shell_print("  clear             - Clear terminal screen\n");
        shell_print("  echo <text>       - Display text arguments\n");
        shell_print("  reboot/restart    - Perform ACPI/controller system reboot\n");
        shell_print("  panic             - Trigger intentional kernel panic for diagnostics\n\n");
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
        shell_print("  Protection: CR0.WP enforced, EFER.NXE enabled (W^X)\n");
        const cpu_features_t *f = cpu_get_features();
        kprintf("  Processor:  %s (%s)\n", f->brand, f->vendor);
        kprintf("  Topology:   Family=%u Model=%u Stepping=%u Active CPUs=%u\n",
                f->family, f->model, f->stepping, cpu_count());
        kprintf("  Features:   [APIC=%u x2APIC=%u SSE=%u SSE2=%u SSE4.1=%u AVX=%u NX=%u InvTSC=%u]\n\n",
                (f->flags & CPU_FEAT_APIC) ? 1 : 0,
                (f->flags & CPU_FEAT_x2APIC) ? 1 : 0,
                (f->flags & CPU_FEAT_SSE) ? 1 : 0,
                (f->flags & CPU_FEAT_SSE2) ? 1 : 0,
                (f->flags & CPU_FEAT_SSE4_1) ? 1 : 0,
                (f->flags & CPU_FEAT_AVX) ? 1 : 0,
                (f->flags & CPU_FEAT_NX) ? 1 : 0,
                (f->flags & CPU_FEAT_INVARIANT_TSC) ? 1 : 0);
    } else if (strcmp(argv[0], "acpi") == 0) {
        uint32_t count = acpi_get_table_count();
        kprintf("\n=== ACPI System Description Tables (%u detected) ===\n", count);
        for (uint32_t i = 0; i < count; i++) {
            const acpi_sdt_header_t *h = acpi_get_table(i);
            kprintf("  [%2u] Table '%.4s' Rev=%u Length=%-5u OEM='%.6s' TableID='%.8s'\n",
                    i, h->signature, h->revision, h->length, h->oem_id, h->oem_table_id);
        }
        const acpi_madt_info_t *madt = acpi_get_madt_info();
        if (madt) {
            kprintf("\n=== ACPI MADT (Multiple APIC Description Table) ===\n");
            kprintf("  Local APIC Address: %p\n", (void *)madt->lapic_phys_addr);
            kprintf("  Dual-PIC Controller: %s\n", madt->has_dual_pic ? "Yes (8259 PC-AT Compatible)" : "No");
            kprintf("  Processor Count:    %u\n", madt->cpu_count);
            for (uint32_t i = 0; i < madt->cpu_count; i++) {
                kprintf("    -> CPU %u: APIC ID=%u, ACPI ID=%u, Flags=0x%x\n",
                        i, madt->cpus[i].apic_id, madt->cpus[i].acpi_id, madt->cpus[i].flags);
            }
            kprintf("  I/O APIC Count:     %u\n", madt->ioapic_count);
            for (uint32_t i = 0; i < madt->ioapic_count; i++) {
                kprintf("    -> IOAPIC %u: ID=%u, Base=%p, GSI Base=%u\n",
                        i, madt->ioapics[i].id, (void *)madt->ioapics[i].phys_addr, madt->ioapics[i].gsi_base);
            }
            kprintf("  Interrupt Overrides: %u\n", madt->iso_count);
            for (uint32_t i = 0; i < madt->iso_count; i++) {
                kprintf("    -> ISO %u: Bus=%u (ISA) IRQ %u -> GSI %u (Flags=0x%x)\n",
                        i, madt->isos[i].bus, madt->isos[i].source_irq, madt->isos[i].gsi, madt->isos[i].flags);
            }
        }
        shell_print("\n");
    } else if (strcmp(argv[0], "apic") == 0) {
        if (!apic_is_enabled()) {
            shell_print("\nLocal APIC is not enabled (running on legacy PIC).\n\n");
        } else {
            kprintf("\n=== Local APIC (Advanced Programmable Interrupt Controller) ===\n");
            kprintf("  Status:     ENABLED (Software APIC Active)\n");
            kprintf("  APIC ID:    %u\n", apic_get_id());
            kprintf("  Version:    0x%x\n", apic_get_version());
            kprintf("  Timer HZ:   %u Hz\n\n", apic_get_timer_freq());
        }
    } else if (strcmp(argv[0], "smp") == 0) {
        uint32_t total = cpu_count();
        uint32_t online = cpu_online_count();
        uint32_t cur = cpu_get_current_id();

        kprintf("\n=== Symmetric Multiprocessing (SMP) Subsystem ===\n");
        kprintf("  Topology:        %u discovered cores, %u online cores\n", total, online);
        kprintf("  Current CPU:     CPU #%u (APIC ID %u)\n", cur, apic_get_id());
        kprintf("  -------------------------------------------------------------------------\n");
        kprintf("  CPU#  ROLE  STATUS   APIC_ID  ACPI_ID  STACK_TOP           TICKS      IPIS\n");
        kprintf("  -------------------------------------------------------------------------\n");
        for (uint32_t i = 0; i < total; i++) {
            cpu_t *c = cpu_get(i);
            if (c) {
                kprintf("  %-4u  %-4s  %-7s  %-7u  %-7u  0x%-16x  %-9u  %-5u\n",
                        c->id,
                        c->is_bsp ? "BSP" : "AP",
                        c->is_online ? "ONLINE" : "OFFLINE",
                        c->apic_id, c->acpi_id,
                        c->kernel_stack,
                        (uint32_t)c->lapic_timer_ticks,
                        c->ipi_count);
            }
        }
        kprintf("  -------------------------------------------------------------------------\n\n");
    } else if (strcmp(argv[0], "smptest") == 0) {
        uint32_t online = cpu_online_count();
        shell_print("Spawning SMP multi-core concurrent test tasks...\n");
        for (uint32_t i = 0; i < online; i++) {
            char name[16];
            name[0] = 's'; name[1] = 'm'; name[2] = 'p'; name[3] = '_';
            name[4] = 'c'; name[5] = 'p'; name[6] = 'u'; name[7] = '0' + (char)i;
            name[8] = '\0';
            task_t *t = task_create_affined(name, smp_worker_func, (void *)(uintptr_t)i, PRIORITY_NORMAL, 1ULL << i);
            if (t) {
                kprintf("  Spawned '%s' (Task ID %u) bound to CPU #%u\n", name, t->id, i);
            }
        }
        shell_print("All SMP test tasks queued with strict core affinity.\n\n");
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
    } else if ((strcmp(argv[0], "list") == 0 && argc >= 2 && strcmp(argv[1], "files") == 0) ||
               strcmp(argv[0], "ls") == 0) {
        cmd_list_files();
    } else if (strcmp(argv[0], "peek") == 0 || strcmp(argv[0], "cat") == 0) {
        if (argc < 2) {
            shell_print("Usage: peek <filename>\n");
        } else {
            cmd_peek_file(argv[1]);
        }
    } else if (strcmp(argv[0], "touch") == 0) {
        if (argc < 2) {
            shell_print("Usage: touch <filename>\n");
        } else {
            cmd_touch(argv[1]);
        }
    } else if (strcmp(argv[0], "write") == 0) {
        if (argc < 3) {
            shell_print("Usage: write <filename> <text...>\n");
        } else {
            char text_buf[256];
            text_buf[0] = '\0';
            size_t off = 0;
            for (int i = 2; i < argc && off < sizeof(text_buf) - 2; i++) {
                size_t wlen = strlen(argv[i]);
                if (off + wlen + 2 < sizeof(text_buf)) {
                    memcpy(text_buf + off, argv[i], wlen);
                    off += wlen;
                    if (i + 1 < argc) text_buf[off++] = ' ';
                }
            }
            text_buf[off++] = '\n';
            text_buf[off] = '\0';
            cmd_write_file(argv[1], text_buf);
        }
    } else if (strcmp(argv[0], "remount") == 0) {
        if (argc < 2) {
            shell_print("Usage: remount <ro|rw>\n");
        } else if (strcmp(argv[1], "rw") == 0) {
            if (storage_set_writable(1) == 0) {
                shell_print("PurpleFS volume remounted READ-WRITE.\n");
            } else {
                shell_print("Error: No mounted filesystem to remount.\n");
            }
        } else if (strcmp(argv[1], "ro") == 0) {
            if (storage_set_writable(0) == 0) {
                shell_print("PurpleFS volume remounted READ-ONLY.\n");
            } else {
                shell_print("Error: No mounted filesystem to remount.\n");
            }
        } else {
            shell_print("Usage: remount <ro|rw>\n");
        }
    } else if (strcmp(argv[0], "sync") == 0) {
        if (storage_is_fs_mounted()) {
            pfs2_sync(storage_get_fs());
            shell_print("Filesystem transactions committed and cache flushed to disk.\n");
        } else {
            shell_print("No mounted filesystem to sync.\n");
        }
    } else if ((strcmp(argv[0], "disk") == 0 && argc >= 2 && strcmp(argv[1], "status") == 0) ||
               strcmp(argv[0], "disks") == 0) {
        cmd_disk_status();
    } else if ((strcmp(argv[0], "filesystem") == 0 && argc >= 2 && strcmp(argv[1], "status") == 0) ||
               strcmp(argv[0], "df") == 0) {
        cmd_filesystem_status();
    } else if (strcmp(argv[0], "aegisdetect") == 0 || strcmp(argv[0], "integrity") == 0) {
        if (argc >= 2 && strcmp(argv[1], "verify") == 0) {
            shell_print("Running AegisDetect filesystem integrity verification...\n");
            aegis_detect_verify_storage();
        }
        aegis_detect_print_status();
    } else if (strcmp(argv[0], "devices") == 0 || strcmp(argv[0], "purpledetect") == 0) {
        purple_detect_print_table();
    } else if (strcmp(argv[0], "lspci") == 0 || strcmp(argv[0], "pci") == 0) {
        pci_print_devices();
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
