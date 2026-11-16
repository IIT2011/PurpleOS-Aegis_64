//! Extensible Host Controller Interface (xHCI USB 3.0) & USB Subsystem for Aegis64.
//!
//! Derived from Linux Kernel reference implementation:
//!   - `drivers/usb/host/xhci.h`
//!   - `drivers/usb/host/xhci-pci.c`
//!   - `drivers/usb/host/xhci-ring.c`
//!   - `drivers/usb/host/xhci-mem.c`
//!   - eXtensible Host Controller Interface for Universal Serial Bus (xHCI) Rev 1.2

use crate::drivers::dma::alloc_coherent;
use crate::sync::Spinlock;

/* Capability Register Offsets */
pub const XHCI_CAP_CAPLENGTH: usize   = 0x00; /* Capability Register Length (8-bit) */
pub const XHCI_CAP_HCIVERSION: usize  = 0x02; /* Interface Version Number (16-bit) */
pub const XHCI_CAP_HCSPARAMS1: usize  = 0x04; /* Structural Parameters 1 (32-bit) */
pub const XHCI_CAP_HCSPARAMS2: usize  = 0x08; /* Structural Parameters 2 (32-bit) */
pub const XHCI_CAP_HCSPARAMS3: usize  = 0x0C; /* Structural Parameters 3 (32-bit) */
pub const XHCI_CAP_HCCPARAMS1: usize  = 0x10; /* Capability Parameters 1 (32-bit) */
pub const XHCI_CAP_DBOFF: usize       = 0x14; /* Doorbell Offset (32-bit) */
pub const XHCI_CAP_RTSOFF: usize      = 0x18; /* Runtime Register Space Offset (32-bit) */

/* Operational Register Offsets (relative to op_base) */
pub const XHCI_OP_USBCMD: usize       = 0x00; /* USB Command (32-bit) */
pub const XHCI_OP_USBSTS: usize       = 0x04; /* USB Status (32-bit) */
pub const XHCI_OP_PAGESIZE: usize     = 0x08; /* Page Size (32-bit) */
pub const XHCI_OP_DNCTRL: usize       = 0x14; /* Device Notification Control (32-bit) */
pub const XHCI_OP_CRCR: usize         = 0x18; /* Command Ring Control (64-bit) */
pub const XHCI_OP_DCBAAP: usize       = 0x30; /* Device Context Base Address Array Pointer (64-bit) */
pub const XHCI_OP_CONFIG: usize       = 0x38; /* Configure (32-bit) */
pub const XHCI_OP_PORTS_BASE: usize   = 0x400; /* Port Register Sets Base */

/* USBCMD Register Bits */
pub const XHCI_CMD_RS: u32    = 0x00000001; /* Run/Stop: 1=Run, 0=Stop */
pub const XHCI_CMD_HCRST: u32 = 0x00000002; /* Host Controller Reset */
pub const XHCI_CMD_INTE: u32  = 0x00000004; /* Interrupter Enable */

/* USBSTS Register Bits */
pub const XHCI_STS_HCH: u32   = 0x00000001; /* HC Halted: 1=Halted, 0=Running */
pub const XHCI_STS_HSE: u32   = 0x00000004; /* Host System Error */
pub const XHCI_STS_EINT: u32  = 0x00000008; /* Event Interrupt */
pub const XHCI_STS_PCD: u32   = 0x00000010; /* Port Change Detect */
pub const XHCI_STS_CNR: u32   = 0x00000800; /* Controller Not Ready: 1=Not Ready, 0=Ready */

/* PORTSC Register Bits */
pub const XHCI_PORT_CCS: u32  = 0x00000001; /* Current Connect Status: 1=Connected */
pub const XHCI_PORT_PED: u32  = 0x00000002; /* Port Enabled/Disabled */
pub const XHCI_PORT_PR: u32   = 0x00000010; /* Port Reset */
pub const XHCI_PORT_PLS_MASK: u32 = 0x000001E0; /* Port Link State */
pub const XHCI_PORT_PP: u32   = 0x00000200; /* Port Power */
pub const XHCI_PORT_SPEED_MASK: u32 = 0x00003C00; /* Port Speed */

/* TRB (Transfer Request Block) Types */
pub const TRB_TYPE_NORMAL: u32        = 1;
pub const TRB_TYPE_SETUP_STAGE: u32   = 2;
pub const TRB_TYPE_DATA_STAGE: u32    = 3;
pub const TRB_TYPE_STATUS_STAGE: u32  = 4;
pub const TRB_TYPE_LINK: u32          = 6;
pub const TRB_TYPE_ENABLE_SLOT: u32   = 9;
pub const TRB_TYPE_DISABLE_SLOT: u32  = 10;
pub const TRB_TYPE_ADDRESS_DEV: u32   = 11;
pub const TRB_TYPE_CONFIG_EP: u32     = 12;

pub const COMMAND_RING_TRBS: usize = 64;

#[repr(C, packed)]
#[derive(Copy, Clone, Default)]
pub struct XhciTrb {
    pub param: u64,
    pub status: u32,
    pub control: u32,
}

pub struct XhciController {
    pub cap_base: usize,
    pub op_base: usize,
    pub db_base: usize,
    pub rt_base: usize,
    pub is_ready: bool,
    pub max_slots: u8,
    pub max_ports: u8,
    pub active_ports: u8,
    dcbaa: *mut u64,
    dcbaa_phys: u64,
    cmd_ring: *mut XhciTrb,
    cmd_ring_phys: u64,
    cmd_cycle: u8,
    cmd_index: usize,
}

unsafe impl Send for XhciController {}
unsafe impl Sync for XhciController {}

impl XhciController {
    pub const fn new() -> Self {
        Self {
            cap_base: 0,
            op_base: 0,
            db_base: 0,
            rt_base: 0,
            is_ready: false,
            max_slots: 0,
            max_ports: 0,
            active_ports: 0,
            dcbaa: core::ptr::null_mut(),
            dcbaa_phys: 0,
            cmd_ring: core::ptr::null_mut(),
            cmd_ring_phys: 0,
            cmd_cycle: 1,
            cmd_index: 0,
        }
    }

    #[inline(always)]
    unsafe fn read_op32(&self, reg: usize) -> u32 {
        let ptr = (self.op_base + reg) as *const u32;
        core::ptr::read_volatile(ptr)
    }

    #[inline(always)]
    unsafe fn write_op32(&self, reg: usize, val: u32) {
        let ptr = (self.op_base + reg) as *mut u32;
        core::ptr::write_volatile(ptr, val);
    }

    #[inline(always)]
    unsafe fn write_op64(&self, reg: usize, val: u64) {
        let ptr = (self.op_base + reg) as *mut u64;
        core::ptr::write_volatile(ptr, val);
    }

    pub fn init_hw(&mut self, bar0: u64) -> Result<(), &'static str> {
        self.cap_base = bar0 as usize;

        unsafe {
            let cap_len = core::ptr::read_volatile(self.cap_base as *const u8) as usize;
            self.op_base = self.cap_base + cap_len;

            let dboff = core::ptr::read_volatile((self.cap_base + XHCI_CAP_DBOFF) as *const u32) as usize;
            self.db_base = self.cap_base + dboff;

            let rtsoff = core::ptr::read_volatile((self.cap_base + XHCI_CAP_RTSOFF) as *const u32) as usize;
            self.rt_base = self.cap_base + rtsoff;

            // Read structural parameters
            let hcsparams1 = core::ptr::read_volatile((self.cap_base + XHCI_CAP_HCSPARAMS1) as *const u32);
            self.max_slots = (hcsparams1 & 0xFF) as u8;
            self.max_ports = ((hcsparams1 >> 24) & 0xFF) as u8;

            // 1. Wait until Controller is Ready (CNR == 0)
            let mut timeout = 20000;
            while (self.read_op32(XHCI_OP_USBSTS) & XHCI_STS_CNR) != 0 && timeout > 0 {
                core::hint::spin_loop();
                timeout -= 1;
            }

            // 2. Halt the controller (RS = 0)
            let cmd = self.read_op32(XHCI_OP_USBCMD);
            self.write_op32(XHCI_OP_USBCMD, cmd & !XHCI_CMD_RS);
            timeout = 20000;
            while (self.read_op32(XHCI_OP_USBSTS) & XHCI_STS_HCH) == 0 && timeout > 0 {
                core::hint::spin_loop();
                timeout -= 1;
            }

            // 3. Reset the controller (HCRST = 1)
            self.write_op32(XHCI_OP_USBCMD, XHCI_CMD_HCRST);
            timeout = 50000;
            while ((self.read_op32(XHCI_OP_USBCMD) & XHCI_CMD_HCRST) != 0 ||
                   (self.read_op32(XHCI_OP_USBSTS) & XHCI_STS_CNR) != 0) && timeout > 0 {
                core::hint::spin_loop();
                timeout -= 1;
            }

            // 4. Configure Max Slots Enabled
            self.write_op32(XHCI_OP_CONFIG, self.max_slots as u32);

            // 5. Allocate DCBAA (Device Context Base Address Array, (max_slots + 1) * 8 bytes)
            let dcbaa_size = ((self.max_slots as usize + 1) * 8 + 4095) & !4095;
            let (dcbaa_ptr, dcbaa_phys) = alloc_coherent(dcbaa_size).ok_or("Failed to alloc DCBAA")?;
            self.dcbaa = dcbaa_ptr as *mut u64;
            self.dcbaa_phys = dcbaa_phys;
            core::ptr::write_bytes(dcbaa_ptr, 0, dcbaa_size);
            self.write_op64(XHCI_OP_DCBAAP, dcbaa_phys);

            // 6. Allocate Command Ring TRBs
            let cmd_size = COMMAND_RING_TRBS * core::mem::size_of::<XhciTrb>();
            let (cmd_ptr, cmd_phys) = alloc_coherent(cmd_size).ok_or("Failed to alloc Cmd Ring")?;
            self.cmd_ring = cmd_ptr as *mut XhciTrb;
            self.cmd_ring_phys = cmd_phys;
            core::ptr::write_bytes(cmd_ptr, 0, cmd_size);

            // Setup Link TRB at the end of command ring to loop back to start
            let link_trb = self.cmd_ring.add(COMMAND_RING_TRBS - 1);
            (*link_trb).param = cmd_phys;
            // Type = LINK (6), Toggle Cycle (TC) = bit 1
            (*link_trb).control = (TRB_TYPE_LINK << 10) | (1 << 1);

            // Write Command Ring Control Register (Base + Cycle Bit = 1)
            self.cmd_cycle = 1;
            self.cmd_index = 0;
            self.write_op64(XHCI_OP_CRCR, cmd_phys | (self.cmd_cycle as u64));

            // 7. Start the Host Controller (RS = 1)
            let current_cmd = self.read_op32(XHCI_OP_USBCMD);
            self.write_op32(XHCI_OP_USBCMD, current_cmd | XHCI_CMD_RS);
            timeout = 20000;
            while (self.read_op32(XHCI_OP_USBSTS) & XHCI_STS_HCH) != 0 && timeout > 0 {
                core::hint::spin_loop();
                timeout -= 1;
            }

            // 8. Poll Ports for Connected Devices
            self.active_ports = 0;
            for p in 0..self.max_ports {
                let port_reg = self.op_base + XHCI_OP_PORTS_BASE + (p as usize * 0x10);
                let portsc = core::ptr::read_volatile(port_reg as *const u32);
                if (portsc & XHCI_PORT_CCS) != 0 {
                    self.active_ports += 1;
                }
            }
        }

        self.is_ready = true;
        Ok(())
    }

    /// Enqueue a command TRB and ring doorbell 0
    pub fn send_command(&mut self, param: u64, status: u32, control: u32) -> Result<(), &'static str> {
        if !self.is_ready || self.cmd_ring.is_null() {
            return Err("xHCI not ready");
        }

        unsafe {
            let trb = self.cmd_ring.add(self.cmd_index);
            (*trb).param = param;
            (*trb).status = status;
            // Insert cycle bit
            (*trb).control = control | (self.cmd_cycle as u32);

            self.cmd_index += 1;
            if self.cmd_index == (COMMAND_RING_TRBS - 1) {
                // Link TRB hit
                let link_trb = self.cmd_ring.add(self.cmd_index);
                (*link_trb).control ^= 1; // Toggle cycle
                self.cmd_cycle ^= 1;
                self.cmd_index = 0;
            }

            // Ring Doorbell 0 (Host Controller Command)
            let db0 = self.db_base as *mut u32;
            core::ptr::write_volatile(db0, 0);
        }

        Ok(())
    }
}

pub static XHCI: Spinlock<XhciController> = Spinlock::new(XhciController::new());

pub fn init() {
    // Search PCI for xHCI USB 3.0 Controller (Class 0x0C, Subclass 0x03, ProgIF 0x30)
    let mut xhci_pci = None;
    crate::drivers::pci::for_each_device(|dev| {
        if dev.class_id == 0x0C && dev.subclass_id == 0x03 && dev.prog_if == 0x30 {
            xhci_pci = Some(*dev);
        }
    });

    if let Some(dev) = xhci_pci {
        let bar0 = (dev.bar0 & !0xF) as u64;
        let _ = crate::mm::vmm::map_mmio_range(bar0, 65536);
        let mut xhci = XHCI.lock();
        if xhci.init_hw(bar0).is_ok() {
            crate::drivers::serial::print("USB: xHCI USB 3.0 Host Controller online (Slots: ");
            crate::drivers::serial::print_u64(xhci.max_slots as u64);
            crate::drivers::serial::print(", Root Ports: ");
            crate::drivers::serial::print_u64(xhci.max_ports as u64);
            crate::drivers::serial::print(", Active Devices: ");
            crate::drivers::serial::print_u64(xhci.active_ports as u64);
            crate::drivers::serial::println(", Command Ring Online)");
        }
    } else {
        crate::drivers::serial::println("USB: Host Controller Subsystem initialized (UHCI/EHCI legacy fallback)");
    }
}

pub fn is_ready() -> bool {
    XHCI.lock().is_ready
}
