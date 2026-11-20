//! Purple Files UI state, layout, hit-testing, and Live Aero ARGB8888 renderer.

use crate::abi::{BTN_LEFT, BTN_RIGHT};
use crate::font;
use crate::fs::{
    self, contains_ci, copy_item, create_file, delete_item, join_path, launch_app, list_dir,
    mkdir, parent_path, read_preview, rename_or_move, EntryKind, FileEntry, StackStr, MAX_ENTRIES,
    MAX_PATH,
};
use crate::gfx::{
    argb, draw_arrow_left, draw_arrow_right, draw_arrow_up, draw_file_icon, draw_folder_icon,
    draw_grid_glyph, draw_line_h, draw_line_v, draw_list_glyph, draw_mac_window_buttons_right,
    draw_purple_files_logo, fill_rect, fill_rounded_rect, fill_rounded_rect_raw,
    fill_rounded_rect_vgrad, rgb, stroke_rounded_rect,
};

const TITLEBAR_H: i32 = 44;
const TOOLBAR_H: i32 = 40;
const STATUSBAR_H: i32 = 26;
const SIDEBAR_W: i32 = 186;
const INSPECTOR_W: i32 = 236;

// Palette — Deep Obsidian-Plum Live Aero Glass + Royal Amethyst Accents
const COL_BG_CANVAS: u32 = argb(0xEA, 0x16, 0x12, 0x24);
const COL_BG_SIDEBAR: u32 = argb(0xD2, 0x11, 0x0D, 0x1C);
const COL_BG_HEADER: u32 = argb(0xDE, 0x1B, 0x15, 0x2D);
const COL_BG_TOOLBAR: u32 = argb(0xD8, 0x17, 0x13, 0x27);
const COL_BG_STATUS: u32 = argb(0xE2, 0x12, 0x0E, 0x1E);
const COL_BG_SURFACE: u32 = argb(0xCC, 0x25, 0x1F, 0x3D);
const COL_BG_SURFACE_HOVER: u32 = argb(0xE0, 0x32, 0x29, 0x52);
const COL_BG_SELECTED: u32 = argb(0xEE, 0x6D, 0x28, 0xD9);
const COL_BORDER_SUBTLE: u32 = argb(0x40, 0xA7, 0x8B, 0xFA);
const COL_BORDER_ACCENT: u32 = argb(0xCC, 0xA8, 0x55, 0xF7);
const COL_TEXT_PRIMARY: u32 = rgb(0xF5, 0xF3, 0xFF);
const COL_TEXT_SECONDARY: u32 = rgb(0xB4, 0xA9, 0xD6);
const COL_TEXT_MUTED: u32 = rgb(0x7E, 0x73, 0xA1);
const COL_ACCENT: u32 = rgb(0x93, 0x33, 0xEA);
const COL_ACCENT_LIGHT: u32 = rgb(0xC0, 0x84, 0xFC);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Grid,
    List,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    Name,
    Size,
    Type,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FocusTarget {
    None,
    PathBar,
    SearchBar,
    ModalInput,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModalKind {
    None,
    NewFolder,
    NewFile,
    Rename,
    ConfirmDelete,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ClipOp {
    None,
    Copy,
    Cut,
}

#[derive(Clone, Copy)]
struct SidebarPlace {
    label: &'static [u8],
    path: &'static [u8],
}

const PLACES: [SidebarPlace; 6] = [
    SidebarPlace { label: b"Home", path: b"/Home" },
    SidebarPlace { label: b"Root (/)", path: b"/" },
    SidebarPlace { label: b"Applications", path: b"/System/Applications" },
    SidebarPlace { label: b"System", path: b"/System" },
    SidebarPlace { label: b"Documents", path: b"/Documents" },
    SidebarPlace { label: b"Downloads", path: b"/Downloads" },
];

pub struct AppState {
    pub width: i32,
    pub height: i32,
    pub should_exit: bool,
    pub dirty: bool,
    pub is_maximized: bool,
    pub is_minimized: bool,

    // Navigation
    pub cwd: StackStr<MAX_PATH>,
    pub history: [StackStr<MAX_PATH>; 24],
    pub history_len: usize,
    pub history_idx: usize,

    // Directory entries & filtered view indices
    pub entries: [FileEntry; MAX_ENTRIES],
    pub entry_count: usize,
    pub view_indices: [usize; MAX_ENTRIES],
    pub view_count: usize,

    pub selected: Option<usize>, // index into view_indices
    pub hovered_item: Option<usize>,
    pub scroll_y: i32,
    pub view_mode: ViewMode,
    pub sort_by: SortBy,
    pub sort_asc: bool,
    pub show_inspector: bool,

    // Inputs
    focus: FocusTarget,
    path_input: StackStr<MAX_PATH>,
    search_input: StackStr<64>,

    // Clipboard
    clip_op: ClipOp,
    clip_path: StackStr<MAX_PATH>,
    clip_name: StackStr<64>,
    clip_is_dir: bool,

    // Preview cache
    preview_path: StackStr<MAX_PATH>,
    preview_buf: [u8; 4096],
    preview_len: usize,

    // Modal & Context menu
    modal: ModalKind,
    modal_input: StackStr<64>,
    ctx_open: bool,
    ctx_x: i32,
    ctx_y: i32,

    // Pointer / Hover state
    mouse_x: i32,
    mouse_y: i32,
    prev_buttons: u32,
    hovered_win_btn: u8, // 0=none, 1=min, 2=max, 3=close
    tick: u64,
    last_click_idx: Option<usize>,
    last_click_tick: u64,

    // Status message
    status_msg: StackStr<64>,
}

impl AppState {
    pub fn new(w: u32, h: u32) -> Self {
        let cwd = fs::get_initial_cwd();
        let mut st = Self {
            width: w as i32,
            height: h as i32,
            should_exit: false,
            dirty: true,
            is_maximized: false,
            is_minimized: false,

            cwd,
            history: [StackStr::new(); 24],
            history_len: 1,
            history_idx: 0,

            entries: [FileEntry::empty(); MAX_ENTRIES],
            entry_count: 0,
            view_indices: [0; MAX_ENTRIES],
            view_count: 0,

            selected: None,
            hovered_item: None,
            scroll_y: 0,
            view_mode: ViewMode::Grid,
            sort_by: SortBy::Name,
            sort_asc: true,
            show_inspector: true,

            focus: FocusTarget::None,
            path_input: cwd,
            search_input: StackStr::new(),

            clip_op: ClipOp::None,
            clip_path: StackStr::new(),
            clip_name: StackStr::new(),
            clip_is_dir: false,

            preview_path: StackStr::new(),
            preview_buf: [0; 4096],
            preview_len: 0,

            modal: ModalKind::None,
            modal_input: StackStr::new(),
            ctx_open: false,
            ctx_x: 0,
            ctx_y: 0,

            mouse_x: 0,
            mouse_y: 0,
            prev_buttons: 0,
            hovered_win_btn: 0,
            tick: 0,
            last_click_idx: None,
            last_click_tick: 0,

            status_msg: StackStr::from_bytes(b"Ready"),
        };
        st.history[0] = cwd;
        st.refresh();
        st
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w as i32;
        self.height = h as i32;
        self.ctx_open = false;
        self.dirty = true;
    }

    pub fn tick_idle(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    pub fn navigate_to(&mut self, path: &[u8]) {
        let mut norm = StackStr::<MAX_PATH>::new();
        fs::normalize_into(path, &mut norm);

        if self.history_idx + 1 < self.history_len {
            self.history_len = self.history_idx + 1;
        }
        if self.history_len < 24 {
            self.history[self.history_len] = norm;
            self.history_idx = self.history_len;
            self.history_len += 1;
        } else {
            for i in 1..24 {
                self.history[i - 1] = self.history[i];
            }
            self.history[23] = norm;
            self.history_idx = 23;
        }

        self.cwd = norm;
        self.path_input = norm;
        self.scroll_y = 0;
        self.selected = None;
        self.search_input.clear();
        self.refresh();
    }

    pub fn go_back(&mut self) {
        if self.history_idx > 0 {
            self.history_idx -= 1;
            self.cwd = self.history[self.history_idx];
            self.path_input = self.cwd;
            self.scroll_y = 0;
            self.selected = None;
            self.refresh();
        }
    }

    pub fn go_forward(&mut self) {
        if self.history_idx + 1 < self.history_len {
            self.history_idx += 1;
            self.cwd = self.history[self.history_idx];
            self.path_input = self.cwd;
            self.scroll_y = 0;
            self.selected = None;
            self.refresh();
        }
    }

    pub fn go_up(&mut self) {
        if self.cwd.as_bytes() != b"/" {
            let p = parent_path(self.cwd.as_bytes());
            self.navigate_to(p.as_bytes());
        }
    }

    pub fn refresh(&mut self) {
        self.entry_count = list_dir(self.cwd.as_bytes(), &mut self.entries);
        self.rebuild_view();
        self.dirty = true;
    }

    fn rebuild_view(&mut self) {
        self.view_count = 0;
        let query = self.search_input.as_bytes();
        for i in 0..self.entry_count {
            if contains_ci(self.entries[i].name.as_bytes(), query) {
                self.view_indices[self.view_count] = i;
                self.view_count += 1;
            }
        }
        // Sort view_indices (folders first, then chosen SortBy)
        let n = self.view_count;
        for i in 0..n {
            for j in (i + 1)..n {
                let a = &self.entries[self.view_indices[i]];
                let b = &self.entries[self.view_indices[j]];
                if self.should_swap(a, b) {
                    self.view_indices.swap(i, j);
                }
            }
        }
        if let Some(sel) = self.selected {
            if sel >= self.view_count {
                self.selected = None;
            }
        }
        self.update_preview();
        self.dirty = true;
    }

    fn should_swap(&self, a: &FileEntry, b: &FileEntry) -> bool {
        if a.is_dir != b.is_dir {
            return !a.is_dir && b.is_dir;
        }
        let ord = match self.sort_by {
            SortBy::Name => cmp_ci(a.name.as_bytes(), b.name.as_bytes()),
            SortBy::Size => a.size_bytes.cmp(&b.size_bytes),
            SortBy::Type => cmp_ci(a.type_label(), b.type_label()),
        };
        if self.sort_asc {
            ord == core::cmp::Ordering::Greater
        } else {
            ord == core::cmp::Ordering::Less
        }
    }

    fn selected_entry(&self) -> Option<&FileEntry> {
        self.selected
            .and_then(|vi| self.view_indices.get(vi).copied())
            .map(|idx| &self.entries[idx])
    }

    fn update_preview(&mut self) {
        if !self.show_inspector {
            return;
        }
        if let Some(ent) = self.selected_entry() {
            if ent.is_dir {
                self.preview_path.clear();
                self.preview_len = 0;
                return;
            }
            let full = join_path(self.cwd.as_bytes(), ent.name.as_bytes());
            let text_preview = matches!(ent.kind, EntryKind::Text | EntryKind::Code | EntryKind::Config);
            if full.as_bytes() != self.preview_path.as_bytes() {
                self.preview_path = full;
                if text_preview {
                    self.preview_len = read_preview(full.as_bytes(), &mut self.preview_buf);
                } else {
                    self.preview_len = 0;
                }
            }
        } else {
            self.preview_path.clear();
            self.preview_len = 0;
        }
    }

    fn open_selected(&mut self) {
        if let Some(ent) = self.selected_entry().copied() {
            if ent.is_dir {
                let next = join_path(self.cwd.as_bytes(), ent.name.as_bytes());
                self.navigate_to(next.as_bytes());
            } else if ent.kind == EntryKind::App {
                if launch_app(ent.name.as_bytes()) {
                    self.status_msg.set(b"Launched app");
                } else {
                    self.status_msg.set(b"Failed to launch app");
                }
                self.dirty = true;
            } else {
                self.show_inspector = true;
                self.update_preview();
                self.status_msg.set(b"Opened preview");
                self.dirty = true;
            }
        }
    }

    fn do_copy(&mut self, cut: bool) {
        if let Some(ent) = self.selected_entry().copied() {
            self.clip_op = if cut { ClipOp::Cut } else { ClipOp::Copy };
            self.clip_path = join_path(self.cwd.as_bytes(), ent.name.as_bytes());
            self.clip_name = ent.name;
            self.clip_is_dir = ent.is_dir;
            self.status_msg
                .set(if cut { b"Cut to clipboard" } else { b"Copied to clipboard" });
            self.dirty = true;
        }
    }

    fn do_paste(&mut self) {
        if self.clip_op == ClipOp::None || self.clip_name.is_empty() {
            return;
        }
        let dst = join_path(self.cwd.as_bytes(), self.clip_name.as_bytes());
        let ok = match self.clip_op {
            ClipOp::Copy => copy_item(self.clip_path.as_bytes(), dst.as_bytes(), self.clip_is_dir),
            ClipOp::Cut => {
                let r = rename_or_move(self.clip_path.as_bytes(), dst.as_bytes());
                if r {
                    self.clip_op = ClipOp::None;
                    self.clip_name.clear();
                }
                r
            }
            ClipOp::None => false,
        };
        self.status_msg
            .set(if ok { b"Pasted item" } else { b"Paste failed" });
        self.refresh();
    }

    fn open_modal(&mut self, kind: ModalKind) {
        self.modal = kind;
        self.ctx_open = false;
        self.modal_input.clear();
        if kind == ModalKind::Rename {
            if let Some(ent) = self.selected_entry() {
                self.modal_input = ent.name;
            }
        }
        self.focus = if kind == ModalKind::ConfirmDelete {
            FocusTarget::None
        } else {
            FocusTarget::ModalInput
        };
        self.dirty = true;
    }

    fn confirm_modal(&mut self) {
        let name = self.modal_input;
        match self.modal {
            ModalKind::NewFolder if !name.is_empty() => {
                let ok = mkdir(self.cwd.as_bytes(), name.as_bytes());
                self.status_msg
                    .set(if ok { b"Folder created" } else { b"mkdir failed" });
                self.refresh();
            }
            ModalKind::NewFile if !name.is_empty() => {
                let ok = create_file(self.cwd.as_bytes(), name.as_bytes());
                self.status_msg
                    .set(if ok { b"File created" } else { b"touch failed" });
                self.refresh();
            }
            ModalKind::Rename if !name.is_empty() => {
                if let Some(ent) = self.selected_entry().copied() {
                    let src = join_path(self.cwd.as_bytes(), ent.name.as_bytes());
                    let dst = join_path(self.cwd.as_bytes(), name.as_bytes());
                    let ok = rename_or_move(src.as_bytes(), dst.as_bytes());
                    self.status_msg
                        .set(if ok { b"Renamed item" } else { b"Rename failed" });
                    self.refresh();
                }
            }
            ModalKind::ConfirmDelete => {
                if let Some(ent) = self.selected_entry().copied() {
                    let target = join_path(self.cwd.as_bytes(), ent.name.as_bytes());
                    let ok = delete_item(target.as_bytes(), ent.is_dir);
                    self.status_msg
                        .set(if ok { b"Deleted item" } else { b"Delete failed" });
                    self.selected = None;
                    self.refresh();
                }
            }
            _ => {}
        }
        self.modal = ModalKind::None;
        self.focus = FocusTarget::None;
        self.dirty = true;
    }

    // ========================================================================
    // Event Handling
    // ========================================================================

    pub fn handle_pointer(&mut self, x: i32, y: i32, buttons: u32) {
        self.mouse_x = x;
        self.mouse_y = y;

        // Hover check for right-side macOS window buttons
        let cls_cx = self.width - 20;
        let max_cx = self.width - 42;
        let min_cx = self.width - 64;
        let btn_cy = TITLEBAR_H / 2;

        let new_hover_btn = if dist_sq(x, y, cls_cx, btn_cy) <= 100 {
            3
        } else if dist_sq(x, y, max_cx, btn_cy) <= 100 {
            2
        } else if dist_sq(x, y, min_cx, btn_cy) <= 100 {
            1
        } else {
            0
        };
        if new_hover_btn != self.hovered_win_btn {
            self.hovered_win_btn = new_hover_btn;
            self.dirty = true;
        }

        let new_hover_item = self.hit_test_item(x, y);
        if new_hover_item != self.hovered_item {
            self.hovered_item = new_hover_item;
            self.dirty = true;
        }

        let left_down = (buttons & BTN_LEFT) != 0 && (self.prev_buttons & BTN_LEFT) == 0;
        let right_down = (buttons & BTN_RIGHT) != 0 && (self.prev_buttons & BTN_RIGHT) == 0;
        self.prev_buttons = buttons;

        if left_down {
            self.on_left_click(x, y, new_hover_btn);
        } else if right_down {
            self.on_right_click(x, y);
        }
    }

    fn on_left_click(&mut self, x: i32, y: i32, win_btn: u8) {
        // 1. Right-side macOS-style Window Controls
        match win_btn {
            3 => {
                self.should_exit = true;
                return;
            }
            2 => {
                self.is_maximized = !self.is_maximized;
                self.is_minimized = false;
                self.status_msg.set(if self.is_maximized {
                    b"Maximized view"
                } else {
                    b"Restored view"
                });
                self.dirty = true;
                return;
            }
            1 => {
                self.is_minimized = !self.is_minimized;
                self.status_msg.set(if self.is_minimized {
                    b"Minimized"
                } else {
                    b"Restored"
                });
                self.dirty = true;
                return;
            }
            _ => {}
        }

        if self.is_minimized {
            self.is_minimized = false;
            self.dirty = true;
            return;
        }

        // 2. Modal dialog buttons
        if self.modal != ModalKind::None {
            let mw = 360;
            let mh = 154;
            let mx = (self.width - mw) / 2;
            let my = (self.height - mh) / 2;
            if x >= mx + mw - 96 && x <= mx + mw - 16 && y >= my + mh - 42 && y <= my + mh - 14 {
                self.confirm_modal();
                return;
            }
            if x >= mx + mw - 184 && x <= mx + mw - 104 && y >= my + mh - 42 && y <= my + mh - 14 {
                self.modal = ModalKind::None;
                self.focus = FocusTarget::None;
                self.dirty = true;
                return;
            }
            return;
        }

        // 3. Context Menu click
        if self.ctx_open {
            if self.handle_context_menu_click(x, y) {
                return;
            }
            self.ctx_open = false;
            self.dirty = true;
        }

        // 4. Titlebar navigation & path bar
        if y < TITLEBAR_H {
            self.focus = FocusTarget::None;
            if x >= 164 && x < 192 {
                self.go_back();
                return;
            }
            if x >= 194 && x < 222 {
                self.go_forward();
                return;
            }
            if x >= 224 && x < 252 {
                self.go_up();
                return;
            }
            if x >= 254 && x < 282 {
                self.refresh();
                self.status_msg.set(b"Refreshed");
                return;
            }
            let path_x = 294;
            let path_w = (self.width - 84 - path_x).max(120);
            if x >= path_x && x < path_x + path_w {
                self.focus = FocusTarget::PathBar;
                self.path_input = self.cwd;
                self.dirty = true;
                return;
            }
            return;
        }

        // 5. Toolbar actions
        if y >= TITLEBAR_H && y < TITLEBAR_H + TOOLBAR_H {
            self.focus = FocusTarget::None;
            if x >= 14 && x < 104 {
                self.open_modal(ModalKind::NewFolder);
                return;
            }
            if x >= 112 && x < 192 {
                self.open_modal(ModalKind::NewFile);
                return;
            }
            if x >= 200 && x < 284 {
                self.show_inspector = !self.show_inspector;
                self.update_preview();
                self.dirty = true;
                return;
            }
            // View toggle (Grid / List) & Sort & Search on right
            let search_w = 180;
            let search_x = self.width - search_w - 14;
            if x >= search_x && x < search_x + search_w {
                self.focus = FocusTarget::SearchBar;
                self.dirty = true;
                return;
            }
            let sort_x = search_x - 92;
            if x >= sort_x && x < sort_x + 84 {
                self.sort_by = match self.sort_by {
                    SortBy::Name => SortBy::Size,
                    SortBy::Size => SortBy::Type,
                    SortBy::Type => SortBy::Name,
                };
                self.rebuild_view();
                return;
            }
            let view_x = sort_x - 66;
            if x >= view_x && x < view_x + 58 {
                self.view_mode = if x < view_x + 29 {
                    ViewMode::Grid
                } else {
                    ViewMode::List
                };
                self.dirty = true;
                return;
            }
            return;
        }

        self.focus = FocusTarget::None;

        // 6. Sidebar Quick Access
        if x < SIDEBAR_W && y >= TITLEBAR_H + TOOLBAR_H && y < self.height - STATUSBAR_H {
            let rel_y = y - (TITLEBAR_H + TOOLBAR_H + 34);
            if rel_y >= 0 {
                let idx = (rel_y / 34) as usize;
                if idx < PLACES.len() {
                    self.navigate_to(PLACES[idx].path);
                    return;
                }
            }
            return;
        }

        // 7. Inspector quick actions
        if self.show_inspector && x >= self.width - INSPECTOR_W {
            let ix = self.width - INSPECTOR_W;
            let btn_y = TITLEBAR_H + TOOLBAR_H + 132;
            if y >= btn_y && y <= btn_y + 26 && self.selected.is_some() {
                if x >= ix + 14 && x < ix + 64 {
                    self.open_modal(ModalKind::Rename);
                } else if x >= ix + 70 && x < ix + 114 {
                    self.do_copy(false);
                } else if x >= ix + 120 && x < ix + 160 {
                    self.do_copy(true);
                } else if x >= ix + 166 && x < ix + 222 {
                    self.open_modal(ModalKind::ConfirmDelete);
                }
            }
            return;
        }

        // 8. Main File Canvas (select / double-click to open)
        if let Some(hit) = self.hit_test_item(x, y) {
            let is_double = self.last_click_idx == Some(hit)
                && self.tick.wrapping_sub(self.last_click_tick) < 35;
            self.selected = Some(hit);
            self.last_click_idx = Some(hit);
            self.last_click_tick = self.tick;
            self.update_preview();
            self.dirty = true;
            if is_double {
                self.open_selected();
            }
        } else {
            self.selected = None;
            self.dirty = true;
        }
    }

    fn on_right_click(&mut self, x: i32, y: i32) {
        if self.modal != ModalKind::None || self.is_minimized {
            return;
        }
        if let Some(hit) = self.hit_test_item(x, y) {
            self.selected = Some(hit);
            self.update_preview();
        }
        self.ctx_open = true;
        self.ctx_x = x.min(self.width - 166).max(8);
        self.ctx_y = y.min(self.height - 236).max(TITLEBAR_H);
        self.dirty = true;
    }

    fn handle_context_menu_click(&mut self, x: i32, y: i32) -> bool {
        let mw = 158;
        let item_h = 26;
        let items = 8;
        let mh = items * item_h + 12;
        if x < self.ctx_x || x > self.ctx_x + mw || y < self.ctx_y || y > self.ctx_y + mh {
            return false;
        }
        let idx = (y - self.ctx_y - 6) / item_h;
        self.ctx_open = false;
        match idx {
            0 => self.open_selected(),
            1 => self.open_modal(ModalKind::Rename),
            2 => self.do_copy(false),
            3 => self.do_copy(true),
            4 => self.do_paste(),
            5 => self.open_modal(ModalKind::ConfirmDelete),
            6 => self.open_modal(ModalKind::NewFolder),
            7 => self.open_modal(ModalKind::NewFile),
            _ => {}
        }
        self.dirty = true;
        true
    }

    pub fn handle_key(&mut self, key: u8) {
        // Handle active text input targets first
        if self.focus == FocusTarget::ModalInput {
            match key {
                0x1B => {
                    self.modal = ModalKind::None;
                    self.focus = FocusTarget::None;
                }
                b'\r' | b'\n' => self.confirm_modal(),
                0x08 | 0x7F => {
                    self.modal_input.pop_byte();
                }
                b if (0x20..=0x7E).contains(&b) => {
                    self.modal_input.push_byte(b);
                }
                _ => {}
            }
            self.dirty = true;
            return;
        }

        if self.focus == FocusTarget::PathBar {
            match key {
                0x1B => self.focus = FocusTarget::None,
                b'\r' | b'\n' => {
                    let target = self.path_input;
                    self.focus = FocusTarget::None;
                    self.navigate_to(target.as_bytes());
                }
                0x08 | 0x7F => {
                    self.path_input.pop_byte();
                }
                b if (0x20..=0x7E).contains(&b) => {
                    self.path_input.push_byte(b);
                }
                _ => {}
            }
            self.dirty = true;
            return;
        }

        if self.focus == FocusTarget::SearchBar {
            match key {
                0x1B => {
                    self.focus = FocusTarget::None;
                    self.search_input.clear();
                    self.rebuild_view();
                }
                b'\r' | b'\n' => self.focus = FocusTarget::None,
                0x08 | 0x7F => {
                    self.search_input.pop_byte();
                    self.rebuild_view();
                }
                b if (0x20..=0x7E).contains(&b) => {
                    self.search_input.push_byte(b);
                    self.rebuild_view();
                }
                _ => {}
            }
            self.dirty = true;
            return;
        }

        if self.modal == ModalKind::ConfirmDelete {
            if key == b'\r' || key == b'\n' || key == b'y' || key == b'Y' {
                self.confirm_modal();
            } else if key == 0x1B || key == b'n' || key == b'N' {
                self.modal = ModalKind::None;
                self.dirty = true;
            }
            return;
        }

        // Global shortcuts (including Ctrl+key ASCII codes)
        match key {
            0x03 => self.do_copy(false), // Ctrl+C
            0x18 => self.do_copy(true),  // Ctrl+X
            0x16 => self.do_paste(),     // Ctrl+V
            0x06 | b'/' => {
                self.focus = FocusTarget::SearchBar;
                self.dirty = true;
            }
            0x0C => {
                self.focus = FocusTarget::PathBar;
                self.path_input = self.cwd;
                self.dirty = true;
            }
            0x0E => self.open_modal(ModalKind::NewFile),   // Ctrl+N
            b'n' | b'N' => self.open_modal(ModalKind::NewFolder),
            b'r' | b'R' => self.open_modal(ModalKind::Rename),
            0x7F => self.open_modal(ModalKind::ConfirmDelete), // Delete
            0x08 => self.go_up(),                              // Backspace
            b'\r' | b'\n' => self.open_selected(),
            0x1B => {
                self.ctx_open = false;
                self.selected = None;
                self.dirty = true;
            }
            b'j' | b'J' => self.move_selection(1),
            b'k' | b'K' => self.move_selection(-1),
            _ => {}
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.view_count == 0 {
            return;
        }
        let next = match self.selected {
            Some(i) => {
                let ni = (i as isize + delta).clamp(0, (self.view_count - 1) as isize);
                ni as usize
            }
            None => 0,
        };
        self.selected = Some(next);
        self.update_preview();
        self.dirty = true;
    }

    // ========================================================================
    // Layout & Hit-testing
    // ========================================================================

    fn content_rect(&self) -> (i32, i32, i32, i32) {
        let x = SIDEBAR_W;
        let y = TITLEBAR_H + TOOLBAR_H;
        let right_pad = if self.show_inspector { INSPECTOR_W } else { 0 };
        let w = (self.width - x - right_pad).max(120);
        let h = (self.height - y - STATUSBAR_H).max(80);
        (x, y, w, h)
    }

    fn hit_test_item(&self, mx: i32, my: i32) -> Option<usize> {
        let (cx, cy, cw, ch) = self.content_rect();
        if mx < cx || mx >= cx + cw || my < cy || my >= cy + ch {
            return None;
        }
        match self.view_mode {
            ViewMode::Grid => {
                let pad = 14;
                let cell_w = 114;
                let cell_h = 108;
                let gap = 10;
                let cols = ((cw - pad * 2 + gap) / (cell_w + gap)).max(1);
                let rx = mx - (cx + pad);
                let ry = my - (cy + pad) + self.scroll_y;
                if rx < 0 || ry < 0 {
                    return None;
                }
                let col = rx / (cell_w + gap);
                let row = ry / (cell_h + gap);
                if col >= cols {
                    return None;
                }
                if rx % (cell_w + gap) >= cell_w || ry % (cell_h + gap) >= cell_h {
                    return None;
                }
                let idx = (row * cols + col) as usize;
                if idx < self.view_count {
                    Some(idx)
                } else {
                    None
                }
            }
            ViewMode::List => {
                let header_h = 28;
                let row_h = 30;
                let ry = my - (cy + header_h) + self.scroll_y;
                if ry < 0 {
                    return None;
                }
                let idx = (ry / row_h) as usize;
                if idx < self.view_count {
                    Some(idx)
                } else {
                    None
                }
            }
        }
    }

    // ========================================================================
    // Rendering
    // ========================================================================

    pub fn render(&mut self, buf: &mut [u32]) {
        let bw = self.width;
        let bh = self.height;

        if self.is_minimized {
            // Compact shaded bar when minimized
            for px in buf.iter_mut() {
                *px = 0;
            }
            fill_rounded_rect_raw(buf, bw, bh, 0, 0, bw, TITLEBAR_H, 12, COL_BG_HEADER);
            self.render_titlebar(buf, bw, bh);
            self.dirty = false;
            return;
        }

        // 1. Base Live Aero Glass Panels
        fill_rounded_rect_raw(buf, bw, bh, 0, 0, bw, TITLEBAR_H, 0, COL_BG_HEADER);
        fill_rounded_rect_raw(
            buf,
            bw,
            bh,
            0,
            TITLEBAR_H,
            bw,
            TOOLBAR_H,
            0,
            COL_BG_TOOLBAR,
        );
        fill_rounded_rect_raw(
            buf,
            bw,
            bh,
            0,
            TITLEBAR_H + TOOLBAR_H,
            SIDEBAR_W,
            bh - TITLEBAR_H - TOOLBAR_H - STATUSBAR_H,
            0,
            COL_BG_SIDEBAR,
        );
        fill_rounded_rect_raw(
            buf,
            bw,
            bh,
            SIDEBAR_W,
            TITLEBAR_H + TOOLBAR_H,
            bw - SIDEBAR_W,
            bh - TITLEBAR_H - TOOLBAR_H - STATUSBAR_H,
            0,
            COL_BG_CANVAS,
        );
        fill_rounded_rect_raw(
            buf,
            bw,
            bh,
            0,
            bh - STATUSBAR_H,
            bw,
            STATUSBAR_H,
            0,
            COL_BG_STATUS,
        );

        // Subtle glass dividers
        draw_line_h(buf, bw, bh, 0, TITLEBAR_H - 1, bw, COL_BORDER_SUBTLE);
        draw_line_h(
            buf,
            bw,
            bh,
            0,
            TITLEBAR_H + TOOLBAR_H - 1,
            bw,
            COL_BORDER_SUBTLE,
        );
        draw_line_v(
            buf,
            bw,
            bh,
            SIDEBAR_W - 1,
            TITLEBAR_H + TOOLBAR_H,
            bh - TITLEBAR_H - TOOLBAR_H - STATUSBAR_H,
            COL_BORDER_SUBTLE,
        );
        draw_line_h(buf, bw, bh, 0, bh - STATUSBAR_H, bw, COL_BORDER_SUBTLE);

        // 2. Render sections
        self.render_titlebar(buf, bw, bh);
        self.render_toolbar(buf, bw, bh);
        self.render_sidebar(buf, bw, bh);
        self.render_files(buf, bw, bh);
        if self.show_inspector {
            self.render_inspector(buf, bw, bh);
        }
        self.render_statusbar(buf, bw, bh);

        if self.ctx_open {
            self.render_context_menu(buf, bw, bh);
        }
        if self.modal != ModalKind::None {
            self.render_modal(buf, bw, bh);
        }

        // Subtle outer window rim
        stroke_rounded_rect(buf, bw, bh, 0, 0, bw, bh, 1, COL_BORDER_SUBTLE);
        self.dirty = false;
    }

    fn render_titlebar(&self, buf: &mut [u32], bw: i32, bh: i32) {
        draw_purple_files_logo(buf, bw, bh, 14, 13);
        font::draw_str(buf, bw, bh, 44, 15, b"Purple Files", COL_TEXT_PRIMARY);

        // Navigation buttons (Back, Forward, Up, Refresh)
        let can_back = self.history_idx > 0;
        let can_fwd = self.history_idx + 1 < self.history_len;
        let can_up = self.cwd.as_bytes() != b"/";

        self.draw_nav_pill(buf, bw, bh, 164, 9, 26, 26, can_back);
        draw_arrow_left(
            buf,
            bw,
            bh,
            177,
            22,
            if can_back { COL_TEXT_PRIMARY } else { COL_TEXT_MUTED },
        );

        self.draw_nav_pill(buf, bw, bh, 194, 9, 26, 26, can_fwd);
        draw_arrow_right(
            buf,
            bw,
            bh,
            207,
            22,
            if can_fwd { COL_TEXT_PRIMARY } else { COL_TEXT_MUTED },
        );

        self.draw_nav_pill(buf, bw, bh, 224, 9, 26, 26, can_up);
        draw_arrow_up(
            buf,
            bw,
            bh,
            237,
            22,
            if can_up { COL_TEXT_PRIMARY } else { COL_TEXT_MUTED },
        );

        self.draw_nav_pill(buf, bw, bh, 254, 9, 26, 26, true);
        font::draw_str(buf, bw, bh, 263, 15, b"R", COL_ACCENT_LIGHT);

        // Path / Breadcrumb Bar
        let path_x = 294;
        let path_w = (bw - 88 - path_x).max(120);
        let path_bg = if self.focus == FocusTarget::PathBar {
            argb(0xEE, 0x2B, 0x22, 0x47)
        } else {
            COL_BG_SURFACE
        };
        fill_rounded_rect(buf, bw, bh, path_x, 8, path_w, 28, 7, path_bg);
        stroke_rounded_rect(
            buf,
            bw,
            bh,
            path_x,
            8,
            path_w,
            28,
            7,
            if self.focus == FocusTarget::PathBar {
                COL_BORDER_ACCENT
            } else {
                COL_BORDER_SUBTLE
            },
        );
        let display_path = if self.focus == FocusTarget::PathBar {
            self.path_input.as_bytes()
        } else {
            self.cwd.as_bytes()
        };
        font::draw_str_truncated(
            buf,
            bw,
            bh,
            path_x + 10,
            15,
            display_path,
            path_w - 20,
            COL_TEXT_PRIMARY,
        );

        // Right-side macOS-style circular Close, Minimize, Maximize/Restore buttons
        let cls_cx = bw - 20;
        let max_cx = bw - 42;
        let min_cx = bw - 64;
        draw_mac_window_buttons_right(
            buf,
            bw,
            bh,
            min_cx,
            max_cx,
            cls_cx,
            TITLEBAR_H / 2,
            self.hovered_win_btn,
            self.is_maximized,
        );
    }

    fn draw_nav_pill(
        &self,
        buf: &mut [u32],
        bw: i32,
        bh: i32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        enabled: bool,
    ) {
        let col = if enabled {
            COL_BG_SURFACE
        } else {
            argb(0x70, 0x1E, 0x18, 0x33)
        };
        fill_rounded_rect(buf, bw, bh, x, y, w, h, 6, col);
        stroke_rounded_rect(buf, bw, bh, x, y, w, h, 6, COL_BORDER_SUBTLE);
    }

    fn render_toolbar(&self, buf: &mut [u32], bw: i32, bh: i32) {
        let ty = TITLEBAR_H + 6;
        self.draw_toolbar_btn(buf, bw, bh, 14, ty, 90, 28, b"+ Folder", false);
        self.draw_toolbar_btn(buf, bw, bh, 112, ty, 80, 28, b"+ File", false);
        self.draw_toolbar_btn(
            buf,
            bw,
            bh,
            200,
            ty,
            84,
            28,
            b"Inspect",
            self.show_inspector,
        );

        let search_w = 180;
        let search_x = bw - search_w - 14;
        let s_bg = if self.focus == FocusTarget::SearchBar {
            argb(0xEE, 0x2B, 0x22, 0x47)
        } else {
            COL_BG_SURFACE
        };
        fill_rounded_rect(buf, bw, bh, search_x, ty, search_w, 28, 7, s_bg);
        stroke_rounded_rect(
            buf,
            bw,
            bh,
            search_x,
            ty,
            search_w,
            28,
            7,
            if self.focus == FocusTarget::SearchBar {
                COL_BORDER_ACCENT
            } else {
                COL_BORDER_SUBTLE
            },
        );
        if self.search_input.is_empty() {
            font::draw_str(buf, bw, bh, search_x + 10, ty + 7, b"Search...", COL_TEXT_MUTED);
        } else {
            font::draw_str_truncated(
                buf,
                bw,
                bh,
                search_x + 10,
                ty + 7,
                self.search_input.as_bytes(),
                search_w - 20,
                COL_TEXT_PRIMARY,
            );
        }

        // Sort button
        let sort_x = search_x - 92;
        let sort_label: &[u8] = match self.sort_by {
            SortBy::Name => b"Sort: Name",
            SortBy::Size => b"Sort: Size",
            SortBy::Type => b"Sort: Type",
        };
        self.draw_toolbar_btn(buf, bw, bh, sort_x, ty, 84, 28, sort_label, false);

        // View toggle (Grid / List)
        let view_x = sort_x - 66;
        fill_rounded_rect(buf, bw, bh, view_x, ty, 58, 28, 6, COL_BG_SURFACE);
        stroke_rounded_rect(buf, bw, bh, view_x, ty, 58, 28, 6, COL_BORDER_SUBTLE);
        if self.view_mode == ViewMode::Grid {
            fill_rounded_rect(buf, bw, bh, view_x + 2, ty + 2, 26, 24, 5, COL_ACCENT);
        } else {
            fill_rounded_rect(buf, bw, bh, view_x + 30, ty + 2, 26, 24, 5, COL_ACCENT);
        }
        draw_grid_glyph(buf, bw, bh, view_x + 15, ty + 14, COL_TEXT_PRIMARY);
        draw_list_glyph(buf, bw, bh, view_x + 43, ty + 14, COL_TEXT_PRIMARY);
    }

    fn draw_toolbar_btn(
        &self,
        buf: &mut [u32],
        bw: i32,
        bh: i32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        label: &[u8],
        active: bool,
    ) {
        let bg = if active { COL_BG_SELECTED } else { COL_BG_SURFACE };
        fill_rounded_rect(buf, bw, bh, x, y, w, h, 6, bg);
        stroke_rounded_rect(
            buf,
            bw,
            bh,
            x,
            y,
            w,
            h,
            6,
            if active { COL_BORDER_ACCENT } else { COL_BORDER_SUBTLE },
        );
        let tw = font::measure_str(label);
        font::draw_str(
            buf,
            bw,
            bh,
            x + (w - tw) / 2,
            y + 7,
            label,
            COL_TEXT_PRIMARY,
        );
    }

    fn render_sidebar(&self, buf: &mut [u32], bw: i32, bh: i32) {
        let top = TITLEBAR_H + TOOLBAR_H;
        font::draw_str(buf, bw, bh, 16, top + 12, b"FAVORITES", COL_TEXT_MUTED);

        for (i, place) in PLACES.iter().enumerate() {
            let py = top + 34 + (i as i32) * 34;
            let active = self.cwd.as_bytes() == place.path;
            if active {
                fill_rounded_rect_vgrad(
                    buf,
                    bw,
                    bh,
                    10,
                    py,
                    SIDEBAR_W - 20,
                    28,
                    7,
                    rgb(0x8B, 0x5C, 0xF6),
                    rgb(0x6D, 0x28, 0xD9),
                    false,
                );
            }
            // Mini icon dot
            fill_rounded_rect(
                buf,
                bw,
                bh,
                20,
                py + 9,
                10,
                10,
                3,
                if active { COL_TEXT_PRIMARY } else { COL_ACCENT_LIGHT },
            );
            font::draw_str(
                buf,
                bw,
                bh,
                38,
                py + 7,
                place.label,
                if active { COL_TEXT_PRIMARY } else { COL_TEXT_SECONDARY },
            );
        }

        // Bottom OS badge card
        let card_y = bh - STATUSBAR_H - 58;
        if card_y > top + 250 {
            fill_rounded_rect(buf, bw, bh, 12, card_y, SIDEBAR_W - 24, 46, 8, COL_BG_SURFACE);
            stroke_rounded_rect(buf, bw, bh, 12, card_y, SIDEBAR_W - 24, 46, 8, COL_BORDER_SUBTLE);
            font::draw_str(buf, bw, bh, 22, card_y + 8, b"Purple OS VFS", COL_ACCENT_LIGHT);
            font::draw_str(buf, bw, bh, 22, card_y + 24, b"AegisShell ABI", COL_TEXT_MUTED);
        }
    }

    fn render_files(&self, buf: &mut [u32], bw: i32, bh: i32) {
        let (cx, cy, cw, ch) = self.content_rect();
        if self.view_count == 0 {
            let msg = if self.search_input.is_empty() {
                b"This folder is empty" as &[u8]
            } else {
                b"No matching files"
            };
            let tw = font::measure_str(msg);
            font::draw_str(
                buf,
                bw,
                bh,
                cx + (cw - tw) / 2,
                cy + ch / 2 - 8,
                msg,
                COL_TEXT_MUTED,
            );
            return;
        }

        match self.view_mode {
            ViewMode::Grid => {
                let pad = 14;
                let cell_w = 114;
                let cell_h = 108;
                let gap = 10;
                let cols = ((cw - pad * 2 + gap) / (cell_w + gap)).max(1);

                for vi in 0..self.view_count {
                    let col = (vi as i32) % cols;
                    let row = (vi as i32) / cols;
                    let ix = cx + pad + col * (cell_w + gap);
                    let iy = cy + pad + row * (cell_h + gap) - self.scroll_y;
                    if iy + cell_h < cy || iy >= cy + ch {
                        continue;
                    }
                    let ent = &self.entries[self.view_indices[vi]];
                    let is_sel = self.selected == Some(vi);
                    let is_hov = self.hovered_item == Some(vi);

                    if is_sel {
                        fill_rounded_rect(buf, bw, bh, ix, iy, cell_w, cell_h, 10, COL_BG_SELECTED);
                        stroke_rounded_rect(buf, bw, bh, ix, iy, cell_w, cell_h, 10, COL_ACCENT_LIGHT);
                    } else if is_hov {
                        fill_rounded_rect(
                            buf,
                            bw,
                            bh,
                            ix,
                            iy,
                            cell_w,
                            cell_h,
                            10,
                            COL_BG_SURFACE_HOVER,
                        );
                        stroke_rounded_rect(buf, bw, bh, ix, iy, cell_w, cell_h, 10, COL_BORDER_SUBTLE);
                    }

                    let icon_x = ix + (cell_w - 48) / 2;
                    let icon_y = iy + 12;
                    if ent.is_dir {
                        draw_folder_icon(buf, bw, bh, icon_x, icon_y, 48, is_sel);
                    } else {
                        draw_file_icon(
                            buf,
                            bw,
                            bh,
                            icon_x,
                            icon_y,
                            48,
                            ent.badge_color(),
                            ent.ext_bytes(),
                        );
                    }

                    let name = ent.name.as_bytes();
                    let tw = font::measure_str(name).min(cell_w - 12);
                    let tx = ix + (cell_w - tw) / 2;
                    font::draw_str_truncated(
                        buf,
                        bw,
                        bh,
                        tx,
                        iy + 68,
                        name,
                        cell_w - 12,
                        COL_TEXT_PRIMARY,
                    );

                    let sub = ent.size_label.as_bytes();
                    let sw = font::measure_str(sub);
                    font::draw_str(
                        buf,
                        bw,
                        bh,
                        ix + (cell_w - sw) / 2,
                        iy + 86,
                        sub,
                        if is_sel { COL_TEXT_PRIMARY } else { COL_TEXT_MUTED },
                    );
                }
            }
            ViewMode::List => {
                // Column header
                fill_rect(buf, bw, bh, cx, cy, cw, 26, argb(0xA0, 0x1E, 0x18, 0x33));
                font::draw_str(buf, bw, bh, cx + 38, cy + 6, b"Name", COL_TEXT_MUTED);
                font::draw_str(buf, bw, bh, cx + cw - 240, cy + 6, b"Type", COL_TEXT_MUTED);
                font::draw_str(buf, bw, bh, cx + cw - 130, cy + 6, b"Size", COL_TEXT_MUTED);
                draw_line_h(buf, bw, bh, cx, cy + 26, cw, COL_BORDER_SUBTLE);

                let row_h = 30;
                for vi in 0..self.view_count {
                    let ry = cy + 28 + (vi as i32) * row_h - self.scroll_y;
                    if ry + row_h < cy + 28 || ry >= cy + ch {
                        continue;
                    }
                    let ent = &self.entries[self.view_indices[vi]];
                    let is_sel = self.selected == Some(vi);
                    let is_hov = self.hovered_item == Some(vi);

                    if is_sel {
                        fill_rounded_rect(buf, bw, bh, cx + 6, ry + 1, cw - 12, row_h - 2, 6, COL_BG_SELECTED);
                    } else if is_hov {
                        fill_rounded_rect(
                            buf,
                            bw,
                            bh,
                            cx + 6,
                            ry + 1,
                            cw - 12,
                            row_h - 2,
                            6,
                            COL_BG_SURFACE_HOVER,
                        );
                    } else if vi % 2 == 1 {
                        fill_rect(buf, bw, bh, cx + 6, ry + 1, cw - 12, row_h - 2, argb(0x30, 0xFF, 0xFF, 0xFF));
                    }

                    if ent.is_dir {
                        draw_folder_icon(buf, bw, bh, cx + 14, ry + 8, 16, is_sel);
                    } else {
                        draw_file_icon(buf, bw, bh, cx + 14, ry + 7, 16, ent.badge_color(), b"");
                    }

                    font::draw_str_truncated(
                        buf,
                        bw,
                        bh,
                        cx + 38,
                        ry + 8,
                        ent.name.as_bytes(),
                        cw - 290,
                        COL_TEXT_PRIMARY,
                    );
                    font::draw_str(
                        buf,
                        bw,
                        bh,
                        cx + cw - 240,
                        ry + 8,
                        ent.type_label(),
                        COL_TEXT_SECONDARY,
                    );
                    font::draw_str(
                        buf,
                        bw,
                        bh,
                        cx + cw - 130,
                        ry + 8,
                        ent.size_label.as_bytes(),
                        COL_TEXT_SECONDARY,
                    );
                }
            }
        }
    }

    fn render_inspector(&self, buf: &mut [u32], bw: i32, bh: i32) {
        let ix = bw - INSPECTOR_W;
        let iy = TITLEBAR_H + TOOLBAR_H;
        let ih = bh - iy - STATUSBAR_H;

        fill_rect(buf, bw, bh, ix, iy, INSPECTOR_W, ih, COL_BG_SIDEBAR);
        draw_line_v(buf, bw, bh, ix, iy, ih, COL_BORDER_SUBTLE);

        if let Some(ent) = self.selected_entry() {
            if ent.is_dir {
                draw_folder_icon(buf, bw, bh, ix + (INSPECTOR_W - 48) / 2, iy + 18, 48, false);
            } else {
                draw_file_icon(
                    buf,
                    bw,
                    bh,
                    ix + (INSPECTOR_W - 48) / 2,
                    iy + 18,
                    48,
                    ent.badge_color(),
                    ent.ext_bytes(),
                );
            }
            font::draw_str_truncated(
                buf,
                bw,
                bh,
                ix + 14,
                iy + 76,
                ent.name.as_bytes(),
                INSPECTOR_W - 28,
                COL_TEXT_PRIMARY,
            );
            font::draw_str(buf, bw, bh, ix + 14, iy + 96, ent.type_label(), COL_ACCENT_LIGHT);
            font::draw_str(
                buf,
                bw,
                bh,
                ix + 120,
                iy + 96,
                ent.size_label.as_bytes(),
                COL_TEXT_SECONDARY,
            );
            font::draw_str(
                buf,
                bw,
                bh,
                ix + 14,
                iy + 112,
                ent.perms.as_bytes(),
                COL_TEXT_MUTED,
            );

            // Quick action buttons
            let by = iy + 132;
            self.draw_toolbar_btn(buf, bw, bh, ix + 14, by, 50, 26, b"Ren", false);
            self.draw_toolbar_btn(buf, bw, bh, ix + 70, by, 44, 26, b"Copy", false);
            self.draw_toolbar_btn(buf, bw, bh, ix + 120, by, 40, 26, b"Cut", false);
            self.draw_toolbar_btn(buf, bw, bh, ix + 166, by, 56, 26, b"Del", false);

            // Text preview box
            if self.preview_len > 0 {
                let py = by + 38;
                let ph = ih - (py - iy) - 12;
                if ph > 40 {
                    fill_rounded_rect(buf, bw, bh, ix + 12, py, INSPECTOR_W - 24, ph, 8, COL_BG_SURFACE);
                    let mut line_y = py + 8;
                    let mut pos = 0;
                    let raw = &self.preview_buf[..self.preview_len];
                    while pos < raw.len() && line_y + font::FONT_H < py + ph - 6 {
                        let start = pos;
                        while pos < raw.len() && raw[pos] != b'\n' && raw[pos] != b'\r' {
                            pos += 1;
                        }
                        font::draw_str_truncated(
                            buf,
                            bw,
                            bh,
                            ix + 20,
                            line_y,
                            &raw[start..pos],
                            INSPECTOR_W - 40,
                            COL_TEXT_SECONDARY,
                        );
                        line_y += 16;
                        while pos < raw.len() && (raw[pos] == b'\n' || raw[pos] == b'\r') {
                            pos += 1;
                        }
                    }
                }
            }
        } else {
            font::draw_str(
                buf,
                bw,
                bh,
                ix + 24,
                iy + ih / 2 - 8,
                b"Select an item",
                COL_TEXT_MUTED,
            );
        }
    }

    fn render_statusbar(&self, buf: &mut [u32], bw: i32, bh: i32) {
        let sy = bh - STATUSBAR_H + 6;
        let mut count_str = StackStr::<24>::new();
        fs::push_u64(&mut count_str, self.view_count as u64);
        let _ = count_str.push_str(b" items");
        font::draw_str(buf, bw, bh, 14, sy, count_str.as_bytes(), COL_TEXT_SECONDARY);

        if self.clip_op != ClipOp::None {
            let mut clip = StackStr::<48>::new();
            let _ = clip.push_str(if self.clip_op == ClipOp::Copy {
                b"Copied: "
            } else {
                b"Cut: "
            });
            let _ = clip.push_str(self.clip_name.as_bytes());
            font::draw_str_truncated(buf, bw, bh, 120, sy, clip.as_bytes(), 240, COL_ACCENT_LIGHT);
        }

        let st = self.status_msg.as_bytes();
        let sw = font::measure_str(st);
        font::draw_str(buf, bw, bh, (bw - sw - 14).max(120), sy, st, COL_TEXT_MUTED);
    }

    fn render_context_menu(&self, buf: &mut [u32], bw: i32, bh: i32) {
        let labels: [&[u8]; 8] = [
            b"Open",
            b"Rename",
            b"Copy",
            b"Cut",
            b"Paste",
            b"Delete",
            b"New Folder",
            b"New File",
        ];
        let mw = 158;
        let item_h = 26;
        let mh = (labels.len() as i32) * item_h + 12;
        fill_rounded_rect(buf, bw, bh, self.ctx_x, self.ctx_y, mw, mh, 8, argb(0xF2, 0x22, 0x1B, 0x38));
        stroke_rounded_rect(buf, bw, bh, self.ctx_x, self.ctx_y, mw, mh, 8, COL_BORDER_ACCENT);

        for (i, label) in labels.iter().enumerate() {
            let iy = self.ctx_y + 6 + (i as i32) * item_h;
            if self.mouse_x >= self.ctx_x
                && self.mouse_x <= self.ctx_x + mw
                && self.mouse_y >= iy
                && self.mouse_y < iy + item_h
            {
                fill_rounded_rect(buf, bw, bh, self.ctx_x + 4, iy, mw - 8, item_h, 5, COL_BG_SELECTED);
            }
            let col = if i == 5 {
                rgb(0xFB, 0x71, 0x85)
            } else {
                COL_TEXT_PRIMARY
            };
            font::draw_str(buf, bw, bh, self.ctx_x + 14, iy + 6, label, col);
        }
    }

    fn render_modal(&self, buf: &mut [u32], bw: i32, bh: i32) {
        fill_rect(buf, bw, bh, 0, 0, bw, bh, argb(0x88, 0x09, 0x06, 0x12));
        let mw = 360;
        let mh = 154;
        let mx = (bw - mw) / 2;
        let my = (bh - mh) / 2;

        fill_rounded_rect(buf, bw, bh, mx, my, mw, mh, 12, argb(0xF5, 0x22, 0x1B, 0x38));
        stroke_rounded_rect(buf, bw, bh, mx, my, mw, mh, 12, COL_BORDER_ACCENT);

        let title: &[u8] = match self.modal {
            ModalKind::NewFolder => b"Create New Folder",
            ModalKind::NewFile => b"Create New File",
            ModalKind::Rename => b"Rename Item",
            ModalKind::ConfirmDelete => b"Confirm Delete",
            ModalKind::None => b"",
        };
        font::draw_str(buf, bw, bh, mx + 18, my + 16, title, COL_TEXT_PRIMARY);

        if self.modal == ModalKind::ConfirmDelete {
            if let Some(ent) = self.selected_entry() {
                font::draw_str_truncated(
                    buf,
                    bw,
                    bh,
                    mx + 18,
                    my + 52,
                    ent.name.as_bytes(),
                    mw - 36,
                    COL_ACCENT_LIGHT,
                );
            }
            font::draw_str(
                buf,
                bw,
                bh,
                mx + 18,
                my + 74,
                b"This action cannot be undone.",
                COL_TEXT_SECONDARY,
            );
            self.draw_toolbar_btn(buf, bw, bh, mx + mw - 184, my + mh - 42, 80, 28, b"Cancel", false);
            fill_rounded_rect(buf, bw, bh, mx + mw - 96, my + mh - 42, 80, 28, 6, rgb(0xE1, 0x1D, 0x48));
            font::draw_str(buf, bw, bh, mx + mw - 80, my + mh - 35, b"Delete", COL_TEXT_PRIMARY);
        } else {
            fill_rounded_rect(buf, bw, bh, mx + 18, my + 52, mw - 36, 32, 7, COL_BG_CANVAS);
            stroke_rounded_rect(buf, bw, bh, mx + 18, my + 52, mw - 36, 32, 7, COL_BORDER_ACCENT);
            font::draw_str_truncated(
                buf,
                bw,
                bh,
                mx + 28,
                my + 61,
                self.modal_input.as_bytes(),
                mw - 56,
                COL_TEXT_PRIMARY,
            );
            self.draw_toolbar_btn(buf, bw, bh, mx + mw - 184, my + mh - 42, 80, 28, b"Cancel", false);
            self.draw_toolbar_btn(buf, bw, bh, mx + mw - 96, my + mh - 42, 80, 28, b"Confirm", true);
        }
    }
}

#[inline]
fn dist_sq(x0: i32, y0: i32, x1: i32, y1: i32) -> i32 {
    let dx = x0 - x1;
    let dy = y0 - y1;
    dx * dx + dy * dy
}

fn cmp_ci(a: &[u8], b: &[u8]) -> core::cmp::Ordering {
    let n = a.len().min(b.len());
    for i in 0..n {
        let ca = fs::to_lower(a[i]);
        let cb = fs::to_lower(b[i]);
        if ca != cb {
            return ca.cmp(&cb);
        }
    }
    a.len().cmp(&b.len())
}
