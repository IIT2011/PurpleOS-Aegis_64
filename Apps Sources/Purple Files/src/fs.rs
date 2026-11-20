//! Purple OS AegisShell-backed filesystem & command layer (`0x110A` / `0x110B`).
//! Uses fixed-capacity no_std buffers so no heap allocator is required.

use crate::abi;
use crate::gfx::rgb;

pub const MAX_ENTRIES: usize = 256;
pub const MAX_PATH: usize = 220;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Dir,
    App,
    Code,
    Text,
    Image,
    Archive,
    Config,
    File,
}

#[derive(Clone, Copy)]
pub struct StackStr<const N: usize> {
    pub buf: [u8; N],
    pub len: usize,
}

impl<const N: usize> StackStr<N> {
    pub const fn new() -> Self {
        Self {
            buf: [0; N],
            len: 0,
        }
    }

    pub fn from_bytes(s: &[u8]) -> Self {
        let mut out = Self::new();
        out.set(s);
        out
    }

    pub fn set(&mut self, s: &[u8]) {
        let n = s.len().min(N);
        self.buf[..n].copy_from_slice(&s[..n]);
        self.len = n;
    }

    pub fn push_byte(&mut self, b: u8) -> bool {
        if self.len < N {
            self.buf[self.len] = b;
            self.len += 1;
            true
        } else {
            false
        }
    }

    pub fn pop_byte(&mut self) -> bool {
        if self.len > 0 {
            self.len -= 1;
            true
        } else {
            false
        }
    }

    pub fn push_str(&mut self, s: &[u8]) -> bool {
        if self.len + s.len() <= N {
            self.buf[self.len..self.len + s.len()].copy_from_slice(s);
            self.len += s.len();
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[derive(Clone, Copy)]
pub struct FileEntry {
    pub name: StackStr<64>,
    pub size_bytes: u64,
    pub size_label: StackStr<16>,
    pub perms: StackStr<12>,
    pub kind: EntryKind,
    pub is_dir: bool,
}

impl FileEntry {
    pub const fn empty() -> Self {
        Self {
            name: StackStr::new(),
            size_bytes: 0,
            size_label: StackStr::new(),
            perms: StackStr::new(),
            kind: EntryKind::File,
            is_dir: false,
        }
    }

    pub fn ext_bytes(&self) -> &[u8] {
        let b = self.name.as_bytes();
        let mut i = b.len();
        while i > 0 {
            i -= 1;
            if b[i] == b'.' {
                return &b[i + 1..];
            }
        }
        b""
    }

    pub fn badge_color(&self) -> u32 {
        match self.kind {
            EntryKind::Dir => rgb(0x8B, 0x5C, 0xF6),
            EntryKind::App => rgb(0xA8, 0x55, 0xF7),
            EntryKind::Code => rgb(0x38, 0xBD, 0xF8),
            EntryKind::Text => rgb(0x34, 0xD3, 0x99),
            EntryKind::Image => rgb(0xF4, 0x72, 0xB6),
            EntryKind::Archive => rgb(0xFB, 0xBF, 0x24),
            EntryKind::Config => rgb(0xFB, 0x92, 0x3C),
            EntryKind::File => rgb(0x94, 0xA3, 0xB8),
        }
    }

    pub fn type_label(&self) -> &'static [u8] {
        match self.kind {
            EntryKind::Dir => b"Folder",
            EntryKind::App => b"Purple App",
            EntryKind::Code => b"Source Code",
            EntryKind::Text => b"Document",
            EntryKind::Image => b"Image",
            EntryKind::Archive => b"Archive",
            EntryKind::Config => b"Config",
            EntryKind::File => b"File",
        }
    }
}

pub fn classify_name(name: &[u8], is_dir: bool) -> EntryKind {
    if is_dir {
        return EntryKind::Dir;
    }
    if ends_with_ci(name, b".apapplication") || ends_with_ci(name, b".elf") {
        return EntryKind::App;
    }
    if ends_with_ci(name, b".rs")
        || ends_with_ci(name, b".c")
        || ends_with_ci(name, b".h")
        || ends_with_ci(name, b".py")
        || ends_with_ci(name, b".sh")
        || ends_with_ci(name, b".asm")
    {
        return EntryKind::Code;
    }
    if ends_with_ci(name, b".txt")
        || ends_with_ci(name, b".md")
        || ends_with_ci(name, b".log")
        || ends_with_ci(name, b".doc")
    {
        return EntryKind::Text;
    }
    if ends_with_ci(name, b".png")
        || ends_with_ci(name, b".jpg")
        || ends_with_ci(name, b".bmp")
        || ends_with_ci(name, b".tga")
    {
        return EntryKind::Image;
    }
    if ends_with_ci(name, b".zip")
        || ends_with_ci(name, b".tar")
        || ends_with_ci(name, b".gz")
        || ends_with_ci(name, b".img")
    {
        return EntryKind::Archive;
    }
    if ends_with_ci(name, b".toml")
        || ends_with_ci(name, b".policy")
        || ends_with_ci(name, b".sig")
        || ends_with_ci(name, b".cfg")
        || ends_with_ci(name, b".json")
    {
        return EntryKind::Config;
    }
    EntryKind::File
}

pub fn ends_with_ci(hay: &[u8], suf: &[u8]) -> bool {
    if hay.len() < suf.len() {
        return false;
    }
    let off = hay.len() - suf.len();
    for i in 0..suf.len() {
        if to_lower(hay[off + i]) != to_lower(suf[i]) {
            return false;
        }
    }
    true
}

#[inline]
pub const fn to_lower(b: u8) -> u8 {
    if b >= b'A' && b <= b'Z' {
        b + 32
    } else {
        b
    }
}

pub fn contains_ci(hay: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if hay.len() < needle.len() {
        return false;
    }
    for i in 0..=(hay.len() - needle.len()) {
        let mut ok = true;
        for j in 0..needle.len() {
            if to_lower(hay[i + j]) != to_lower(needle[j]) {
                ok = false;
                break;
            }
        }
        if ok {
            return true;
        }
    }
    false
}

pub fn format_size(bytes: u64, is_dir: bool) -> StackStr<16> {
    let mut s = StackStr::<16>::new();
    if is_dir {
        s.set(b"Folder");
        return s;
    }
    if bytes < 1024 {
        push_u64(&mut s, bytes);
        let _ = s.push_str(b" B");
    } else if bytes < 1024 * 1024 {
        let kb = bytes / 1024;
        let tenths = ((bytes % 1024) * 10) / 1024;
        push_u64(&mut s, kb);
        let _ = s.push_byte(b'.');
        let _ = s.push_byte(b'0' + (tenths as u8));
        let _ = s.push_str(b" KB");
    } else {
        let mb = bytes / (1024 * 1024);
        let tenths = ((bytes % (1024 * 1024)) * 10) / (1024 * 1024);
        push_u64(&mut s, mb);
        let _ = s.push_byte(b'.');
        let _ = s.push_byte(b'0' + (tenths as u8));
        let _ = s.push_str(b" MB");
    }
    s
}

pub fn push_u64<const N: usize>(s: &mut StackStr<N>, mut v: u64) {
    if v == 0 {
        let _ = s.push_byte(b'0');
        return;
    }
    let mut tmp = [0u8; 20];
    let mut i = 0;
    while v > 0 {
        tmp[i] = b'0' + ((v % 10) as u8);
        v /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        let _ = s.push_byte(tmp[i]);
    }
}

pub fn join_path(base: &[u8], child: &[u8]) -> StackStr<MAX_PATH> {
    let mut out = StackStr::<MAX_PATH>::new();
    if child.starts_with(b"/") {
        normalize_into(child, &mut out);
        return out;
    }
    let mut raw = StackStr::<MAX_PATH>::new();
    let _ = raw.push_str(base);
    if !base.ends_with(b"/") {
        let _ = raw.push_byte(b'/');
    }
    let _ = raw.push_str(child);
    normalize_into(raw.as_bytes(), &mut out);
    out
}

pub fn parent_path(path: &[u8]) -> StackStr<MAX_PATH> {
    let mut out = StackStr::<MAX_PATH>::new();
    let trimmed = if path.len() > 1 && path.ends_with(b"/") {
        &path[..path.len() - 1]
    } else {
        path
    };
    let mut i = trimmed.len();
    while i > 0 {
        i -= 1;
        if trimmed[i] == b'/' {
            if i == 0 {
                out.set(b"/");
            } else {
                out.set(&trimmed[..i]);
            }
            return out;
        }
    }
    out.set(b"/");
    out
}

pub fn normalize_into(raw: &[u8], out: &mut StackStr<MAX_PATH>) {
    out.clear();
    let mut seg_starts = [0usize; 32];
    let mut seg_lens = [0usize; 32];
    let mut depth = 0usize;

    let mut i = 0;
    while i < raw.len() {
        while i < raw.len() && raw[i] == b'/' {
            i += 1;
        }
        let start = i;
        while i < raw.len() && raw[i] != b'/' {
            i += 1;
        }
        let len = i - start;
        if len == 0 {
            continue;
        }
        let seg = &raw[start..start + len];
        if seg == b"." {
            continue;
        } else if seg == b".." {
            if depth > 0 {
                depth -= 1;
            }
        } else if depth < 32 {
            seg_starts[depth] = start;
            seg_lens[depth] = len;
            depth += 1;
        }
    }

    if depth == 0 {
        out.set(b"/");
        return;
    }
    for d in 0..depth {
        let _ = out.push_byte(b'/');
        let st = seg_starts[d];
        let ln = seg_lens[d];
        let _ = out.push_str(&raw[st..st + ln]);
    }
}

/// Appends a shell-safe quoted argument (using double quotes if needed, stripping control chars).
fn push_shell_arg(cmd: &mut StackStr<240>, arg: &[u8]) -> bool {
    let needs_quote = arg.iter().any(|&b| b == b' ' || b == b'\t');
    if needs_quote {
        if !cmd.push_byte(b'"') {
            return false;
        }
    }
    for &b in arg {
        if b < 0x20 || b > 0x7E || b == b'"' || b == b';' || b == b'|' || b == b'&' {
            continue;
        }
        if !cmd.push_byte(b) {
            return false;
        }
    }
    if needs_quote {
        if !cmd.push_byte(b'"') {
            return false;
        }
    }
    true
}

pub fn run_shell(cmd: &[u8], out_buf: &mut [u8; 4096]) -> Option<usize> {
    if cmd.is_empty() || cmd.len() > 240 {
        return None;
    }
    let res = abi::shell_exec(cmd, out_buf);
    if res < 0 {
        None
    } else {
        Some((res as usize).min(4096))
    }
}

pub fn get_initial_cwd() -> StackStr<MAX_PATH> {
    let mut prompt_buf = [0u8; 96];
    let n = abi::shell_prompt(&mut prompt_buf);
    if n > 0 {
        let slice = &prompt_buf[..(n as usize).min(96)];
        // Extract first '/' path segment if present in prompt (e.g. "aegis:/System> ")
        if let Some(pos) = slice.iter().position(|&b| b == b'/') {
            let mut end = pos;
            while end < slice.len()
                && slice[end] != b'>'
                && slice[end] != b'$'
                && slice[end] != b' '
                && slice[end] != b'\n'
                && slice[end] != b'\r'
            {
                end += 1;
            }
            let mut out = StackStr::new();
            normalize_into(&slice[pos..end], &mut out);
            return out;
        }
    }
    StackStr::from_bytes(b"/")
}

/// Lists directory contents into `entries` and returns count.
pub fn list_dir(path: &[u8], entries: &mut [FileEntry; MAX_ENTRIES]) -> usize {
    let mut cmd = StackStr::<240>::new();
    let _ = cmd.push_str(b"ls -la ");
    let _ = push_shell_arg(&mut cmd, path);

    let mut out_buf = [0u8; 4096];
    let mut len = match run_shell(cmd.as_bytes(), &mut out_buf) {
        Some(n) => n,
        None => 0,
    };

    // Fallback to plain `ls <path>` if `ls -la` failed or returned an error
    if len == 0 || starts_with_err(&out_buf[..len]) {
        cmd.clear();
        let _ = cmd.push_str(b"ls ");
        let _ = push_shell_arg(&mut cmd, path);
        len = run_shell(cmd.as_bytes(), &mut out_buf).unwrap_or(0);
    }

    parse_ls_output(&out_buf[..len], entries)
}

fn starts_with_err(b: &[u8]) -> bool {
    contains_ci(b, b"unknown option") || contains_ci(b, b"invalid flag")
}

fn parse_ls_output(raw: &[u8], entries: &mut [FileEntry; MAX_ENTRIES]) -> usize {
    let mut count = 0usize;
    let mut i = 0usize;

    while i < raw.len() && count < MAX_ENTRIES {
        let start = i;
        while i < raw.len() && raw[i] != b'\n' && raw[i] != b'\r' {
            i += 1;
        }
        let line = trim_ws(&raw[start..i]);
        while i < raw.len() && (raw[i] == b'\n' || raw[i] == b'\r') {
            i += 1;
        }

        if line.is_empty() || line.starts_with(b"total ") {
            continue;
        }

        if let Some(entry) = parse_ls_line(line) {
            let nm = entry.name.as_bytes();
            if nm == b"." || nm == b".." || nm.is_empty() {
                continue;
            }
            entries[count] = entry;
            count += 1;
        }
    }
    count
}

fn parse_ls_line(line: &[u8]) -> Option<FileEntry> {
    let mut entry = FileEntry::empty();

    // Case 1: Long listing format e.g. `drwxr-xr-x ... <size> ... <name>`
    if line.len() > 10 && (line[0] == b'd' || line[0] == b'-' || line[0] == b'l') {
        let is_dir = line[0] == b'd';
        entry.is_dir = is_dir;
        entry.perms.set(&line[..10.min(line.len())]);

        // Tokenize by whitespace
        let mut tokens: [&[u8]; 12] = [&[]; 12];
        let mut tcount = 0usize;
        let mut idx = 0usize;
        while idx < line.len() && tcount < 12 {
            while idx < line.len() && (line[idx] == b' ' || line[idx] == b'\t') {
                idx += 1;
            }
            let ts = idx;
            while idx < line.len() && line[idx] != b' ' && line[idx] != b'\t' {
                idx += 1;
            }
            if idx > ts {
                tokens[tcount] = &line[ts..idx];
                tcount += 1;
            }
        }

        if tcount >= 2 {
            let name_tok = tokens[tcount - 1];
            let clean_name = if name_tok.ends_with(b"/") && name_tok.len() > 1 {
                &name_tok[..name_tok.len() - 1]
            } else {
                name_tok
            };
            entry.name.set(clean_name);

            // Try to find numeric size token
            let mut size_val = 0u64;
            for k in 1..(tcount - 1) {
                if let Some(v) = parse_u64(tokens[k]) {
                    size_val = v;
                    if k >= 3 {
                        break;
                    }
                }
            }
            entry.size_bytes = size_val;
            entry.size_label = format_size(size_val, is_dir);
            entry.kind = classify_name(clean_name, is_dir);
            return Some(entry);
        }
    }

    // Case 2: `[DIR] name` or `DIR name`
    if line.starts_with(b"[DIR]") || line.starts_with(b"<DIR>") {
        let rest = trim_ws(&line[5..]);
        entry.is_dir = true;
        entry.name.set(rest);
        entry.perms.set(b"drwxr-xr-x");
        entry.size_label = format_size(0, true);
        entry.kind = EntryKind::Dir;
        return Some(entry);
    }

    // Case 3: Simple name (trailing `/` indicates directory)
    let is_dir = line.ends_with(b"/");
    let clean = if is_dir && line.len() > 1 {
        &line[..line.len() - 1]
    } else {
        line
    };
    entry.is_dir = is_dir;
    entry.name.set(clean);
    entry.perms
        .set(if is_dir { b"drwxr-xr-x" } else { b"-rw-r--r--" });
    entry.size_label = format_size(0, is_dir);
    entry.kind = classify_name(clean, is_dir);
    Some(entry)
}

fn trim_ws(mut s: &[u8]) -> &[u8] {
    while !s.is_empty() && (s[0] == b' ' || s[0] == b'\t') {
        s = &s[1..];
    }
    while !s.is_empty() && (s[s.len() - 1] == b' ' || s[s.len() - 1] == b'\t') {
        s = &s[..s.len() - 1];
    }
    s
}

fn parse_u64(s: &[u8]) -> Option<u64> {
    if s.is_empty() {
        return None;
    }
    let mut val = 0u64;
    for &b in s {
        if b < b'0' || b > b'9' {
            return None;
        }
        val = val.saturating_mul(10).saturating_add((b - b'0') as u64);
    }
    Some(val)
}

// ============================================================================
// Standard File Operations
// ============================================================================

pub fn mkdir(parent: &[u8], name: &[u8]) -> bool {
    let target = join_path(parent, name);
    let mut cmd = StackStr::<240>::new();
    let _ = cmd.push_str(b"mkdir ");
    if !push_shell_arg(&mut cmd, target.as_bytes()) {
        return false;
    }
    let mut out = [0u8; 4096];
    run_shell(cmd.as_bytes(), &mut out).is_some()
}

pub fn create_file(parent: &[u8], name: &[u8]) -> bool {
    let target = join_path(parent, name);
    let mut cmd = StackStr::<240>::new();
    let _ = cmd.push_str(b"touch ");
    if !push_shell_arg(&mut cmd, target.as_bytes()) {
        return false;
    }
    let mut out = [0u8; 4096];
    run_shell(cmd.as_bytes(), &mut out).is_some()
}

pub fn rename_or_move(src: &[u8], dst: &[u8]) -> bool {
    let mut cmd = StackStr::<240>::new();
    let _ = cmd.push_str(b"mv ");
    if !push_shell_arg(&mut cmd, src) {
        return false;
    }
    let _ = cmd.push_byte(b' ');
    if !push_shell_arg(&mut cmd, dst) {
        return false;
    }
    let mut out = [0u8; 4096];
    run_shell(cmd.as_bytes(), &mut out).is_some()
}

pub fn copy_item(src: &[u8], dst: &[u8], is_dir: bool) -> bool {
    let mut cmd = StackStr::<240>::new();
    let _ = cmd.push_str(if is_dir { b"cp -r " } else { b"cp " });
    if !push_shell_arg(&mut cmd, src) {
        return false;
    }
    let _ = cmd.push_byte(b' ');
    if !push_shell_arg(&mut cmd, dst) {
        return false;
    }
    let mut out = [0u8; 4096];
    run_shell(cmd.as_bytes(), &mut out).is_some()
}

pub fn delete_item(path: &[u8], is_dir: bool) -> bool {
    let mut cmd = StackStr::<240>::new();
    let _ = cmd.push_str(if is_dir { b"rm -rf " } else { b"rm " });
    if !push_shell_arg(&mut cmd, path) {
        return false;
    }
    let mut out = [0u8; 4096];
    run_shell(cmd.as_bytes(), &mut out).is_some()
}

pub fn read_preview(path: &[u8], out: &mut [u8; 4096]) -> usize {
    let mut cmd = StackStr::<240>::new();
    let _ = cmd.push_str(b"cat ");
    if !push_shell_arg(&mut cmd, path) {
        return 0;
    }
    run_shell(cmd.as_bytes(), out).unwrap_or(0)
}

pub fn launch_app(name: &[u8]) -> bool {
    let app_id = if ends_with_ci(name, b".apapplication") {
        &name[..name.len() - b".apapplication".len()]
    } else {
        name
    };
    let mut cmd = StackStr::<240>::new();
    let _ = cmd.push_str(b"launch ");
    if !push_shell_arg(&mut cmd, app_id) {
        return false;
    }
    let mut out = [0u8; 4096];
    run_shell(cmd.as_bytes(), &mut out).is_some()
}
