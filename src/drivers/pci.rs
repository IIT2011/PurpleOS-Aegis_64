use crate::arch::x86_64::io::{inl, outl};
use crate::sync::Spinlock;

const PCI_CONFIG_ADDRESS: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

#[derive(Copy, Clone, Default)]
pub struct PciDevice {
    pub bus: u8,
    pub slot: u8,
    pub func: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_id: u8,
    pub subclass_id: u8,
    pub prog_if: u8,
    pub bar0: u32,
    pub bar1: u32,
    pub bar2: u32,
    pub bar3: u32,
    pub bar4: u32,
    pub bar5: u32,
    pub irq_line: u8,
}

struct PciState {
    devices: [PciDevice; 32],
    count: usize,
}

static PCI_STATE: Spinlock<PciState> = Spinlock::new(PciState {
    devices: [PciDevice {
        bus: 0,
        slot: 0,
        func: 0,
        vendor_id: 0,
        device_id: 0,
        class_id: 0,
        subclass_id: 0,
        prog_if: 0,
        bar0: 0,
        bar1: 0,
        bar2: 0,
        bar3: 0,
        bar4: 0,
        bar5: 0,
        irq_line: 0,
    }; 32],
    count: 0,
});

pub fn read_config_u32(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    let address = ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC)
        | 0x80000000;
    unsafe {
        outl(PCI_CONFIG_ADDRESS, address);
        inl(PCI_CONFIG_DATA)
    }
}

pub fn write_config_u32(bus: u8, slot: u8, func: u8, offset: u8, val: u32) {
    let address = ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC)
        | 0x80000000;
    unsafe {
        outl(PCI_CONFIG_ADDRESS, address);
        outl(PCI_CONFIG_DATA, val);
    }
}

pub fn enable_bus_mastering(bus: u8, slot: u8, func: u8) {
    let cmd = read_config_u32(bus, slot, func, 0x04);
    write_config_u32(bus, slot, func, 0x04, (cmd & 0xffff) | 0x07);
}

pub fn init() {
    let mut state = PCI_STATE.lock();
    state.count = 0;

    crate::drivers::serial::println("PCI: Scanning PCI Bus hierarchy...");

    for bus in 0..=8 {
        for slot in 0..32 {
            let first_id = read_config_u32(bus, slot, 0, 0);
            if matches!(first_id as u16, 0 | 0xffff) {
                continue;
            }
            let header = read_config_u32(bus, slot, 0, 0x0c);
            let functions = if header & (1 << 23) != 0 { 8 } else { 1 };
            for func in 0..functions {
                let id = if func == 0 {
                    first_id
                } else {
                    read_config_u32(bus, slot, func, 0)
                };
                let vendor = id as u16;
                if vendor == 0xffff || vendor == 0 {
                    continue;
                }
                let device = (id >> 16) as u16;
                let reg8 = read_config_u32(bus, slot, func, 8);
                let class_id = ((reg8 >> 24) & 0xFF) as u8;
                let subclass_id = ((reg8 >> 16) & 0xFF) as u8;
                let prog_if = ((reg8 >> 8) & 0xFF) as u8;

                let bar0 = read_config_u32(bus, slot, func, 0x10);
                let bar1 = read_config_u32(bus, slot, func, 0x14);
                let bar2 = read_config_u32(bus, slot, func, 0x18);
                let bar3 = read_config_u32(bus, slot, func, 0x1C);
                let bar4 = read_config_u32(bus, slot, func, 0x20);
                let bar5 = read_config_u32(bus, slot, func, 0x24);
                let reg3c = read_config_u32(bus, slot, func, 0x3C);
                let irq_line = (reg3c & 0xFF) as u8;

                if state.count < state.devices.len() {
                    let idx = state.count;
                    state.devices[idx] = PciDevice {
                        bus,
                        slot,
                        func,
                        vendor_id: vendor,
                        device_id: device,
                        class_id,
                        subclass_id,
                        prog_if,
                        bar0,
                        bar1,
                        bar2,
                        bar3,
                        bar4,
                        bar5,
                        irq_line,
                    };
                    state.count += 1;
                }

                crate::drivers::serial::print("  [");
                crate::drivers::serial::print_u64(bus as u64);
                crate::drivers::serial::print(":");
                crate::drivers::serial::print_u64(slot as u64);
                crate::drivers::serial::print("] Device ");
                crate::drivers::serial::print_hex(vendor as u64);
                crate::drivers::serial::print(":");
                crate::drivers::serial::print_hex(device as u64);
                crate::drivers::serial::print(" (Class=");
                crate::drivers::serial::print_hex(class_id as u64);
                crate::drivers::serial::print(", Sub=");
                crate::drivers::serial::print_hex(subclass_id as u64);
                crate::drivers::serial::println(")");
            }
        }
    }
}

pub fn for_each_device<F: FnMut(&PciDevice)>(mut f: F) {
    let state = PCI_STATE.lock();
    for i in 0..state.count {
        f(&state.devices[i]);
    }
}

pub fn find_device(vendor: u16, device: u16) -> Option<PciDevice> {
    let state = PCI_STATE.lock();
    for i in 0..state.count {
        if state.devices[i].vendor_id == vendor && state.devices[i].device_id == device {
            return Some(state.devices[i]);
        }
    }
    None
}
