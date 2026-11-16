//! Intel 82540EM / 8254x Gigabit Ethernet NIC Driver for Aegis64.
//!
//! Derived from Linux Kernel reference implementation:
//!   - `drivers/net/ethernet/intel/e1000/e1000_hw.h`
//!   - `drivers/net/ethernet/intel/e1000/e1000_hw.c`
//!   - `drivers/net/ethernet/intel/e1000/e1000_main.c`

use crate::drivers::pci::find_device;
use crate::drivers::dma::alloc_coherent;
use crate::sync::Spinlock;

pub const E1000_VENDOR_INTEL: u16 = 0x8086;
pub const E1000_DEV_82540EM: u16 = 0x100E;
pub const E1000_DEV_82545EM: u16 = 0x100F;
pub const E1000_DEV_82574L: u16  = 0x10D3;

/* Device Registers from Linux e1000_hw.h */
pub const E1000_CTRL: usize     = 0x00000; /* Device Control - RW */
pub const E1000_STATUS: usize   = 0x00008; /* Device Status - RO */
pub const E1000_EECD: usize     = 0x00010; /* EEPROM/Flash Control - RW */
pub const E1000_EERD: usize     = 0x00014; /* EEPROM Read - RW */
pub const E1000_CTRL_EXT: usize = 0x00018; /* Extended Device Control - RW */
pub const E1000_ICR: usize      = 0x000C0; /* Interrupt Cause Read - R/clr */
pub const E1000_IMS: usize      = 0x000D0; /* Interrupt Mask Set - RW */
pub const E1000_IMC: usize      = 0x000D8; /* Interrupt Mask Clear - WO */
pub const E1000_RCTL: usize     = 0x00100; /* RX Control - RW */
pub const E1000_TCTL: usize     = 0x00400; /* TX Control - RW */
pub const E1000_TIPG: usize     = 0x00410; /* TX Inter-packet gap - RW */
pub const E1000_RDBAL: usize    = 0x02800; /* RX Descriptor Base Address Low - RW */
pub const E1000_RDBAH: usize    = 0x02804; /* RX Descriptor Base Address High - RW */
pub const E1000_RDLEN: usize    = 0x02808; /* RX Descriptor Length - RW */
pub const E1000_RDH: usize      = 0x02810; /* RX Descriptor Head - RW */
pub const E1000_RDT: usize      = 0x02818; /* RX Descriptor Tail - RW */
pub const E1000_TDBAL: usize    = 0x03800; /* TX Descriptor Base Address Low - RW */
pub const E1000_TDBAH: usize    = 0x03804; /* TX Descriptor Base Address High - RW */
pub const E1000_TDLEN: usize    = 0x03808; /* TX Descriptor Length - RW */
pub const E1000_TDH: usize      = 0x03810; /* TX Descriptor Head - RW */
pub const E1000_TDT: usize      = 0x03818; /* TX Descriptor Tail - RW */
pub const E1000_MTA: usize      = 0x05200; /* Multicast Table Array - RW (128 entries) */
pub const E1000_RAL: usize      = 0x05400; /* Receive Address Low - RW */
pub const E1000_RAH: usize      = 0x05404; /* Receive Address High - RW */

/* Control Register Bits */
pub const E1000_CTRL_SLU: u32   = 0x00000040; /* Set Link Up */
pub const E1000_CTRL_RST: u32   = 0x04000000; /* Global Reset */

/* RCTL (Receive Control) Bits */
pub const E1000_RCTL_EN: u32    = 0x00000002; /* Receiver Enable */
pub const E1000_RCTL_SBP: u32   = 0x00000004; /* Store Bad Packets */
pub const E1000_RCTL_UPE: u32   = 0x00000008; /* Unicast Promiscuous Enabled */
pub const E1000_RCTL_MPE: u32   = 0x00000010; /* Multicast Promiscuous Enabled */
pub const E1000_RCTL_LPE: u32   = 0x00000020; /* Long Packet Enable */
pub const E1000_RCTL_BAM: u32   = 0x00008000; /* Broadcast Accept Mode */
pub const E1000_RCTL_BSIZE_2048: u32 = 0x00000000; /* Buffer Size 2048 bytes */
pub const E1000_RCTL_SECRC: u32 = 0x04000000; /* Strip Ethernet CRC */

/* TCTL (Transmit Control) Bits */
pub const E1000_TCTL_EN: u32    = 0x00000002; /* Transmit Enable */
pub const E1000_TCTL_PSP: u32   = 0x00000008; /* Pad Short Packets */
pub const E1000_TCTL_CT_SHIFT: u32 = 4;       /* Collision Threshold */
pub const E1000_TCTL_COLD_SHIFT: u32 = 12;    /* Collision Distance */

/* EERD (EEPROM Read Register) */
pub const E1000_EERD_START: u32 = 0x00000001; /* Start Read */
pub const E1000_EERD_DONE: u32  = 0x00000010; /* Read Done */
pub const E1000_EERD_ADDR_SHIFT: u32 = 8;
pub const E1000_EERD_DATA_SHIFT: u32 = 16;

/* Descriptor Ring Sizes */
pub const NUM_RX_DESCRIPTORS: usize = 32;
pub const NUM_TX_DESCRIPTORS: usize = 32;
pub const RX_BUFFER_SIZE: usize = 2048;

#[repr(C, packed)]
#[derive(Copy, Clone, Default)]
pub struct E1000RxDesc {
    pub buffer_addr: u64,
    pub length: u16,
    pub csum: u16,
    pub status: u8,
    pub errors: u8,
    pub special: u16,
}

#[repr(C, packed)]
#[derive(Copy, Clone, Default)]
pub struct E1000TxDesc {
    pub buffer_addr: u64,
    pub length: u16,
    pub cso: u8,
    pub cmd: u8,
    pub status: u8,
    pub css: u8,
    pub special: u16,
}

pub struct E1000Device {
    pub mmio_base: usize,
    pub mac: [u8; 6],
    pub is_ready: bool,
    rx_descs: *mut E1000RxDesc,
    rx_descs_phys: u64,
    rx_buffers: [*mut u8; NUM_RX_DESCRIPTORS],
    rx_tail: usize,
    tx_descs: *mut E1000TxDesc,
    tx_descs_phys: u64,
    tx_buffers: [*mut u8; NUM_TX_DESCRIPTORS],
    tx_tail: usize,
}

unsafe impl Send for E1000Device {}
unsafe impl Sync for E1000Device {}

impl E1000Device {
    pub const fn new() -> Self {
        Self {
            mmio_base: 0,
            mac: [0; 6],
            is_ready: false,
            rx_descs: core::ptr::null_mut(),
            rx_descs_phys: 0,
            rx_buffers: [core::ptr::null_mut(); NUM_RX_DESCRIPTORS],
            rx_tail: 0,
            tx_descs: core::ptr::null_mut(),
            tx_descs_phys: 0,
            tx_buffers: [core::ptr::null_mut(); NUM_TX_DESCRIPTORS],
            tx_tail: 0,
        }
    }

    #[inline(always)]
    unsafe fn write_reg(&self, reg: usize, val: u32) {
        let ptr = (self.mmio_base + reg) as *mut u32;
        core::ptr::write_volatile(ptr, val);
    }

    #[inline(always)]
    unsafe fn read_reg(&self, reg: usize) -> u32 {
        let ptr = (self.mmio_base + reg) as *const u32;
        core::ptr::read_volatile(ptr)
    }

    /// Read 16-bit word from EEPROM using EERD register as defined in Linux e1000_hw.c
    fn read_eeprom(&self, addr: u8) -> u16 {
        unsafe {
            self.write_reg(E1000_EERD, ((addr as u32) << E1000_EERD_ADDR_SHIFT) | E1000_EERD_START);
            let mut timeout = 10000;
            while timeout > 0 {
                let val = self.read_reg(E1000_EERD);
                if (val & E1000_EERD_DONE) != 0 {
                    return ((val >> E1000_EERD_DATA_SHIFT) & 0xFFFF) as u16;
                }
                timeout -= 1;
            }
        }
        0
    }

    /// Initialize the E1000 controller
    pub fn init_hw(&mut self, bar0: u64) -> Result<(), &'static str> {
        self.mmio_base = bar0 as usize;

        // 1. Read MAC address
        let w0 = self.read_eeprom(0);
        let w1 = self.read_eeprom(1);
        let w2 = self.read_eeprom(2);

        if w0 != 0 || w1 != 0 || w2 != 0 {
            self.mac[0] = (w0 & 0xFF) as u8;
            self.mac[1] = (w0 >> 8) as u8;
            self.mac[2] = (w1 & 0xFF) as u8;
            self.mac[3] = (w1 >> 8) as u8;
            self.mac[4] = (w2 & 0xFF) as u8;
            self.mac[5] = (w2 >> 8) as u8;
        } else {
            // Fallback: Read RAL/RAH registers
            unsafe {
                let ral = self.read_reg(E1000_RAL);
                let rah = self.read_reg(E1000_RAH);
                self.mac[0] = (ral & 0xFF) as u8;
                self.mac[1] = ((ral >> 8) & 0xFF) as u8;
                self.mac[2] = ((ral >> 16) & 0xFF) as u8;
                self.mac[3] = ((ral >> 24) & 0xFF) as u8;
                self.mac[4] = (rah & 0xFF) as u8;
                self.mac[5] = ((rah >> 8) & 0xFF) as u8;
            }
        }

        // If MAC is still zero (e.g. QEMU without EEPROM attached), provide standard QEMU default
        if self.mac == [0, 0, 0, 0, 0, 0] {
            self.mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        }

        // 2. Allocate DMA memory for RX Ring
        let rx_ring_size = NUM_RX_DESCRIPTORS * core::mem::size_of::<E1000RxDesc>();
        let (rx_ptr, rx_phys) = unsafe { alloc_coherent(rx_ring_size) }.ok_or("Failed to alloc RX Ring")?;
        self.rx_descs = rx_ptr as *mut E1000RxDesc;
        self.rx_descs_phys = rx_phys;

        // Allocate RX Buffers
        for i in 0..NUM_RX_DESCRIPTORS {
            let (buf_ptr, buf_phys) = unsafe { alloc_coherent(RX_BUFFER_SIZE) }.ok_or("Failed to alloc RX buf")?;
            self.rx_buffers[i] = buf_ptr;
            unsafe {
                let desc = self.rx_descs.add(i);
                (*desc).buffer_addr = buf_phys;
                (*desc).status = 0;
            }
        }

        // 3. Allocate DMA memory for TX Ring
        let tx_ring_size = NUM_TX_DESCRIPTORS * core::mem::size_of::<E1000TxDesc>();
        let (tx_ptr, tx_phys) = unsafe { alloc_coherent(tx_ring_size) }.ok_or("Failed to alloc TX Ring")?;
        self.tx_descs = tx_ptr as *mut E1000TxDesc;
        self.tx_descs_phys = tx_phys;

        // Allocate TX Buffers
        for i in 0..NUM_TX_DESCRIPTORS {
            let (buf_ptr, buf_phys) = unsafe { alloc_coherent(RX_BUFFER_SIZE) }.ok_or("Failed to alloc TX buf")?;
            self.tx_buffers[i] = buf_ptr;
            unsafe {
                let desc = self.tx_descs.add(i);
                (*desc).buffer_addr = buf_phys;
                (*desc).cmd = 0;
                (*desc).status = 1; // Mark initially free/done
            }
        }

        unsafe {
            // 4. Clear Multicast Table Array (MTA)
            for i in 0..128 {
                self.write_reg(E1000_MTA + (i * 4), 0);
            }

            // 5. Setup RX Registers
            self.write_reg(E1000_RDBAL, (self.rx_descs_phys & 0xFFFFFFFF) as u32);
            self.write_reg(E1000_RDBAH, (self.rx_descs_phys >> 32) as u32);
            self.write_reg(E1000_RDLEN, rx_ring_size as u32);
            self.write_reg(E1000_RDH, 0);
            self.write_reg(E1000_RDT, (NUM_RX_DESCRIPTORS - 1) as u32);
            self.rx_tail = 0;

            // Enable RX (EN | BAM | SECRC)
            self.write_reg(
                E1000_RCTL,
                E1000_RCTL_EN | E1000_RCTL_BAM | E1000_RCTL_SECRC | E1000_RCTL_BSIZE_2048,
            );

            // 6. Setup TX Registers
            self.write_reg(E1000_TDBAL, (self.tx_descs_phys & 0xFFFFFFFF) as u32);
            self.write_reg(E1000_TDBAH, (self.tx_descs_phys >> 32) as u32);
            self.write_reg(E1000_TDLEN, tx_ring_size as u32);
            self.write_reg(E1000_TDH, 0);
            self.write_reg(E1000_TDT, 0);
            self.tx_tail = 0;

            // Configure Inter-Packet Gap (IPGT = 10, IPGR1 = 8, IPGR2 = 6 as per standard e1000)
            self.write_reg(E1000_TIPG, 0x0060200A);

            // Enable TX (EN | PSP | Collision settings)
            self.write_reg(
                E1000_TCTL,
                E1000_TCTL_EN | E1000_TCTL_PSP | (15 << E1000_TCTL_CT_SHIFT) | (64 << E1000_TCTL_COLD_SHIFT),
            );

            // 7. Enable Link (Set Link Up)
            let ctrl = self.read_reg(E1000_CTRL);
            self.write_reg(E1000_CTRL, ctrl | E1000_CTRL_SLU);

            // 8. Disable/Clear Interrupts
            self.write_reg(E1000_IMC, 0xFFFFFFFF);
            let _ = self.read_reg(E1000_ICR);
        }

        self.is_ready = true;
        Ok(())
    }

    /// Transmit raw ethernet frame
    pub fn transmit(&mut self, packet: &[u8]) -> Result<(), &'static str> {
        if !self.is_ready || packet.len() > RX_BUFFER_SIZE {
            return Err("E1000 not ready or packet too large");
        }

        unsafe {
            let tail = self.tx_tail;
            let desc = self.tx_descs.add(tail);

            // Copy payload to DMA buffer
            let buf = self.tx_buffers[tail];
            core::ptr::copy_nonoverlapping(packet.as_ptr(), buf, packet.len());

            // Setup TX descriptor (EOP = End of Packet 0x01, IFCS = Insert FCS 0x02, RS = Report Status 0x08)
            (*desc).length = packet.len() as u16;
            (*desc).cmd = 0x01 | 0x02 | 0x08;
            (*desc).status = 0;

            // Advance tail pointer
            self.tx_tail = (tail + 1) % NUM_TX_DESCRIPTORS;
            self.write_reg(E1000_TDT, self.tx_tail as u32);

            // Poll for completion (RS bit)
            let mut timeout = 10000;
            while ((*desc).status & 0x01) == 0 && timeout > 0 {
                core::hint::spin_loop();
                timeout -= 1;
            }
        }

        Ok(())
    }

    /// Poll for received packets (returns bytes read)
    pub fn poll_receive(&mut self, buf: &mut [u8]) -> usize {
        if !self.is_ready {
            return 0;
        }

        unsafe {
            let tail = self.rx_tail;
            let desc = self.rx_descs.add(tail);

            // Check if DD (Descriptor Done) bit 0 is set
            if ((*desc).status & 0x01) == 0 {
                return 0; // No packet ready
            }

            let len = ((*desc).length as usize).min(buf.len());
            let src_buf = self.rx_buffers[tail];
            core::ptr::copy_nonoverlapping(src_buf, buf.as_mut_ptr(), len);

            // Clear status and replenish descriptor to ring
            (*desc).status = 0;
            self.write_reg(E1000_RDT, tail as u32);
            self.rx_tail = (tail + 1) % NUM_RX_DESCRIPTORS;

            len
        }
    }
}

pub static E1000: Spinlock<E1000Device> = Spinlock::new(E1000Device::new());

pub fn init() {
    if let Some(dev) = find_device(E1000_VENDOR_INTEL, E1000_DEV_82540EM) {
        let bar0 = (dev.bar0 & !0xF) as u64;
        let _ = crate::mm::vmm::map_mmio_range(bar0, 128 * 1024);
        let mut nic = E1000.lock();
        if nic.init_hw(bar0).is_ok() {
            crate::drivers::serial::print("E1000: Intel 82540EM initialized (MAC: ");
            for i in 0..6 {
                crate::drivers::serial::print_hex(nic.mac[i] as u64);
                if i < 5 {
                    crate::drivers::serial::print(":");
                }
            }
            crate::drivers::serial::println(", Rings: 32 RX / 32 TX DMA Descriptors)");
        }
    }
}

pub fn is_ready() -> bool {
    E1000.lock().is_ready
}

pub fn get_mac() -> [u8; 6] {
    E1000.lock().mac
}
