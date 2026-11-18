//! PurpleOS Search Path Database Subsystem (/System/CLI/path.dtbase)
//!
//! Provides dynamic search path configuration for executables, applications, and scripts.
//! Unlike `cmd.dtbase`, this file deliberately omits cryptographic signature checks
//! so that users and system management scripts can configure search directories frequently.

use crate::drivers::serial;
use crate::sync::Spinlock;

pub const PATH_DATABASE_PATH: &str = "/System/CLI/path.dtbase";
pub const FALLBACK_PATH_DATABASE: &str = "path.dtbase";

pub const MAX_SEARCH_PATHS: usize = 16;
pub const MAX_PATH_LEN: usize = 64;

#[derive(Copy, Clone)]
pub struct PathEntry {
    pub path: [u8; MAX_PATH_LEN],
    pub len: u8,
}

impl PathEntry {
    pub const fn empty() -> Self {
        Self {
            path: [0; MAX_PATH_LEN],
            len: 0,
        }
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.path[..self.len as usize]).unwrap_or("")
    }
}

pub struct PathTable {
    pub paths: [PathEntry; MAX_SEARCH_PATHS],
    pub count: usize,
    pub initialized: bool,
}

impl PathTable {
    pub const fn new() -> Self {
        Self {
            paths: [PathEntry::empty(); MAX_SEARCH_PATHS],
            count: 0,
            initialized: false,
        }
    }
}

static PATH_TABLE: Spinlock<PathTable> = Spinlock::new(PathTable::new());
static PATH_SCRATCH: Spinlock<[u8; 4096]> = Spinlock::new([0u8; 4096]);

/// Initialize search paths by reading `/System/CLI/path.dtbase` from storage.
/// No security checks are performed on this file as per system specification.
pub fn init() -> bool {
    let mut scratch = PATH_SCRATCH.lock();
    scratch.fill(0);

    let read_bytes = match crate::fs::vfs::read(PATH_DATABASE_PATH, &mut *scratch) {
        Ok(b) if b > 0 => b,
        _ => match crate::fs::vfs::read(FALLBACK_PATH_DATABASE, &mut *scratch) {
            Ok(b) if b > 0 => b,
            _ => {
                serial::print("[PATHDB] Notice: Path database '");
                serial::print(PATH_DATABASE_PATH);
                serial::println("' not found; using fallback root paths.");
                setup_fallback_paths();
                return false;
            }
        },
    };

    parse_paths(&scratch[..read_bytes])
}

fn setup_fallback_paths() {
    let mut table = PATH_TABLE.lock();
    table.count = 0;
    let defaults = ["/System/Applications", "/System/CLI/scripts", "/System/Drivers", "/tmp", "/"];
    for &d in &defaults {
        if table.count < MAX_SEARCH_PATHS {
            let idx = table.count;
            let b = d.as_bytes();
            let len = b.len().min(MAX_PATH_LEN);
            table.paths[idx].path[..len].copy_from_slice(&b[..len]);
            table.paths[idx].len = len as u8;
            table.count += 1;
        }
    }
    table.initialized = true;
}

fn parse_paths(data: &[u8]) -> bool {
    let mut table = PATH_TABLE.lock();
    table.count = 0;

    let mut line_start = 0usize;
    while line_start < data.len() {
        let mut line_end = line_start;
        while line_end < data.len() && data[line_end] != b'\n' {
            line_end += 1;
        }

        let line = &data[line_start..line_end];
        let trimmed = trim_slice(line);

        if !trimmed.starts_with(b"#") && !trimmed.is_empty() {
            let raw_path = if trimmed.starts_with(b"path = ") {
                trim_slice(&trimmed[7..])
            } else if trimmed.starts_with(b"path=") {
                trim_slice(&trimmed[5..])
            } else if trimmed.starts_with(b"/") {
                trimmed
            } else {
                b""
            };

            if !raw_path.is_empty() && table.count < MAX_SEARCH_PATHS {
                let idx = table.count;
                table.paths[idx] = PathEntry::empty();
                let len = raw_path.len().min(MAX_PATH_LEN);
                table.paths[idx].path[..len].copy_from_slice(&raw_path[..len]);
                table.paths[idx].len = len as u8;
                table.count += 1;
            }
        }

        line_start = if line_end < data.len() { line_end + 1 } else { line_end };
    }

    table.initialized = true;
    serial::print("[PATHDB] Loaded ");
    serial::print_u64(table.count as u64);
    serial::print(" search path(s) from ");
    serial::print(PATH_DATABASE_PATH);
    serial::println(" (Unrestricted User Configuration)");

    true
}

fn trim_slice(s: &[u8]) -> &[u8] {
    let mut start = 0;
    while start < s.len() && (s[start] == b' ' || s[start] == b'\t' || s[start] == b'\r') {
        start += 1;
    }
    let mut end = s.len();
    while end > start && (s[end - 1] == b' ' || s[end - 1] == b'\t' || s[end - 1] == b'\r') {
        end -= 1;
    }
    &s[start..end]
}

/// Hot-reload search paths from `/System/CLI/path.dtbase`.
pub fn reload() -> Result<usize, &'static str> {
    if !init() {
        return Err("Failed to reload /System/CLI/path.dtbase from storage");
    }
    Ok(PATH_TABLE.lock().count)
}

/// Return total count of search paths.
pub fn count() -> usize {
    PATH_TABLE.lock().count
}

/// Iterate over all registered search paths.
pub fn for_each_path<F: FnMut(&str)>(mut f: F) {
    let table = PATH_TABLE.lock();
    for i in 0..table.count {
        f(table.paths[i].as_str());
    }
}

/// Check if a directory is present in the search paths.
pub fn contains_path(path: &str) -> bool {
    let table = PATH_TABLE.lock();
    for i in 0..table.count {
        if table.paths[i].as_str() == path {
            return true;
        }
    }
    false
}

/// Add a new directory to the in-memory search paths.
pub fn add_path(p: &str) -> Result<(), &'static str> {
    let clean = p.trim();
    if clean.is_empty() {
        return Err("Path cannot be empty");
    }
    let mut table = PATH_TABLE.lock();
    for i in 0..table.count {
        if table.paths[i].as_str() == clean {
            return Ok(()); // Already present
        }
    }
    if table.count >= MAX_SEARCH_PATHS {
        return Err("Maximum search paths reached");
    }
    let idx = table.count;
    table.paths[idx] = PathEntry::empty();
    let b = clean.as_bytes();
    let len = b.len().min(MAX_PATH_LEN);
    table.paths[idx].path[..len].copy_from_slice(&b[..len]);
    table.paths[idx].len = len as u8;
    table.count += 1;
    Ok(())
}

/// Remove a directory from the in-memory search paths.
pub fn remove_path(p: &str) -> Result<(), &'static str> {
    let clean = p.trim();
    let mut table = PATH_TABLE.lock();
    let mut found_idx = None;
    for i in 0..table.count {
        if table.paths[i].as_str() == clean {
            found_idx = Some(i);
            break;
        }
    }
    if let Some(idx) = found_idx {
        for i in idx..(table.count - 1) {
            table.paths[i] = table.paths[i + 1];
        }
        table.count -= 1;
        Ok(())
    } else {
        Err("Path not found in search paths")
    }
}

/// Display all search paths from `/System/CLI/path.dtbase`.
pub fn print_paths() {
    let table = PATH_TABLE.lock();
    serial::println("=======================================================================");
    serial::println("  PurpleOS Search Path Database (/System/CLI/path.dtbase)");
    serial::println("=======================================================================");
    serial::print("  Database Path:     ");
    serial::println(PATH_DATABASE_PATH);
    serial::print("  Total Paths:       ");
    serial::print_u64(table.count as u64);
    serial::println("");
    serial::println("  Security Policy:   Unrestricted (Free User/Script Customization)");
    serial::println("  Active Search Hierarchy:");
    for i in 0..table.count {
        serial::print("    [");
        serial::print_u64((i + 1) as u64);
        serial::print("] ");
        serial::println(table.paths[i].as_str());
    }
    serial::println("=======================================================================");
}

/// Resolve a filename against the active search paths.
pub fn resolve_file<'a>(filename: &str, out_buf: &'a mut [u8; 128]) -> Option<&'a str> {
    let table = PATH_TABLE.lock();
    let mut scratch = [0u8; 128];
    for i in 0..table.count {
        let p = table.paths[i].as_str();
        scratch.fill(0);
        let p_bytes = p.as_bytes();
        let f_bytes = filename.as_bytes();
        let need_slash = !p.ends_with('/');
        let total_len = p_bytes.len() + (if need_slash { 1 } else { 0 }) + f_bytes.len();
        if total_len < scratch.len() {
            scratch[..p_bytes.len()].copy_from_slice(p_bytes);
            let mut off = p_bytes.len();
            if need_slash {
                scratch[off] = b'/';
                off += 1;
            }
            scratch[off..off + f_bytes.len()].copy_from_slice(f_bytes);
            if let Ok(candidate) = core::str::from_utf8(&scratch[..total_len]) {
                if crate::fs::vfs::file_size(candidate).is_ok() {
                    out_buf[..total_len].copy_from_slice(&scratch[..total_len]);
                    return core::str::from_utf8(&out_buf[..total_len]).ok();
                }
            }
        }
    }
    None
}
