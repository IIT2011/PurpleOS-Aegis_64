use crate::sync::Spinlock;

#[derive(Copy, Clone)]
pub struct Symbol {
    pub name: &'static str,
    pub addr: u64,
}

static SYMBOLS: Spinlock<[Symbol; 10]> = Spinlock::new([
    Symbol { name: "kprintf", addr: 0 },
    Symbol { name: "serial_write", addr: 0 },
    Symbol { name: "pmm_alloc_frame", addr: 0 },
    Symbol { name: "pmm_free_frame", addr: 0 },
    Symbol { name: "vmm_map_page", addr: 0 },
    Symbol { name: "task_create", addr: 0 },
    Symbol { name: "task_yield", addr: 0 },
    Symbol { name: "pit_get_ticks", addr: 0 },
    Symbol { name: "pci_enable_bus_mastering", addr: 0 },
    Symbol { name: "purplefs_sync", addr: 0 },
]);

pub fn init() {
    {
        let mut syms = SYMBOLS.lock();
        syms[0].addr = crate::drivers::serial::print as *const () as usize as u64;
        syms[1].addr = crate::drivers::serial::putchar as *const () as usize as u64;
        syms[2].addr = crate::mm::pmm::alloc_frame as *const () as usize as u64;
        syms[3].addr = crate::mm::pmm::free_frame as *const () as usize as u64;
        syms[4].addr = crate::mm::vmm::map_page as *const () as usize as u64;
        syms[5].addr = crate::kernel::task::spawn as *const () as usize as u64;
        syms[6].addr = crate::kernel::task::yield_now as *const () as usize as u64;
        syms[7].addr = crate::drivers::pit::get_ticks as *const () as usize as u64;
        syms[8].addr = crate::drivers::pci::enable_bus_mastering as *const () as usize as u64;
        syms[9].addr = crate::kernel::storage::sync as *const () as usize as u64;
    }

    crate::drivers::serial::print("KSYM: Kernel Symbol Table Initialized (");
    crate::drivers::serial::print_u64(SYMBOLS.lock().len() as u64);
    crate::drivers::serial::println(" core symbols registered)");
}

pub fn lookup(name: &str) -> Option<u64> {
    let syms = SYMBOLS.lock();
    for sym in syms.iter() {
        if sym.name == name {
            return Some(sym.addr);
        }
    }
    None
}

pub fn list_symbols() {
    crate::drivers::serial::println("Aegis64 Kernel Symbol Table:");
    let syms = SYMBOLS.lock();
    for sym in syms.iter() {
        crate::drivers::serial::print("  0x");
        crate::drivers::serial::print_hex(sym.addr);
        crate::drivers::serial::print("  ");
        crate::drivers::serial::println(sym.name);
    }
}
