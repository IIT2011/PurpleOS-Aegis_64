use core::ops::Range;

const HEADER_SIZE: usize = 64;
const SEGMENT_SIZE: usize = 40;
const PAGE_SIZE: u64 = 4096;
// Use a private PML4 slot, separate from the kernel's low identity mappings.
pub const USER_MIN: u64 = 1 << 39;
pub const STACK_TOP: u64 = 0x0000_7fff_f000_0000;
const MAX_STACK: u64 = 1024 * 1024;
pub const RESOURCE_BASE: u64 = STACK_TOP - MAX_STACK - 2 * 1024 * 1024;
pub const HEAP_SIZE: u64 = 32 * 1024 * 1024;
pub const HEAP_BASE: u64 = RESOURCE_BASE - HEAP_SIZE - PAGE_SIZE;
const MAX_IMAGE: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Format,
    Unsupported,
    Layout,
    Permissions,
    Checksum,
}

pub struct Segment<'a> {
    pub address: u64,
    pub memory_size: u64,
    pub flags: u32,
    pub data: &'a [u8],
}

impl Segment<'_> {
    fn page_range(&self) -> Range<u64> {
        self.address..self.address + self.memory_size.div_ceil(PAGE_SIZE) * PAGE_SIZE
    }
}

pub struct Image<'a> {
    pub entry: u64,
    pub stack_size: u64,
    pub segments: [Option<Segment<'a>>; 16],
}

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn wide(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

impl<'a> Image<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Error> {
        let header = bytes.get(..HEADER_SIZE).ok_or(Error::Format)?;
        if &header[..8] != b"\x7fAPE\x01\x01\x64\x86" {
            return Err(Error::Unsupported);
        }
        let count = u16::from_le_bytes([header[36], header[37]]) as usize;
        if word(header, 8) as usize != HEADER_SIZE || wide(header, 28) != bytes.len() as u64
            || !(1..=16).contains(&count) || header[38..40] != [0; 2]
            || header[48..64] != [0; 16]
        {
            return Err(Error::Format);
        }

        let table_end = HEADER_SIZE + count * SEGMENT_SIZE;
        let table = bytes.get(HEADER_SIZE..table_end).ok_or(Error::Format)?;
        let stack_size = match word(header, 40) {
            0 => 64 * 1024,
            size => u64::from(size).div_ceil(PAGE_SIZE) * PAGE_SIZE,
        };
        if stack_size > MAX_STACK {
            return Err(Error::Layout);
        }
        let image_limit = HEAP_BASE - PAGE_SIZE;
        let entry = wide(header, 12);
        let base = wide(header, 20);
        let mut lowest = u64::MAX;
        let mut total_mapped = 0;
        let mut executable_entry = false;
        let mut segments: [Option<Segment<'a>>; 16] = [const { None }; 16];
        let mut payload_cursor = table_end;

        for (index, raw) in table.chunks_exact(SEGMENT_SIZE).enumerate() {
            let kind = word(raw, 0);
            let flags = word(raw, 4);
            let expected_flags = match kind {
                1 => 5,
                2 => 1,
                3 | 4 => 3,
                _ => return Err(Error::Format),
            };
            if flags != expected_flags {
                return Err(Error::Permissions);
            }
            let offset = usize::try_from(wide(raw, 8)).map_err(|_| Error::Layout)?;
            let size = usize::try_from(wide(raw, 16)).map_err(|_| Error::Layout)?;
            let address = wide(raw, 24);
            let memory_size = wide(raw, 32);
            if memory_size == 0 || memory_size > MAX_IMAGE || size as u64 > memory_size
                || address < USER_MIN || !address.is_multiple_of(PAGE_SIZE)
                || address.checked_add(memory_size.div_ceil(PAGE_SIZE) * PAGE_SIZE)
                    .map_or(true, |end| end > image_limit)
            {
                return Err(Error::Layout);
            }
            let data = if kind == 4 {
                if offset != 0 || size != 0 {
                    return Err(Error::Layout);
                }
                &bytes[..0]
            } else {
                if offset != payload_cursor || size == 0 {
                    return Err(Error::Layout);
                }
                payload_cursor = offset.checked_add(size).ok_or(Error::Layout)?;
                bytes.get(offset..payload_cursor).ok_or(Error::Layout)?
            };
            let segment = Segment { address, memory_size, flags, data };
            let range = segment.page_range();
            if segments.iter().flatten().any(|other| {
                let other = other.page_range();
                range.start < other.end && other.start < range.end
            }) {
                return Err(Error::Layout);
            }
            total_mapped += range.end - range.start;
            if total_mapped > MAX_IMAGE {
                return Err(Error::Layout);
            }
            lowest = lowest.min(address);
            executable_entry |= kind == 1 && (address..address + size as u64).contains(&entry);
            segments[index] = Some(segment);
        }
        if !executable_entry || lowest != base || payload_cursor != bytes.len() {
            return Err(Error::Layout);
        }
        if crc32(&bytes[table_end..]) != word(header, 44) {
            return Err(Error::Checksum);
        }
        Ok(Self { entry, stack_size, segments })
    }
}

pub fn init() {
    crate::drivers::serial::println("APEXEC: Native image validation available");
}

/// Construct a private address space from an already validated APEXEC image.
pub struct AddressSpace(*mut crate::mm::vmm::PageTable);

impl AddressSpace {
    pub fn new(image: &Image<'_>) -> Option<Self> {
        use crate::mm::vmm::{create_pml4, PAGE_NO_EXECUTE, PAGE_WRITABLE};
        let space = Self(create_pml4()?);
        for segment in image.segments.iter().flatten() {
            let mut flags = 0;
            if segment.flags & 2 != 0 { flags |= PAGE_WRITABLE; }
            if segment.flags & 4 == 0 { flags |= PAGE_NO_EXECUTE; }
            space.map_data(segment.address, segment.memory_size, segment.data, flags)?;
        }
        Some(space)
    }

    pub fn map_data(&self, address: u64, length: u64, bytes: &[u8], flags: u64) -> Option<()> {
        map_private(self.0, address, length, bytes, flags)
    }

    pub fn install(
        self, name: &str, priority: crate::kernel::task::TaskPriority, point: (u64, u64),
        context: crate::kernel::task::NativeContext,
    ) -> Option<u32> {
        let stack = crate::mm::pmm::alloc_frames(4)?;
        let pml4 = self.0;
        core::mem::forget(self);
        crate::kernel::task::install_user_task(name, priority, pml4, stack, point, Some(context))
    }
}

pub(crate) fn map_private(
    pml4: *mut crate::mm::vmm::PageTable, address: u64, length: u64, bytes: &[u8], flags: u64,
) -> Option<()> {
    use crate::mm::{pmm, vmm};
    for offset in (0..length).step_by(PAGE_SIZE as usize) {
        let result = (|| {
            let page = pmm::alloc_frame()?;
            let initialized = bytes.get(offset as usize..).unwrap_or(&[]);
            let count = initialized.len().min(PAGE_SIZE as usize);
            unsafe {
                let destination = page as *mut u8;
                destination.write_bytes(0, PAGE_SIZE as usize);
                core::ptr::copy_nonoverlapping(initialized.as_ptr(), destination, count);
            }
            if vmm::map_page(pml4, address + offset, page as u64, flags | vmm::PAGE_USER).is_err() {
                let _ = pmm::free_frame(page);
                return None;
            }
            Some(())
        })();
        if result.is_none() {
            vmm::free_user_range(pml4, address, address + offset);
            return None;
        }
    }
    Some(())
}

impl Drop for AddressSpace {
    fn drop(&mut self) {
        crate::mm::vmm::destroy_pml4(self.0);
    }
}
