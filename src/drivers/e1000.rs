use crate::drivers::pci::find_device;

pub const E1000_VENDOR_INTEL: u16 = 0x8086;
pub const E1000_DEV_82540EM: u16 = 0x100E;

pub struct E1000Device {
    pub bar0: u64,
    pub mac: [u8; 6],
    pub is_ready: bool,
}

static mut E1000: E1000Device = E1000Device {
    bar0: 0,
    mac: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
    is_ready: false,
};

pub fn init() {
    if let Some(dev) = find_device(E1000_VENDOR_INTEL, E1000_DEV_82540EM) {
        unsafe {
            E1000.bar0 = (dev.bar0 & !0xF) as u64;
            E1000.is_ready = true;
        }
        crate::drivers::serial::println("E1000: Intel 82540EM Gigabit Ethernet NIC initialized");
    }
}

pub fn is_ready() -> bool {
    unsafe { E1000.is_ready }
}
