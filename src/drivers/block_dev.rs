use crate::sync::Spinlock;

pub const BLOCK_DEV_OK: i32 = 0;
pub const BLOCK_DEV_ERR_IO: i32 = -1;
pub const BLOCK_DEV_ERR_BOUNDS: i32 = -2;

pub trait BlockDevice: Send + Sync {
    fn name(&self) -> &str;
    fn block_size(&self) -> u32;
    fn total_blocks(&self) -> u64;
    fn read_blocks(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> i32;
    fn write_blocks(&mut self, lba: u64, count: u32, buf: &[u8]) -> i32;
    fn flush(&mut self) -> i32 {
        BLOCK_DEV_OK
    }
}

pub struct BlockDevRegistry {
    devices: [Option<&'static mut dyn BlockDevice>; 16],
    count: usize,
}

pub static REGISTRY: Spinlock<BlockDevRegistry> = Spinlock::new(BlockDevRegistry {
    devices: [None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None],
    count: 0,
});

pub fn register(dev: &'static mut dyn BlockDevice) {
    let mut reg = REGISTRY.lock();
    if reg.count < reg.devices.len() {
        let idx = reg.count;
        let dname = dev.name().trim_matches(char::from(0)).trim();
        if dname == "sata0" {
            crate::drivers::serial::println("BlockDev: Registered 'sata0' [AHCI/SATA]");
        } else if dname == "nvme0n1" {
            crate::drivers::serial::println("BlockDev: Registered 'nvme0n1' [NVMe PCIe]");
        } else {
            crate::drivers::serial::print("BlockDev: Registered '");
            crate::drivers::serial::print(dname);
            crate::drivers::serial::print("', Blocks=");
            crate::drivers::serial::print_u64(dev.total_blocks());
            crate::drivers::serial::println("");
        }
        reg.devices[idx] = Some(dev);
        reg.count += 1;
    }
}

pub fn count() -> usize {
    REGISTRY.lock().count
}

pub fn with_dev<F, R>(idx: usize, f: F) -> Option<R>
where
    F: FnOnce(&mut dyn BlockDevice) -> R,
{
    let mut reg = REGISTRY.lock();
    if idx < reg.count {
        if let Some(ref mut d) = reg.devices[idx] {
            return Some(f(*d));
        }
    }
    None
}

pub fn for_each_dev<F: FnMut(usize, &dyn BlockDevice)>(mut f: F) {
    let reg = REGISTRY.lock();
    for i in 0..reg.count {
        if let Some(ref d) = reg.devices[i] {
            f(i, *d);
        }
    }
}
