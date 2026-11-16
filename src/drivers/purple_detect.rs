use crate::sync::Spinlock;
use crate::drivers::serial;
use crate::drivers::video;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum DeviceCategory {
    Storage,
    Input,
    Display,
    Audio,
    Network,
    Serial,
    Timer,
    Interrupt,
    Bridge,
    Bus,
    Unknown,
}

impl DeviceCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeviceCategory::Storage => "STORAGE",
            DeviceCategory::Input => "INPUT",
            DeviceCategory::Display => "DISPLAY",
            DeviceCategory::Audio => "AUDIO",
            DeviceCategory::Network => "NETWORK",
            DeviceCategory::Serial => "SERIAL",
            DeviceCategory::Timer => "TIMER",
            DeviceCategory::Interrupt => "INTERRUPT",
            DeviceCategory::Bridge => "BRIDGE",
            DeviceCategory::Bus => "BUS",
            DeviceCategory::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum DeviceState {
    Discovered,
    Active,
    Supported,
    Unsupported,
}

impl DeviceState {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeviceState::Discovered => "DISCOVERED",
            DeviceState::Active => "ACTIVE",
            DeviceState::Supported => "SUPPORTED",
            DeviceState::Unsupported => "UNSUPPORTED",
        }
    }
}

#[derive(Copy, Clone)]
pub struct PurpleDevice {
    pub id: u32,
    pub category: DeviceCategory,
    pub state: DeviceState,
    pub name: [u8; 16],
    pub vendor: [u8; 24],
    pub model: [u8; 32],
    pub bus: [u8; 8],
    pub address: u64,
    pub irq: u8,
}

impl PurpleDevice {
    pub const fn empty() -> Self {
        Self {
            id: 0,
            category: DeviceCategory::Unknown,
            state: DeviceState::Discovered,
            name: [0; 16],
            vendor: [0; 24],
            model: [0; 32],
            bus: [0; 8],
            address: 0,
            irq: 0,
        }
    }
}

const MAX_DEVICES: usize = 32;

struct PurpleRegistry {
    devices: [PurpleDevice; MAX_DEVICES],
    count: usize,
    next_id: u32,
}

static REGISTRY: Spinlock<PurpleRegistry> = Spinlock::new(PurpleRegistry {
    devices: [PurpleDevice::empty(); MAX_DEVICES],
    count: 0,
    next_id: 1,
});

fn copy_slice(dst: &mut [u8], src: &str) {
    let bytes = src.as_bytes();
    let len = bytes.len().min(dst.len() - 1);
    dst[..len].copy_from_slice(&bytes[..len]);
    dst[len] = 0;
}

pub fn register(
    category: DeviceCategory,
    name: &str,
    vendor: &str,
    model: &str,
    bus: &str,
    address: u64,
    irq: u8,
    state: DeviceState,
) -> Option<u32> {
    let mut reg = REGISTRY.lock();
    if reg.count >= MAX_DEVICES {
        return None;
    }

    let id = reg.next_id;
    reg.next_id += 1;

    let idx = reg.count;
    let mut dev = PurpleDevice::empty();
    dev.id = id;
    dev.category = category;
    dev.state = state;
    dev.address = address;
    dev.irq = irq;

    copy_slice(&mut dev.name, name);
    copy_slice(&mut dev.vendor, vendor);
    copy_slice(&mut dev.model, model);
    copy_slice(&mut dev.bus, bus);

    reg.devices[idx] = dev;
    reg.count += 1;
    Some(id)
}

pub fn init() {
    serial::println("PurpleDetect: Peripheral Discovery and Hardware Inventory Daemon online");
}

pub fn scan_all() {
    // 1. Builtin ISA System Peripherals
    register(DeviceCategory::Serial, "COM1", "National Semi", "16550A UART Serial", "ISA", 0x3F8, 4, DeviceState::Active);
    register(DeviceCategory::Timer, "PIT", "Intel", "8254 Programmable Timer", "ISA", 0x40, 0, DeviceState::Active);
    register(DeviceCategory::Input, "PS2KBD", "IBM/Standard", "PS/2 Keyboard Controller", "ISA", 0x60, 1, DeviceState::Active);
    register(DeviceCategory::Input, "PS2MOU", "Synaptics", "PS/2 Auxiliary Mouse", "ISA", 0x60, 12, DeviceState::Active);
    register(DeviceCategory::Interrupt, "LAPIC", "Intel/AMD", "Local APIC Controller", "MMIO", 0xFEE00000, 0, DeviceState::Active);
    register(DeviceCategory::Storage, "ATA_PRI", "Standard", "IDE Primary Controller", "ISA", 0x1F0, 14, DeviceState::Active);
    register(DeviceCategory::Storage, "ATA_SEC", "Standard", "IDE Secondary Controller", "ISA", 0x170, 15, DeviceState::Active);

    // 2. Discover and Register PCI Devices
    crate::drivers::pci::for_each_device(|pdev| {
        let cat = match pdev.class_id {
            0x01 => DeviceCategory::Storage,
            0x02 => DeviceCategory::Network,
            0x03 => DeviceCategory::Display,
            0x04 => DeviceCategory::Audio,
            0x06 => DeviceCategory::Bridge,
            0x0C => DeviceCategory::Bus,
            _ => DeviceCategory::Unknown,
        };

        let vendor_name = match pdev.vendor_id {
            0x8086 => "Intel",
            0x1234 => "Bochs/QEMU",
            0x10EC => "Realtek",
            0x10DE => "NVIDIA",
            0x1002 => "AMD",
            _ => "Generic PCI",
        };

        let model_name = match (pdev.vendor_id, pdev.device_id) {
            (0x8086, 0x1237) => "440FX PCI Bridge",
            (0x8086, 0x7000) => "PIIX3 ISA Bridge",
            (0x1234, 0x1111) => "Standard VGA VBE Graphics",
            (0x8086, 0x100E) => "82540EM Gigabit NIC",
            (0x8086, 0x2922) => "ICH9 SATA AHCI Controller",
            (0x10EC, 0x8139) => "RTL-8139 Fast Ethernet",
            _ => match cat {
                DeviceCategory::Storage => "Mass Storage Device",
                DeviceCategory::Network => "Network Controller",
                DeviceCategory::Display => "VGA Display Adapter",
                DeviceCategory::Audio => "High Definition Audio",
                DeviceCategory::Bridge => "PCI Host/Bridge",
                DeviceCategory::Bus => "USB Controller",
                _ => "PCI Peripheral",
            },
        };

        let bdf_addr = ((pdev.bus as u64) << 16) | ((pdev.slot as u64) << 11) | ((pdev.func as u64) << 8);
        register(cat, "PCI_DEV", vendor_name, model_name, "PCI", bdf_addr, pdev.irq_line, DeviceState::Active);
    });

    let reg = REGISTRY.lock();
    serial::print("PurpleDetect: Hardware inventory completed: ");
    serial::print_u64(reg.count as u64);
    serial::println(" discovered peripherals registered.");
}

pub fn print_table() {
    let reg = REGISTRY.lock();
    let header = "\n=== PurpleDetect Hardware & Peripheral Inventory ===\n  Discovered Devices: ";
    serial::print(header);
    serial::print_u64(reg.count as u64);
    serial::println("\n\n  ID  CATEGORY    STATE       BUS    ADDRESS     IRQ  VENDOR / MODEL");
    serial::println("  --------------------------------------------------------------------------------");

    if video::is_available() {
        video::puts(header);
        // Video puts
    }

    for i in 0..reg.count {
        let d = &reg.devices[i];
        serial::print("  #");
        serial::print_u64(d.id as u64);
        serial::print("  ");
        serial::print(d.category.as_str());
        serial::print("  ");
        serial::print(d.state.as_str());
        serial::print("  ");
        let bus_str = core::str::from_utf8(&d.bus).unwrap_or("").trim_matches(char::from(0));
        serial::print(bus_str);
        serial::print("  0x");
        serial::print_hex(d.address);
        serial::print("  ");
        serial::print_u64(d.irq as u64);
        serial::print("  ");
        let ven_str = core::str::from_utf8(&d.vendor).unwrap_or("").trim_matches(char::from(0));
        let mod_str = core::str::from_utf8(&d.model).unwrap_or("").trim_matches(char::from(0));
        serial::print(ven_str);
        serial::print(" - ");
        serial::println(mod_str);
    }
    serial::println("  --------------------------------------------------------------------------------\n");
}
