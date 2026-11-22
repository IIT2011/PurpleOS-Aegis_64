use crate::compositor::COMPOSITOR;
use crate::kernel::{apapplication, apexec, posix, task, uaccess};
use crate::mm::vmm::{PAGE_NO_EXECUTE, PAGE_WRITABLE};
use crate::security::aegis_detect::{self, AppSigner};
use crate::sync::Spinlock;
use crate::mm::pmm::PageBuffer;

const POLICY_PATH: &str = "/System/Applications/trust.policy";
const POLICY_TAG: &[u8] = b"PurpleOS/APP-TRUST/v1\0";
const POLICY_LIMIT: usize = 16 * 1024;
const BUNDLE_LIMIT: usize = apapplication::MAX_BUNDLE_BYTES;
const CONSOLE: u32 = 1;
const SHELL: u32 = 4;
const ALLOWED_CAPABILITIES: u32 = apapplication::GUI | CONSOLE | SHELL;
const MAX_PRIORITY: u32 = task::TaskPriority::Normal as u32;
const PAGE: u64 = crate::mm::pmm::PAGE_SIZE as u64;
const STARTUP_ADDRESS: u64 = apexec::RESOURCE_BASE;
const APP_DIR: &str = "System/Applications/";
const APP_EXT: &str = ".apapplication";
const MAX_ID: usize = 55 - APP_DIR.len() - APP_EXT.len();

pub const ABI_VERSION: u64 = 1;

pub const SYS_ABI_VERSION: u64 = 0x1100;
pub const SYS_STARTUP: u64 = 0x1101;
pub const SYS_WINDOW_CREATE: u64 = 0x1102;
pub const SYS_WINDOW_CLOSE: u64 = 0x1103;
pub const SYS_WINDOW_PRESENT: u64 = 0x1104;
pub const SYS_WINDOW_POLL: u64 = 0x1105;
pub const SYS_WINDOW_SIZE: u64 = 0x1106;
pub const SYS_CONSOLE_WRITE: u64 = 0x1107;
pub const SYS_MEMORY_MAP: u64 = 0x1108;
pub const SYS_MEMORY_UNMAP: u64 = 0x1109;
pub const SYS_SHELL_COMMAND: u64 = 0x110a;
pub const SYS_SHELL_PROMPT: u64 = 0x110b;
#[cfg(feature = "development-sudo")]
const SYS_TEST_SUDO: u64 = 0x110d;

#[cfg(all(feature = "development-sudo", any(not(debug_assertions), feature = "signed-components")))]
compile_error!("development-sudo is restricted to unprotected debug kernels");
pub const SYS_WINDOW_GLASS: u64 = 0x110c;

static POLICY: Spinlock<Option<PageBuffer>> = Spinlock::new(None);
const MAX_LAUNCHERS: usize = 32;
static LAUNCHERS: Spinlock<[Option<AppId>; MAX_LAUNCHERS]> = Spinlock::new([None; MAX_LAUNCHERS]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppId {
    bytes: [u8; MAX_ID],
    len: usize,
}

impl AppId {
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
}

pub fn launcher_page<const N: usize>(page: usize) -> ([Option<AppId>; N], bool) {
    let entries = LAUNCHERS.lock();
    let start = page.saturating_mul(N);
    let mut result = [None; N];
    for (offset, entry) in result.iter_mut().enumerate() {
        *entry = entries.get(start.saturating_add(offset)).copied().flatten();
    }
    (
        result,
        entries
            .get(start.saturating_add(N))
            .is_some_and(Option::is_some),
    )
}

fn snapshot(path: &str, limit: usize) -> Result<PageBuffer, &'static str> {
    crate::kernel::storage::with_fs(|fs| {
        let inode = fs
            .lookup(path.trim_start_matches('/'))
            .ok_or("File not found")?;
        let size = fs.get_file_size(inode).ok_or("Could not read file size")?;
        if size == 0 || size > limit {
            return Err("File exceeds runtime limits");
        }
        let mut bytes = PageBuffer::new(size).ok_or("Out of memory")?;
        if fs.read_file(inode, &mut bytes) != size {
            return Err("Incomplete file read");
        }
        Ok(bytes)
    })
    .unwrap_or(Err("System filesystem unavailable"))
}

fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn policy_entry(line: &str) -> Option<(&str, [u8; 32], u32, u32)> {
    let mut fields = line.split_ascii_whitespace();
    let id = fields
        .next()
        .filter(|id| apapplication::valid_id(id) && id.len() <= MAX_ID)?;
    let key = fields.next()?.as_bytes();
    let capabilities = u32::from_str_radix(fields.next()?, 16).ok()?;
    let priority = fields.next()?.parse().ok()?;
    if key.len() != 64
        || capabilities & !ALLOWED_CAPABILITIES != 0
        || !(1..=MAX_PRIORITY).contains(&priority)
        || fields.next().is_some()
    {
        return None;
    }
    let mut public_key = [0; 32];
    for (slot, pair) in public_key.iter_mut().zip(key.chunks_exact(2)) {
        *slot = (hex(pair[0])? << 4) | hex(pair[1])?;
    }
    Some((id, public_key, capabilities, priority))
}

/// Load a boot-scoped trust snapshot. Missing or invalid policy disables launch.
pub fn init() {
    let Ok(bytes) = snapshot(POLICY_PATH, POLICY_LIMIT) else {
        return;
    };
    let Ok(text) = core::str::from_utf8(&bytes) else {
        return;
    };
    let mut lines = text.lines();
    if lines.next() != Some("APTRUST1")
        || !lines.all(|line| policy_entry(line).is_some())
        || !aegis_detect::verify_control_file(POLICY_PATH, POLICY_TAG, &bytes)
    {
        crate::drivers::serial::println("AegisDetect: GUI application trust policy rejected");
        return;
    }
    let mut catalog = [None; MAX_LAUNCHERS];
    let mut count = 0;
    for (id, _, mask, _) in text.lines().skip(1).filter_map(policy_entry) {
        if count == catalog.len() {
            break;
        }
        if mask & apapplication::GUI == 0
            || catalog[..count]
                .iter()
                .flatten()
                .any(|entry: &AppId| entry.as_str() == id)
        {
            continue;
        }
        let Ok(path) = application_path(id) else {
            continue;
        };
        if crate::fs::vfs::file_size(&path).is_err() {
            continue;
        }
        let mut entry = AppId {
            bytes: [0; MAX_ID],
            len: id.len(),
        };
        entry.bytes[..id.len()].copy_from_slice(id.as_bytes());
        catalog[count] = Some(entry);
        count += 1;
    }
    *LAUNCHERS.lock() = catalog;
    *POLICY.lock() = Some(bytes);
    crate::drivers::serial::println("AegisDetect: GUI application trust policy authenticated");
}

fn signer(id: &str, key_id: &[u8; 32]) -> Option<([u8; 32], u32, u32)> {
    let policy = POLICY.lock();
    let text = core::str::from_utf8(policy.as_deref()?).ok()?;
    let mut matches = text
        .lines()
        .skip(1)
        .filter_map(policy_entry)
        .filter(|record| record.0 == id && crate::crypto::sha256::hash(&record.1) == *key_id);
    let first = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some((first.1, first.2, first.3))
}

fn application_path(id: &str) -> Result<alloc::string::String, &'static str> {
    if !apapplication::valid_id(id) || id.len() > MAX_ID {
        return Err("Invalid application ID");
    }
    let mut path = alloc::string::String::new();
    path.try_reserve_exact(APP_DIR.len() + id.len() + APP_EXT.len())
        .map_err(|_| "Out of memory")?;
    path.push_str(APP_DIR);
    path.push_str(id);
    path.push_str(APP_EXT);
    Ok(path)
}

pub fn launch(id: &str) -> Result<u32, &'static str> {
    let path = application_path(id)?;
    let bytes = snapshot(&path, BUNDLE_LIMIT)?;
    let limits = {
        let engine = COMPOSITOR.lock();
        (engine.width as u32, engine.height as u32)
    };
    let bundle = apapplication::Bundle::parse(&bytes, BUNDLE_LIMIT, limits)
        .map_err(|_| "Malformed application bundle")?;
    let (key, mask, priority) =
        signer(id, bundle.key_id).ok_or("Application signer is not trusted")?;
    let policy = [AppSigner {
        public_key: &key,
        app_id: id,
        scope: apapplication::Scope::System,
        capability_ceiling: mask,
        priority_ceiling: priority,
    }];
    let app = aegis_detect::verify_application(
        &bytes,
        id,
        apapplication::Scope::System,
        &policy,
        (ALLOWED_CAPABILITIES, MAX_PRIORITY),
        BUNDLE_LIMIT,
        limits,
    )
    .map_err(|_| "AegisDetect rejected the application")?;
    if app.bundle.capabilities & apapplication::GUI == 0 {
        return Err("A GUI capability is required");
    }

    let space = apexec::AddressSpace::new(&app.image).ok_or("Unable to map executable")?;
    let stack_base = apexec::STACK_TOP - app.image.stack_size;
    space
        .map_data(
            stack_base,
            app.image.stack_size,
            &[],
            PAGE_WRITABLE | PAGE_NO_EXECUTE,
        )
        .ok_or("Unable to map user stack")?;
    let mut cursor = STARTUP_ADDRESS + PAGE;
    let mut map_section = |bytes: &[u8]| -> Result<(u64, u64), &'static str> {
        if bytes.is_empty() {
            return Ok((0, 0));
        }
        let address = cursor;
        let size = bytes.len() as u64;
        cursor = cursor
            .checked_add(size.next_multiple_of(PAGE))
            .ok_or("Invalid resource range")?;
        if cursor > stack_base - PAGE {
            return Err("Resource section exceeds address space");
        }
        space
            .map_data(address, size, bytes, PAGE_NO_EXECUTE)
            .ok_or("Unable to map application section")?;
        Ok((address, size))
    };
    let (resources, resources_len) = map_section(app.bundle.resources)?;
    let (metadata, metadata_len) = map_section(app.bundle.metadata)?;

    let mut compositor = COMPOSITOR.lock();
    let window = compositor
        .create_client_window(
            None,
            app.bundle.name,
            app.bundle.width as usize,
            app.bundle.height as usize,
        )
        .ok_or("Unable to allocate initial window")?;
    let committed = (|| {
        let fields = [
            ABI_VERSION,
            window as u64,
            u64::from(app.bundle.width),
            u64::from(app.bundle.height),
            resources,
            resources_len,
            metadata,
            metadata_len,
        ];
        let mut record = [0; 64];
        for (field, slot) in fields.into_iter().zip(record.chunks_exact_mut(8)) {
            slot.copy_from_slice(&field.to_le_bytes());
        }
        space.map_data(STARTUP_ADDRESS, PAGE, &record, PAGE_NO_EXECUTE)?;
        space.install(
            app.bundle.name,
            task::TaskPriority::from_u32(app.bundle.priority),
            (app.image.entry, apexec::STACK_TOP - 16),
            task::NativeContext {
                capabilities: app.bundle.capabilities,
            },
        )
    })();
    let Some(id) = committed else {
        compositor.close_window(window);
        return Err("Unable to start application task");
    };
    if let Some(win) = compositor.get_window_mut(window) {
        win.owner = Some(id);
        win.client_event([crate::compositor::client::EVENT_FOCUS, 1, 0, 0]);
    }
    Ok(id)
}

pub fn dispatch(
    owner: u32,
    native: task::NativeContext,
    number: u64,
    a: u64,
    b: u64,
    c: u64,
) -> i64 {
    use crate::compositor::client;
    if native.capabilities & apapplication::GUI == 0 {
        return -posix::EPERM;
    }
    let result = match number {
        crate::kernel::syscall::SYS_EXIT | crate::kernel::syscall::SYS_EXIT_GROUP => {
            let mut engine = COMPOSITOR.lock();
            let mut to_close = [0; crate::compositor::MAX_WINDOWS];
            let mut count = 0;
            for win in engine.windows.iter().flatten() {
                if win.owner == Some(owner) {
                    to_close[count] = win.id;
                    count += 1;
                }
            }
            for &id in &to_close[..count] {
                engine.close_window(id);
            }
            drop(engine);
            task::exit()
        }
        crate::kernel::syscall::SYS_SCHED_YIELD => {
            task::yield_now();
            Ok(0)
        }
        crate::kernel::syscall::SYS_GETPID => Ok(i64::from(owner)),
        SYS_ABI_VERSION => Ok(ABI_VERSION as i64),
        SYS_STARTUP => Ok(STARTUP_ADDRESS as i64),
        SYS_WINDOW_CREATE => client::create(owner, a, b, c),
        SYS_WINDOW_CLOSE => client::close(owner, a),
        SYS_WINDOW_PRESENT => client::present(owner, a, b, c),
        SYS_WINDOW_POLL => client::poll(owner, a, b),
        SYS_WINDOW_SIZE => client::size(owner, a),
        SYS_WINDOW_GLASS => client::set_glass(owner, a, b),
        SYS_CONSOLE_WRITE if native.capabilities & CONSOLE != 0 => write_console(a, b),
        SYS_MEMORY_MAP => map_memory(a),
        SYS_MEMORY_UNMAP => unmap_memory(a),
        SYS_SHELL_COMMAND if native.capabilities & SHELL != 0 => shell_command(owner, a, b, c),
        SYS_SHELL_PROMPT if native.capabilities & SHELL != 0 => shell_prompt(owner, a, b),
        #[cfg(feature = "development-sudo")]
        SYS_TEST_SUDO if native.capabilities & SHELL != 0 => test_sudo(owner, a, b, c),
        _ => Err(-posix::EPERM),
    };
    result.unwrap_or_else(|error| error)
}

fn map_memory(length: u64) -> Result<i64, i64> {
    if length == 0 || length > apexec::HEAP_SIZE {
        return Err(-posix::EINVAL);
    }
    let length = length.next_multiple_of(PAGE);
    let mut scheduler = task::SCHED.lock();
    let index = scheduler.current[task::get_current_cpu()].ok_or(-posix::ESRCH)?;
    let task = &mut scheduler.tasks[index];
    if task.native_heap.len() >= task::NATIVE_MAPPINGS {
        return Err(-posix::ENOSPC);
    }
    task.native_heap.try_reserve(1).map_err(|_| -posix::ENOMEM)?;
    let limit = apexec::HEAP_BASE + apexec::HEAP_SIZE;
    let mut address = apexec::HEAP_BASE;
    loop {
        let end = address + length;
        if end > limit {
            return Err(-posix::ENOMEM);
        }
        let overlap = task
            .native_heap
            .iter()
            .filter(|&&(base, size)| address < base + size && base < end)
            .map(|&(base, size)| base + size)
            .max();
        let Some(after) = overlap else {
            break;
        };
        address = after;
    }
    apexec::map_private(
        task.cr3 as *mut _,
        address,
        length,
        &[],
        PAGE_WRITABLE | PAGE_NO_EXECUTE,
    )
    .ok_or(-posix::ENOMEM)?;
    task.native_heap.push((address, length));
    Ok(address as i64)
}

fn unmap_memory(address: u64) -> Result<i64, i64> {
    let mut scheduler = task::SCHED.lock();
    let index = scheduler.current[task::get_current_cpu()].ok_or(-posix::ESRCH)?;
    let task = &mut scheduler.tasks[index];
    let index = task
        .native_heap
        .iter()
        .position(|&(base, _)| base == address)
        .ok_or(-posix::EINVAL)?;
    let (_, length) = task.native_heap.swap_remove(index);
    crate::mm::vmm::free_user_range(task.cr3 as *mut _, address, address + length);
    Ok(0)
}

fn write_console(source: u64, length: u64) -> Result<i64, i64> {
    let mut bytes = [0; 256];
    let size = usize::try_from(length).map_err(|_| -posix::EINVAL)?;
    let bytes = bytes.get_mut(..size).ok_or(-posix::EINVAL)?;
    uaccess::copy_from_user(source, bytes)?;
    let text = core::str::from_utf8(bytes).map_err(|_| -posix::EINVAL)?;
    if text
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(-posix::EINVAL);
    }
    crate::drivers::serial::print(text);
    Ok(size as i64)
}

fn shell_command(owner: u32, source: u64, length: u64, destination: u64) -> Result<i64, i64> {
    let mut input = [0; 240];
    let input = input.get_mut(..length as usize).ok_or(-posix::EINVAL)?;
    uaccess::copy_from_user(source, input)?;
    let line = core::str::from_utf8(input)
        .map_err(|_| -posix::EINVAL)?
        .trim();
    if !line
        .bytes()
        .all(|byte| (b' '..=b'~').contains(&byte) && !b"&;<".contains(&byte))
    {
        return Err(-posix::EINVAL);
    }
    let capacity = crate::drivers::serial::CAPTURE_LIMIT;
    uaccess::validate_user_range(destination, capacity, true)?;
    let mut output = alloc::vec::Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| -posix::ENOMEM)?;
    output.resize(capacity, 0);
    let length = crate::kernel::shell::gui_command(owner, line, &mut output);
    uaccess::copy_to_user(&output[..length], destination)?;
    Ok(length as i64)
}

#[cfg(feature = "development-sudo")]
fn test_sudo(owner: u32, source: u64, length: u64, destination: u64) -> Result<i64, i64> {
    const PASSWORD: &[u8] = b"12345";
    let mut request = [0u8; 245];
    let request = request.get_mut(..length as usize).ok_or(-posix::EINVAL)?;
    uaccess::copy_from_user(source, request)?;
    let (password, command) = request.split_at_checked(PASSWORD.len()).ok_or(-posix::EINVAL)?;
    if password != PASSWORD {
        return Err(-posix::EACCES);
    }
    let command = core::str::from_utf8(command).map_err(|_| -posix::EINVAL)?;
    let command = command.trim();
    if command.is_empty()
        || !command.bytes().all(|byte| (b' '..=b'~').contains(&byte) && !b"&;<>|".contains(&byte))
    {
        return Err(-posix::EINVAL);
    }
    let capacity = crate::drivers::serial::CAPTURE_LIMIT;
    uaccess::validate_user_range(destination, capacity, true)?;
    let mut output = alloc::vec::Vec::new();
    output.try_reserve_exact(capacity).map_err(|_| -posix::ENOMEM)?;
    output.resize(capacity, 0);
    let size = crate::kernel::shell::gui_sudo_command(owner, command, &mut output);
    uaccess::copy_to_user(&output[..size], destination)?;
    Ok(size as i64)
}

fn shell_prompt(owner: u32, destination: u64, capacity: u64) -> Result<i64, i64> {
    let mut prompt = [0; crate::kernel::shell::GUI_PROMPT_CAPACITY];
    let length = crate::kernel::shell::gui_prompt(owner, &mut prompt);
    if capacity < length as u64 {
        return Err(-posix::ENOSPC);
    }
    uaccess::copy_to_user(&prompt[..length], destination)?;
    Ok(length as i64)
}
