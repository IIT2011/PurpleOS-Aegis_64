pub mod ahci;
pub mod ata_pio;
pub mod nvme;
pub mod sd;
pub mod usb;

use crate::drivers::block_dev::BLOCK_DEV_ERR_BOUNDS;
use core::hint::spin_loop;

const DMA_PAGE: usize = crate::mm::pmm::PAGE_SIZE;
// The local APIC counter keeps running with interrupts masked during I/O.
const APIC_TIMER_PERIOD_MS: u64 = 10;
const TIMER_SAMPLE_INTERVAL: usize = 64;
const STOPPED_TIMER_LIMIT: usize = 4_000_000;
const IO_TIMEOUT_MS: u64 = 30_000;

fn wait_for(ready: impl FnMut() -> bool) -> bool {
    wait_for_ms(IO_TIMEOUT_MS, ready)
}

fn wait_for_ms(timeout_ms: u64, mut ready: impl FnMut() -> bool) -> bool {
    use crate::drivers::apic::{read_reg, LAPIC_TIMER_CURR, LAPIC_TIMER_INIT};
    if ready() {
        return true;
    }

    let period = u64::from(unsafe { read_reg(LAPIC_TIMER_INIT) });
    let budget = timeout_ms
        .saturating_mul(period)
        .div_ceil(APIC_TIMER_PERIOD_MS);
    let mut previous = u64::from(unsafe { read_reg(LAPIC_TIMER_CURR) });
    let mut elapsed = 0u64;
    let mut stalled = 0;
    while stalled < STOPPED_TIMER_LIMIT {
        for _ in 0..TIMER_SAMPLE_INTERVAL {
            if ready() {
                return true;
            }
            spin_loop();
        }

        let current = u64::from(unsafe { read_reg(LAPIC_TIMER_CURR) });
        if current == previous || current > period || previous > period {
            stalled += TIMER_SAMPLE_INTERVAL;
        } else {
            let delta = if current < previous {
                previous - current
            } else {
                period - current + previous
            };
            elapsed = elapsed.saturating_add(delta);
            if elapsed >= budget {
                return false;
            }
            stalled = 0;
        }
        previous = current;
    }
    false
}

fn transfer_len(
    lba: u64,
    count: u32,
    blocks: u64,
    block_size: usize,
    buffer_len: usize,
) -> Result<usize, i32> {
    if count == 0 {
        return Ok(0);
    }
    let len = (count as usize)
        .checked_mul(block_size)
        .ok_or(BLOCK_DEV_ERR_BOUNDS)?;
    if lba
        .checked_add(u64::from(count))
        .is_none_or(|end| end > blocks)
        || len > buffer_len
    {
        return Err(BLOCK_DEV_ERR_BOUNDS);
    }
    Ok(len)
}

fn read_reg(base: usize, offset: usize) -> u32 {
    unsafe { core::ptr::read_volatile((base + offset) as *const u32) }
}

fn write_reg(base: usize, offset: usize, value: u32) {
    unsafe { core::ptr::write_volatile((base + offset) as *mut u32, value) }
}

fn write_addr(base: usize, offset: usize, addr: usize) {
    write_reg(base, offset, addr as u32);
    write_reg(base, offset + 4, (addr as u64 >> 32) as u32);
}

fn map_bar(low: u32, high: u32, bytes: usize) -> Result<usize, &'static str> {
    // Only assigned PCI memory BARs in the canonical identity-mapped range.
    let addr = match low & 7 {
        0 => u64::from(low & !15),
        4 => (u64::from(high) << 32) | u64::from(low & !15),
        _ => return Err("Unsupported storage PCI BAR"),
    };
    if addr == 0
        || addr
            .checked_add(bytes as u64)
            .is_none_or(|end| end >= (1 << 47))
    {
        return Err("Invalid storage PCI BAR address");
    }
    crate::mm::vmm::map_mmio_range(addr, bytes)?;
    Ok(addr as usize)
}
