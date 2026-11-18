//! PurpleOS / Aegis64 Command Database Subsystem (/System/CLI/cmd.dtbase)
//!
//! Provides dynamic, storage-backed command resolution, metadata introspection,
//! and cryptographic Creator Signature verification. Commands are loaded directly
//! from `/System/CLI/cmd.dtbase` on the filesystem rather than hardcoded in the kernel.
//!
//! Integrity is enforced via a Cryptographic Signature specifically signed by the
//! Creator (Aegis Root Developer) using HMAC-SHA256 with the Creator Root Signing Key.

use crate::drivers::serial;
use crate::sync::Spinlock;

pub const CMD_DATABASE_PATH: &str = "/System/CLI/cmd.dtbase";
pub const FALLBACK_DATABASE_PATH: &str = "cmd.dtbase";

pub const CREATOR_SIGN_KEY: &[u8] = b"PURPLEOS_AEGIS_CREATOR_ROOT_KEY_2026";

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
    pub signature_expected: [u8; 64],
    pub signature_computed: [u8; 32],
    pub signature_hex: [u8; 64],
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
            signature_expected: [0; 64],
            signature_computed: [0; 32],
            signature_hex: [0; 64],
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
static RAW_BUFFER: Spinlock<[u8; 24576]> = Spinlock::new([0u8; 24576]);

const HEX_CHARS: &[u8] = b"0123456789abcdef";

/// Initialize the command database by reading `/System/CLI/cmd.dtbase` from storage,
/// verifying its Cryptographic Creator Signature, and parsing all command records.
pub fn init() -> bool {
    let mut raw_buf = RAW_BUFFER.lock();
    raw_buf.fill(0);

    let read_bytes = match crate::fs::vfs::read(CMD_DATABASE_PATH, &mut *raw_buf) {
        Ok(b) if b > 0 => b,
        _ => match crate::fs::vfs::read(FALLBACK_DATABASE_PATH, &mut *raw_buf) {
            Ok(b) if b > 0 => b,
            _ => {
                serial::print("[CMDDB] Warning: Command database '");
                serial::print(CMD_DATABASE_PATH);
                serial::println("' not found on storage.");
                return false;
            }
        },
    };

    parse_and_verify(&raw_buf[..read_bytes])
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
    table.signature_verified = false;
    table.signature_expected.fill(0);
    table.signature_computed.fill(0);
    table.signature_hex.fill(0);
    table.signer_name.fill(0);
    table.signer_len = 0;

    // 1. Locate `# SIGNATURE: ` and `# SIGNER: ` lines in header
    let mut expected_hex = [0u8; 64];
    let mut payload_start = 0usize;
    let mut found_sig = false;

    let mut line_start = 0usize;
    while line_start < data.len() {
        let mut line_end = line_start;
        while line_end < data.len() && data[line_end] != b'\n' {
            line_end += 1;
        }

        let line = &data[line_start..line_end];
        let trimmed = trim_slice(line);

        if trimmed.starts_with(b"# SIGNATURE:") {
            let rest = trim_slice(&trimmed[12..]);
            if rest.len() >= 64 {
                expected_hex.copy_from_slice(&rest[..64]);
                table.signature_expected = expected_hex;
                found_sig = true;
            }
        } else if trimmed.starts_with(b"# SIGNER:") {
            let rest = trim_slice(&trimmed[9..]);
            let len = rest.len().min(32);
            table.signer_name[..len].copy_from_slice(&rest[..len]);
            table.signer_len = len as u8;
        } else if found_sig && !trimmed.starts_with(b"#") && !trimmed.is_empty() {
            // Found start of payload
            payload_start = line_start;
            break;
        }

        line_start = if line_end < data.len() { line_end + 1 } else { line_end };
    }

    if !found_sig || payload_start >= data.len() {
        serial::println("[CMDDB] Error: Missing or invalid Creator Signature header in cmd.dtbase");
        return false;
    }

    // 2. Cryptographic Creator Signature Verification (HMAC-SHA256 with Creator Key)
    let payload = &data[payload_start..];
    table.payload_bytes = payload.len();
    let sig_digest = crate::crypto::sha256::hmac(CREATOR_SIGN_KEY, payload);
    table.signature_computed = sig_digest;

    let mut computed_hex = [0u8; 64];
    for (i, &b) in sig_digest.iter().enumerate() {
        computed_hex[i * 2] = HEX_CHARS[(b >> 4) as usize];
        computed_hex[i * 2 + 1] = HEX_CHARS[(b & 0x0f) as usize];
    }
    table.signature_hex = computed_hex;

    let is_verified = computed_hex == expected_hex;
    table.signature_verified = is_verified;

    if is_verified {
        serial::print("[CMDDB] Authenticated ");
        serial::print(CMD_DATABASE_PATH);
        serial::println(" — Cryptographically Signed by Creator!");
        serial::print("[CMDDB] Verified Signer: ");
        if table.signer_len > 0 {
            serial::println(table.signer());
        } else {
            serial::println("Creator (Aegis Root Developer)");
        }
    } else {
        serial::println("[SECURITY] ALERT: /System/CLI/cmd.dtbase Signature INVALID! Not signed by Creator!");
    }

    // 3. Parse Command Records from payload
    let mut current_idx: Option<usize> = None;
    line_start = payload_start;

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

        line_start = if line_end < data.len() { line_end + 1 } else { line_end };
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
    let name_b = name.as_bytes();
    for i in 0..table.count {
        let entry = &table.entries[i];
        if entry.name_len as usize == name_b.len() && &entry.name[..entry.name_len as usize] == name_b {
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
    let read_bytes = match crate::fs::vfs::read(CMD_DATABASE_PATH, &mut *raw_buf) {
        Ok(b) if b > 0 => b,
        _ => match crate::fs::vfs::read(FALLBACK_DATABASE_PATH, &mut *raw_buf) {
            Ok(b) if b > 0 => b,
            _ => return false,
        },
    };

    let table = CMD_TABLE.lock();
    if !table.initialized {
        return false;
    }

    // Compute fresh HMAC-SHA256 signature on disk payload
    let data = &raw_buf[..read_bytes];
    let mut payload_start = 0usize;
    let mut line_start = 0usize;
    let mut found_sig = false;

    while line_start < data.len() {
        let mut line_end = line_start;
        while line_end < data.len() && data[line_end] != b'\n' {
            line_end += 1;
        }
        let line = &data[line_start..line_end];
        let trimmed = trim_slice(line);
        if trimmed.starts_with(b"# SIGNATURE:") {
            found_sig = true;
        } else if found_sig && !trimmed.starts_with(b"#") && !trimmed.is_empty() {
            payload_start = line_start;
            break;
        }
        line_start = if line_end < data.len() { line_end + 1 } else { line_end };
    }

    if !found_sig || payload_start >= data.len() {
        return false;
    }

    let disk_sig = crate::crypto::sha256::hmac(CREATOR_SIGN_KEY, &data[payload_start..]);
    disk_sig == table.signature_computed && table.signature_verified
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

    serial::println("\nType 'help <command>' for detailed manual, or 'cmd info' for signature status.");
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
