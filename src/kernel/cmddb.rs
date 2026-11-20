//! PurpleOS / Aegis64 Command Database Subsystem (/System/CLI/cmd.dtbase)
//!
//! Provides dynamic, storage-backed command resolution, metadata introspection,
//! and cryptographic Creator Signature verification. Commands are loaded directly
//! from `/System/CLI/cmd.dtbase` on the filesystem rather than hardcoded in the kernel.
//!
//! Integrity is enforced via a Cryptographic Signature specifically signed by the
//! Creator (Aegis Root Developer) using RSA-2048 PKCS#1 v1.5 with SHA-256.

use crate::drivers::serial;
use crate::sync::Spinlock;

pub const CMD_DATABASE_PATH: &str = "/System/CLI/cmd.dtbase";
pub const FALLBACK_DATABASE_PATH: &str = "cmd.dtbase";

pub const CREATOR_SIGNER: &str = crate::security::aegis_detect::CREATOR_SIGNER;
pub const CREATOR_KEY_ID: &str = crate::security::aegis_detect::CREATOR_KEY_ID;

pub const MAX_COMMANDS: usize = 160;
pub const MAX_NAME_LEN: usize = 24;
pub const MAX_CAT_LEN: usize = 16;
pub const MAX_TARGET_LEN: usize = 24;
pub const MAX_DESC_LEN: usize = 64;
pub const MAX_USAGE_LEN: usize = 48;

#[derive(Copy, Clone)]
pub struct CmdRecord {
    pub name: [u8; MAX_NAME_LEN],
    pub name_len: u8,
    pub category: [u8; MAX_CAT_LEN],
    pub cat_len: u8,
    pub target: [u8; MAX_TARGET_LEN],
    pub target_len: u8,
    pub desc: [u8; MAX_DESC_LEN],
    pub desc_len: u8,
    pub usage: [u8; MAX_USAGE_LEN],
    pub usage_len: u8,
}

impl CmdRecord {
    pub const fn empty() -> Self {
        Self {
            name: [0; MAX_NAME_LEN],
            name_len: 0,
            category: [0; MAX_CAT_LEN],
            cat_len: 0,
            target: [0; MAX_TARGET_LEN],
            target_len: 0,
            desc: [0; MAX_DESC_LEN],
            desc_len: 0,
            usage: [0; MAX_USAGE_LEN],
            usage_len: 0,
        }
    }

    pub fn name(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len as usize]).unwrap_or("")
    }

    pub fn category(&self) -> &str {
        core::str::from_utf8(&self.category[..self.cat_len as usize]).unwrap_or("")
    }

    pub fn target(&self) -> &str {
        core::str::from_utf8(&self.target[..self.target_len as usize]).unwrap_or("")
    }

    pub fn desc(&self) -> &str {
        core::str::from_utf8(&self.desc[..self.desc_len as usize]).unwrap_or("")
    }

    pub fn usage(&self) -> &str {
        core::str::from_utf8(&self.usage[..self.usage_len as usize]).unwrap_or("")
    }

    pub fn is_ring3(&self) -> bool {
        self.target().starts_with("ring3:")
    }

    pub fn is_ring0(&self) -> bool {
        self.target().starts_with("ring0:")
    }
}

pub struct CmdTable {
    pub entries: [CmdRecord; MAX_COMMANDS],
    pub count: usize,
    pub signature_verified: bool,
    pub payload_hash: [u8; 32],
    pub signer_name: [u8; 32],
    pub signer_len: u8,
    pub payload_bytes: usize,
    pub initialized: bool,
}

impl CmdTable {
    pub const fn new() -> Self {
        Self {
            entries: [CmdRecord::empty(); MAX_COMMANDS],
            count: 0,
            signature_verified: false,
            payload_hash: [0; 32],
            signer_name: [0; 32],
            signer_len: 0,
            payload_bytes: 0,
            initialized: false,
        }
    }

    pub fn signer(&self) -> &str {
        core::str::from_utf8(&self.signer_name[..self.signer_len as usize]).unwrap_or("Creator")
    }
}

static CMD_TABLE: Spinlock<CmdTable> = Spinlock::new(CmdTable::new());
const MAX_DATABASE_SIZE: usize = 24_576;
static RAW_BUFFER: Spinlock<[u8; MAX_DATABASE_SIZE + 1]> =
    Spinlock::new([0u8; MAX_DATABASE_SIZE + 1]);

/// Initialize the command database by reading `/System/CLI/cmd.dtbase` from storage,
/// verifying its Cryptographic Creator Signature, and parsing all command records.
pub fn init() -> bool {
    let mut raw_buf = RAW_BUFFER.lock();
    raw_buf.fill(0);

    let read_bytes = read_database(&mut *raw_buf).unwrap_or(0);
    parse_and_verify(&raw_buf[..read_bytes])
}

fn read_database(buffer: &mut [u8]) -> Result<usize, &'static str> {
    let path = if crate::fs::vfs::file_size(CMD_DATABASE_PATH).is_ok() {
        CMD_DATABASE_PATH
    } else {
        FALLBACK_DATABASE_PATH
    };
    crate::fs::vfs::read(path, buffer)
}

/// Hot-reload the command database dynamically from `/System/CLI/cmd.dtbase`.
pub fn reload() -> Result<usize, &'static str> {
    if !init() {
        return Err("Failed to load /System/CLI/cmd.dtbase from storage");
    }
    let count = CMD_TABLE.lock().count;
    Ok(count)
}

fn parse_and_verify(data: &[u8]) -> bool {
    let mut table = CMD_TABLE.lock();
    table.count = 0;
    table.initialized = false;
    table.payload_bytes = 0;
    table.signature_verified = false;
    table.payload_hash.fill(0);
    table.signer_name.fill(0);
    table.signer_len = 0;

    if data.len() > MAX_DATABASE_SIZE {
        serial::println("[CMDDB] Rejected oversized command database.");
        return false;
    }

    let Some(payload) = crate::security::aegis_detect::verify_command_database(data) else {
        serial::println("[CMDDB] Rejected command database: missing or invalid Creator signature.");
        return false;
    };

    table.payload_bytes = payload.len();
    table.payload_hash = crate::crypto::sha256::hash(payload);
    table.signature_verified = true;
    let signer = CREATOR_SIGNER.as_bytes();
    let signer_len = signer.len().min(table.signer_name.len());
    table.signer_name[..signer_len].copy_from_slice(&signer[..signer_len]);
    table.signer_len = signer_len as u8;
    serial::println("[CMDDB] Authenticated /System/CLI/cmd.dtbase \u{2014} Cryptographically Signed by Creator!");

    // 3. Parse Command Records from payload
    let mut current_idx: Option<usize> = None;
    let data = payload;
    let mut line_start = 0;

    while line_start < data.len() {
        let mut line_end = line_start;
        while line_end < data.len() && data[line_end] != b'\n' {
            line_end += 1;
        }

        let line = &data[line_start..line_end];
        let trimmed = trim_slice(line);

        if trimmed.starts_with(b"[cmd:") && trimmed.ends_with(b"]") {
            if table.count < MAX_COMMANDS {
                let name_bytes = &trimmed[5..trimmed.len() - 1];
                let idx = table.count;
                table.entries[idx] = CmdRecord::empty();
                let len = name_bytes.len().min(MAX_NAME_LEN);
                table.entries[idx].name[..len].copy_from_slice(&name_bytes[..len]);
                table.entries[idx].name_len = len as u8;
                current_idx = Some(idx);
                table.count += 1;
            } else {
                current_idx = None;
            }
        } else if let Some(idx) = current_idx {
            if trimmed.starts_with(b"cat=") {
                let val = &trimmed[4..];
                let len = val.len().min(MAX_CAT_LEN);
                table.entries[idx].category[..len].copy_from_slice(&val[..len]);
                table.entries[idx].cat_len = len as u8;
            } else if trimmed.starts_with(b"target=") {
                let val = &trimmed[7..];
                let len = val.len().min(MAX_TARGET_LEN);
                table.entries[idx].target[..len].copy_from_slice(&val[..len]);
                table.entries[idx].target_len = len as u8;
            } else if trimmed.starts_with(b"desc=") {
                let val = &trimmed[5..];
                let len = val.len().min(MAX_DESC_LEN);
                table.entries[idx].desc[..len].copy_from_slice(&val[..len]);
                table.entries[idx].desc_len = len as u8;
            } else if trimmed.starts_with(b"usage=") {
                let val = &trimmed[6..];
                let len = val.len().min(MAX_USAGE_LEN);
                table.entries[idx].usage[..len].copy_from_slice(&val[..len]);
                table.entries[idx].usage_len = len as u8;
            }
        }

        line_start = if line_end < data.len() {
            line_end + 1
        } else {
            line_end
        };
    }

    table.initialized = true;
    serial::print("[CMDDB] Loaded ");
    serial::print_u64(table.count as u64);
    serial::println(" commands into kernel execution dispatcher.");

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

/// Lookup a command by name from the authenticated database.
pub fn lookup(name: &str) -> Option<CmdRecord> {
    let table = CMD_TABLE.lock();
    if !table.signature_verified {
        return None;
    }
    let name_b = name.as_bytes();
    for i in 0..table.count {
        let entry = &table.entries[i];
        if entry.name_len as usize == name_b.len()
            && &entry.name[..entry.name_len as usize] == name_b
        {
            return Some(*entry);
        }
    }
    None
}

/// Check if a command is defined in the database.
pub fn contains(name: &str) -> bool {
    lookup(name).is_some()
}

/// Return total count of loaded commands.
pub fn count() -> usize {
    CMD_TABLE.lock().count
}

/// Check if the database passed cryptographic Creator Signature verification.
pub fn is_verified() -> bool {
    CMD_TABLE.lock().signature_verified
}

/// Verify integrity of the command database currently in memory against on-disk file.
pub fn verify_integrity() -> bool {
    let mut raw_buf = RAW_BUFFER.lock();
    let Ok(read_bytes) = read_database(&mut *raw_buf) else {
        return false;
    };

    let table = CMD_TABLE.lock();
    if !table.initialized || read_bytes > MAX_DATABASE_SIZE {
        return false;
    }

    let Some(payload) =
        crate::security::aegis_detect::verify_command_database(&raw_buf[..read_bytes])
    else {
        return false;
    };
    crate::crypto::sha256::hash(payload) == table.payload_hash && table.signature_verified
}

/// Autocomplete matching for interactive shell Tab completion.
pub fn find_completions(
    prefix: &str,
    out_names: &mut [[u8; MAX_NAME_LEN]; 32],
    out_lens: &mut [usize; 32],
) -> usize {
    let table = CMD_TABLE.lock();
    let pfx = prefix.as_bytes();
    let mut matched = 0usize;

    for i in 0..table.count {
        if matched >= out_names.len() {
            break;
        }
        let entry = &table.entries[i];
        let name_slice = &entry.name[..entry.name_len as usize];
        if name_slice.starts_with(pfx) {
            out_names[matched] = entry.name;
            out_lens[matched] = entry.name_len as usize;
            matched += 1;
        }
    }

    matched
}

/// Print formatted categorized commands directly from `/System/CLI/cmd.dtbase`.
pub fn print_categorized_help() {
    let table = CMD_TABLE.lock();
    serial::println("\nAegisShell / Purple Shell Commands (MS-DOS & Linux POSIX Hybrid Engine) [Defined in /System/CLI/cmd.dtbase]:");

    let categories = [
        ("Navigation", "Directory & Navigation"),
        ("FileOps", "File Operations & Text Processing"),
        ("Storage", "Storage & Hardware Devices"),
        ("Network", "Networking & Communications"),
        ("System", "System Architecture & Execution"),
        ("Security", "Security & Cryptographic Signatures"),
        ("Userland", "Ring 3 (Userland / User Mode)"),
        ("CLI", "CLI Quality of Life & Path Management"),
    ];

    for &(cat_key, cat_title) in &categories {
        let mut count_in_cat = 0usize;
        for i in 0..table.count {
            if table.entries[i].category() == cat_key {
                count_in_cat += 1;
            }
        }

        if count_in_cat == 0 {
            continue;
        }

        serial::print("\n  ");
        serial::print(cat_title);
        serial::println(":");

        for i in 0..table.count {
            let entry = &table.entries[i];
            if entry.category() == cat_key {
                serial::print("    ");
                let usage = entry.usage();
                serial::print(usage);
                let u_len = usage.len();
                if u_len < 32 {
                    for _ in 0..(32 - u_len) {
                        serial::print(" ");
                    }
                } else {
                    serial::print(" ");
                }
                serial::println(entry.desc());
            }
        }
    }

    serial::println(
        "\nType 'help <command>' for detailed manual, or 'cmd info' for signature status.",
    );
}

/// Print manual entry for a specific command from the database.
pub fn print_command_help(name: &str) -> bool {
    if let Some(entry) = lookup(name) {
        serial::println("=======================================================================");
        serial::print("  PurpleOS Command Manual: ");
        serial::println(entry.name());
        serial::println("=======================================================================");
        serial::print("  Category:     ");
        serial::println(entry.category());
        serial::print("  Privilege:    ");
        if entry.is_ring3() {
            serial::println("Ring 3 (User Mode Application)");
        } else {
            serial::println("Ring 0 (Kernel Supervisor Builtin)");
        }
        serial::print("  Target:       ");
        serial::println(entry.target());
        serial::print("  Usage:        ");
        serial::println(entry.usage());
        serial::print("  Description:  ");
        serial::println(entry.desc());
        serial::println("=======================================================================");
        true
    } else {
        false
    }
}
