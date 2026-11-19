#![no_std]
#![no_main]

extern crate alloc;

use core::arch::global_asm;
use core::panic::PanicInfo;

// Link early boot & low-level assembly stubs directly into the binary
global_asm!(include_str!("boot.s"), options(att_syntax));
global_asm!(include_str!("arch/x86_64/interrupts.s"), options(att_syntax));
global_asm!(include_str!("kernel/switch.s"), options(att_syntax));
global_asm!(include_str!("kernel/syscall_entry.s"), options(att_syntax));
global_asm!(include_str!("kernel/smp_trampoline.s"), options(att_syntax));

pub mod sync;
pub mod arch;
pub mod mm;
pub mod drivers;
pub mod fs;
pub mod crypto;
pub mod security;
pub mod kernel;
pub mod net;
pub mod compositor;

extern "C" {
    static _kernel_start: u8;
    static _kernel_end: u8;
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    drivers::serial::println("=================================================");
    drivers::serial::println("!!! KERNEL PANIC (Pure Rust Bare-Metal Crash) !!!");
    drivers::serial::println("=================================================");
    if let Some(loc) = info.location() {
        drivers::serial::print("Location: ");
        drivers::serial::print(loc.file());
        drivers::serial::print(":");
        drivers::serial::print_u64(loc.line() as u64);
        drivers::serial::println("");
    }
    kprintln!("Message: {}", info.message());
    loop {
        arch::x86_64::cpu::cli();
        arch::x86_64::cpu::hlt();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start_rust(mbi_addr: u32, magic: u32) -> ! {
    // 1. Initialize COM1 UART serial logging
    drivers::serial::init();
    security::aegis_detect::init();

    drivers::serial::println("=================================================");
    drivers::serial::println("Aegis64 Kernel (Purple OS) Initializing (Pure Rust Edition)");
    drivers::serial::println("=================================================");

    let kstart = &raw const _kernel_start as usize;
    let kend = &raw const _kernel_end as usize;
    drivers::serial::print("Kernel binary loaded: 0x");
    drivers::serial::print_hex(kstart as u64);
    drivers::serial::print(" to 0x");
    drivers::serial::print_hex(kend as u64);
    drivers::serial::println("");

    use arch::x86_64::boot::{AEGIS_UEFI_BOOT_MAGIC, MULTIBOOT_BOOTLOADER_MAGIC};
    let mbi = if matches!(magic, MULTIBOOT_BOOTLOADER_MAGIC | AEGIS_UEFI_BOOT_MAGIC) && mbi_addr != 0 {
        drivers::serial::println(if magic == AEGIS_UEFI_BOOT_MAGIC {
            "AegisX64 native UEFI bootloader detected"
        } else {
            "Multiboot 1 bootloader detected"
        });
        Some(unsafe { &*(mbi_addr as *const arch::x86_64::boot::MultibootInfo) })
    } else {
        None
    };

    // 2. Discover CPU features via CPUID
    arch::x86_64::cpu::init_bsp();

    // 3. Physical Memory Manager (PMM)
    mm::pmm::init(mbi, kstart, kend);

    // 4. Virtual Memory Manager (VMM) & CR0.WP
    mm::vmm::init();

    // 4b. Dynamic Kernel Heap Allocator (GlobalAlloc)
    mm::heap::init();

    // 5. GDT, TSS and IDT
    arch::x86_64::gdt::init();
    arch::x86_64::idt::init();

    // 6. Fast Hardware Syscall ABI (MSRs)
    kernel::syscall::init();

    // 7. Core Hardware Timers & Controllers
    arch::x86_64::pic::init();
    drivers::pit::init(100); // 100 Hz = 10 ms ticks
    drivers::keyboard::init();
    drivers::mouse::init();
    drivers::rtc::init();

    // 8. ACPI & Local APIC
    let _ = kernel::acpi::init(mbi);
    let madt = kernel::acpi::get_madt_info();
    let _ = drivers::apic::init(madt.lapic_phys_addr);

    // 9. AegisSched_64 Scheduler
    kernel::task::init();

    // 10. Symmetric Multiprocessing (SMP AP Cores)
    kernel::smp::init();

    // 11. PCI Bus Enumeration
    drivers::pci::init();

    // 12. Storage Subsystem & PurpleFS Auto-Mount
    kernel::storage::init();
    fs::fs_registry::init();

    // 13. Security & Integrity Subsystems
    security::aegis_detect::enforce_boot_policy();
    kernel::app_runtime::init();

    // 14. Peripheral Discovery Daemon
    drivers::purple_detect::init();
    drivers::purple_detect::scan_all();

    // 15. Dynamic Linker, Symbols & Drivers
    kernel::ksym::init();
    drivers::netdev::init();
    net::init();
    drivers::usb::init();
    drivers::e1000::init();
    drivers::rtl8139::init();
    drivers::audio::init();
    if !mbi.is_some_and(drivers::video::has_boot_framebuffer) {
        drivers::gpu::init();
    }
    drivers::gpu_accel::init();
    kernel::apdrv::init();
    kernel::apexec::init();

    // 16. Linear Framebuffer Driver Initialization
    drivers::video::init(mbi);

    // 16b. Direct Boot to Native Compositor (No CUI on display)
    compositor::init();
    compositor::start();

    // 17. Enable Hardware Interrupts
    arch::x86_64::cpu::sti();
    drivers::serial::println("Interrupts enabled, entering AegisShell 2.0 (Rust Edition)...");

    // 18. Launch Interactive Command Shell
    kernel::shell::get_shell().run();
}
