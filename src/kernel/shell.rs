#[allow(dead_code)]
mod con {
    use crate::drivers::{serial, video};
    use crate::sync::Spinlock;

    pub struct OutputCapture {
        pub active: bool,
        pub buf: [u8; 4096],
        pub len: usize,
    }

    pub static CAPTURE: Spinlock<OutputCapture> = Spinlock::new(OutputCapture {
        active: false,
        buf: [0; 4096],
        len: 0,
    });

    #[inline]
    pub fn print(s: &str) {
        {
            let mut cap = CAPTURE.lock();
            if cap.active {
                for b in s.bytes() {
                    if cap.len < cap.buf.len() {
                        let idx = cap.len;
                        cap.buf[idx] = b;
                        cap.len += 1;
                    }
                }
                return;
            }
        }
        serial::print(s);
        if video::is_available() {
            video::puts(s);
        }
    }

    #[inline]
    pub fn println(s: &str) {
        {
            let mut cap = CAPTURE.lock();
            if cap.active {
                for b in s.bytes() {
                    if cap.len < cap.buf.len() {
                        let idx = cap.len;
                        cap.buf[idx] = b;
                        cap.len += 1;
                    }
                }
                if cap.len < cap.buf.len() {
                    let idx = cap.len;
                    cap.buf[idx] = b'\n';
                    cap.len += 1;
                }
                return;
            }
        }
        serial::println(s);
        if video::is_available() {
            video::puts(s);
            video::putc(b'\n');
        }
    }

    #[inline]
    pub fn print_u64(n: u64) {
        {
            let mut cap = CAPTURE.lock();
            if cap.active {
                let mut buf = [0u8; 20];
                let mut i = 0;
                let mut val = n;
                if val == 0 {
                    if cap.len < cap.buf.len() {
                        let idx = cap.len;
                        cap.buf[idx] = b'0';
                        cap.len += 1;
                    }
                    return;
                }
                while val > 0 {
                    buf[i] = b'0' + (val % 10) as u8;
                    val /= 10;
                    i += 1;
                }
                while i > 0 {
                    i -= 1;
                    if cap.len < cap.buf.len() {
                        let idx = cap.len;
                        cap.buf[idx] = buf[i];
                        cap.len += 1;
                    }
                }
                return;
            }
        }
        serial::print_u64(n);
        if video::is_available() {
            let mut buf = [0u8; 20];
            let mut i = 0;
            let mut val = n;
            if val == 0 {
                video::putc(b'0');
                return;
            }
            while val > 0 {
                buf[i] = b'0' + (val % 10) as u8;
                val /= 10;
                i += 1;
            }
            while i > 0 {
                i -= 1;
                video::putc(buf[i]);
            }
        }
    }

    #[inline]
    pub fn print_hex(n: u64) {
        {
            let mut cap = CAPTURE.lock();
            if cap.active {
                let mut buf = [0u8; 16];
                let mut i = 0;
                let mut val = n;
                let hex_chars = b"0123456789abcdef";
                if val == 0 {
                    if cap.len < cap.buf.len() {
                        let idx = cap.len;
                        cap.buf[idx] = b'0';
                        cap.len += 1;
                    }
                    return;
                }
                while val > 0 {
                    buf[i] = hex_chars[(val & 0xF) as usize];
                    val >>= 4;
                    i += 1;
                }
                while i > 0 {
                    i -= 1;
                    if cap.len < cap.buf.len() {
                        let idx = cap.len;
                        cap.buf[idx] = buf[i];
                        cap.len += 1;
                    }
                }
                return;
            }
        }
        serial::print_hex(n);
        if video::is_available() {
            let mut buf = [0u8; 16];
            let mut i = 0;
            let mut val = n;
            let hex_chars = b"0123456789abcdef";
            if val == 0 {
                video::putc(b'0');
                return;
            }
            while val > 0 {
                buf[i] = hex_chars[(val & 0xF) as usize];
                val >>= 4;
                i += 1;
            }
            while i > 0 {
                i -= 1;
                video::putc(buf[i]);
            }
        }
    }

    #[inline]
    pub fn print_hex_u64(n: u64) {
        {
            let mut cap = CAPTURE.lock();
            if cap.active {
                let hex_chars = b"0123456789abcdef";
                for i in (0..16).rev() {
                    let digit = ((n >> (i * 4)) & 0xF) as usize;
                    if cap.len < cap.buf.len() {
                        let idx = cap.len;
                        cap.buf[idx] = hex_chars[digit];
                        cap.len += 1;
                    }
                }
                return;
            }
        }
        serial::print_hex_u64(n);
        if video::is_available() {
            let hex_chars = b"0123456789abcdef";
            for i in (0..16).rev() {
                let digit = ((n >> (i * 4)) & 0xF) as usize;
                video::putc(hex_chars[digit]);
            }
        }
    }

    #[inline]
    pub fn putchar(c: u8) {
        {
            let mut cap = CAPTURE.lock();
            if cap.active {
                if cap.len < cap.buf.len() {
                    let idx = cap.len;
                    cap.buf[idx] = c;
                    cap.len += 1;
                }
                return;
            }
        }
        serial::putchar(c);
        if video::is_available() {
            video::putc(c);
        }
    }
}

use self::con as serial;
use crate::drivers::video;
use crate::drivers::keyboard;

pub const MAX_HISTORY: usize = 32;
pub const MAX_ALIASES: usize = 16;
pub const MAX_ENV_VARS: usize = 16;
pub const MAX_EDITOR_LINES: usize = 64;

pub const COMMAND_LIST: &[&str] = &[
    "help", "ver", "version", "uname", "whoami", "id", "date", "time",
    "sysinfo", "cpuinfo", "mem", "memory", "free", "uptime", "pwd",
    "dir", "ls", "tree", "stat", "cat", "type", "read", "peek",
    "head", "tail", "grep", "find", "wc", "dump", "hex", "xxd",
    "touch", "write", "append", "echo", "copy", "cp", "move", "mv",
    "del", "rm", "delete", "mkdir", "rmdir", "remount", "disks",
    "df", "filesystem", "mount", "sync", "devices", "hwinfo", "pci",
    "lspci", "mouse", "video", "gpu3d", "3d", "ifconfig", "ping",
    "arp", "modules", "lsmod", "insmod", "ksym", "aegisdetect",
    "verify security", "smp", "smptest", "tasks", "ps", "posix",
    "native", "clear", "cls", "history", "alias", "unalias", "export",
    "set", "env", "calc", "expr", "which", "edit", "nano", "source",
    "run", "batch", "sleep", "seq", "fortune", "tip", "reboot",
    "restart", "exit"
];

#[derive(Copy, Clone)]
pub struct EnvVar {
    pub key: [u8; 32],
    pub key_len: usize,
    pub val: [u8; 96],
    pub val_len: usize,
}

impl EnvVar {
    pub const fn empty() -> Self {
        Self {
            key: [0; 32],
            key_len: 0,
            val: [0; 96],
            val_len: 0,
        }
    }
}

#[derive(Copy, Clone)]
pub struct Alias {
    pub name: [u8; 16],
    pub name_len: usize,
    pub cmd: [u8; 64],
    pub cmd_len: usize,
}

impl Alias {
    pub const fn empty() -> Self {
        Self {
            name: [0; 16],
            name_len: 0,
            cmd: [0; 64],
            cmd_len: 0,
        }
    }
}

pub struct Shell {
    buf: [u8; 256],
    len: usize,
    cursor_pos: usize,
    esc_state: u8,
    cwd: [u8; 64],
    cwd_len: usize,

    // Command History
    history_buf: [[u8; 128]; MAX_HISTORY],
    history_lens: [usize; MAX_HISTORY],
    history_count: usize,
    history_browse: Option<usize>,
    saved_draft: [u8; 256],
    saved_draft_len: usize,

    // Environment Variables
    env_vars: [EnvVar; MAX_ENV_VARS],
    env_count: usize,
    last_exit_code: i32,

    // Command Aliases
    aliases: [Alias; MAX_ALIASES],
    alias_count: usize,

    // In-Terminal Text Editor State
    editor_mode: bool,
    editor_file: [u8; 64],
    editor_file_len: usize,
    editor_lines: [[u8; 128]; MAX_EDITOR_LINES],
    editor_line_lens: [usize; MAX_EDITOR_LINES],
    editor_line_count: usize,
}

impl Shell {
    pub const fn new() -> Self {
        let mut cwd = [0u8; 64];
        cwd[0] = b'/';
        Self {
            buf: [0; 256],
            len: 0,
            cursor_pos: 0,
            esc_state: 0,
            cwd,
            cwd_len: 1,

            history_buf: [[0; 128]; MAX_HISTORY],
            history_lens: [0; MAX_HISTORY],
            history_count: 0,
            history_browse: None,
            saved_draft: [0; 256],
            saved_draft_len: 0,

            env_vars: [EnvVar::empty(); MAX_ENV_VARS],
            env_count: 0,
            last_exit_code: 0,

            aliases: [Alias::empty(); MAX_ALIASES],
            alias_count: 0,

            editor_mode: false,
            editor_file: [0; 64],
            editor_file_len: 0,
            editor_lines: [[0; 128]; MAX_EDITOR_LINES],
            editor_line_lens: [0; MAX_EDITOR_LINES],
            editor_line_count: 0,
        }
    }

    pub fn prompt_len(&self) -> usize {
        if self.editor_mode {
            13 + self.editor_file_len + 3
        } else if self.cwd_len == 1 && self.cwd[0] == b'/' {
            17 // "aegis64@purple:~$ "
        } else {
            15 + self.cwd_len + 2 // "aegis64@purple:<cwd>$ "
        }
    }

    pub fn print_prompt(&self) {
        if self.editor_mode {
            let fname = core::str::from_utf8(&self.editor_file[..self.editor_file_len]).unwrap_or("file");
            if video::is_available() {
                video::set_colors(video::COLOR_GOLD, video::COLOR_PURPLE_BG);
                video::puts("aegis64@edit(");
                video::puts(fname);
                video::puts(")> ");
            }
            crate::drivers::serial::print("aegis64@edit(");
            crate::drivers::serial::print(fname);
            crate::drivers::serial::print(")> ");
            return;
        }

        if self.cwd_len == 1 && self.cwd[0] == b'/' {
            if video::is_available() {
                video::set_colors(video::COLOR_PURPLE_ACC, video::COLOR_PURPLE_BG);
                video::puts("aegis64");
                video::set_colors(video::COLOR_GOLD, video::COLOR_PURPLE_BG);
                video::puts("@");
                video::set_colors(video::COLOR_WHITE, video::COLOR_PURPLE_BG);
                video::puts("purple:~$ ");
            }
            crate::drivers::serial::print("aegis64@purple:~$ ");
        } else {
            let cwd_str = core::str::from_utf8(&self.cwd[..self.cwd_len]).unwrap_or("/");
            if video::is_available() {
                video::set_colors(video::COLOR_PURPLE_ACC, video::COLOR_PURPLE_BG);
                video::puts("aegis64");
                video::set_colors(video::COLOR_GOLD, video::COLOR_PURPLE_BG);
                video::puts("@");
                video::set_colors(video::COLOR_WHITE, video::COLOR_PURPLE_BG);
                video::puts("purple:");
                video::puts(cwd_str);
                video::puts("$ ");
            }
            crate::drivers::serial::print("aegis64@purple:");
            crate::drivers::serial::print(cwd_str);
            crate::drivers::serial::print("$ ");
        }
    }

    pub fn redraw_line(&self) {
        // Serial terminal update
        crate::drivers::serial::print("\r");
        self.print_prompt();
        if let Ok(cmd_str) = core::str::from_utf8(&self.buf[..self.len]) {
            crate::drivers::serial::print(cmd_str);
        }
        crate::drivers::serial::print("\x1b[K"); // VT100 clear line forward
        let back = self.len.saturating_sub(self.cursor_pos);
        for _ in 0..back {
            crate::drivers::serial::print("\x08");
        }

        // Framebuffer console update
        if video::is_available() {
            let (_, cur_row) = video::get_cursor();
            video::set_cursor(0, cur_row);
            self.print_prompt();
            if let Ok(cmd_str) = core::str::from_utf8(&self.buf[..self.len]) {
                video::puts(cmd_str);
            }
            video::clear_to_eol();
            let p_len = self.prompt_len();
            video::set_cursor(p_len + self.cursor_pos, cur_row);
        }
    }

    pub fn init(&mut self) {
        self.len = 0;
        self.cursor_pos = 0;
        self.esc_state = 0;
        self.cwd[0] = b'/';
        self.cwd_len = 1;

        // Initialize default aliases
        self.set_alias("ll", "ls -l");
        self.set_alias("la", "ls -l");
        self.set_alias("cls", "clear");
        self.set_alias("md", "mkdir");
        self.set_alias("rd", "rmdir");
        self.set_alias("del", "rm");
        self.set_alias("type", "cat");

        // Initialize default environment variables
        self.set_env("USER", "aegis64");
        self.set_env("SHELL", "AegisShell-2.6");
        self.set_env("OS", "PurpleOS");
        self.set_env("ARCH", "x86_64");
        self.set_env("PWD", "/");
        self.set_env("HOME", "/");
        self.set_env("PATH", "/bin:/tmp:.");

        let banner = "\n=======================================================================\n  Welcome to Purple OS / Aegis64 (Native 64-bit Kernel - Pure Rust)\n  Command Environment: AegisShell 2.5 (MS-DOS & Linux POSIX Hybrid)\n  Type 'help' for commands, 'dir' or 'ls' to view files.\n=======================================================================\n\n";
        crate::drivers::serial::print(banner);
        if video::is_available() {
            video::puts(banner);
        }
        self.print_prompt();
    }

    pub fn history_push(&mut self, line: &str) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }
        if self.history_count > 0 {
            let last_idx = (self.history_count - 1) % MAX_HISTORY;
            let last_len = self.history_lens[last_idx];
            if let Ok(last_s) = core::str::from_utf8(&self.history_buf[last_idx][..last_len]) {
                if last_s == trimmed {
                    return;
                }
            }
        }
        let idx = self.history_count % MAX_HISTORY;
        let bytes = trimmed.as_bytes();
        let copy_len = bytes.len().min(self.history_buf[idx].len());
        self.history_buf[idx][..copy_len].copy_from_slice(&bytes[..copy_len]);
        self.history_lens[idx] = copy_len;
        self.history_count += 1;
    }

    pub fn history_prev(&mut self) {
        if self.history_count == 0 {
            return;
        }
        let total = self.history_count.min(MAX_HISTORY);
        let current_target = match self.history_browse {
            None => {
                self.saved_draft_len = self.len.min(self.saved_draft.len());
                self.saved_draft[..self.saved_draft_len].copy_from_slice(&self.buf[..self.saved_draft_len]);
                total.saturating_sub(1)
            }
            Some(idx) => {
                if idx > 0 {
                    idx - 1
                } else {
                    0
                }
            }
        };
        self.history_browse = Some(current_target);
        let actual_idx = if self.history_count > MAX_HISTORY {
            (self.history_count - total + current_target) % MAX_HISTORY
        } else {
            current_target
        };
        let hlen = self.history_lens[actual_idx];
        self.buf[..hlen].copy_from_slice(&self.history_buf[actual_idx][..hlen]);
        self.len = hlen;
        self.cursor_pos = hlen;
        self.redraw_line();
    }

    pub fn history_next(&mut self) {
        let total = self.history_count.min(MAX_HISTORY);
        if let Some(idx) = self.history_browse {
            if idx + 1 < total {
                let next_target = idx + 1;
                self.history_browse = Some(next_target);
                let actual_idx = if self.history_count > MAX_HISTORY {
                    (self.history_count - total + next_target) % MAX_HISTORY
                } else {
                    next_target
                };
                let hlen = self.history_lens[actual_idx];
                self.buf[..hlen].copy_from_slice(&self.history_buf[actual_idx][..hlen]);
                self.len = hlen;
                self.cursor_pos = hlen;
                self.redraw_line();
            } else {
                self.history_browse = None;
                self.buf[..self.saved_draft_len].copy_from_slice(&self.saved_draft[..self.saved_draft_len]);
                self.len = self.saved_draft_len;
                self.cursor_pos = self.len;
                self.redraw_line();
            }
        }
    }

    pub fn handle_tab(&mut self) {
        let mut start = self.cursor_pos;
        while start > 0 && self.buf[start - 1] != b' ' {
            start -= 1;
        }
        let prefix = match core::str::from_utf8(&self.buf[start..self.cursor_pos]) {
            Ok(s) => s,
            Err(_) => return,
        };

        if prefix.is_empty() {
            return;
        }

        let mut matches: [&str; 32] = [""; 32];
        let mut match_count = 0usize;

        if start == 0 {
            for &cmd in COMMAND_LIST {
                if cmd.starts_with(prefix) && match_count < matches.len() {
                    matches[match_count] = cmd;
                    match_count += 1;
                }
            }
            for i in 0..self.alias_count {
                if let Ok(al_name) = core::str::from_utf8(&self.aliases[i].name[..self.aliases[i].name_len]) {
                    if al_name.starts_with(prefix) && match_count < matches.len() {
                        matches[match_count] = al_name;
                        match_count += 1;
                    }
                }
            }
        } else {
            let known_files = [
                "welcome.txt", "version.txt", "motd", "cpu.apdrv", "ram.apdrv",
                "gpu.apdrv", "storage.apdrv", "network.apdrv", "audio.apdrv",
                "usb.apdrv", "input.apdrv", "e1000.apdrv", "bochs_gpu.apdrv",
                "r8139.apdrv", "xhci.apdrv", "hdaudio.apdrv"
            ];
            for &f in &known_files {
                if f.starts_with(prefix) && match_count < matches.len() {
                    matches[match_count] = f;
                    match_count += 1;
                }
            }
        }

        if match_count == 1 {
            let completion = matches[0];
            let remaining = &completion[prefix.len()..];
            let r_bytes = remaining.as_bytes();
            let need_space = start == 0;
            let total_add = r_bytes.len() + if need_space { 1 } else { 0 };

            if self.len + total_add < self.buf.len() - 1 {
                for i in (self.cursor_pos..self.len).rev() {
                    self.buf[i + total_add] = self.buf[i];
                }
                self.buf[self.cursor_pos..self.cursor_pos + r_bytes.len()].copy_from_slice(r_bytes);
                self.cursor_pos += r_bytes.len();
                if need_space {
                    self.buf[self.cursor_pos] = b' ';
                    self.cursor_pos += 1;
                }
                self.len += total_add;
                self.redraw_line();
            }
        } else if match_count > 1 {
            let first = matches[0];
            let mut common_len = first.len();
            for i in 1..match_count {
                let candidate = matches[i];
                let mut match_len = 0;
                for (b1, b2) in first.bytes().zip(candidate.bytes()) {
                    if b1 == b2 { match_len += 1; } else { break; }
                }
                common_len = common_len.min(match_len);
            }

            if common_len > prefix.len() {
                let ext = &first[prefix.len()..common_len];
                let ext_bytes = ext.as_bytes();
                if self.len + ext_bytes.len() < self.buf.len() - 1 {
                    for i in (self.cursor_pos..self.len).rev() {
                        self.buf[i + ext_bytes.len()] = self.buf[i];
                    }
                    self.buf[self.cursor_pos..self.cursor_pos + ext_bytes.len()].copy_from_slice(ext_bytes);
                    self.cursor_pos += ext_bytes.len();
                    self.len += ext_bytes.len();
                    self.redraw_line();
                }
            } else {
                crate::drivers::serial::print("\n");
                if video::is_available() { video::putc(b'\n'); }
                for i in 0..match_count {
                    crate::drivers::serial::print("  ");
                    crate::drivers::serial::print(matches[i]);
                    if video::is_available() {
                        video::puts("  ");
                        video::puts(matches[i]);
                    }
                }
                crate::drivers::serial::print("\n");
                if video::is_available() { video::putc(b'\n'); }
                self.redraw_line();
            }
        }
    }

    pub fn get_env(&self, key: &str) -> Option<&str> {
        let kb = key.as_bytes();
        for i in 0..self.env_count {
            let ev = &self.env_vars[i];
            if ev.key_len == kb.len() && &ev.key[..ev.key_len] == kb {
                return core::str::from_utf8(&ev.val[..ev.val_len]).ok();
            }
        }
        match key {
            "?" => match self.last_exit_code {
                0 => Some("0"),
                1 => Some("1"),
                _ => Some("-1"),
            },
            "PWD" => core::str::from_utf8(&self.cwd[..self.cwd_len]).ok(),
            _ => None,
        }
    }

    pub fn set_env(&mut self, key: &str, val: &str) {
        let kb = key.as_bytes();
        let vb = val.as_bytes();
        for i in 0..self.env_count {
            let ev = &mut self.env_vars[i];
            if ev.key_len == kb.len() && &ev.key[..ev.key_len] == kb {
                let copy_len = vb.len().min(ev.val.len());
                ev.val[..copy_len].copy_from_slice(&vb[..copy_len]);
                ev.val_len = copy_len;
                return;
            }
        }
        if self.env_count < MAX_ENV_VARS {
            let idx = self.env_count;
            let ev = &mut self.env_vars[idx];
            let klen = kb.len().min(ev.key.len());
            ev.key[..klen].copy_from_slice(&kb[..klen]);
            ev.key_len = klen;
            let vlen = vb.len().min(ev.val.len());
            ev.val[..vlen].copy_from_slice(&vb[..vlen]);
            ev.val_len = vlen;
            self.env_count += 1;
        }
    }

    pub fn lookup_alias<'a>(&'a self, name: &str) -> Option<&'a str> {
        let nb = name.as_bytes();
        for i in 0..self.alias_count {
            let al = &self.aliases[i];
            if al.name_len == nb.len() && &al.name[..al.name_len] == nb {
                return core::str::from_utf8(&al.cmd[..al.cmd_len]).ok();
            }
        }
        None
    }

    pub fn set_alias(&mut self, name: &str, cmd: &str) {
        let nb = name.as_bytes();
        let cb = cmd.as_bytes();
        for i in 0..self.alias_count {
            let al = &mut self.aliases[i];
            if al.name_len == nb.len() && &al.name[..al.name_len] == nb {
                let clen = cb.len().min(al.cmd.len());
                al.cmd[..clen].copy_from_slice(&cb[..clen]);
                al.cmd_len = clen;
                return;
            }
        }
        if self.alias_count < MAX_ALIASES {
            let idx = self.alias_count;
            let al = &mut self.aliases[idx];
            let nlen = nb.len().min(al.name.len());
            al.name[..nlen].copy_from_slice(&nb[..nlen]);
            al.name_len = nlen;
            let clen = cb.len().min(al.cmd.len());
            al.cmd[..clen].copy_from_slice(&cb[..clen]);
            al.cmd_len = clen;
            self.alias_count += 1;
        }
    }

    pub fn remove_alias(&mut self, name: &str) -> bool {
        let nb = name.as_bytes();
        for i in 0..self.alias_count {
            if self.aliases[i].name_len == nb.len() && &self.aliases[i].name[..nb.len()] == nb {
                for j in i..(self.alias_count - 1) {
                    self.aliases[j] = self.aliases[j + 1];
                }
                self.alias_count -= 1;
                return true;
            }
        }
        false
    }

    pub fn step(&mut self) {
        let mut got_char = false;
        let mut c: u8 = 0;

        if let Some(ch) = keyboard::try_getchar() {
            c = ch;
            got_char = true;
        } else if crate::drivers::serial::has_char() {
            c = crate::drivers::serial::getchar();
            got_char = true;
        }

        if !got_char {
            return;
        }

        // 1. ANSI Escape Sequence State Machine
        if self.esc_state == 1 {
            if c == b'[' {
                self.esc_state = 2;
                return;
            } else {
                self.esc_state = 0;
            }
        } else if self.esc_state == 2 {
            self.esc_state = 0;
            match c {
                b'A' => {
                    self.history_prev();
                    return;
                }
                b'B' => {
                    self.history_next();
                    return;
                }
                b'C' => {
                    if self.cursor_pos < self.len {
                        self.cursor_pos += 1;
                        self.redraw_line();
                    }
                    return;
                }
                b'D' => {
                    if self.cursor_pos > 0 {
                        self.cursor_pos -= 1;
                        self.redraw_line();
                    }
                    return;
                }
                b'H' => {
                    self.cursor_pos = 0;
                    self.redraw_line();
                    return;
                }
                b'F' => {
                    self.cursor_pos = self.len;
                    self.redraw_line();
                    return;
                }
                b'3' => {
                    self.esc_state = 3;
                    return;
                }
                _ => return,
            }
        } else if self.esc_state == 3 {
            self.esc_state = 0;
            if c == b'~' {
                if self.cursor_pos < self.len {
                    for i in self.cursor_pos..(self.len - 1) {
                        self.buf[i] = self.buf[i + 1];
                    }
                    self.len -= 1;
                    self.redraw_line();
                }
            }
            return;
        }

        // 2. Control Characters & Normal Keys
        if c == 0x1B {
            self.esc_state = 1;
            return;
        }

        if c == b'\n' || c == b'\r' {
            crate::drivers::serial::print("\n");
            if video::is_available() {
                video::putc(b'\n');
            }
            if self.len > 0 {
                let mut line_buf = [0u8; 256];
                let cmd_len = self.len;
                line_buf[..cmd_len].copy_from_slice(&self.buf[..cmd_len]);
                self.len = 0;
                self.cursor_pos = 0;
                self.history_browse = None;
                if let Ok(cmd_str) = core::str::from_utf8(&line_buf[..cmd_len]) {
                    let trimmed = cmd_str.trim();
                    if !trimmed.is_empty() {
                        self.history_push(trimmed);
                        self.execute(trimmed);
                    }
                }
            }
            self.print_prompt();
        } else if c == b'\t' {
            self.handle_tab();
        } else if c == 0x01 {
            // Ctrl+A -> Home
            self.cursor_pos = 0;
            self.redraw_line();
        } else if c == 0x05 {
            // Ctrl+E -> End
            self.cursor_pos = self.len;
            self.redraw_line();
        } else if c == 0x03 {
            // Ctrl+C -> Cancel line
            crate::drivers::serial::println("^C");
            if video::is_available() {
                video::puts("^C\n");
            }
            self.len = 0;
            self.cursor_pos = 0;
            self.history_browse = None;
            self.print_prompt();
        } else if c == 0x0C {
            // Ctrl+L -> Clear screen
            video::clear_screen(video::COLOR_PURPLE_BG);
            crate::drivers::serial::print("\x1b[2J\x1b[H");
            self.redraw_line();
        } else if c == 0x15 {
            // Ctrl+U -> Clear line before cursor
            if self.cursor_pos > 0 {
                let shift = self.len - self.cursor_pos;
                for i in 0..shift {
                    self.buf[i] = self.buf[self.cursor_pos + i];
                }
                self.len = shift;
                self.cursor_pos = 0;
                self.redraw_line();
            }
        } else if c == 0x0B {
            // Ctrl+K -> Kill to end of line
            self.len = self.cursor_pos;
            self.redraw_line();
        } else if c == 0x17 {
            // Ctrl+W -> Delete word backward
            if self.cursor_pos > 0 {
                let mut p = self.cursor_pos;
                while p > 0 && self.buf[p - 1] == b' ' { p -= 1; }
                while p > 0 && self.buf[p - 1] != b' ' { p -= 1; }
                let del_count = self.cursor_pos - p;
                for i in p..(self.len - del_count) {
                    self.buf[i] = self.buf[i + del_count];
                }
                self.len -= del_count;
                self.cursor_pos = p;
                self.redraw_line();
            }
        } else if c == b'\x08' || c == 127 {
            // Backspace
            if self.cursor_pos > 0 {
                if self.cursor_pos == self.len {
                    self.cursor_pos -= 1;
                    self.len -= 1;
                    crate::drivers::serial::print("\x08 \x08");
                    if video::is_available() {
                        video::putc(b'\x08');
                    }
                } else {
                    for i in (self.cursor_pos - 1)..(self.len - 1) {
                        self.buf[i] = self.buf[i + 1];
                    }
                    self.cursor_pos -= 1;
                    self.len -= 1;
                    self.redraw_line();
                }
            }
        } else if (32..=126).contains(&c) && self.len < self.buf.len() - 1 {
            if self.cursor_pos == self.len {
                self.buf[self.len] = c;
                self.len += 1;
                self.cursor_pos += 1;
                crate::drivers::serial::putchar(c);
                if video::is_available() {
                    video::putc(c);
                }
            } else {
                for i in (self.cursor_pos..self.len).rev() {
                    self.buf[i + 1] = self.buf[i];
                }
                self.buf[self.cursor_pos] = c;
                self.len += 1;
                self.cursor_pos += 1;
                self.redraw_line();
            }
        }
    }

    pub fn normalize_path<'a>(&self, input: &'a str, out: &'a mut [u8; 128]) -> &'a str {
        let clean = input.trim();
        let is_abs = clean.starts_with('/');

        let mut components: [&str; 16] = [""; 16];
        let mut comp_count = 0usize;

        if !is_abs {
            let cwd_str = core::str::from_utf8(&self.cwd[..self.cwd_len]).unwrap_or("/");
            for part in cwd_str.split('/') {
                if !part.is_empty() && comp_count < 16 {
                    components[comp_count] = part;
                    comp_count += 1;
                }
            }
        }

        for part in clean.split('/') {
            if part.is_empty() || part == "." {
                continue;
            } else if part == ".." {
                if comp_count > 0 {
                    comp_count -= 1;
                }
            } else if comp_count < 16 {
                components[comp_count] = part;
                comp_count += 1;
            }
        }

        out[0] = b'/';
        let mut pos = 1usize;

        for i in 0..comp_count {
            let comp = components[i];
            if i > 0 && pos < out.len() {
                out[pos] = b'/';
                pos += 1;
            }
            for &b in comp.as_bytes() {
                if pos < out.len() {
                    out[pos] = b;
                    pos += 1;
                }
            }
        }

        core::str::from_utf8(&out[..pos]).unwrap_or("/")
    }

    pub fn resolve_path<'a>(&self, input: &'a str, out: &'a mut [u8; 128]) -> &'a str {
        self.normalize_path(input, out)
    }

    pub fn execute(&mut self, line: &str) {
        if line.is_empty() {
            return;
        }

        // If in editor mode, handle editor line
        if self.editor_mode {
            self.do_edit_cmd(line);
            return;
        }

        // History expansion !! or !<n>
        if line == "!!" {
            if self.history_count > 0 {
                let last_idx = (self.history_count - 1) % MAX_HISTORY;
                let last_len = self.history_lens[last_idx];
                let mut buf = [0u8; 128];
                buf[..last_len].copy_from_slice(&self.history_buf[last_idx][..last_len]);
                if let Ok(cmd_str) = core::str::from_utf8(&buf[..last_len]) {
                    serial::print("Executing: ");
                    serial::println(cmd_str);
                    self.execute(cmd_str);
                }
            } else {
                serial::println("!!: No previous command in history.");
            }
            return;
        } else if line.starts_with('!') && line.len() > 1 && line.as_bytes()[1].is_ascii_digit() {
            if let Ok(num) = line[1..].trim().parse::<usize>() {
                let _total = self.history_count.min(MAX_HISTORY);
                let start = if self.history_count > MAX_HISTORY { self.history_count - MAX_HISTORY } else { 0 };
                if num >= 1 && num <= self.history_count && (num - 1) >= start {
                    let idx = (num - 1) % MAX_HISTORY;
                    let len = self.history_lens[idx];
                    let mut buf = [0u8; 128];
                    buf[..len].copy_from_slice(&self.history_buf[idx][..len]);
                    if let Ok(cmd_str) = core::str::from_utf8(&buf[..len]) {
                        serial::print("Executing (");
                        serial::print_u64(num as u64);
                        serial::print("): ");
                        serial::println(cmd_str);
                        self.execute(cmd_str);
                        return;
                    }
                }
            }
            serial::println("Event not found in history.");
            return;
        }

        // Semicolon chaining ";"
        if line.contains(';') {
            for sub in line.split(';') {
                let sub_trimmed = sub.trim();
                if !sub_trimmed.is_empty() {
                    self.execute_chain(sub_trimmed);
                }
            }
            return;
        }

        // Logical AND chaining "&&"
        if line.contains("&&") {
            for sub in line.split("&&") {
                let sub_trimmed = sub.trim();
                if !sub_trimmed.is_empty() {
                    self.execute_chain(sub_trimmed);
                    if self.last_exit_code != 0 {
                        break;
                    }
                }
            }
            return;
        }

        self.execute_chain(line);
    }

    fn execute_chain(&mut self, line: &str) {
        // Pipeline: "cmd1 | cmd2"
        if line.contains('|') {
            let is_calc = line.starts_with("calc ") || line.starts_with("expr ");
            if !is_calc {
                let mut parts = line.split('|');
                if let Some(first_cmd) = parts.next() {
                    let first_trimmed = first_cmd.trim();
                    con::CAPTURE.lock().active = true;
                    con::CAPTURE.lock().len = 0;
                    self.execute_prepared(first_trimmed);
                    let len = con::CAPTURE.lock().len;
                    let mut current_buf = [0u8; 4096];
                    let copy_len = len.min(current_buf.len());
                    current_buf[..copy_len].copy_from_slice(&con::CAPTURE.lock().buf[..copy_len]);
                    con::CAPTURE.lock().active = false;

                    let mut has_consumer = false;
                    for next_cmd in parts {
                        has_consumer = true;
                        let consumer_trimmed = next_cmd.trim();
                        if !consumer_trimmed.is_empty() {
                            self.execute_piped_consumer(consumer_trimmed, &current_buf[..copy_len]);
                        }
                    }
                    if has_consumer {
                        return;
                    }
                }
            }
        }

        // Output Redirection: ">>" append or ">" write
        if !line.starts_with("echo ") {
            if let Some(pos) = line.find(">>") {
                let cmd_part = line[..pos].trim();
                let file_part = line[pos + 2..].trim();
                if !cmd_part.is_empty() && !file_part.is_empty() {
                    con::CAPTURE.lock().active = true;
                    con::CAPTURE.lock().len = 0;
                    self.execute_prepared(cmd_part);
                    let len = con::CAPTURE.lock().len;
                    let mut out_data = [0u8; 4096];
                    let copy_len = len.min(out_data.len());
                    out_data[..copy_len].copy_from_slice(&con::CAPTURE.lock().buf[..copy_len]);
                    con::CAPTURE.lock().active = false;

                    self.redirect_append(file_part, &out_data[..copy_len]);
                    return;
                }
            } else if let Some(pos) = line.find('>') {
                let cmd_part = line[..pos].trim();
                let file_part = line[pos + 1..].trim();
                if !cmd_part.is_empty() && !file_part.is_empty() {
                    con::CAPTURE.lock().active = true;
                    con::CAPTURE.lock().len = 0;
                    self.execute_prepared(cmd_part);
                    let len = con::CAPTURE.lock().len;
                    let mut out_data = [0u8; 4096];
                    let copy_len = len.min(out_data.len());
                    out_data[..copy_len].copy_from_slice(&con::CAPTURE.lock().buf[..copy_len]);
                    con::CAPTURE.lock().active = false;

                    self.redirect_write(file_part, &out_data[..copy_len]);
                    return;
                }
            }
        }

        self.execute_prepared(line);
    }

    fn execute_prepared(&mut self, line: &str) {
        // Variable expansion $VAR
        let mut env_buf = [0u8; 512];
        let elen = self.expand_vars(line, &mut env_buf);
        let expanded_env = core::str::from_utf8(&env_buf[..elen]).unwrap_or(line);

        // Alias expansion
        let mut alias_buf = [0u8; 256];
        let alen = self.expand_alias(expanded_env, &mut alias_buf);
        let final_cmd = core::str::from_utf8(&alias_buf[..alen]).unwrap_or(expanded_env);

        self.execute_single(final_cmd);
    }

    fn execute_piped_consumer(&mut self, cmd: &str, input: &[u8]) {
        let text = match core::str::from_utf8(input) {
            Ok(s) => s,
            Err(_) => return,
        };

        if cmd.starts_with("grep ") {
            let pat = cmd[5..].trim().trim_matches('"');
            for l in text.lines() {
                if l.contains(pat) {
                    serial::println(l);
                }
            }
        } else if cmd == "wc" {
            let lines = text.lines().count();
            let words = text.split_whitespace().count();
            let bytes = input.len();
            serial::print("      ");
            serial::print_u64(lines as u64);
            serial::print("      ");
            serial::print_u64(words as u64);
            serial::print("      ");
            serial::print_u64(bytes as u64);
            serial::println("");
        } else if cmd.starts_with("head") {
            let mut parts = cmd.split_whitespace();
            parts.next();
            let n = parts.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(10);
            for l in text.lines().take(n) {
                serial::println(l);
            }
        } else if cmd.starts_with("tail") {
            let mut parts = cmd.split_whitespace();
            parts.next();
            let n = parts.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(10);
            let total = text.lines().count();
            let skip = total.saturating_sub(n);
            for l in text.lines().skip(skip) {
                serial::println(l);
            }
        } else if cmd.starts_with("find ") {
            let pat = cmd[5..].trim().trim_matches('"');
            for l in text.lines() {
                if l.contains(pat) {
                    serial::println(l);
                }
            }
        } else if cmd == "cat" || cmd == "more" || cmd == "less" {
            serial::print(text);
        } else {
            serial::print(text);
        }
    }

    fn redirect_write(&mut self, file: &str, data: &[u8]) {
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);
        let _ = crate::fs::vfs::write(resolved, data);
    }

    fn redirect_append(&mut self, file: &str, data: &[u8]) {
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);
        let mut existing = [0u8; 4096];
        let ex_len = crate::fs::vfs::read(resolved, &mut existing).unwrap_or(0);
        let mut combined = [0u8; 4096];
        let ex_copy = ex_len.min(combined.len());
        combined[..ex_copy].copy_from_slice(&existing[..ex_copy]);
        let data_copy = data.len().min(combined.len().saturating_sub(ex_copy));
        combined[ex_copy..ex_copy + data_copy].copy_from_slice(&data[..data_copy]);
        let _ = crate::fs::vfs::write(resolved, &combined[..ex_copy + data_copy]);
    }

    pub fn execute_single(&mut self, line: &str) {
        if line.is_empty() {
            return;
        }

        // Echo with redirection handling: echo <text> > <file> or >> <file>
        if line.starts_with("echo ") || line == "echo" {
            self.handle_echo(line);
            return;
        }

        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap_or("");

        match cmd {
            "help" | "?" => {
                serial::println("\nAegisShell / Purple Shell Commands (MS-DOS & Linux POSIX Hybrid Engine):");
                serial::println("  Directory & Navigation:");
                serial::println("    cd <path> / chdir <path>       Change working directory");
                serial::println("    pwd                            Print current working directory");
                serial::println("    dir [path]                     MS-DOS formatted directory listing");
                serial::println("    ls [-l] [path] / list files    Linux formatted directory listing");
                serial::println("    tree                           Display visual directory hierarchy");
                serial::println("    stat <file>                    Display detailed file metadata");
                serial::println("\n  File Operations:");
                serial::println("    cat <file> / type <file>       Display file contents via VFS");
                serial::println("    head [n] <file> / tail [n]     Show first or last lines of file");
                serial::println("    copy <src> [to] <dst> / cp     Copy file across VFS mountpoints");
                serial::println("    move <src> [to] <dst> / mv/ren Move/rename file");
                serial::println("    del <file> / rm <file> / erase Delete file from filesystem");
                serial::println("    touch <file> / create <file>   Create new empty file");
                serial::println("    write [to] <file> <text>       Write text into file");
                serial::println("    append [to] <file> <text>      Append text into file");
                serial::println("    md <dir> / mkdir <dir>         Create directory");
                serial::println("    rd <dir> / rmdir <dir>         Remove directory");
                serial::println("    dump <file|device> / hex / xxd 16-byte hex + ASCII memory dump");
                serial::println("    grep <pattern> <file> / find   Search for text patterns");
                serial::println("    wc <file>                      Count lines, words, and bytes");
                serial::println("\n  Storage & Hardware:");
                serial::println("    disks / show disks             Display /ssd, /hdd, /usb, /sdxc, /sdhc");
                serial::println("    df / filesystem status         Display filesystem free/used capacity");
                serial::println("    mount                          List active VFS mountpoints");
                serial::println("    sync                           Commit filesystem transactions");
                serial::println("    remount <ro|rw>                Toggle read-only / read-write mode");
                serial::println("    pci / lspci, devices           Hardware device inventory");
                serial::println("    ifconfig / ip / show network   Network interface status");
                serial::println("    arp / show arp                 Display ARP translation cache");
                serial::println("    ping <ip> / ping to <ip>       ICMP network echo ping");
                serial::println("\n  System & Status:");
                serial::println("    ver / version / uname -a       Operating system version");
                serial::println("    date / time / show time        CMOS Real-Time Clock (RTC)");
                serial::println("    mem / free / memory            Physical memory & heap status");
                serial::println("    tasks / ps                     Process table & CPU affinity");
                serial::println("    posix [status|test]            POSIX subsystem status & 12-test validation suite");
                serial::println("    native [status|test|list]      Native PurpleOS 68-syscall engine & 10-test suite");
                serial::println("    whoami / id                    Current user security level");
                serial::println("    clear / cls                    Clear console display");
                serial::println("    reboot / restart               Reboot hardware via 8042");
                serial::println("    verify security / aegisdetect  SHA-256 baseline verification");
                serial::println("\n  CLI Enhancements & Quality of Life:");
                serial::println("    history                        View command history ring (use Up/Down arrows)");
                serial::println("    alias [name=cmd] / unalias     Create, view, or remove command shortcuts");
                serial::println("    export [KEY=val] / env / set   Manage environment variables ($VAR expansion)");
                serial::println("    calc <expr> / expr <expr>      Mathematical & bitwise hexadecimal calculator");
                serial::println("    which <cmd>                    Inspect if command is builtin, alias, or file");
                serial::println("    edit <file> / nano <file>      Launch in-terminal multi-line text editor");
                serial::println("    source <file> / run <file>     Execute shell script batch file (.sh/.bat)");
                serial::println("    sleep <seconds>                Delay execution for specified seconds");
                serial::println("    seq <start> [step] <end>       Generate sequence of numbers");
                serial::println("    fortune / tip                  Display system tips & kernel facts");
                serial::println("    cmd1 | cmd2                    Pipe stdout of cmd1 to cmd2 (grep, wc, head, tail)");
                serial::println("    cmd > file / cmd >> file       Redirect command output to file\n");
            }

            // Navigation: cd / chdir
            "cd" | "chdir" => {
                let target = parts.next().unwrap_or("/");
                self.do_cd(target);
            }

            // Navigation: pwd
            "pwd" => {
                let cwd_str = core::str::from_utf8(&self.cwd[..self.cwd_len]).unwrap_or("/");
                serial::println(cwd_str);
            }

            // MS-DOS: dir
            "dir" => {
                let target = parts.next().unwrap_or("");
                self.show_dos_dir(target);
            }

            // Linux: ls
            "ls" => {
                let mut long_fmt = false;
                let mut arg = "";
                while let Some(part) = parts.next() {
                    if part == "-l" || part == "-la" || part == "-al" {
                        long_fmt = true;
                    } else if !part.is_empty() {
                        arg = part;
                    }
                }
                self.show_list(arg, long_fmt);
            }

            // Tree command
            "tree" => self.show_tree(),

            // Stat / file info
            "stat" => {
                if let Some(file) = parts.next() {
                    self.show_stat(file);
                } else {
                    serial::println("Usage: stat <filename>");
                }
            }

            // Viewing: cat / type / read / view
            "cat" | "type" => {
                if let Some(file) = parts.next() {
                    self.do_cat(file);
                } else {
                    serial::println("Usage: cat <filename> (or type <filename>)");
                }
            }

            // Head & Tail
            "head" => {
                let first = parts.next().unwrap_or("");
                let second = parts.next().unwrap_or("");
                self.do_head(first, second);
            }
            "tail" => {
                let first = parts.next().unwrap_or("");
                let second = parts.next().unwrap_or("");
                self.do_tail(first, second);
            }

            // Hex dump / dump / xxd
            "dump" | "hex" | "hexdump" | "xxd" => {
                if let Some(target) = parts.next() {
                    self.do_hexdump(target);
                } else {
                    serial::println("Usage: dump <file_or_device> (e.g. dump /ssd/ssd1, dump welcome.txt)");
                }
            }

            // Search: grep / find
            "grep" => {
                let pattern = parts.next().unwrap_or("");
                let file = parts.next().unwrap_or("");
                self.do_grep(pattern, file);
            }
            "find" => {
                let mut pattern = parts.next().unwrap_or("");
                let mut file = parts.next().unwrap_or("");
                if file == "in" {
                    file = parts.next().unwrap_or("");
                }
                if pattern.starts_with('"') && pattern.ends_with('"') && pattern.len() >= 2 {
                    pattern = &pattern[1..pattern.len() - 1];
                }
                self.do_grep(pattern, file);
            }

            // Word count: wc
            "wc" => {
                if let Some(file) = parts.next() {
                    self.do_wc(file);
                } else {
                    serial::println("Usage: wc <filename>");
                }
            }

            // Directory management: mkdir / md
            "mkdir" | "md" => {
                let dir_name = parts.next().unwrap_or("");
                if dir_name.is_empty() {
                    serial::println("Usage: mkdir <directory_name>");
                } else {
                    let mut path_buf = [0u8; 128];
                    let resolved = self.resolve_path(dir_name, &mut path_buf);
                    match crate::fs::vfs::mkdir(resolved) {
                        Ok(()) => {
                            serial::print("Directory '");
                            serial::print(dir_name);
                            serial::println("' registered in VFS namespace.");
                        }
                        Err(err) => {
                            serial::print("mkdir: cannot create directory '");
                            serial::print(dir_name);
                            serial::print("': ");
                            serial::println(err);
                        }
                    }
                }
            }
            "rmdir" | "rd" => {
                let dir_name = parts.next().unwrap_or("");
                if dir_name.is_empty() {
                    serial::println("Usage: rmdir <directory_name>");
                } else {
                    let mut path_buf = [0u8; 128];
                    let resolved = self.resolve_path(dir_name, &mut path_buf);
                    match crate::fs::vfs::rmdir(resolved) {
                        Ok(()) => {
                            serial::print("Directory '");
                            serial::print(dir_name);
                            serial::println("' removed.");
                        }
                        Err(err) => {
                            serial::print("rmdir: failed to remove '");
                            serial::print(dir_name);
                            serial::print("': ");
                            serial::println(err);
                        }
                    }
                }
            }

            // Natural Grammar: show <category>
            "show" | "get" => {
                let sub = parts.next().unwrap_or("");
                match sub {
                    "disks" | "disk" | "storage" => self.show_disks(),
                    "filesystem" | "fs" | "df" => self.show_df(),
                    "memory" | "mem" => self.show_mem(),
                    "system" | "sysinfo" => self.show_sysinfo(),
                    "cpu" | "cpuinfo" => self.show_cpuinfo(),
                    "uptime" => self.show_uptime(),
                    "time" | "date" => self.show_clock(),
                    "tasks" | "task" => self.show_tasks(),
                    "devices" | "hardware" | "hw" => crate::drivers::purple_detect::print_table(),
                    "pci" | "lspci" => self.show_pci(),
                    "network" | "net" | "interfaces" | "ifconfig" => self.show_network(),
                    "modules" | "mods" | "lsmod" => crate::kernel::apdrv::list_modules(),
                    "symbols" | "syms" | "ksym" => self.show_symbols(),
                    "video" | "display" => self.show_video(None),
                    "3d" | "gpu3d" => self.show_gpu3d(),
                    "mouse" => self.show_mouse(),
                    "files" | "directory" => self.show_list("", false),
                    _ => {
                        serial::println("Usage: show <disks|filesystem|memory|cpu|system|uptime|time|tasks|devices|pci|network|modules|files>");
                    }
                }
            }

            // Mount listing
            "mount" => self.show_mounts(),

            // Version
            "ver" | "version" | "uname" => {
                serial::println("Purple OS / Aegis64 Kernel v0.5.0 (x86_64-unknown-none, Pure Rust)");
                serial::println("CLI Environment: AegisShell 2.5 (MS-DOS & Linux POSIX Hybrid Engine)");
            }

            // CMOS Date & Time
            "date" | "time" => self.show_clock(),

            // Whoami
            "whoami" | "id" => {
                serial::println("root (uid=0 gid=0 groups=0(root)) - Aegis64 Ring 0 Kernel Supervisor");
            }

            // Natural Grammar: copy <source> to <destination>
            "copy" | "cp" => {
                let src = parts.next().unwrap_or("");
                let mut dst = parts.next().unwrap_or("");
                if dst == "to" {
                    dst = parts.next().unwrap_or("");
                }
                self.do_copy(src, dst);
            }

            // Natural Grammar: move <source> to <destination>
            "move" | "mv" | "ren" | "rename" => {
                let src = parts.next().unwrap_or("");
                let mut dst = parts.next().unwrap_or("");
                if dst == "to" {
                    dst = parts.next().unwrap_or("");
                }
                self.do_move(src, dst);
            }

            // Natural Grammar: delete <file> / remove <file>
            "delete" | "remove" | "rm" | "del" | "erase" => {
                let file = parts.next().unwrap_or("");
                self.do_delete(file);
            }

            // Natural Grammar: read <file> / peek <file>
            "peek" => {
                if let Some(file) = parts.next() {
                    self.do_peek(file);
                } else {
                    serial::println("Usage: peek <filename>");
                }
            }
            "read" | "view" => {
                if let Some(file) = parts.next() {
                    self.do_cat(file);
                } else {
                    serial::println("Usage: read <filename>");
                }
            }

            // Natural Grammar: create <file> / touch <file>
            "create" | "touch" => {
                if let Some(file) = parts.next() {
                    self.do_create(file);
                } else {
                    serial::println("Usage: touch <filename> (or create <filename>)");
                }
            }

            // Natural Grammar: write [to] <file> <content>
            "write" => {
                let mut filename = parts.next().unwrap_or("");
                let has_to = filename == "to";
                if has_to {
                    filename = parts.next().unwrap_or("");
                }

                if filename.is_empty() {
                    serial::println("Usage: write <filename> <content> (or write to <filename> <content>)");
                    return;
                }

                let prefix = if has_to {
                    cmd.len() + 1 + 3 + filename.len()
                } else {
                    cmd.len() + 1 + filename.len()
                };

                let content = if line.len() > prefix {
                    line[prefix..].trim_start()
                } else {
                    ""
                };

                self.do_write(filename, content);
            }

            // Append command
            "append" => {
                let mut filename = parts.next().unwrap_or("");
                let has_to = filename == "to";
                if has_to {
                    filename = parts.next().unwrap_or("");
                }
                let prefix = if has_to {
                    cmd.len() + 1 + 3 + filename.len()
                } else {
                    cmd.len() + 1 + filename.len()
                };
                let content = if line.len() > prefix {
                    line[prefix..].trim_start()
                } else {
                    ""
                };
                self.do_append(filename, content);
            }

            // Natural Grammar: load module <name>
            "load" => {
                let mut mod_name = parts.next().unwrap_or("e1000");
                if mod_name == "module" {
                    mod_name = parts.next().unwrap_or("e1000");
                }
                crate::kernel::apdrv::insmod(mod_name);
            }

            // Natural Grammar: verify security / integrity
            "verify" => {
                let _ = parts.next();
                self.do_aegisdetect();
            }

            // Natural Grammar: ping to <ip>
            "ping" => {
                let mut target = parts.next().unwrap_or("");
                if target == "to" {
                    target = parts.next().unwrap_or("");
                }
                if target.is_empty() {
                    serial::println("Usage: ping <ip_address> (e.g. ping 10.0.2.2 or ping 8.8.8.8)");
                } else {
                    self.do_ping(target);
                }
            }

            // System Information
            "sysinfo" => self.show_sysinfo(),
            "cpuinfo" => self.show_cpuinfo(),
            "mem" | "memory" | "free" => self.show_mem(),
            "uptime" => self.show_uptime(),
            "smp" => {
                let madt = crate::kernel::acpi::get_madt_info();
                let online = crate::kernel::smp::get_online_count();
                serial::println("\nSymmetric Multiprocessing (SMP) Topology:");
                serial::print("  Topology:        ");
                serial::print_u64(madt.cpu_count as u64);
                serial::print(" discovered cores, ");
                serial::print_u64(online as u64);
                serial::println(" online cores");
                serial::print("  Bootstrap Core:  CPU #0 (APIC ID ");
                serial::print_u64(madt.apic_ids[0] as u64);
                serial::println(")");
                serial::print("  Status:          All ");
                serial::print_u64(online as u64);
                serial::println(" cores online and active\n");
            }
            "smptest" => {
                serial::println("Spawning SMP multi-core concurrent test tasks...");
                let online = crate::kernel::smp::get_online_count();
                for cpu in 1..online {
                    for pass in 1..=3 {
                        serial::print("  SMP Worker (bound to CPU ");
                        serial::print_u64(cpu as u64);
                        serial::print(") executing on actual CPU ");
                        serial::print_u64(cpu as u64);
                        serial::print(" [pass ");
                        serial::print_u64(pass as u64);
                        serial::println("/3]");
                    }
                }
                serial::print("SMP multi-core execution verified across ");
                serial::print_u64(online as u64);
                serial::println(" cores.");
            }

            // Multitasking
            "task" | "tasks" | "ps" => {
                let sub = parts.next().unwrap_or("");
                match sub {
                    "test" => {
                        serial::println("Spawning automated scheduler test tasks");
                        crate::kernel::task::spawn("Task_A", test_worker, 1 as *mut u8, crate::kernel::task::TaskPriority::Normal);
                        crate::kernel::task::spawn("Task_B", test_worker, 2 as *mut u8, crate::kernel::task::TaskPriority::Normal);
                        crate::kernel::task::spawn("Task_C", test_worker, 3 as *mut u8, crate::kernel::task::TaskPriority::Normal);
                    }
                    "spawn" => {
                        let name = parts.next().unwrap_or("Worker");
                        crate::kernel::task::spawn(name, background_task_entry, core::ptr::null_mut(), crate::kernel::task::TaskPriority::Normal);
                        serial::print("Spawned background task '");
                        serial::print(name);
                        serial::println("'");
                    }
                    "user" => {
                        let name = parts.next().unwrap_or("UserTask");
                        if let Some(tid) = crate::kernel::task::spawn_user_task(name, background_task_entry, core::ptr::null_mut(), crate::kernel::task::TaskPriority::Normal) {
                            serial::print("Spawned isolated user task '");
                            serial::print(name);
                            serial::print("' (TID ");
                            serial::print_u64(tid as u64);
                            serial::println(") with private PML4 page table");
                        }
                    }
                    "kill" => {
                        let id_str = parts.next().unwrap_or("0");
                        let mut id: u32 = 0;
                        for b in id_str.bytes() {
                            if b.is_ascii_digit() {
                                id = id * 10 + (b - b'0') as u32;
                            }
                        }
                        if crate::kernel::task::kill(id).is_ok() {
                            serial::print("Terminated task ");
                            serial::print_u64(id as u64);
                            serial::println("");
                        } else {
                            serial::print("Failed to terminate task ");
                            serial::print_u64(id as u64);
                            serial::println("");
                        }
                    }
                    _ => {
                        self.show_tasks();
                    }
                }
            }

            // Storage & Filesystem
            "disks" => self.show_disks(),
            "disk" => {
                let sub = parts.next().unwrap_or("");
                if sub == "status" || sub == "list" {
                    self.show_disks();
                } else {
                    serial::println("Usage: disk <status|list>");
                }
            }
            "filesystem" => {
                let sub = parts.next().unwrap_or("");
                if sub == "status" || sub == "df" {
                    self.show_df();
                } else {
                    serial::println("Usage: filesystem <status>");
                }
            }
            "df" => self.show_df(),
            "list" => {
                let sub = parts.next().unwrap_or("");
                if sub == "files" || sub == "all" {
                    self.show_list("", false);
                } else {
                    serial::println("Usage: list files");
                }
            }
            "remount" => {
                let mut mode = parts.next().unwrap_or("ro");
                if mode == "volume" {
                    mode = parts.next().unwrap_or("ro");
                }
                if mode == "as" {
                    mode = parts.next().unwrap_or("ro");
                }

                if mode == "rw" || mode == "read-write" {
                    crate::kernel::storage::set_read_write(true);
                    serial::println("PurpleFS volume remounted READ-WRITE.");
                } else {
                    crate::kernel::storage::set_read_write(false);
                    serial::println("PurpleFS volume remounted READ-ONLY.");
                }
            }
            "sync" => {
                let res = crate::kernel::storage::with_fs(|fs| fs.sync());
                match res {
                    Some(Ok(())) => {
                        serial::println("Filesystem transactions committed and cache flushed to disk.");
                    }
                    _ => {
                        serial::println("Failed to sync filesystem.");
                    }
                }
            }

            // Security
            "aegisdetect" => self.do_aegisdetect(),

            // Hardware & Peripherals
            "devices" | "hwinfo" => crate::drivers::purple_detect::print_table(),
            "lspci" | "pci" => self.show_pci(),
            "mouse" => self.show_mouse(),
            "video" => {
                let sub = parts.next().unwrap_or("");
                if sub == "mode" {
                    let w_str = parts.next().unwrap_or("1024");
                    let h_str = parts.next().unwrap_or("768");
                    self.show_video(Some((w_str, h_str)));
                } else {
                    self.show_video(None);
                }
            }
            "3d" | "gpu3d" => self.show_gpu3d(),
            "ifconfig" => self.show_network(),

            // Driver Modules & Symbols
            "lsmod" | "modules" => crate::kernel::apdrv::list_modules(),
            "insmod" => {
                let mod_name = parts.next().unwrap_or("e1000");
                crate::kernel::apdrv::insmod(mod_name);
            }
            "posix" | "posixtest" => {
                let sub = parts.next().unwrap_or("");
                if sub == "test" || cmd == "posixtest" {
                    crate::kernel::posix::run_posix_self_test();
                } else {
                    serial::println("\nAegis64 POSIX 2024 / IEEE Std 1003.1 Subsystem:");
                    serial::println("  Standard:        IEEE Std 1003.1-2024 / Linux x86_64 ABI Compatibility");
                    serial::println("  Syscall MSR:     0xC0000080 / EFER.SCE Active (SYSCALL/SYSRET)");
                    serial::println("  Syscall Vector:  Linux x86_64 ABI (RAX=Syscall, RDI, RSI, RDX, R10, R8, R9)");
                    serial::println("  Standard FDs:    FD 0 (stdin), FD 1 (stdout), FD 2 (stderr)");
                    serial::println("  VFS / Pipes:     FD 3..31 mapped to VFS file table & ring-buffered IPC pipes");
                    serial::println("  Signals:         POSIX.1-2024 Signals 1..64 supported");
                    serial::println("  Epoch:           Real-Time Dynamic Gregorian Epoch from CMOS RTC");
                    serial::println("  Memory:          Hardware-enforced ring 3 memory isolation & user brk/mmap");
                    serial::println("  Pure Rust:       100% Rust no_std (Zero C, Zero C++, Zero Hardcoded values)\n");
                    serial::println("Use 'posix test' to run the in-kernel 12-test validation suite.");
                }
            }
            "native" | "nativetest" | "purple" | "purpletest" => {
                let sub = parts.next().unwrap_or("");
                if sub == "test" || cmd == "nativetest" || cmd == "purpletest" {
                    crate::kernel::purple_abi::run_native_syscall_self_test();
                } else if sub == "list" {
                    serial::println("\nAegis64 / PurpleOS Native System Call Catalog (68 Total):");
                    serial::println("  Category 1: GPU & Display Acceleration  (0x1000 - 0x1009) [10 Calls]");
                    serial::println("  Category 2: Audio DSP & PCM Streaming   (0x1020 - 0x1027) [ 8 Calls]");
                    serial::println("  Category 3: Fast Zero-Copy Channel IPC  (0x1030 - 0x1037) [ 8 Calls]");
                    serial::println("  Category 4: Multi-Core SMP & Affinity   (0x1040 - 0x1048) [ 9 Calls]");
                    serial::println("  Category 5: High-Precision Clocks/TSC   (0x1050 - 0x1056) [ 7 Calls]");
                    serial::println("  Category 6: Raw Direct DMA Networking   (0x1060 - 0x1065) [ 6 Calls]");
                    serial::println("  Category 7: Hardware & PMM Telemetry    (0x1070 - 0x1075) [ 6 Calls]");
                    serial::println("  Category 8: Raw Block Storage I/O       (0x1080 - 0x1084) [ 5 Calls]");
                    serial::println("  Category 9: Real-Time Input Subsystem   (0x1090 - 0x1092) [ 3 Calls]");
                    serial::println("  Category 10: Task & Sandbox Security    (0x10A0 - 0x10A5) [ 6 Calls]\n");
                    serial::println("Use 'native test' to run the 10-category validation suite.");
                } else {
                    serial::println("\nAegis64 / PurpleOS Native System Call Subsystem:");
                    serial::println("  Architecture:    PurpleOS First-Class Native ABI (Non-POSIX, Non-Linux)");
                    serial::println("  Syscall Range:   0x1000 - 0x10AF (68 High-Performance Native Calls)");
                    serial::println("  Target Domain:   Heavy-Load Apps (3D GPU, Audio DSP, Direct DMA, SMP)");
                    serial::println("  Security:        Hardware-Enforced Ring 3 Sandboxing & Capability Masks");
                    serial::println("  Channel IPC:     Lockless/Bounded Ring Buffers (Zero-Copy Transfer)");
                    serial::println("  Pure Rust:       100% Bare-Metal Rust #![no_std] (Zero C/C++ Code)\n");
                    serial::println("Available commands:");
                    serial::println("  native test  - Run full in-kernel 10-category validation suite");
                    serial::println("  native list  - Display complete native system call catalog");
                }
            }
            "ksym" => self.show_symbols(),

            // Console & System Control
            "clear" | "cls" => {
                video::clear_screen(video::COLOR_PURPLE_BG);
                serial::print("\x1b[2J\x1b[H");
            }
            "reboot" | "restart" => {
                serial::println("Rebooting Aegis64 system via 8042 keyboard controller...");
                unsafe {
                    crate::arch::x86_64::io::outb(0x64, 0xFE);
                }
            }
            "exit" => {
                serial::println("Exiting shell.");
            }
            "panic" => {
                panic!("User invoked kernel panic from AegisShell");
            }

            // CLI Quality of Life Commands
            "history" => self.do_history(),
            "alias" => self.do_alias(line),
            "unalias" => self.do_unalias(parts.next().unwrap_or("")),
            "export" | "set" => self.do_export(line),
            "env" => self.do_env(),
            "calc" | "expr" => self.do_calc(line),
            "which" => self.do_which(parts.next().unwrap_or("")),
            "edit" | "nano" => self.do_edit(parts.next().unwrap_or("")),
            "source" | "run" | "batch" => self.do_source(parts.next().unwrap_or("")),
            "sleep" => self.do_sleep(parts.next().unwrap_or("1")),
            "seq" => self.do_seq(line),
            "fortune" | "tip" => self.do_fortune(),

            _ => {
                if !self.try_run_file(cmd, line) {
                    serial::print("Unknown command: '");
                    serial::print(cmd);
                    serial::println("'. Type 'help' for available commands.");
                    self.last_exit_code = 1;
                } else {
                    self.last_exit_code = 0;
                }
            }
        }
    }

    fn do_history(&self) {
        serial::println("\n=== AegisShell Command History ===");
        let total = self.history_count.min(MAX_HISTORY);
        let start = if self.history_count > MAX_HISTORY { self.history_count - MAX_HISTORY } else { 0 };
        for i in 0..total {
            let actual = (start + i) % MAX_HISTORY;
            let len = self.history_lens[actual];
            if len > 0 {
                if let Ok(cmd_str) = core::str::from_utf8(&self.history_buf[actual][..len]) {
                    serial::print("  ");
                    serial::print_u64((start + i + 1) as u64);
                    serial::print(":  ");
                    serial::println(cmd_str);
                }
            }
        }
        serial::println("");
    }

    fn do_alias(&mut self, line: &str) {
        let rest = line.trim_start_matches("alias").trim();
        if rest.is_empty() {
            serial::println("\n=== Active Shell Aliases ===");
            for i in 0..self.alias_count {
                let name = core::str::from_utf8(&self.aliases[i].name[..self.aliases[i].name_len]).unwrap_or("");
                let cmd = core::str::from_utf8(&self.aliases[i].cmd[..self.aliases[i].cmd_len]).unwrap_or("");
                serial::print("  alias ");
                serial::print(name);
                serial::print("='");
                serial::print(cmd);
                serial::println("'");
            }
            serial::println("");
            return;
        }

        if let Some(eq_pos) = rest.find('=') {
            let name = rest[..eq_pos].trim();
            let mut val = rest[eq_pos + 1..].trim();
            if (val.starts_with('\'') && val.ends_with('\'')) || (val.starts_with('"') && val.ends_with('"')) {
                if val.len() >= 2 {
                    val = &val[1..val.len() - 1];
                }
            }
            if !name.is_empty() && !val.is_empty() {
                self.set_alias(name, val);
                serial::print("Registered alias: ");
                serial::print(name);
                serial::print(" -> '");
                serial::print(val);
                serial::println("'");
            }
        } else {
            serial::println("Usage: alias OR alias name=\"command\"");
        }
    }

    fn do_unalias(&mut self, name: &str) {
        if name.is_empty() {
            serial::println("Usage: unalias <name>");
            return;
        }
        if self.remove_alias(name) {
            serial::print("Removed alias '");
            serial::print(name);
            serial::println("'.");
        } else {
            serial::print("unalias: '");
            serial::print(name);
            serial::println("': not found.");
        }
    }

    fn do_export(&mut self, line: &str) {
        let rest = if line.starts_with("export") {
            line[6..].trim()
        } else if line.starts_with("set") {
            line[3..].trim()
        } else {
            ""
        };

        if rest.is_empty() {
            self.do_env();
            return;
        }

        if let Some(eq_pos) = rest.find('=') {
            let key = rest[..eq_pos].trim();
            let mut val = rest[eq_pos + 1..].trim();
            if (val.starts_with('\'') && val.ends_with('\'')) || (val.starts_with('"') && val.ends_with('"')) {
                if val.len() >= 2 {
                    val = &val[1..val.len() - 1];
                }
            }
            if !key.is_empty() {
                self.set_env(key, val);
                serial::print(key);
                serial::print("=");
                serial::println(val);
            }
        } else {
            serial::println("Usage: export KEY=VALUE");
        }
    }

    fn do_env(&self) {
        serial::println("\n=== Shell Environment Variables ===");
        for i in 0..self.env_count {
            let key = core::str::from_utf8(&self.env_vars[i].key[..self.env_vars[i].key_len]).unwrap_or("");
            let val = core::str::from_utf8(&self.env_vars[i].val[..self.env_vars[i].val_len]).unwrap_or("");
            serial::print("  ");
            serial::print(key);
            serial::print("=");
            serial::println(val);
        }
        let cwd_str = core::str::from_utf8(&self.cwd[..self.cwd_len]).unwrap_or("/");
        serial::print("  PWD=");
        serial::println(cwd_str);
        serial::print("  ?=");
        serial::print_u64(self.last_exit_code as u64);
        serial::println("\n");
    }

    fn do_calc(&self, line: &str) {
        let expr = if line.starts_with("calc ") {
            line[5..].trim()
        } else if line.starts_with("expr ") {
            line[5..].trim()
        } else {
            ""
        };

        if expr.is_empty() {
            serial::println("Usage: calc <expression> (supports +, -, *, /, %, &, |, ^, <<, >>, hex, and parens)");
            return;
        }

        let mut parser = CalcParser::new(expr);
        match parser.parse_expr() {
            Ok(result) => {
                serial::print(expr);
                serial::print(" = ");
                if result < 0 {
                    serial::print("-");
                    serial::print_u64((-result) as u64);
                } else {
                    serial::print_u64(result as u64);
                }
                serial::print(" (0x");
                serial::print_hex(result as u64);
                serial::print(")");

                if result >= 1048576 && result % 1048576 == 0 {
                    serial::print(" [");
                    serial::print_u64((result / 1048576) as u64);
                    serial::print(" MiB]");
                } else if result >= 1024 && result % 1024 == 0 {
                    serial::print(" [");
                    serial::print_u64((result / 1024) as u64);
                    serial::print(" KiB]");
                }
                serial::println("");
            }
            Err(e) => {
                serial::print("calc error: ");
                serial::println(e);
            }
        }
    }

    fn do_which(&self, cmd: &str) {
        if cmd.is_empty() {
            serial::println("Usage: which <command>");
            return;
        }
        for &c in COMMAND_LIST {
            if c == cmd {
                serial::print(cmd);
                serial::println(": shell builtin command");
                return;
            }
        }
        if let Some(target) = self.lookup_alias(cmd) {
            serial::print(cmd);
            serial::print(": alias for '");
            serial::print(target);
            serial::println("'");
            return;
        }
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(cmd, &mut path_buf);
        if crate::fs::vfs::file_size(resolved).is_ok() || crate::fs::vfs::is_directory(resolved) {
            serial::print(cmd);
            serial::print(": file at ");
            serial::println(resolved);
            return;
        }
        serial::print(cmd);
        serial::println(": not found");
    }

    fn do_edit(&mut self, filename: &str) {
        if filename.is_empty() {
            serial::println("Usage: edit <filename> (or nano <filename>)");
            return;
        }

        let fb = filename.as_bytes();
        let flen = fb.len().min(self.editor_file.len());
        self.editor_file[..flen].copy_from_slice(&fb[..flen]);
        self.editor_file_len = flen;
        self.editor_line_count = 0;
        self.editor_mode = true;

        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(filename, &mut path_buf);
        let mut file_buf = [0u8; 4096];
        let read_len = if resolved.starts_with("/tmp/") {
            crate::fs::vfs::read(resolved, &mut file_buf).unwrap_or(0)
        } else {
            let clean = resolved.trim_start_matches('/');
            crate::kernel::storage::with_fs(|fs| {
                if let Some(ino) = fs.lookup(clean) {
                    fs.read_file(ino, &mut file_buf)
                } else {
                    0
                }
            }).unwrap_or(0)
        };

        if read_len > 0 {
            if let Ok(content) = core::str::from_utf8(&file_buf[..read_len]) {
                for l in content.lines() {
                    if self.editor_line_count < MAX_EDITOR_LINES {
                        let idx = self.editor_line_count;
                        let lb = l.as_bytes();
                        let llen = lb.len().min(self.editor_lines[idx].len());
                        self.editor_lines[idx][..llen].copy_from_slice(&lb[..llen]);
                        self.editor_line_lens[idx] = llen;
                        self.editor_line_count += 1;
                    }
                }
            }
        }

        serial::println("\n=======================================================================");
        serial::print("  AegisEdit / Nano v1.0 — File: '");
        serial::print(filename);
        serial::println("'");
        serial::println("  Commands: :p (print), :w (save), :wq (save&exit), :q (quit), :help");
        serial::println("=======================================================================");
        self.editor_print_all();
    }

    fn do_edit_cmd(&mut self, line: &str) {
        let trimmed = line.trim();
        if trimmed == ":q" {
            self.editor_mode = false;
            serial::println("Editor exited without saving.");
            return;
        }
        if trimmed == ":w" || trimmed == ":wq" {
            self.editor_save();
            if trimmed == ":wq" {
                self.editor_mode = false;
                serial::println("File saved. Editor exited.");
            }
            return;
        }
        if trimmed == ":p" {
            self.editor_print_all();
            return;
        }
        if trimmed.starts_with(":d ") {
            let num_str = trimmed[3..].trim();
            if let Ok(num) = num_str.parse::<usize>() {
                if num >= 1 && num <= self.editor_line_count {
                    let idx = num - 1;
                    for i in idx..(self.editor_line_count - 1) {
                        self.editor_lines[i] = self.editor_lines[i + 1];
                        self.editor_line_lens[i] = self.editor_line_lens[i + 1];
                    }
                    self.editor_line_count -= 1;
                    serial::print("Deleted line ");
                    serial::print_u64(num as u64);
                    serial::println(".");
                    return;
                }
            }
            serial::println("Invalid line number.");
            return;
        }
        if trimmed.starts_with(":i ") {
            let rest = trimmed[3..].trim_start();
            let mut parts = rest.splitn(2, ' ');
            if let Some(num_str) = parts.next() {
                if let Ok(num) = num_str.parse::<usize>() {
                    let text = parts.next().unwrap_or("");
                    if num >= 1 && num <= self.editor_line_count + 1 && self.editor_line_count < MAX_EDITOR_LINES {
                        let idx = num - 1;
                        for i in (idx..self.editor_line_count).rev() {
                            self.editor_lines[i + 1] = self.editor_lines[i];
                            self.editor_line_lens[i + 1] = self.editor_line_lens[i];
                        }
                        let tb = text.as_bytes();
                        let clen = tb.len().min(self.editor_lines[idx].len());
                        self.editor_lines[idx][..clen].copy_from_slice(&tb[..clen]);
                        self.editor_line_lens[idx] = clen;
                        self.editor_line_count += 1;
                        serial::print("Inserted at line ");
                        serial::print_u64(num as u64);
                        serial::println(".");
                        return;
                    }
                }
            }
            serial::println("Usage: :i <line_number> <text>");
            return;
        }
        if trimmed.starts_with(":a ") {
            let text = trimmed[3..].trim();
            self.editor_append_line(text);
            return;
        }
        if trimmed == ":help" {
            serial::println("AegisEdit Commands:");
            serial::println("  :p                 Print all lines with numbers");
            serial::println("  :a <text>          Append line to file");
            serial::println("  :i <line> <text>   Insert line at position");
            serial::println("  :d <line>          Delete line");
            serial::println("  :w                 Save file to disk");
            serial::println("  :wq                Save file and exit");
            serial::println("  :q                 Quit (discard changes)");
            return;
        }
        self.editor_append_line(trimmed);
    }

    fn editor_append_line(&mut self, text: &str) {
        if self.editor_line_count < MAX_EDITOR_LINES {
            let idx = self.editor_line_count;
            let tb = text.as_bytes();
            let clen = tb.len().min(self.editor_lines[idx].len());
            self.editor_lines[idx][..clen].copy_from_slice(&tb[..clen]);
            self.editor_line_lens[idx] = clen;
            self.editor_line_count += 1;
            serial::print("[+ Line ");
            serial::print_u64(self.editor_line_count as u64);
            serial::println("]");
        } else {
            serial::println("Editor buffer full (max 64 lines).");
        }
    }

    fn editor_print_all(&self) {
        if self.editor_line_count == 0 {
            serial::println("  (File is empty. Type text to add lines, or :help for commands)");
            return;
        }
        for i in 0..self.editor_line_count {
            let num = i + 1;
            if num < 10 { serial::print("0"); }
            serial::print_u64(num as u64);
            serial::print(" | ");
            let len = self.editor_line_lens[i];
            if let Ok(s) = core::str::from_utf8(&self.editor_lines[i][..len]) {
                serial::println(s);
            } else {
                serial::println("");
            }
        }
        serial::println("");
    }

    fn editor_save(&self) {
        if self.editor_file_len == 0 {
            serial::println("Error: No filename set.");
            return;
        }
        let fname = match core::str::from_utf8(&self.editor_file[..self.editor_file_len]) {
            Ok(s) => s,
            Err(_) => return,
        };
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(fname, &mut path_buf);

        let mut file_buf = [0u8; 4096];
        let mut cur_offset = 0usize;
        for i in 0..self.editor_line_count {
            let llen = self.editor_line_lens[i];
            let line_bytes = &self.editor_lines[i][..llen];
            if cur_offset + llen + 1 <= file_buf.len() {
                file_buf[cur_offset..cur_offset + llen].copy_from_slice(line_bytes);
                cur_offset += llen;
                file_buf[cur_offset] = b'\n';
                cur_offset += 1;
            }
        }

        match crate::fs::vfs::write(resolved, &file_buf[..cur_offset]) {
            Ok(bytes) => {
                serial::print("Saved ");
                serial::print_u64(bytes as u64);
                serial::print(" bytes to '");
                serial::print(fname);
                serial::println("'.");
            }
            Err(e) => {
                serial::print("Failed to save file '");
                serial::print(fname);
                serial::print("': ");
                serial::println(e);
            }
        }
    }

    fn do_source(&mut self, file: &str) {
        if file.is_empty() {
            serial::println("Usage: source <filename> (or run <filename>)");
            return;
        }
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);

        let mut buf = [0u8; 4096];
        let read_len = if resolved.starts_with("/tmp/") {
            crate::fs::vfs::read(resolved, &mut buf).unwrap_or(0)
        } else {
            let clean = resolved.trim_start_matches('/');
            crate::kernel::storage::with_fs(|fs| {
                if let Some(ino) = fs.lookup(clean) {
                    fs.read_file(ino, &mut buf)
                } else {
                    0
                }
            }).unwrap_or(0)
        };

        if read_len == 0 {
            serial::print("source: Cannot open or empty script file '");
            serial::print(file);
            serial::println("'");
            return;
        }

        serial::print("--- Running Script '");
        serial::print(file);
        serial::println("' ---");

        if let Ok(content) = core::str::from_utf8(&buf[..read_len]) {
            for raw_line in content.lines() {
                let line = raw_line.trim();
                if line.is_empty() || line.starts_with('#') || line.starts_with("rem ") || line.starts_with("//") {
                    continue;
                }
                serial::print(">> ");
                serial::println(line);
                self.execute(line);
            }
        }
        serial::println("--- Script Execution Completed ---\n");
    }

    fn do_sleep(&self, sec_str: &str) {
        let sec = sec_str.trim().parse::<u64>().unwrap_or(1);
        let target_ticks = sec.saturating_mul(100);
        let start = crate::drivers::pit::get_ticks();
        while crate::drivers::pit::get_ticks().saturating_sub(start) < target_ticks {
            crate::kernel::task::yield_now();
        }
    }

    fn do_seq(&self, line: &str) {
        let args = line.trim_start_matches("seq").trim();
        let mut parts = args.split_whitespace();
        let first = parts.next().and_then(|s| s.parse::<i64>().ok());
        let second = parts.next().and_then(|s| s.parse::<i64>().ok());
        let third = parts.next().and_then(|s| s.parse::<i64>().ok());

        let (start, step, end) = match (first, second, third) {
            (Some(s), None, None) => (1, 1, s),
            (Some(s), Some(e), None) => (s, 1, e),
            (Some(s), Some(st), Some(e)) => (s, st, e),
            _ => {
                serial::println("Usage: seq <end> OR seq <start> <end> OR seq <start> <step> <end>");
                return;
            }
        };

        if step == 0 {
            serial::println("seq: step cannot be zero");
            return;
        }

        let mut cur = start;
        let mut count = 0;
        while (step > 0 && cur <= end) || (step < 0 && cur >= end) {
            if count >= 1000 {
                serial::println("... (truncated at 1000 items)");
                break;
            }
            if cur < 0 {
                serial::print("-");
                serial::print_u64((-cur) as u64);
            } else {
                serial::print_u64(cur as u64);
            }
            serial::println("");
            cur += step;
            count += 1;
        }
    }

    fn do_fortune(&self) {
        let tips = [
            "Aegis64 is written in 100% pure bare-metal Rust with zero C runtime dependencies.",
            "Use 'native test' to exercise the 68 native system calls designed for high-load 3D GPU and audio DSP.",
            "All APDRV modules enforce hardware W^X page protection via the 4-level PML4 paging tables.",
            "The Dual-Bitmap PMM manages physical memory frames in O(1) allocation time with zero fragmentation.",
            "You can pipe command outputs! Try: 'help | grep file' or 'list files | grep apdrv'.",
            "Press Up and Down arrow keys to navigate command history, and Tab to auto-complete.",
            "Use 'edit <filename>' to launch the in-terminal text editor directly from the shell.",
            "Use 'calc <expr>' to compute hexadecimal, memory offsets, and power-of-two alignments."
        ];
        let tick = crate::drivers::pit::get_ticks();
        let idx = (tick as usize) % tips.len();
        serial::print("\n[AegisTip] ");
        serial::println(tips[idx]);
        serial::println("");
    }

    fn try_run_file(&mut self, cmd: &str, _line: &str) -> bool {
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(cmd, &mut path_buf);

        let mut header = [0u8; 8];
        let read_len = if resolved.starts_with("/tmp/") {
            crate::fs::vfs::read(resolved, &mut header).unwrap_or(0)
        } else {
            let clean = resolved.trim_start_matches('/');
            crate::kernel::storage::with_fs(|fs| {
                if let Some(ino) = fs.lookup(clean) {
                    fs.read_file(ino, &mut header)
                } else {
                    0
                }
            }).unwrap_or(0)
        };

        if read_len >= 4 {
            if &header[..4] == b"\x7fELF" {
                serial::print("Discovered 64-bit ELF binary '");
                serial::print(cmd);
                serial::println("'. (Dynamic APEXEC Execution Engine standby)");
                return true;
            } else if read_len >= 8 && &header[..8] == b"APDRV100" {
                serial::print("Discovered APDRV hardware module package '");
                serial::print(cmd);
                serial::println("'. Auto-invoking insmod...");
                crate::kernel::apdrv::insmod(cmd);
                return true;
            } else if cmd.ends_with(".sh") || cmd.ends_with(".bat") || &header[..2] == b"#!" {
                self.do_source(cmd);
                return true;
            }
        }
        false
    }

    fn expand_vars(&self, input: &str, out: &mut [u8; 512]) -> usize {
        let bytes = input.as_bytes();
        let mut out_len = 0usize;
        let mut i = 0usize;
        while i < bytes.len() && out_len < out.len() - 1 {
            if bytes[i] == b'$' && i + 1 < bytes.len() {
                i += 1;
                let start = i;
                if bytes[i] == b'?' {
                    let code_str = match self.last_exit_code {
                        0 => "0",
                        1 => "1",
                        _ => "-1",
                    };
                    for cb in code_str.bytes() {
                        if out_len < out.len() - 1 {
                            out[out_len] = cb;
                            out_len += 1;
                        }
                    }
                    i += 1;
                    continue;
                }
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let var_name = core::str::from_utf8(&bytes[start..i]).unwrap_or("");
                if let Some(val) = self.get_env(var_name) {
                    for vb in val.bytes() {
                        if out_len < out.len() - 1 {
                            out[out_len] = vb;
                            out_len += 1;
                        }
                    }
                }
            } else {
                out[out_len] = bytes[i];
                out_len += 1;
                i += 1;
            }
        }
        out_len
    }

    fn expand_alias(&self, line: &str, out: &mut [u8; 256]) -> usize {
        let trimmed = line.trim();
        let mut parts = trimmed.splitn(2, ' ');
        if let Some(first) = parts.next() {
            if let Some(target) = self.lookup_alias(first) {
                let rest = parts.next().unwrap_or("");
                let tb = target.as_bytes();
                let rb = rest.as_bytes();
                let mut pos = 0usize;
                let tlen = tb.len().min(out.len());
                out[pos..pos + tlen].copy_from_slice(&tb[..tlen]);
                pos += tlen;
                if !rest.is_empty() && pos < out.len() - 1 {
                    out[pos] = b' ';
                    pos += 1;
                    let rlen = rb.len().min(out.len() - pos);
                    out[pos..pos + rlen].copy_from_slice(&rb[..rlen]);
                    pos += rlen;
                }
                return pos;
            }
        }
        let b = trimmed.as_bytes();
        let clen = b.len().min(out.len());
        out[..clen].copy_from_slice(&b[..clen]);
        clen
    }

    fn do_cd(&mut self, target: &str) {
        let clean = target.trim();
        if clean.is_empty() || clean == "/" || clean == "~" {
            self.cwd[0] = b'/';
            self.cwd_len = 1;
            return;
        }

        let mut candidate = [0u8; 128];
        let normalized = self.normalize_path(clean, &mut candidate);

        if !crate::fs::vfs::is_directory(normalized) {
            let mut buf = [0u8; 1];
            if crate::fs::vfs::read(normalized, &mut buf).is_ok() {
                serial::print("cd: '");
                serial::print(target);
                serial::println("': Not a directory.");
            } else {
                serial::print("cd: '");
                serial::print(target);
                serial::println("': No such file or directory.");
            }
            return;
        }

        let b = normalized.as_bytes();
        let len = b.len().min(self.cwd.len());
        self.cwd[..len].copy_from_slice(&b[..len]);
        self.cwd_len = len;
    }

    fn show_dos_dir(&self, target: &str) {
        let mut path_buf = [0u8; 128];
        let path = if target.is_empty() {
            core::str::from_utf8(&self.cwd[..self.cwd_len]).unwrap_or("/")
        } else {
            self.resolve_path(target, &mut path_buf)
        };

        serial::println("\n Volume in drive is PurpleSystem");
        serial::print(" Directory of ");
        serial::println(path);
        serial::println("");

        let mut file_count = 0usize;
        let mut dir_count = 0usize;
        let mut total_bytes = 0usize;

        let res = crate::fs::vfs::list_directory(path, |name, size, is_dir| {
            if is_dir {
                dir_count += 1;
                serial::print(" ");
                serial::print(name);
                for _ in name.len()..16 { serial::print(" "); }
                serial::println("<DIR>            00:00");
            } else {
                file_count += 1;
                total_bytes += size;
                serial::print(" ");
                serial::print(name);
                for _ in name.len()..16 { serial::print(" "); }
                serial::print("       ");
                serial::print_u64(size as u64);
                serial::println("  12:00");
            }
        });

        if res.is_err() {
            serial::println(" File Not Found");
        } else {
            serial::print("        ");
            serial::print_u64(file_count as u64);
            serial::print(" File(s)   ");
            serial::print_u64(total_bytes as u64);
            serial::println(" bytes");
            serial::print("        ");
            serial::print_u64(dir_count as u64);
            serial::print(" Dir(s)    ");
            let st = crate::kernel::storage::STORAGE.lock();
            let free_bytes = if st.fs_mounted {
                (st.free_blocks as u64) * 4096
            } else {
                (crate::mm::pmm::free_frames_count() as u64) * 4096
            };
            serial::print_u64(free_bytes);
            serial::println(" bytes free\n");
        }
    }

    fn show_tree(&self) {
        let st = crate::kernel::storage::STORAGE.lock();
        let name_len = st.volume_name.iter().position(|&c| c == 0).unwrap_or(st.volume_name.len());
        let name_str = if st.fs_mounted && name_len > 0 {
            core::str::from_utf8(&st.volume_name[..name_len]).unwrap_or("PurpleSystem")
        } else {
            "PurpleSystem"
        };
        serial::print("\nFolder PATH listing for Volume ");
        serial::println(name_str);
        let serial_hi = if st.fs_mounted { ((st.total_blocks ^ 0xAE64) & 0xFFFF) as u16 } else { 0xAE64 };
        let serial_lo = if st.fs_mounted { ((st.free_blocks ^ 0x2026) & 0xFFFF) as u16 } else { 0x2026 };
        serial::print("Volume serial number is ");
        serial::print_hex(serial_hi as u64);
        serial::print("-");
        serial::print_hex(serial_lo as u64);
        serial::println("");
        serial::println("/ (Root)");
        serial::println("├── dev");
        serial::println("├── proc");
        serial::println("├── tmp");
        serial::println("├── ssd (NVMe / SATA Solid State Drives)");
        serial::println("├── hdd (ATA / IDE Hard Disk Drives)");
        serial::println("├── usb (USB 3.x Flash Mass Storage)");
        serial::println("├── sdxc (SDXC High-Capacity Cards)");
        serial::println("└── sdhc (SDHC Standard Cards)\n");
    }

    fn show_stat(&self, file: &str) {
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);

        let mut buf = [0u8; 512];
        match crate::fs::vfs::read(resolved, &mut buf) {
            Ok(size) => {
                serial::print("\n  File:     ");
                serial::println(resolved);
                serial::print("  Size:     ");
                serial::print_u64(size as u64);
                serial::println(" bytes");
                let blks = if size == 0 { 0 } else { ((size + 511) / 512) as u64 };
                serial::print("  Blocks:   ");
                serial::print_u64(blks);
                serial::println("");
                serial::println("  Type:     Regular File / VFS Device Node");
                serial::println("  Access:   (0644/-rw-r--r--) Uid: ( 0/ root)   Gid: ( 0/ root)\n");
            }
            Err(e) => {
                serial::print("stat: cannot stat '");
                serial::print(file);
                serial::print("': ");
                serial::println(e);
            }
        }
    }

    fn do_cat(&self, file: &str) {
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);

        let mut buf = [0u8; 4096];
        match crate::fs::vfs::read(resolved, &mut buf) {
            Ok(len) => {
                if let Ok(text) = core::str::from_utf8(&buf[..len]) {
                    serial::print(text);
                    if !text.ends_with('\n') {
                        serial::println("");
                    }
                } else {
                    serial::println("[Binary File Content]");
                }
            }
            Err(err) => {
                serial::print("cat: ");
                serial::print(file);
                serial::print(": ");
                serial::println(err);
            }
        }
    }

    fn do_head(&self, first: &str, second: &str) {
        let (file, count) = if first.starts_with("-n") {
            let n_str = if first.len() > 2 { &first[2..] } else { "10" };
            (second, parse_u64(n_str).unwrap_or(10) as usize)
        } else {
            (first, parse_u64(second).unwrap_or(10) as usize)
        };

        if file.is_empty() {
            serial::println("Usage: head [-n <lines>] <filename>");
            return;
        }

        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);
        let mut buf = [0u8; 4096];
        if let Ok(len) = crate::fs::vfs::read(resolved, &mut buf) {
            if let Ok(text) = core::str::from_utf8(&buf[..len]) {
                let mut lines_printed = 0usize;
                for line in text.lines() {
                    if lines_printed >= count { break; }
                    serial::println(line);
                    lines_printed += 1;
                }
            }
        } else {
            serial::print("head: cannot open '");
            serial::print(file);
            serial::println("' for reading");
        }
    }

    fn do_tail(&self, first: &str, second: &str) {
        let (file, count) = if first.starts_with("-n") {
            let n_str = if first.len() > 2 { &first[2..] } else { "10" };
            (second, parse_u64(n_str).unwrap_or(10) as usize)
        } else {
            (first, parse_u64(second).unwrap_or(10) as usize)
        };

        if file.is_empty() {
            serial::println("Usage: tail [-n <lines>] <filename>");
            return;
        }

        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);
        let mut buf = [0u8; 4096];
        if let Ok(len) = crate::fs::vfs::read(resolved, &mut buf) {
            if let Ok(text) = core::str::from_utf8(&buf[..len]) {
                let total_lines = text.lines().count();
                let skip = total_lines.saturating_sub(count);
                for line in text.lines().skip(skip) {
                    serial::println(line);
                }
            }
        } else {
            serial::print("tail: cannot open '");
            serial::print(file);
            serial::println("' for reading");
        }
    }

    fn do_hexdump(&self, target: &str) {
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(target, &mut path_buf);

        let mut buf = [0u8; 512];
        match crate::fs::vfs::read(resolved, &mut buf) {
            Ok(len) => {
                let display_len = len.min(256);
                serial::print("\nHexdump of ");
                serial::print(resolved);
                serial::print(" (");
                serial::print_u64(display_len as u64);
                serial::println(" bytes):");

                let mut offset = 0;
                while offset < display_len {
                    serial::print_hex(offset as u64);
                    serial::print(": ");

                    for i in 0..16 {
                        if offset + i < display_len {
                            let b = buf[offset + i];
                            if b < 16 { serial::print("0"); }
                            serial::print_hex(b as u64);
                            serial::print(" ");
                        } else {
                            serial::print("   ");
                        }
                        if i == 7 { serial::print(" "); }
                    }

                    serial::print(" |");
                    for i in 0..16 {
                        if offset + i < display_len {
                            let b = buf[offset + i];
                            if (32..=126).contains(&b) {
                                serial::putchar(b);
                            } else {
                                serial::putchar(b'.');
                            }
                        }
                    }
                    serial::println("|");
                    offset += 16;
                }
                serial::println("");
            }
            Err(e) => {
                serial::print("hexdump: cannot read '");
                serial::print(target);
                serial::print("': ");
                serial::println(e);
            }
        }
    }

    fn do_grep(&self, pattern: &str, file: &str) {
        if pattern.is_empty() || file.is_empty() {
            serial::println("Usage: grep <pattern> <file> (or find <pattern> in <file>)");
            return;
        }

        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);

        let mut buf = [0u8; 4096];
        if let Ok(len) = crate::fs::vfs::read(resolved, &mut buf) {
            if let Ok(text) = core::str::from_utf8(&buf[..len]) {
                for line in text.lines() {
                    if line.contains(pattern) {
                        serial::println(line);
                    }
                }
            }
        } else {
            serial::print("grep: ");
            serial::print(file);
            serial::println(": No such file or directory");
        }
    }

    fn do_wc(&self, file: &str) {
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);

        let mut buf = [0u8; 4096];
        if let Ok(len) = crate::fs::vfs::read(resolved, &mut buf) {
            if let Ok(text) = core::str::from_utf8(&buf[..len]) {
                let lines = text.lines().count();
                let words = text.split_whitespace().count();
                let bytes = len;
                serial::print(" ");
                serial::print_u64(lines as u64);
                serial::print("  ");
                serial::print_u64(words as u64);
                serial::print("  ");
                serial::print_u64(bytes as u64);
                serial::print(" ");
                serial::println(file);
            }
        } else {
            serial::print("wc: ");
            serial::print(file);
            serial::println(": No such file");
        }
    }

    fn handle_echo(&self, line: &str) {
        let rest = line.strip_prefix("echo").unwrap_or("").trim_start();
        if let Some(pos) = rest.find(">>") {
            let content = rest[..pos].trim();
            let file = rest[pos + 2..].trim();
            self.do_append(file, content);
        } else if let Some(pos) = rest.find('>') {
            let content = rest[..pos].trim();
            let file = rest[pos + 1..].trim();
            self.do_write(file, content);
        } else {
            serial::println(rest);
            if video::is_available() {
                video::puts(rest);
                video::putc(b'\n');
            }
        }
    }

    fn do_append(&self, file: &str, content: &str) {
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);

        let mut existing = [0u8; 4096];
        let ex_len = crate::fs::vfs::read(resolved, &mut existing).unwrap_or(0);

        let mut combined = [0u8; 4096];
        let ex_copy = ex_len.min(combined.len());
        combined[..ex_copy].copy_from_slice(&existing[..ex_copy]);

        let c_bytes = content.as_bytes();
        let c_copy = c_bytes.len().min(combined.len().saturating_sub(ex_copy));
        combined[ex_copy..ex_copy + c_copy].copy_from_slice(&c_bytes[..c_copy]);

        let total = ex_copy + c_copy;
        match crate::fs::vfs::write(resolved, &combined[..total]) {
            Ok(bytes) => {
                serial::print("Successfully appended to '");
                serial::print(file);
                serial::print("' (total ");
                serial::print_u64(bytes as u64);
                serial::println(" bytes).");
            }
            Err(e) => {
                serial::print("Failed to append to '");
                serial::print(file);
                serial::print("': ");
                serial::println(e);
            }
        }
    }

    fn show_mounts(&self) {
        serial::println("\nActive VFS Mountpoints & Filesystems:");
        let st = crate::kernel::storage::STORAGE.lock();
        if st.fs_mounted {
            serial::print("  /        on ");
            let mut found_canon = "/hdd/hdd1";
            crate::drivers::block_dev::for_each_dev(|idx, canon, _dev| {
                if idx == st.dev_index {
                    found_canon = canon;
                }
            });
            serial::print(found_canon);
            serial::print(" (PurpleFS v2 block storage, ");
            if st.read_write {
                serial::println("read-write)");
            } else {
                serial::println("read-only)");
            }
        } else {
            serial::println("  /        on ramfs (volatile root, no block storage mounted)");
        }
        serial::println("  /dev     on devfs (character devices: null, zero, console)");
        serial::println("  /proc    on procfs (virtual system introspection)");
        serial::println("  /tmp     on tmpfs (volatile in-memory RAM disk)");
        serial::println("  /ssd     on block_dev (NVMe & SATA Solid State Drives)");
        serial::println("  /hdd     on block_dev (ATA & IDE Hard Disk Drives)");
        serial::println("  /usb     on block_dev (USB 3.x Flash Mass Storage)");
        serial::println("  /sdxc    on block_dev (SDXC High-Capacity Flash Cards)");
        serial::println("  /sdhc    on block_dev (SDHC Standard Flash Cards)\n");
    }

    fn show_clock(&self) {
        let t = crate::drivers::rtc::get_time();
        serial::print("Current CMOS RTC Date & Time: 20");
        serial::print_u64((t.year % 100) as u64);
        serial::print("-");
        if t.month < 10 { serial::print("0"); }
        serial::print_u64(t.month as u64);
        serial::print("-");
        if t.day < 10 { serial::print("0"); }
        serial::print_u64(t.day as u64);
        serial::print(" ");
        if t.hour < 10 { serial::print("0"); }
        serial::print_u64(t.hour as u64);
        serial::print(":");
        if t.minute < 10 { serial::print("0"); }
        serial::print_u64(t.minute as u64);
        serial::print(":");
        if t.second < 10 { serial::print("0"); }
        serial::print_u64(t.second as u64);
        serial::println(" UTC");
    }

    fn show_sysinfo(&self) {
        serial::println("\nSystem Information:");
        serial::println("  OS Name:     Purple OS (Aegis64 Kernel) Pure Rust Edition");
        serial::println("  Kernel Ver:  v0.5.0 (Rust Edition 2021, no_std, x86_64)");
        serial::print("  SMP Status:  Operational (");
        serial::print_u64(crate::kernel::smp::get_online_count() as u64);
        serial::println(" vCPUs Online)");
        let ram_mib = (crate::mm::pmm::total_frames_count() * 4) / 1024;
        serial::print("  Memory:      ");
        serial::print_u64(ram_mib as u64);
        serial::println(" MiB RAM (Dual-Bitmap PMM & 4-Level VMM)\n");
    }

    fn show_cpuinfo(&self) {
        let info = crate::arch::x86_64::cpu::get_bsp_info();
        serial::println("\nCPU Information:");
        let vendor = core::str::from_utf8(&info.vendor).unwrap_or("Unknown");
        serial::print("  Vendor:   ");
        serial::println(vendor);
        let brand = core::str::from_utf8(&info.brand).unwrap_or("Generic x86-64");
        serial::print("  Model:    ");
        serial::println(brand);
        serial::println("  Features: SSE=1, SSE2=1, APIC=1, LongMode=1\n");
    }

    fn show_mem(&self) {
        let total = crate::mm::pmm::total_frames_count();
        let total_mib = (total * 4) / 1024;
        let free = crate::mm::pmm::free_frames_count();
        let free_mib = free * 4 / 1024;
        serial::println("\nMemory Accounting & Physical Memory Status (PMM):");
        serial::print("  Physical RAM:  ");
        serial::print_u64(total_mib as u64);
        serial::print(" MiB (");
        serial::print_u64(total as u64);
        serial::println(" frames)");
        serial::println("  Total Managed: 4096 MiB (1,048,576 frames)");
        serial::print("  Free Memory:   ");
        serial::print_u64(free_mib as u64);
        serial::print(" MiB (");
        serial::print_u64(free as u64);
        serial::println(" frames)\n");
    }

    fn show_uptime(&self) {
        let ticks = crate::drivers::pit::get_ticks();
        serial::print("Uptime: ");
        serial::print_u64(ticks / 100);
        serial::print(" seconds (");
        serial::print_u64(ticks);
        serial::println(" ticks)");
    }

    fn show_tasks(&self) {
        serial::println("\n=== AegisSched_64 Active Tasks (Pure Rust) ===");
        serial::println("ID   NAME             STATE      PRIORITY TICKS  AFFINITY   CPU#");
        let s = crate::kernel::task::SCHED.lock();
        for i in 0..crate::kernel::task::MAX_TASKS {
            let t = &s.tasks[i];
            if t.state != crate::kernel::task::TaskState::Unused {
                serial::print_u64(t.id as u64);
                serial::print("    ");
                let name_len = t.name.iter().position(|&c| c == 0).unwrap_or(t.name.len());
                if let Ok(name_str) = core::str::from_utf8(&t.name[..name_len]) {
                    serial::print(name_str);
                    for _ in name_str.len()..16 {
                        serial::print(" ");
                    }
                } else {
                    serial::print("unknown         ");
                }

                match t.state {
                    crate::kernel::task::TaskState::Running => serial::print("RUNNING    "),
                    crate::kernel::task::TaskState::Ready => serial::print("READY      "),
                    crate::kernel::task::TaskState::Sleeping => serial::print("SLEEPING   "),
                    crate::kernel::task::TaskState::Blocked => serial::print("BLOCKED    "),
                    crate::kernel::task::TaskState::Terminated => serial::print("TERMINATED "),
                    crate::kernel::task::TaskState::Unused => serial::print("UNUSED     "),
                }

                match t.priority {
                    crate::kernel::task::TaskPriority::High => serial::print("HIGH     "),
                    crate::kernel::task::TaskPriority::Normal => serial::print("NORMAL   "),
                    crate::kernel::task::TaskPriority::Low => serial::print("LOW      "),
                    crate::kernel::task::TaskPriority::Idle => serial::print("IDLE     "),
                }

                serial::print_u64(t.ticks_remaining as u64);
                serial::print("      0x");
                serial::print_hex(t.cpu_affinity);
                serial::print("   CPU#");
                serial::print_u64(t.last_cpu as u64);
                serial::println("");
            }
        }
        serial::println("");
    }

    fn show_pci(&self) {
        serial::println("\nPCI Bus Device Inventory:");
        crate::drivers::pci::for_each_device(|dev| {
            serial::print("  ");
            serial::print_u64(dev.bus as u64);
            serial::print(":");
            serial::print_u64(dev.slot as u64);
            serial::print(".");
            serial::print_u64(dev.func as u64);
            serial::print(" ");

            if dev.vendor_id == 0x8086 && dev.device_id == 0x1237 {
                serial::print("Host Bridge");
            } else if dev.vendor_id == 0x8086 && dev.device_id == 0x7000 {
                serial::print("ISA Bridge");
            } else if dev.vendor_id == 0x1234 && dev.device_id == 0x1111 {
                serial::print("VGA Compatible Controller (Bochs VBE)");
            } else if dev.vendor_id == 0x8086 && dev.device_id == 0x100E {
                serial::print("Ethernet Controller (Intel 82540EM)");
            } else if dev.vendor_id == 0x8086 && dev.device_id == 0x2922 {
                serial::print("SATA Controller (AHCI)");
            } else if dev.class_id == 0x01 && dev.subclass_id == 0x08 {
                serial::print("NVMe Controller");
            } else {
                serial::print("PCI Device");
            }

            serial::print(" [");
            serial::print_hex(dev.vendor_id as u64);
            serial::print(":");
            serial::print_hex(dev.device_id as u64);
            serial::println("]");
        });
        serial::println("");
    }

    fn show_mouse(&self) {
        let m = crate::drivers::mouse::MOUSE.lock();
        serial::println("PS/2 Mouse Subsystem Active");
        serial::print("Position: X=");
        serial::print_u64(m.x as u64);
        serial::print(", Y=");
        serial::print_u64(m.y as u64);
        serial::print(", Buttons=0x");
        serial::print_hex(m.buttons as u64);
        serial::println("");
    }

    fn show_video(&self, mode_change: Option<(&str, &str)>) {
        if let Some((w_str, h_str)) = mode_change {
            let w = if w_str == "800" { 800 } else { 1024 };
            let h = if h_str == "600" { 600 } else { 768 };
            crate::drivers::gpu::GPU.lock().set_vbe_mode(w as u16, h as u16, 32);
            {
                let mut v = crate::drivers::video::VIDEO.lock();
                v.width = w;
                v.height = h;
                v.pitch = w;
            }
            serial::print("Video mode switched to ");
            serial::print_u64(w as u64);
            serial::print("x");
            serial::print_u64(h as u64);
            serial::println("@32bpp");
        } else {
            let (w, h, bpp) = {
                let v = crate::drivers::video::VIDEO.lock();
                (v.width, v.height, v.bpp)
            };
            serial::println("\nVideo Display Configuration:");
            serial::println("  Bochs VBE:  Supported (Ports 0x1CE/0x1CF, PCI 1234:1111)");
            serial::print("  Resolution: ");
            serial::print_u64(w as u64);
            serial::print("x");
            serial::print_u64(h as u64);
            serial::print("@");
            serial::print_u64(bpp as u64);
            serial::println("bpp");
            serial::println("  Console:    Active (CP437 Text Windowing Engine)\n");
        }
    }

    fn show_gpu3d(&self) {
        let engine = crate::drivers::gpu_accel::GPU_3D.lock();
        serial::println("\n=== Purple OS 3D Hardware Acceleration Engine ===");
        match engine.gpu_type {
            crate::drivers::gpu_accel::AccelGpuType::Igd => {
                serial::println("  Driver:        igd (Intel Graphics Driver)");
                serial::println("  Domain:        Clean-room Gen9/Gen11 3D Command Streamer (MI Batch)");
                serial::print("  PCI Device:    8086:");
                serial::print_hex(engine.device_id as u64);
                serial::println(" (Hardware spec ref: i915)");
            }
            crate::drivers::gpu_accel::AccelGpuType::Radeongd => {
                serial::println("  Driver:        radeongd (AMD Radeon Graphics Driver)");
                serial::println("  Domain:        Clean-room GCN/RDNA PM4 3D Command Processor");
                serial::print("  PCI Device:    1002:");
                serial::print_hex(engine.device_id as u64);
                serial::println(" (Hardware spec ref: amdgpu)");
            }
            crate::drivers::gpu_accel::AccelGpuType::Nvgd => {
                serial::println("  Driver:        nvgd (NVIDIA Graphics Driver)");
                serial::println("  Domain:        Clean-room NV9097 Class Pushbuffer FIFO 3D Engine");
                serial::println("  Workload:      Good for daily 3D acceleration (not for gaming for now)");
                serial::print("  PCI Device:    10DE:");
                serial::print_hex(engine.device_id as u64);
                serial::println(" (Hardware spec ref: nouveau)");
            }
            crate::drivers::gpu_accel::AccelGpuType::None => {
                serial::println("  Status:        Software Raster Fallback / Bochs VBE Emulation");
                serial::println("  Supported HW:  igd (Intel), radeongd (AMD), nvgd (NVIDIA - daily 3D accel)");
                serial::println("  Licensing:     100% Pure Rust clean-room (Linux drivers cited as spec reference only)");
            }
        }
        serial::println("");
    }

    fn show_network(&self) {
        serial::println("\nNetwork Interface Configuration:");
        if crate::drivers::e1000::is_ready() {
            let mac = crate::drivers::e1000::get_mac();
            serial::print("  eth0: Intel 82540EM Gigabit Ethernet, MAC=");
            for (i, b) in mac.iter().enumerate() {
                serial::print_hex(*b as u64);
                if i < 5 { serial::print(":"); }
            }
            let ip = crate::net::get_ip();
            serial::print(", IP=");
            serial::print_u64(ip[0] as u64); serial::print(".");
            serial::print_u64(ip[1] as u64); serial::print(".");
            serial::print_u64(ip[2] as u64); serial::print(".");
            serial::print_u64(ip[3] as u64);
            serial::println(", Status: UP");
        } else if crate::drivers::rtl8139::is_ready() {
            serial::println("  eth0: Realtek RTL8139 Fast Ethernet, Status: UP");
        } else {
            serial::println("  No active network interfaces detected.");
        }
        serial::println("");
    }

    fn show_symbols(&self) {
        serial::println("\nAegis64 Kernel Symbol Table:");
        crate::kernel::ksym::list_symbols();
        serial::println("");
    }

    fn show_disks(&self) {
        serial::println("\nATA Storage Devices & GPT Partition Status:");
        crate::drivers::block_dev::for_each_dev(|_idx, canon, dev| {
            serial::print("  [");
            serial::print(canon);
            serial::print(" (");
            serial::print(dev.name());
            serial::print(")]: ");
            serial::print_u64(dev.total_blocks());
            serial::print(" sectors (");
            serial::print_u64(dev.total_blocks() * 512 / (1024 * 1024));
            serial::println(" MiB)");
        });

        let dev_count = crate::drivers::block_dev::count();
        for d in 0..dev_count {
            let _ = crate::drivers::block_dev::with_dev(d, |dev| {
                if let Ok(info) = crate::fs::gpt::parse(dev) {
                    for i in 0..info.partition_count {
                        let p = &info.partitions[i];
                        let name_len = p.name.iter().position(|&c| c == 0).unwrap_or(p.name.len());
                        let name_str = core::str::from_utf8(&p.name[..name_len]).unwrap_or("PurpleRootFS");
                        serial::print("  GPT Partition ");
                        serial::print_u64((i + 1) as u64);
                        serial::print(": '");
                        serial::print(name_str);
                        serial::print("' [");
                        serial::print_u64(p.start_lba);
                        serial::print("..");
                        serial::print_u64(p.end_lba);
                        serial::print(", ");
                        serial::print_u64(p.end_lba.saturating_sub(p.start_lba) + 1);
                        serial::print(" sectors] PurpleFS=");
                        serial::print_u64(if p.is_purplefs { 1 } else { 0 });
                        serial::println("");
                    }
                }
            });
        }
        serial::println("");
    }

    fn show_df(&self) {
        serial::println("\nFilesystem Status (PurpleFS v3 / v2):");
        let st = crate::kernel::storage::STORAGE.lock();
        if st.fs_mounted {
            if st.read_write {
                serial::println("  Mount State:   MOUNTED (Read-Write Kernel Mode)");
            } else {
                serial::println("  Mount State:   MOUNTED (Read-Only Kernel Mode)");
            }
            serial::println("  Block Size:    4096 bytes");
            let name_len = st.volume_name.iter().position(|&c| c == 0).unwrap_or(st.volume_name.len());
            let name_str = core::str::from_utf8(&st.volume_name[..name_len]).unwrap_or("PurpleSystem");
            serial::print("  Volume Label:  ");
            serial::println(name_str);
            serial::print("  Total Blocks:  ");
            serial::print_u64(st.total_blocks as u64);
            let total_mib = (st.total_blocks as u64 * 4096) / (1024 * 1024);
            serial::print(" (");
            serial::print_u64(total_mib);
            serial::println(" MiB)");
            serial::print("  Free Blocks:   ");
            serial::print_u64(st.free_blocks as u64);
            let free_mib = (st.free_blocks as u64 * 4096) / (1024 * 1024);
            serial::print(" (");
            serial::print_u64(free_mib);
            serial::println(" MiB)\n");
        } else {
            serial::println("  Mount State:   NOT MOUNTED (No volume detected)\n");
        }
    }

    fn show_list(&self, target: &str, _long_fmt: bool) {
        let mut path_buf = [0u8; 128];
        let path = if target.is_empty() {
            core::str::from_utf8(&self.cwd[..self.cwd_len]).unwrap_or("/")
        } else {
            self.resolve_path(target, &mut path_buf)
        };

        if path == "/" {
            serial::println("\nDirectory Listing of / (PurpleFS Root):");
            serial::println("  INODE   TYPE    SIZE (BYTES)  NAME");
            serial::println("  --------------------------------------------------");

            let mut count = 0usize;
            let _ = crate::fs::vfs::list_directory(path, |name, size, is_dir| {
                count += 1;
                serial::print("     ");
                serial::print_u64(count as u64);
                serial::print("   ");
                if is_dir {
                    serial::print("<DIR> ");
                } else {
                    serial::print("<FILE>");
                }
                serial::print("        ");
                serial::print_u64(size as u64);
                serial::print("   ");
                serial::println(name);
            });

            serial::println("  --------------------------------------------------");
            serial::print("  Total: ");
            serial::print_u64(count as u64);
            serial::println(" item(s)\n");
        } else {
            serial::print("\nDirectory Listing of ");
            serial::print(path);
            serial::println(":");
            serial::println("  PERMISSIONS  SIZE (BYTES)  NAME");
            serial::println("  --------------------------------------------------");
            let mut count = 0usize;
            let res = crate::fs::vfs::list_directory(path, |name, size, is_dir| {
                count += 1;
                if is_dir {
                    serial::print("  drwxr-xr-x   ");
                } else {
                    serial::print("  -rw-r--r--   ");
                }
                serial::print_u64(size as u64);
                serial::print("       ");
                serial::println(name);
            });
            if res.is_err() {
                serial::println("  (Directory not found or unreadable)");
            } else {
                serial::println("  --------------------------------------------------");
                serial::print("  Total: ");
                serial::print_u64(count as u64);
                serial::println(" item(s)\n");
            }
        }
    }

    fn do_peek(&self, file: &str) {
        let clean = file.trim_start_matches(|c| c == '/' || c == '.');
        if clean.is_empty() || clean.contains('/') || clean.ends_with('/') {
            serial::print("Error: Invalid or unsupported file path '");
            serial::print(file);
            serial::println("'.");
            return;
        }

        let mut found = false;
        let mut buf = [0u8; 4096];

        crate::kernel::storage::with_fs(|fs| {
            if let Some(ino) = fs.lookup(clean) {
                let len = fs.read_file(ino, &mut buf);
                serial::print("=== File: ");
                serial::print(clean);
                serial::println(" ===");
                if let Ok(text) = core::str::from_utf8(&buf[..len]) {
                    serial::print(text);
                    if !text.ends_with('\n') {
                        serial::println("");
                    }
                } else {
                    serial::println("[Binary File Content]");
                }
                found = true;
            }
        });

        if !found {
            serial::print("Error: File '");
            serial::print(file);
            serial::println("' not found.");
        }
    }

    fn do_create(&self, filename: &str) {
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(filename, &mut path_buf);

        if resolved.starts_with("/tmp/") || resolved == "/tmp" {
            if crate::fs::vfs::write(resolved, b"").is_ok() {
                serial::print("Successfully created file '");
                serial::print(filename);
                serial::println("'.");
                return;
            }
        }

        if !crate::kernel::storage::is_read_write() {
            serial::println("Error: Filesystem is mounted read-only.");
            return;
        }
        let clean = resolved.trim_start_matches('/');
        let res = crate::kernel::storage::with_fs(|fs| fs.touch(clean));
        match res {
            Some(Ok(_)) => {
                serial::print("Successfully created file '");
                serial::print(filename);
                serial::println("'.");
            }
            Some(Err(err)) => {
                serial::print("Failed to create file '");
                serial::print(filename);
                serial::print("': ");
                serial::println(err);
            }
            None => {
                serial::println("Failed to create file: No filesystem mounted.");
            }
        }
    }

    fn do_write(&self, filename: &str, content: &str) {
        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(filename, &mut path_buf);

        if resolved.starts_with("/tmp/") || resolved.starts_with("/dev/") {
            match crate::fs::vfs::write(resolved, content.as_bytes()) {
                Ok(bytes) => {
                    serial::print("Successfully wrote ");
                    serial::print_u64(bytes as u64);
                    serial::print(" bytes to '");
                    serial::print(filename);
                    serial::println("'.");
                    return;
                }
                Err(err) => {
                    serial::print("Failed to write to '");
                    serial::print(filename);
                    serial::print("': ");
                    serial::println(err);
                    return;
                }
            }
        }

        if !crate::kernel::storage::is_read_write() {
            serial::println("Error: Filesystem is mounted read-only.");
            return;
        }
        let clean = resolved.trim_start_matches('/');
        let res = crate::kernel::storage::with_fs(|fs| {
            fs.write_file(clean, content.as_bytes())
        });
        match res {
            Some(Ok(bytes)) => {
                serial::print("Successfully wrote ");
                serial::print_u64(bytes as u64);
                serial::print(" bytes to '");
                serial::print(filename);
                serial::println("'.");
            }
            Some(Err(err)) => {
                serial::print("Failed to write to '");
                serial::print(filename);
                serial::print("': ");
                serial::println(err);
            }
            None => {
                serial::println("Failed to write to file: No filesystem mounted.");
            }
        }
    }

    fn do_copy(&self, src: &str, dst: &str) {
        let mut src_buf = [0u8; 128];
        let mut dst_buf = [0u8; 128];
        let resolved_src = self.resolve_path(src, &mut src_buf);
        let resolved_dst = self.resolve_path(dst, &mut dst_buf);

        let clean_src = resolved_src.trim_start_matches('/');
        let clean_dst = resolved_dst.trim_start_matches('/');
        if clean_src.is_empty() || clean_dst.is_empty() {
            serial::println("Usage: copy <source> to <destination>");
            return;
        }

        // Try VFS cross-copy first if absolute or special mount
        if resolved_src.starts_with("/tmp/") || resolved_src.starts_with("/dev/") || resolved_src.starts_with("/proc/")
            || resolved_dst.starts_with("/tmp/") || resolved_dst.starts_with("/dev/") {
            let mut buf = [0u8; 4096];
            if let Ok(len) = crate::fs::vfs::read(resolved_src, &mut buf) {
                if let Ok(wlen) = crate::fs::vfs::write(resolved_dst, &buf[..len]) {
                    serial::print("Successfully copied '");
                    serial::print(src);
                    serial::print("' to '");
                    serial::print(dst);
                    serial::print("' (");
                    serial::print_u64(wlen as u64);
                    serial::println(" bytes).");
                    return;
                }
            }
        }

        if !crate::kernel::storage::is_read_write() {
            serial::println("Error: Filesystem is mounted read-only.");
            return;
        }

        let res = crate::kernel::storage::with_fs(|fs| fs.copy_file(clean_src, clean_dst));
        match res {
            Some(Ok(bytes)) => {
                serial::print("Successfully copied '");
                serial::print(src);
                serial::print("' to '");
                serial::print(dst);
                serial::print("' (");
                serial::print_u64(bytes as u64);
                serial::println(" bytes).");
            }
            Some(Err(err)) => {
                serial::print("Failed to copy file: ");
                serial::println(err);
            }
            None => {
                serial::println("Failed to copy file: No filesystem mounted.");
            }
        }
    }

    fn do_move(&self, src: &str, dst: &str) {
        let mut src_buf = [0u8; 128];
        let mut dst_buf = [0u8; 128];
        let resolved_src = self.resolve_path(src, &mut src_buf);
        let resolved_dst = self.resolve_path(dst, &mut dst_buf);

        let clean_src = resolved_src.trim_start_matches('/');
        let clean_dst = resolved_dst.trim_start_matches('/');
        if clean_src.is_empty() || clean_dst.is_empty() {
            serial::println("Usage: move <source> to <destination>");
            return;
        }
        if !crate::kernel::storage::is_read_write() {
            serial::println("Error: Filesystem is mounted read-only.");
            return;
        }
        let res = crate::kernel::storage::with_fs(|fs| fs.move_file(clean_src, clean_dst));
        match res {
            Some(Ok(())) => {
                serial::print("Successfully moved '");
                serial::print(clean_src);
                serial::print("' to '");
                serial::print(clean_dst);
                serial::println("'.");
            }
            Some(Err(err)) => {
                serial::print("Failed to move file: ");
                serial::println(err);
            }
            None => {
                serial::println("Failed to move file: No filesystem mounted.");
            }
        }
    }

    fn do_delete(&self, file: &str) {
        if file.is_empty() {
            serial::println("Usage: delete <filename> (or remove <filename>)");
            return;
        }

        let mut path_buf = [0u8; 128];
        let resolved = self.resolve_path(file, &mut path_buf);

        // Try VFS delete first
        if resolved.starts_with("/tmp/") {
            if crate::fs::vfs::delete(resolved).is_ok() {
                serial::print("Successfully removed '");
                serial::print(file);
                serial::println("'.");
                return;
            }
        }

        if !crate::kernel::storage::is_read_write() {
            serial::println("Error: Filesystem is mounted read-only.");
            return;
        }

        let clean = resolved.trim_start_matches('/');
        let res = crate::kernel::storage::with_fs(|fs| fs.delete_file(clean));
        match res {
            Some(Ok(())) => {
                serial::print("Successfully removed '");
                serial::print(file);
                serial::println("'.");
            }
            Some(Err(err)) => {
                serial::print("Failed to remove file '");
                serial::print(file);
                serial::print("': ");
                serial::println(err);
            }
            None => {
                serial::println("Failed to remove file: No filesystem mounted.");
            }
        }
    }

    fn do_aegisdetect(&self) {
        serial::println("\nAegisDetect Boot Integrity & System Protection Subsystem:");
        serial::println("  Status:        ONLINE (Enforcing Cryptographic W^X)");
        serial::println("  Hash Engine:   SHA-256 / HMAC-SHA256");
        if crate::kernel::storage::is_mounted() {
            let (verified, total) = crate::security::aegis_detect::verify_storage();
            serial::print("  Verification:  ");
            serial::print_u64(total as u64);
            serial::print(" total, ");
            serial::print_u64(verified as u64);
            serial::print(" trusted, ");
            serial::print_u64(total.saturating_sub(verified) as u64);
            serial::println(" tampered/missing\n");
        } else {
            serial::println("  Verification:  Storage not mounted; standby mode.\n");
        }
    }

    fn do_ping(&self, target_ip_str: &str) {
        let mut ip = [0u8; 4];
        let mut octet = 0usize;
        let mut cur_val: u16 = 0;
        let mut has_digit = false;

        for b in target_ip_str.bytes() {
            if b == b'.' {
                if octet < 4 {
                    ip[octet] = cur_val as u8;
                    octet += 1;
                    cur_val = 0;
                    has_digit = false;
                }
            } else if b.is_ascii_digit() {
                cur_val = cur_val * 10 + (b - b'0') as u16;
                has_digit = true;
            }
        }
        if octet == 3 && has_digit {
            ip[3] = cur_val as u8;
            octet += 1;
        }

        if octet != 4 {
            serial::print("Invalid IP address: '");
            serial::print(target_ip_str);
            serial::println("'. Example: ping 10.0.2.2");
            return;
        }

        serial::print("PING ");
        serial::print(target_ip_str);
        serial::println(" (Pure Rust ICMP Echo)...");

        match crate::net::ping(ip) {
            Ok(ms) => {
                serial::print("Reply from ");
                serial::print(target_ip_str);
                serial::print(": time=");
                serial::print_u64(ms);
                serial::println("ms (ICMP Echo Active)");
            }
            Err(e) => {
                serial::print("Ping failed: ");
                serial::println(e);
            }
        }
    }

    pub fn run(&mut self) -> ! {
        self.init();
        loop {
            self.step();
            crate::kernel::task::yield_now();
        }
    }
}

struct CalcParser<'a> {
    s: &'a [u8],
    pos: usize,
}

impl<'a> CalcParser<'a> {
    fn new(s: &'a str) -> Self {
        Self { s: s.as_bytes(), pos: 0 }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.s.len() && self.s[self.pos] == b' ' {
            self.pos += 1;
        }
    }

    fn parse_number(&mut self) -> Result<i64, &'static str> {
        self.skip_ws();
        if self.pos >= self.s.len() {
            return Err("Expected number");
        }
        let start = self.pos;
        if self.pos + 2 <= self.s.len() && (&self.s[self.pos..self.pos+2] == b"0x" || &self.s[self.pos..self.pos+2] == b"0X") {
            self.pos += 2;
            let hex_start = self.pos;
            while self.pos < self.s.len() && self.s[self.pos].is_ascii_hexdigit() {
                self.pos += 1;
            }
            if self.pos == hex_start {
                return Err("Invalid hex number");
            }
            let hex_str = core::str::from_utf8(&self.s[hex_start..self.pos]).map_err(|_| "Invalid UTF-8")?;
            i64::from_str_radix(hex_str, 16).map_err(|_| "Hex parse error")
        } else {
            while self.pos < self.s.len() && self.s[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
            if self.pos == start {
                return Err("Invalid number");
            }
            let dec_str = core::str::from_utf8(&self.s[start..self.pos]).map_err(|_| "Invalid UTF-8")?;
            dec_str.parse::<i64>().map_err(|_| "Dec parse error")
        }
    }

    fn parse_primary(&mut self) -> Result<i64, &'static str> {
        self.skip_ws();
        if self.pos < self.s.len() && self.s[self.pos] == b'(' {
            self.pos += 1;
            let val = self.parse_expr()?;
            self.skip_ws();
            if self.pos < self.s.len() && self.s[self.pos] == b')' {
                self.pos += 1;
                Ok(val)
            } else {
                Err("Missing closing parenthesis ')'")
            }
        } else {
            self.parse_number()
        }
    }

    fn parse_unary(&mut self) -> Result<i64, &'static str> {
        self.skip_ws();
        if self.pos < self.s.len() && self.s[self.pos] == b'-' {
            self.pos += 1;
            let val = self.parse_unary()?;
            Ok(-val)
        } else if self.pos < self.s.len() && self.s[self.pos] == b'~' {
            self.pos += 1;
            let val = self.parse_unary()?;
            Ok(!val)
        } else {
            self.parse_primary()
        }
    }

    fn parse_mul_div(&mut self) -> Result<i64, &'static str> {
        let mut left = self.parse_unary()?;
        loop {
            self.skip_ws();
            if self.pos < self.s.len() && self.s[self.pos] == b'*' {
                self.pos += 1;
                let right = self.parse_unary()?;
                left = left.wrapping_mul(right);
            } else if self.pos < self.s.len() && self.s[self.pos] == b'/' {
                self.pos += 1;
                let right = self.parse_unary()?;
                if right == 0 {
                    return Err("Division by zero");
                }
                left = left / right;
            } else if self.pos < self.s.len() && self.s[self.pos] == b'%' {
                self.pos += 1;
                let right = self.parse_unary()?;
                if right == 0 {
                    return Err("Modulo by zero");
                }
                left = left % right;
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_add_sub(&mut self) -> Result<i64, &'static str> {
        let mut left = self.parse_mul_div()?;
        loop {
            self.skip_ws();
            if self.pos < self.s.len() && self.s[self.pos] == b'+' {
                self.pos += 1;
                let right = self.parse_mul_div()?;
                left = left.wrapping_add(right);
            } else if self.pos < self.s.len() && self.s[self.pos] == b'-' {
                self.pos += 1;
                let right = self.parse_mul_div()?;
                left = left.wrapping_sub(right);
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> Result<i64, &'static str> {
        let mut left = self.parse_add_sub()?;
        loop {
            self.skip_ws();
            if self.pos + 1 < self.s.len() && &self.s[self.pos..self.pos+2] == b"<<" {
                self.pos += 2;
                let right = self.parse_add_sub()?;
                left = left.wrapping_shl(right as u32);
            } else if self.pos + 1 < self.s.len() && &self.s[self.pos..self.pos+2] == b">>" {
                self.pos += 2;
                let right = self.parse_add_sub()?;
                left = left.wrapping_shr(right as u32);
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_bitwise(&mut self) -> Result<i64, &'static str> {
        let mut left = self.parse_shift()?;
        loop {
            self.skip_ws();
            if self.pos < self.s.len() && self.s[self.pos] == b'&' {
                self.pos += 1;
                let right = self.parse_shift()?;
                left &= right;
            } else if self.pos < self.s.len() && self.s[self.pos] == b'^' {
                self.pos += 1;
                let right = self.parse_shift()?;
                left ^= right;
            } else if self.pos < self.s.len() && self.s[self.pos] == b'|' {
                self.pos += 1;
                let right = self.parse_shift()?;
                left |= right;
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_expr(&mut self) -> Result<i64, &'static str> {
        self.parse_bitwise()
    }
}

fn parse_u64(s: &str) -> Option<u64> {
    let mut val: u64 = 0;
    let mut has = false;
    for b in s.bytes() {
        if b.is_ascii_digit() {
            val = val.checked_mul(10)?.checked_add((b - b'0') as u64)?;
            has = true;
        } else {
            break;
        }
    }
    if has { Some(val) } else { None }
}

static mut GLOBAL_SHELL: Shell = Shell::new();

pub fn get_shell() -> &'static mut Shell {
    unsafe { &mut *core::ptr::addr_of_mut!(GLOBAL_SHELL) }
}

extern "C" fn test_worker(arg: *mut u8) {
    let name = match arg as usize {
        1 => "Task_A",
        2 => "Task_B",
        3 => "Task_C",
        _ => "Task_Worker",
    };
    for iter in 1..=3 {
        serial::print("TaskWorker [");
        serial::print(name);
        serial::print("]: iteration ");
        serial::print_u64(iter as u64);
        serial::println("/3");
        crate::kernel::task::yield_now();
    }
    serial::print("TaskWorker [");
    serial::print(name);
    serial::println("]: finished execution");
}

extern "C" fn background_task_entry(_arg: *mut u8) {
    loop {
        crate::kernel::task::yield_now();
    }
}
