pub struct Symbol {
    pub name: &'static str,
    pub addr: u64,
}

static SYMBOLS: [Symbol; 8] = [
    Symbol { name: "kprintf", addr: 0x101000 },
    Symbol { name: "serial_write", addr: 0x101010 },
    Symbol { name: "pmm_alloc_frame", addr: 0x101020 },
    Symbol { name: "pmm_free_frame", addr: 0x101030 },
    Symbol { name: "vmm_map_page", addr: 0x101040 },
    Symbol { name: "task_create", addr: 0x101050 },
    Symbol { name: "task_yield", addr: 0x101060 },
    Symbol { name: "pit_get_ticks", addr: 0x101070 },
];

pub fn init() {
    crate::drivers::serial::print("KSYM: Kernel Symbol Table Initialized (");
    crate::drivers::serial::print_u64(SYMBOLS.len() as u64);
    crate::drivers::serial::println(" core symbols registered)");
}

pub fn lookup(name: &str) -> Option<u64> {
    for sym in &SYMBOLS {
        if sym.name == name {
            return Some(sym.addr);
        }
    }
    None
}
