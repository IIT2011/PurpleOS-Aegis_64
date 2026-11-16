//! Realtek RTL8139 Fast Ethernet NIC Driver for Aegis64.
//!
//! Derived from Linux Kernel reference implementation:
//!   - `drivers/net/ethernet/realtek/8139too.c`
//!   - `drivers/net/ethernet/realtek/8139cp.c`

use crate::arch::x86_64::io::{inb, outb, inw, outw, inl, outl};
use crate::drivers::pci::find_device;
use crate::drivers::dma::alloc_coherent;
use crate::sync::Spinlock;

pub const RTL8139_VENDOR: u16 = 0x10EC;
pub const RTL8139_DEV_ID: u16 = 0x8139;

/* Register Offsets (enum RTL8139_registers from Linux 8139too.c) */
pub const REG_MAC0: u16       = 0x00; /* Ethernet Hardware Address */
pub const REG_MAR0: u16       = 0x08; /* Multicast filter */
pub const REG_TX_STATUS0: u16 = 0x10; /* Transmit status (4 ports: 0x10, 0x14, 0x18, 0x1C) */
pub const REG_TX_ADDR0: u16   = 0x20; /* Tx buffer DMA physical addresses (4 ports) */
pub const REG_RX_BUF: u16     = 0x30; /* Rx ring buffer DMA physical address */
pub const REG_CHIP_CMD: u16   = 0x37; /* Command Register */
pub const REG_CAPR: u16       = 0x38; /* Current Address of Packet Read (RxBufPtr) */
pub const REG_CBR: u16        = 0x3A; /* Current Buffer Address (RxBufAddr) */
pub const REG_INTR_MASK: u16  = 0x3C; /* Interrupt Mask */
pub const REG_INTR_STS: u16   = 0x3E; /* Interrupt Status */
pub const REG_TX_CONFIG: u16  = 0x40; /* Transmit Configuration */
pub const REG_RX_CONFIG: u16  = 0x44; /* Receive Configuration */
pub const REG_CONFIG1: u16    = 0x52; /* Configuration Register 1 */

/* ChipCmd Register Bits */
pub const CMD_RESET: u8 = 0x10; /* Software Reset */
pub const CMD_RX_EN: u8 = 0x08; /* Receiver Enable */
pub const CMD_TX_EN: u8 = 0x04; /* Transmitter Enable */

/* RxConfig Bits */
pub const RX_CFG_AAP: u32  = 0x0001; /* Accept All Packets */
pub const RX_CFG_APM: u32  = 0x0002; /* Accept Physical Match */
pub const RX_CFG_AM: u32   = 0x0004; /* Accept Multicast */
pub const RX_CFG_AB: u32   = 0x0008; /* Accept Broadcast */
pub const RX_CFG_WRAP: u32 = 0x0080; /* Wrap packet past end of buffer */

/* Buffer Sizes */
pub const RX_BUF_LEN: usize = 8192 + 16 + 1500; // 8K + 16-byte status + wrap
pub const TX_BUF_LEN: usize = 2048;

pub struct Rtl8139Device {
    pub io_base: u16,
    pub mac: [u8; 6],
    pub is_ready: bool,
    rx_buf: *mut u8,
    rx_buf_phys: u64,
    tx_bufs: [*mut u8; 4],
    tx_bufs_phys: [u64; 4],
    tx_cur: usize,
    rx_read_ptr: usize,
}

unsafe impl Send for Rtl8139Device {}
unsafe impl Sync for Rtl8139Device {}

impl Rtl8139Device {
    pub const fn new() -> Self {
        Self {
            io_base: 0,
            mac: [0; 6],
            is_ready: false,
            rx_buf: core::ptr::null_mut(),
            rx_buf_phys: 0,
            tx_bufs: [core::ptr::null_mut(); 4],
            tx_bufs_phys: [0; 4],
            tx_cur: 0,
            rx_read_ptr: 0,
        }
    }

    pub fn init_hw(&mut self, io_base: u16) -> Result<(), &'static str> {
        self.io_base = io_base;

        unsafe {
            // 1. Power on chip via Config1 (Wake up)
            outb(self.io_base + REG_CONFIG1, 0x00);

            // 2. Software Reset
            outb(self.io_base + REG_CHIP_CMD, CMD_RESET);
            let mut timeout = 10000;
            while (inb(self.io_base + REG_CHIP_CMD) & CMD_RESET) != 0 && timeout > 0 {
                core::hint::spin_loop();
                timeout -= 1;
            }

            // 3. Read Hardware MAC Address
            for i in 0..6 {
                self.mac[i] = inb(self.io_base + REG_MAC0 + i as u16);
            }

            // 4. Allocate DMA buffer for RX Ring
            let (rx_ptr, rx_phys) = alloc_coherent(RX_BUF_LEN).ok_or("Failed to alloc RTL8139 RX Ring")?;
            self.rx_buf = rx_ptr;
            self.rx_buf_phys = rx_phys;
            outl(self.io_base + REG_RX_BUF, rx_phys as u32);

            // 5. Allocate DMA buffers for TX (4 buffers)
            for i in 0..4 {
                let (tx_ptr, tx_phys) = alloc_coherent(TX_BUF_LEN).ok_or("Failed to alloc RTL8139 TX Buf")?;
                self.tx_bufs[i] = tx_ptr;
                self.tx_bufs_phys[i] = tx_phys;
                outl(self.io_base + REG_TX_ADDR0 + (i as u16 * 4), tx_phys as u32);
            }

            // 6. Set Interrupt Mask to 0 (Polled mode initially)
            outw(self.io_base + REG_INTR_MASK, 0x0000);

            // 7. Configure RX (Accept Broadcast, Physical match, Wrap mode)
            outl(self.io_base + REG_RX_CONFIG, RX_CFG_AB | RX_CFG_APM | RX_CFG_WRAP);

            // 8. Enable RX and TX
            outb(self.io_base + REG_CHIP_CMD, CMD_RX_EN | CMD_TX_EN);
        }

        self.is_ready = true;
        Ok(())
    }

    /// Transmit raw ethernet frame
    pub fn transmit(&mut self, packet: &[u8]) -> Result<(), &'static str> {
        if !self.is_ready || packet.len() > TX_BUF_LEN {
            return Err("RTL8139 not ready or packet too large");
        }

        unsafe {
            let cur = self.tx_cur;
            let dst_buf = self.tx_bufs[cur];
            core::ptr::copy_nonoverlapping(packet.as_ptr(), dst_buf, packet.len());

            // Write length to TxStatus register (pad to min 60 bytes if needed)
            let len = packet.len().max(60);
            outl(self.io_base + REG_TX_STATUS0 + (cur as u16 * 4), len as u32);

            self.tx_cur = (cur + 1) % 4;
        }

        Ok(())
    }

    /// Poll for received packets (returns bytes read)
    pub fn poll_receive(&mut self, buf: &mut [u8]) -> usize {
        if !self.is_ready {
            return 0;
        }

        unsafe {
            let cmd = inb(self.io_base + REG_CHIP_CMD);
            if (cmd & 0x01) != 0 {
                return 0; // Buffer empty (BUFE bit set)
            }

            let cur_ptr = self.rx_read_ptr;
            let header_ptr = self.rx_buf.add(cur_ptr) as *const u16;
            let status = core::ptr::read_volatile(header_ptr);
            let length = core::ptr::read_volatile(header_ptr.add(1)) as usize;

            if (status & 0x01) == 0 || length == 0 || length > 1536 {
                return 0; // Bad packet header
            }

            // Copy packet payload (skip 4-byte header and 4-byte CRC)
            let payload_len = (length - 4).min(buf.len());
            let src = self.rx_buf.add(cur_ptr + 4);
            core::ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), payload_len);

            // Update read pointer (4-byte aligned)
            self.rx_read_ptr = (cur_ptr + length + 4 + 3) & !3;
            if self.rx_read_ptr >= 8192 {
                self.rx_read_ptr -= 8192;
            }

            // Update CAPR register (subtract 16 per RTL8139 hardware erratum specification)
            outw(self.io_base + REG_CAPR, (self.rx_read_ptr as u16).wrapping_sub(16));

            payload_len
        }
    }
}

pub static RTL8139: Spinlock<Rtl8139Device> = Spinlock::new(Rtl8139Device::new());

pub fn init() {
    if let Some(dev) = find_device(RTL8139_VENDOR, RTL8139_DEV_ID) {
        let io_base = (dev.bar0 & !0x3) as u16;
        let mut nic = RTL8139.lock();
        if nic.init_hw(io_base).is_ok() {
            crate::drivers::serial::print("RTL8139: Realtek Fast Ethernet NIC initialized (MAC: ");
            for i in 0..6 {
                crate::drivers::serial::print_hex(nic.mac[i] as u64);
                if i < 5 {
                    crate::drivers::serial::print(":");
                }
            }
            crate::drivers::serial::println(", Port I/O 8K RX DMA Ring Active)");
        }
    }
}

pub fn is_ready() -> bool {
    RTL8139.lock().is_ready
}
