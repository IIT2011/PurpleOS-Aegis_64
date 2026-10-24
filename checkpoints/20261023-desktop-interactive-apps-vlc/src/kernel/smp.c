#include "smp.h"
#include "cpu.h"
#include "apic.h"
#include "idt.h"
#include "pit.h"
#include "pmm.h"
#include "vmm.h"
#include "task.h"
#include "string.h"
#include "serial.h"
#include "io.h"
#include "spinlock.h"

extern uint8_t smp_trampoline_start[];
extern uint8_t smp_trampoline_end[];

static void ipi_resched_handler(interrupt_frame_t *frame) {
    (void)frame;
    uint32_t cpu_id = cpu_get_current_id();
    cpu_t *c = cpu_get(cpu_id);
    if (c) c->ipi_count++;
    schedule();
}

static void ipi_halt_handler(interrupt_frame_t *frame) {
    (void)frame;
    cli();
    while (1) {
        hlt();
    }
}

void smp_send_resched(uint32_t cpu_id) {
    cpu_t *c = cpu_get(cpu_id);
    if (c && c->is_online && !c->is_bsp) {
        apic_send_ipi(c->apic_id, ICR_DELIV_FIXED | ICR_TRIGGER_EDGE | ICR_LEVEL_ASSERT | ICR_DEST_PHYSICAL | LAPIC_IPI_RESCHED);
    }
}

void smp_broadcast_resched(void) {
    if (cpu_count() > 1) {
        apic_send_ipi(0, ICR_DELIV_FIXED | ICR_TRIGGER_EDGE | ICR_LEVEL_ASSERT | ICR_DEST_ALL_EXC_SELF | LAPIC_IPI_RESCHED);
    }
}

void smp_broadcast_halt(void) {
    if (cpu_count() > 1) {
        apic_send_ipi(0, ICR_DELIV_FIXED | ICR_TRIGGER_EDGE | ICR_LEVEL_ASSERT | ICR_DEST_ALL_EXC_SELF | LAPIC_IPI_HALT);
    }
}

void smp_ap_entry(void) {
    /* 1. Load kernel IDT */
    idt_load_current();

    /* 2. Initialize Local APIC on this core */
    apic_init_ap();

    /* 3. Determine logical core ID from APIC ID */
    uint32_t cur_apic = apic_get_id();
    uint32_t cpu_id = 0;
    for (uint32_t i = 0; i < cpu_count(); i++) {
        cpu_t *c = cpu_get(i);
        if (c && c->apic_id == cur_apic) {
            cpu_id = i;
            break;
        }
    }

    cpu_t *cur_cpu = cpu_get(cpu_id);
    if (cur_cpu) {
        cur_cpu->is_online = 1;
    }

    /* 4. Initialize per-CPU scheduler structures */
    scheduler_init_ap(cpu_id);

    kinfo("SMP: AP core #%u (APIC ID %u) started and operational", cpu_id, cur_apic);

    /* 5. Enable interrupts on this core */
    sti();

    /* 6. Enter schedule / idle loop */
    while (1) {
        hlt();
    }
}

void smp_init(void) {
    uint32_t total_cpus = cpu_count();
    if (total_cpus <= 1) {
        kinfo("SMP: Uniprocessor system detected (%u core)", total_cpus);
        return;
    }

    kinfo("SMP: Initializing Symmetric Multiprocessing for %u discovered cores...", total_cpus);

    /* 1. Register IPI interrupt handlers */
    register_interrupt_handler(LAPIC_IPI_RESCHED, ipi_resched_handler);
    register_interrupt_handler(LAPIC_IPI_HALT, ipi_halt_handler);

    /* 2. Copy trampoline code to 0x8000 */
    size_t trampoline_size = (size_t)(smp_trampoline_end - smp_trampoline_start);
    if (trampoline_size > 4096) {
        kerror("SMP: Trampoline size (%u bytes) exceeds 4 KiB limit!", (uint32_t)trampoline_size);
        return;
    }
    memcpy((void *)SMP_TRAMPOLINE_PHYS_ADDR, smp_trampoline_start, trampoline_size);

    /* 3. Boot each registered Application Processor (AP) */
    smp_trampoline_data_t *data = (smp_trampoline_data_t *)SMP_TRAMPOLINE_PHYS_ADDR;

    for (uint32_t i = 1; i < total_cpus; i++) {
        cpu_t *ap = cpu_get(i);
        if (!ap) continue;

        /* Allocate 16 KiB dedicated kernel stack for this AP */
        void *stack = pmm_alloc_frames(4);
        if (!stack) {
            kerror("SMP: Failed to allocate stack for CPU #%u (APIC ID %u)", i, ap->apic_id);
            continue;
        }
        uint64_t stack_top = (uint64_t)(uintptr_t)stack + (4 * PMM_PAGE_SIZE);
        ap->kernel_stack = stack_top;

        /* Set up trampoline data block */
        data->cr3 = read_cr3();
        data->rsp = stack_top;
        data->entry = (uint64_t)(uintptr_t)smp_ap_entry;
        data->ready = 0;
        data->bsp_ack = 0;

        /* Send INIT IPI */
        apic_send_ipi(ap->apic_id, ICR_DELIV_INIT | ICR_TRIGGER_EDGE | ICR_LEVEL_ASSERT | ICR_DEST_PHYSICAL);

        /* Wait 10 ms */
        pit_delay_10ms();

        /* Send Startup IPI (SIPI) with vector 0x08 (0x8000 >> 12) */
        apic_send_ipi(ap->apic_id, ICR_DELIV_STARTUP | ICR_TRIGGER_EDGE | ICR_LEVEL_ASSERT | ICR_DEST_PHYSICAL | SMP_SIPI_VECTOR);

        /* Wait 200 µs */
        pit_delay_us(200);

        /* Send second SIPI if not yet ready */
        if (!data->ready) {
            apic_send_ipi(ap->apic_id, ICR_DELIV_STARTUP | ICR_TRIGGER_EDGE | ICR_LEVEL_ASSERT | ICR_DEST_PHYSICAL | SMP_SIPI_VECTOR);
        }

        /* Wait up to 50 ms for AP to signal ready */
        uint32_t timeout_ms = 50;
        while (!data->ready && timeout_ms > 0) {
            pit_delay_us(1000);
            timeout_ms--;
        }

        if (data->ready) {
            /* Acknowledge and release AP into smp_ap_entry */
            data->bsp_ack = 1;

            /* Wait up to 50 ms for AP to transition to online */
            timeout_ms = 50;
            while (!ap->is_online && timeout_ms > 0) {
                pit_delay_us(1000);
                timeout_ms--;
            }

            if (ap->is_online) {
                kinfo("SMP: AP core #%u (APIC ID %u) successfully brought online", i, ap->apic_id);
            } else {
                kwarn("SMP: AP core #%u (APIC ID %u) booted long mode but failed online handshake", i, ap->apic_id);
            }
        } else {
            kerror("SMP: AP core #%u (APIC ID %u) timed out waiting for SIPI response", i, ap->apic_id);
        }
    }

    kinfo("SMP: Multiprocessing operational. %u/%u CPUs online.", cpu_online_count(), total_cpus);
}

uint32_t smp_get_online_count(void) {
    return cpu_online_count();
}
