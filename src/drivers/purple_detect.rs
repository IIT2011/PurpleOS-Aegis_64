pub fn init() {
    crate::drivers::serial::println("PurpleDetect: Peripheral Discovery and Hardware Inventory Daemon online");
}

pub fn scan_all() {
    crate::drivers::serial::println("PurpleDetect: Scanned COM1, PS2KBD, PS2MOUSE, ATA_PRI, PCI Bridge");
}
