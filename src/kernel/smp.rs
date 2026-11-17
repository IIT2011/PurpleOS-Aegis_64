use crate::arch::x86_64::cpu::{read_cr3, sti, hlt};
use crate::drivers::apic::{send_ipi, ICR_DELIV_FIXED, ICR_DELIV_INIT, ICR_DELIV_STARTUP, ICR_DEST_ALL_EXC_SELF, ICR_DEST_PHYSICAL, ICR_LEVEL_ASSERT, ICR_TRIGGER_EDGE};
use crate::kernel::acpi::get_madt_info;
use crate::mm::pmm::alloc_frames;
use core::sync::atomic::{AtomicU32, Ordering};

pub const SMP_TRAMPOLINE_PHYS: usize = 0x8000;
pub const LAPIC_IPI_RESCHED: u8 = 0xFD;
pub const LAPIC_IPI_HALT: u8 = 0xFE;

static ONLINE_CPUS: AtomicU32 = AtomicU32::new(1); // BSP starts online
static mut APIC_TO_CPU_MAP: [u8; 256] = [0xFF; 256];

#[inline(always)]
pub fn get_cpu_id() -> usize {
    let apic_id = (crate::drivers::apic::get_id() & 0xFF) as usize;
    unsafe {
        let cpu = APIC_TO_CPU_MAP[apic_id];
        if cpu != 0xFF {
            cpu as usize
        } else {
            0
        }
    }
}

#[inline(always)]
pub fn get_current_cpu() -> usize {
    get_cpu_id()
}

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

fn print_core_msg(prefix: &str, cpu_id: u32, apic_id: u32, suffix: &str) {
    let mut buf = [0u8; 96];
    let mut len = 0;
    let p_bytes = prefix.as_bytes();
    buf[len..len + p_bytes.len()].copy_from_slice(p_bytes);
    len += p_bytes.len();
    if cpu_id >= 10 {
        buf[len] = b'0' + ((cpu_id / 10) as u8);
        len += 1;
    }
    buf[len] = b'0' + ((cpu_id % 10) as u8);
    len += 1;
    let mid = b" (APIC ID ";
    buf[len..len + mid.len()].copy_from_slice(mid);
    len += mid.len();
    if apic_id >= 10 {
        buf[len] = b'0' + ((apic_id / 10) as u8);
        len += 1;
    }
    buf[len] = b'0' + ((apic_id % 10) as u8);
    len += 1;
    let mid2 = b") ";
    buf[len..len + mid2.len()].copy_from_slice(mid2);
    len += mid2.len();
    let s_bytes = suffix.as_bytes();
    buf[len..len + s_bytes.len()].copy_from_slice(s_bytes);
    len += s_bytes.len();
    if let Ok(s) = core::str::from_utf8(&buf[..len]) {
        crate::drivers::serial::println(s);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn smp_ap_entry() -> ! {
    let apic_id = crate::drivers::apic::get_id();
    let cpu_id = get_cpu_id();
    let _ = ONLINE_CPUS.fetch_add(1, Ordering::SeqCst);

    crate::arch::x86_64::gdt::init_ap(cpu_id);
    crate::arch::x86_64::idt::load_idt();
    crate::kernel::syscall::init_ap(cpu_id);
    crate::drivers::apic::init_ap();

    print_core_msg("SMP: AP core #", cpu_id as u32, apic_id, "started and operational");

    crate::kernel::task::init_ap(cpu_id);

    sti();

    loop {
        hlt();
    }
}

pub fn init() {
    let madt = get_madt_info();
    let total_cpus = madt.cpu_count;

    unsafe {
        for (idx, &apic) in madt.apic_ids[..total_cpus.min(8)].iter().enumerate() {
            APIC_TO_CPU_MAP[apic as usize] = idx as u8;
        }
    }

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
            (*data).entry = smp_ap_entry as *const () as usize as u64;
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
                // Wait for AP core to increment ONLINE_CPUS in smp_ap_entry
                let expected = (i + 1) as u32;
                let mut ap_wait = 100;
                while ONLINE_CPUS.load(Ordering::SeqCst) < expected && ap_wait > 0 {
                    crate::drivers::pit::delay_ms(1);
                    ap_wait -= 1;
                }

                print_core_msg("SMP: AP core #", i as u32, apic_id as u32, "successfully brought online");
            } else {
                crate::drivers::serial::print("SMP: AP core #");
                crate::drivers::serial::print_u64(i as u64);
                crate::drivers::serial::println(" timed out waiting for SIPI response");
            }
        }
    }

    let online = ONLINE_CPUS.load(Ordering::SeqCst) as u64;
    let total = total_cpus as u64;
    let mut msg = [0u8; 64];
    let prefix = b"SMP: Multiprocessing operational. ";
    msg[..prefix.len()].copy_from_slice(prefix);
    let mut idx = prefix.len();
    msg[idx] = b'0' + (online as u8);
    idx += 1;
    msg[idx] = b'/';
    idx += 1;
    msg[idx] = b'0' + (total as u8);
    idx += 1;
    let suffix = b" CPUs online.";
    msg[idx..idx + suffix.len()].copy_from_slice(suffix);
    idx += suffix.len();
    if let Ok(s) = core::str::from_utf8(&msg[..idx]) {
        crate::drivers::serial::println(s);
    }
}

pub fn get_online_count() -> u32 {
    ONLINE_CPUS.load(Ordering::Relaxed)
}

pub fn broadcast_resched() {
    if get_online_count() > 1 {
        send_ipi(0, ICR_DELIV_FIXED | ICR_TRIGGER_EDGE | ICR_LEVEL_ASSERT | ICR_DEST_ALL_EXC_SELF | (LAPIC_IPI_RESCHED as u32));
    }
}
