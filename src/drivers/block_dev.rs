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
        crate::drivers::serial::print("BlockDev: Registered '");
        crate::drivers::serial::print(dev.name());
        crate::drivers::serial::print("', Blocks=");
        crate::drivers::serial::print_u64(dev.total_blocks());
        crate::drivers::serial::println("");
        reg.devices[idx] = Some(dev);
        reg.count += 1;
    }
}

pub fn count() -> usize {
    REGISTRY.lock().count
}
