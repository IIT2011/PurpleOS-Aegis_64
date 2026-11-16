use crate::arch::x86_64::cpu::{read_cr3, sti, hlt};
use crate::drivers::apic::{send_ipi, ICR_DELIV_FIXED, ICR_DELIV_INIT, ICR_DELIV_STARTUP, ICR_DEST_ALL_EXC_SELF, ICR_DEST_PHYSICAL, ICR_LEVEL_ASSERT, ICR_TRIGGER_EDGE};
use crate::kernel::acpi::get_madt_info;
use crate::mm::pmm::alloc_frames;
use core::sync::atomic::{AtomicU32, Ordering};

pub const SMP_TRAMPOLINE_PHYS: usize = 0x8000;
pub const LAPIC_IPI_RESCHED: u8 = 0xFD;
pub const LAPIC_IPI_HALT: u8 = 0xFE;

static ONLINE_CPUS: AtomicU32 = AtomicU32::new(1); // BSP starts online

#[repr(C, packed)]
struct TrampolineData {
    jmp_code: [u8; 8],
    cr3: u64,
    rsp: u64,
    entry: u64,
    ready: u64,
    bsp_ack: u64,
}

extern "C" {
    static smp_trampoline_start: u8;
    static smp_trampoline_end: u8;
}

#[unsafe(no_mangle)]
pub extern "C" fn smp_ap_entry() -> ! {
    crate::arch::x86_64::gdt::init_ap();
    crate::arch::x86_64::idt::load_idt();
    crate::kernel::syscall::init_ap();
    crate::drivers::apic::init_ap();

    let apic_id = crate::drivers::apic::get_id();
    let cpu_id = ONLINE_CPUS.fetch_add(1, Ordering::SeqCst);

    crate::drivers::serial::print("SMP: AP core #");
    crate::drivers::serial::print_u64(cpu_id as u64);
    crate::drivers::serial::print(" (APIC ID ");
    crate::drivers::serial::print_u64(apic_id as u64);
    crate::drivers::serial::println(") started and operational");

    crate::kernel::task::init_ap(cpu_id as usize);

    sti();

    loop {
        hlt();
    }
}

pub fn init() {
    let madt = get_madt_info();
    let total_cpus = madt.cpu_count;

    if total_cpus <= 1 {
        crate::drivers::serial::println("SMP: Uniprocessor system detected (1 core)");
        return;
    }

    crate::drivers::serial::print("SMP: Initializing Symmetric Multiprocessing for ");
    crate::drivers::serial::print_u64(total_cpus as u64);
    crate::drivers::serial::println(" discovered cores...");

    unsafe {
        // Copy trampoline code to physical 0x8000
        let start_ptr = &raw const smp_trampoline_start;
        let end_ptr = &raw const smp_trampoline_end;
        let size = (end_ptr as usize) - (start_ptr as usize);

        if size > 4096 {
            crate::drivers::serial::println("SMP: Trampoline size exceeds 4 KiB!");
            return;
        }

        core::ptr::copy_nonoverlapping(start_ptr, SMP_TRAMPOLINE_PHYS as *mut u8, size);

        let data = SMP_TRAMPOLINE_PHYS as *mut TrampolineData;

        for i in 1..total_cpus {
            let apic_id = madt.apic_ids[i] as u32;

            // Allocate 16 KiB dedicated kernel stack for AP
            let stack_phys = alloc_frames(4).expect("Failed to alloc AP stack");
            let stack_top = (stack_phys + 16384) as u64;

            (*data).cr3 = read_cr3();
            (*data).rsp = stack_top;
            (*data).entry = smp_ap_entry as usize as u64;
            (*data).ready = 0;
            (*data).bsp_ack = 0;

            // Send INIT IPI
            send_ipi(apic_id, ICR_DELIV_INIT | ICR_TRIGGER_EDGE | ICR_LEVEL_ASSERT | ICR_DEST_PHYSICAL);
            crate::drivers::pit::delay_ms(10);

            // Send Startup IPI (vector 0x08 for 0x8000)
            send_ipi(apic_id, ICR_DELIV_STARTUP | ICR_TRIGGER_EDGE | ICR_LEVEL_ASSERT | ICR_DEST_PHYSICAL | 0x08);
            crate::drivers::pit::delay_ms(1);

            if (*data).ready == 0 {
                send_ipi(apic_id, ICR_DELIV_STARTUP | ICR_TRIGGER_EDGE | ICR_LEVEL_ASSERT | ICR_DEST_PHYSICAL | 0x08);
                crate::drivers::pit::delay_ms(1);
            }

            // Wait for AP ready
            let mut timeout = 50;
            while (*data).ready == 0 && timeout > 0 {
                crate::drivers::pit::delay_ms(1);
                timeout -= 1;
            }

            if (*data).ready != 0 {
                (*data).bsp_ack = 1;
                crate::drivers::serial::print("SMP: AP core #");
                crate::drivers::serial::print_u64(i as u64);
                crate::drivers::serial::print(" (APIC ID ");
                crate::drivers::serial::print_u64(apic_id as u64);
                crate::drivers::serial::println(") successfully brought online");
            } else {
                crate::drivers::serial::print("SMP: AP core #");
                crate::drivers::serial::print_u64(i as u64);
                crate::drivers::serial::println(" timed out waiting for SIPI response");
            }
        }
    }

    crate::drivers::serial::print("SMP: Multiprocessing operational. ");
    crate::drivers::serial::print_u64(ONLINE_CPUS.load(Ordering::SeqCst) as u64);
    crate::drivers::serial::print("/");
    crate::drivers::serial::print_u64(total_cpus as u64);
    crate::drivers::serial::println(" CPUs online.");
}

pub fn get_online_count() -> u32 {
    ONLINE_CPUS.load(Ordering::Relaxed)
}

pub fn broadcast_resched() {
    if get_online_count() > 1 {
        send_ipi(0, ICR_DELIV_FIXED | ICR_TRIGGER_EDGE | ICR_LEVEL_ASSERT | ICR_DEST_ALL_EXC_SELF | (LAPIC_IPI_RESCHED as u32));
    }
}
