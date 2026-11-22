#![no_std]
#![no_main]

use core::{arch::asm, panic::PanicInfo, slice};

mod inter;

use inter::{Glyph, Inter, HEIGHT as ROW_HEIGHT, SUBPIXELS};

const TRANSCRIPT_TOP: usize = 39;
const BAR_TOP: usize = 5;
const BAR_HEIGHT: usize = 27;
const TAB_WIDTH: usize = 104;
const INSET: usize = 16;
const SPLIT_WIDTH: usize = 72;
const NEW_TAB_WIDTH: usize = 36;
const TAB_GAP: usize = 4;
const CLOSE_WIDTH: usize = 20;
const MAX_TABS: usize = 8;

const MAX_W: usize = 1240;
const MAX_H: usize = 740;
const HISTORY: usize = 256;
const COLS: usize = MAX_W / 3;
const INPUT: usize = 240;
const COMMAND_HISTORY: usize = 16;
const OUTPUT: usize = 4096;
const CLIPBOARD: usize = 4096;
const SHELL_COMMAND: u64 = 0x110a;
const TEST_SUDO: u64 = 0x110d;
const SHELL_PROMPT: u64 = 0x110b;
const PROMPT_CAPACITY: usize = 96;

struct Palette {
    elevated: u32,
    accent: u32,
    teal: u32,
    text: u32,
    muted: u32,
}

const COLORS: Palette = Palette {
    elevated: 0x352748,
    accent: 0xc084fc,
    teal: 0x64e7d6,
    text: 0xf4eff8,
    muted: 0xb7aabd,
};

#[repr(C)]
struct Event {
    kind: u32,
    x: i32,
    y: i32,
    detail: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TextPoint {
    line: usize,
    column: usize,
}

#[derive(Clone, Copy)]
struct Selection {
    anchor: TextPoint,
    end: TextPoint,
}

impl Selection {
    fn ordered(self) -> (TextPoint, TextPoint) {
        (self.anchor.min(self.end), self.anchor.max(self.end))
    }
}

struct Transcript {
    lines: [[u8; COLS]; HISTORY],
    lengths: [usize; HISTORY],
    current: usize,
    count: usize,
    scroll: usize,
    selection: Option<Selection>,
}

impl Transcript {
    const fn new() -> Self {
        Self {
            lines: [[0; COLS]; HISTORY],
            lengths: [0; HISTORY],
            current: 0,
            count: 0,
            scroll: 0,
            selection: None,
        }
    }

    fn reset(&mut self) {
        self.lengths.fill(0);
        self.current = 0;
        self.count = 0;
        self.scroll = 0;
        self.selection = None;
    }

    fn text_count(&self) -> usize {
        self.count
            .saturating_sub(usize::from(self.lengths[self.current] == 0))
    }

    fn line(&self, ordinal: usize) -> &[u8] {
        let index = (self.current + HISTORY + 1 - self.count + ordinal) % HISTORY;
        &self.lines[index][..self.lengths[index]]
    }

    fn viewport(&self, height: usize, input: bool) -> core::ops::Range<usize> {
        let rows = body_rows(height).saturating_sub(usize::from(input || self.scroll != 0));
        let end = self.text_count().saturating_sub(self.scroll);
        end.saturating_sub(rows)..end
    }

    fn page(&mut self, older: bool, height: usize) {
        let rows = body_rows(height).saturating_sub(1).max(1);
        self.scroll = if older {
            self.scroll
                .saturating_add(rows.max(2) - 1)
                .min(self.text_count().saturating_sub(rows))
        } else {
            self.scroll.saturating_sub(rows.max(2) - 1)
        };
    }

    fn position(
        &self,
        x: usize,
        y: usize,
        left: usize,
        height: usize,
        font: &Inter,
    ) -> Option<TextPoint> {
        let view = self.viewport(height, true);
        let line = view.start + y.saturating_sub(TRANSCRIPT_TOP) / ROW_HEIGHT;
        if line >= view.end {
            return None;
        }
        let text = self.line(line);
        let x = x.saturating_sub(left) * SUBPIXELS;
        let mut pen = 0;
        for (column, &byte) in text.iter().enumerate() {
            let advance = font.glyph(byte).advance;
            if x < pen + advance / 2 {
                return Some(TextPoint { line, column });
            }
            pen += advance;
        }
        Some(TextPoint {
            line,
            column: text.len(),
        })
    }

    fn copy_selection(&self, buffer: &mut [u8]) -> Option<usize> {
        let (first, last) = self.selection?.ordered();
        if first == last {
            return None;
        }
        let mut used = 0;
        for ordinal in first.line..=last.line {
            let text = self.line(ordinal);
            let start = if ordinal == first.line {
                first.column
            } else {
                0
            };
            let end = if ordinal == last.line {
                last.column
            } else {
                text.len()
            };
            let count = (end - start).min(buffer.len() - used);
            buffer[used..used + count].copy_from_slice(&text[start..start + count]);
            used += count;
            if used == buffer.len() {
                break;
            }
            if ordinal != last.line {
                buffer[used] = b'\n';
                used += 1;
            }
        }
        Some(used)
    }

    fn append(&mut self, bytes: &[u8], wrap: usize, font: &Inter) {
        self.scroll = 0;
        self.selection = None;
        if self.count == 0 {
            self.count = 1;
        }
        let mut width = font.measure(&self.lines[self.current][..self.lengths[self.current]]);
        for &byte in bytes {
            if byte == b'\r' {
                continue;
            }
            let byte = if byte.is_ascii_graphic() || byte == b' ' || byte == b'\n' {
                byte
            } else {
                b' '
            };
            let advance = font.glyph(byte).advance;
            if byte == b'\n'
                || self.lengths[self.current] == COLS
                || (width > 0 && width + advance > wrap * SUBPIXELS)
            {
                self.current = (self.current + 1) % HISTORY;
                self.count = (self.count + 1).min(HISTORY);
                self.lengths[self.current] = 0;
                width = 0;
            }
            if byte != b'\n' {
                self.lines[self.current][self.lengths[self.current]] = byte;
                self.lengths[self.current] += 1;
                width += advance;
            }
        }
    }
}

fn body_rows(height: usize) -> usize {
    height.saturating_sub(TRANSCRIPT_TOP + 6) / ROW_HEIGHT
}

static mut PIXELS: [u32; MAX_W * MAX_H] = [0; MAX_W * MAX_H];
static mut TRANSCRIPTS: [Transcript; MAX_TABS] = [const { Transcript::new() }; MAX_TABS];

#[derive(Clone, Copy)]
struct Line {
    bytes: [u8; INPUT],
    len: usize,
    cursor: usize,
}

impl Line {
    const EMPTY: Self = Self {
        bytes: [0; INPUT],
        len: 0,
        cursor: 0,
    };

    fn text(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    fn insert(&mut self, key: u8) {
        if self.len < INPUT {
            self.bytes
                .copy_within(self.cursor..self.len, self.cursor + 1);
            self.bytes[self.cursor] = key;
            self.cursor += 1;
            self.len += 1;
        }
    }

    fn delete(&mut self) {
        if self.cursor < self.len {
            self.bytes
                .copy_within(self.cursor + 1..self.len, self.cursor);
            self.len -= 1;
        }
    }
}

struct Input {
    line: Line,
    sudo_command: Option<Line>,
    sudo_password: [u8; 5],
    sudo_length: usize,
    draft: Line,
    entries: [Line; COMMAND_HISTORY],
    next: usize,
    count: usize,
    distance: usize,
}

impl Input {
    const fn new() -> Self {
        Self {
            line: Line::EMPTY,
            sudo_command: None,
            sudo_password: [0; 5],
            sudo_length: 0,
            draft: Line::EMPTY,
            entries: [Line::EMPTY; COMMAND_HISTORY],
            next: 0,
            count: 0,
            distance: 0,
        }
    }

    fn record(&mut self) {
        let last = (self.next + COMMAND_HISTORY - 1) % COMMAND_HISTORY;
        if self.count == 0 || self.entries[last].text() != self.line.text() {
            let mut command = self.line;
            command.cursor = command.len;
            self.entries[self.next] = command;
            self.next = (self.next + 1) % COMMAND_HISTORY;
            self.count = (self.count + 1).min(COMMAND_HISTORY);
        }
        self.distance = 0;
        self.line = Line::EMPTY;
    }

    fn recall(&mut self, older: bool) {
        if older && self.distance < self.count {
            if self.distance == 0 {
                self.draft = self.line;
            }
            self.distance += 1;
        } else if !older && self.distance > 0 {
            self.distance -= 1;
        } else {
            return;
        }
        self.line = if self.distance == 0 {
            self.draft
        } else {
            self.entries[(self.next + COMMAND_HISTORY - self.distance) % COMMAND_HISTORY]
        };
    }
}

static mut INPUTS: [Input; MAX_TABS] = [const { Input::new() }; MAX_TABS];

#[derive(Clone, Copy)]
enum Escape {
    None,
    Start,
    Bracket,
    Delete,
    PageUp,
    PageDown,
}

impl Escape {
    fn key(&mut self, byte: u8) -> Option<u8> {
        let current = core::mem::replace(self, Self::None);
        match (current, byte) {
            (_, 27) => {
                *self = Self::Start;
                None
            }
            (Self::Start, b'[') => {
                *self = Self::Bracket;
                None
            }
            (Self::Bracket, b'3') => {
                *self = Self::Delete;
                None
            }
            (Self::Bracket, b'A') => Some(16),
            (Self::Bracket, b'B') => Some(14),
            (Self::Bracket, b'C') => Some(6),
            (Self::Bracket, b'D') => Some(2),
            (Self::Bracket, b'H') => Some(1),
            (Self::Bracket, b'F') => Some(5),
            (Self::Bracket, b'5') => {
                *self = Self::PageUp;
                None
            }
            (Self::Bracket, b'6') => {
                *self = Self::PageDown;
                None
            }
            (Self::Delete, b'~') => Some(4),
            (Self::PageUp, b'~') => Some(128),
            (Self::PageDown, b'~') => Some(129),
            (Self::None, _) | (Self::Start, _) => Some(byte),
            _ => None,
        }
    }
}

struct Frame<'a> {
    pixels: &'a mut [u32],
    font: &'a Inter,
    width: usize,
    height: usize,
}

impl Frame<'_> {
    fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let right = (x + w).min(self.width);
        if x >= right {
            return;
        }
        for row in y..(y + h).min(self.height) {
            self.pixels[row * self.width + x..row * self.width + right].fill(0xFF00_0000 | (color & 0x00FF_FFFF));
        }
    }

    fn rect_alpha(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let right = x.saturating_add(w).min(self.width);
        let bottom = y.saturating_add(h).min(self.height);
        if x >= right { return; }
        for row in y..bottom {
            self.pixels[row * self.width + x..row * self.width + right].fill(color);
        }
    }

    fn text(&mut self, x: usize, y: usize, text: &[u8], color: u32, edge: usize) {
        if y + ROW_HEIGHT > self.height {
            return;
        }
        let edge = edge.min(self.width);
        let mut pen = x * SUBPIXELS;
        for &byte in text {
            let glyph = self.font.glyph(byte);
            if pen + glyph.advance > edge * SUBPIXELS {
                break;
            }
            let left = (pen + SUBPIXELS / 2) / SUBPIXELS;
            for (row, coverage) in glyph.coverage.iter().enumerate() {
                for (column, &alpha) in coverage.iter().enumerate() {
                    let px = left as isize + Glyph::x_offset(column);
                    if alpha == 0 || px < x as isize || px >= edge as isize {
                        continue;
                    }
                    let pixel = &mut self.pixels[(y + row) * self.width + px as usize];
                    *pixel = (u32::from(alpha) << 24) | (color & 0x00FF_FFFF);
                }
            }
            pen += glyph.advance;
        }
    }
}

fn syscall(number: u64, a: u64, b: u64, c: u64) -> i64 {
    let result: i64;
    unsafe {
        asm!("syscall", inlateout("rax") number => result, in("rdi") a,
            in("rsi") b, in("rdx") c, lateout("rcx") _, lateout("r11") _);
    }
    result
}

struct Prompt {
    bytes: [u8; PROMPT_CAPACITY],
    length: usize,
}

impl Prompt {
    fn new() -> Self {
        let mut prompt = Self {
            bytes: [0; PROMPT_CAPACITY],
            length: 0,
        };
        prompt.refresh();
        prompt
    }

    fn refresh(&mut self) {
        let result = syscall(
            SHELL_PROMPT,
            self.bytes.as_mut_ptr() as u64,
            self.bytes.len() as u64,
            0,
        );
        if (1..=self.bytes.len() as i64).contains(&result) {
            self.length = result as usize;
        }
    }

    fn text(&self) -> &[u8] {
        if self.length == 0 {
            b"$ "
        } else {
            &self.bytes[..self.length]
        }
    }
}

#[derive(Clone, Copy)]
enum TabAction { New, Close(usize) }

struct Terminal {
    tab: usize,
    tab_count: usize,
    split: bool,
    focused: bool,
}

impl Terminal {
    fn tab_width(&self, width: usize) -> usize {
        let edge=width.saturating_sub(INSET);
        let split_space=if width>=440 {SPLIT_WIDTH+TAB_GAP} else {0};
        let available=edge.saturating_sub(INSET+split_space+NEW_TAB_WIDTH+TAB_GAP);
        (available/self.tab_count.max(1)).min(TAB_WIDTH).max(32)
    }

    fn split_bounds(&self, width: usize) -> (usize, usize) {
        (width.saturating_sub(INSET+SPLIT_WIDTH),SPLIT_WIDTH)
    }

    fn pane_bounds(&self, width: usize, tab: usize) -> (usize, usize) {
        let edge=width.saturating_sub(INSET);
        let left=INSET+6;
        if self.split && width>=440 {
            let middle=width/2;
            if tab==self.tab {(left,middle-8)} else {(middle+8,edge.saturating_sub(6))}
        } else {(left,edge.saturating_sub(6))}
    }

    fn handle_click(&mut self,x:usize,y:usize,width:usize)->Option<TabAction> {
        if !(BAR_TOP..BAR_TOP+BAR_HEIGHT).contains(&y){return None;}
        let tab_width=self.tab_width(width);
        let tab_end=INSET+tab_width*self.tab_count;
        if (INSET..tab_end).contains(&x) {
            let index=(x-INSET)/tab_width;
            if (x-INSET)%tab_width>=tab_width.saturating_sub(CLOSE_WIDTH){return Some(TabAction::Close(index));}
            self.tab=index;
            return None;
        }
        let new_x=tab_end+TAB_GAP;
        if (new_x..new_x+NEW_TAB_WIDTH).contains(&x) && self.tab_count<MAX_TABS {return Some(TabAction::New);}
        let (split_x,split_w)=self.split_bounds(width);
        if width>=440 && self.tab_count>1 && (split_x..split_x+split_w).contains(&x){self.split=!self.split;}
        None
    }
}

fn pane(
    frame: &mut Frame<'_>,
    transcript: &Transcript,
    left: usize,
    edge: usize,
    palette: &Palette,
    input: Option<&Line>,
    prompt: &Prompt,
    focused: bool,
) {
    let view = transcript.viewport(frame.height, input.is_some());
    let visible = view.len();
    let selection = transcript.selection.map(Selection::ordered);
    for (row, ordinal) in view.enumerate() {
        let text = transcript.line(ordinal);
        let y = TRANSCRIPT_TOP + row * ROW_HEIGHT;
        if let Some((first, last)) = selection {
            if first != last && (first.line..=last.line).contains(&ordinal) {
                let start = if ordinal == first.line {
                    first.column
                } else {
                    0
                };
                let end = if ordinal == last.line {
                    last.column
                } else {
                    text.len()
                };
                let x1 = left + frame.font.measure(&text[..start]) / SUBPIXELS;
                let x2 = left + frame.font.measure(&text[..end]).div_ceil(SUBPIXELS);
                frame.rect_alpha(x1, y, x2.min(edge).saturating_sub(x1), ROW_HEIGHT, 0x70423568);
            }
        }
        let color = if text.starts_with(b"PurpleOS@") {
            palette.teal
        } else {
            palette.text
        };
        frame.text(left, y, text, color, edge);
    }
    if transcript.scroll != 0 {
        frame.text(
            left,
            frame.height.saturating_sub(ROW_HEIGHT + 5),
            b"Scrollback  /  PgDn",
            palette.muted,
            edge,
        );
        return;
    }
    if let Some(line) = input {
        let y = TRANSCRIPT_TOP + visible * ROW_HEIGHT;
        let mut label = prompt.text();
        if frame.font.measure(label).div_ceil(SUBPIXELS) + 25 >= edge.saturating_sub(left) {
            label = b"$ ";
        }
        let input_x = left + (frame.font.measure(label) + SUBPIXELS / 2) / SUBPIXELS;
        frame.text(left, y, label, palette.teal, edge);
        let room = edge.saturating_sub(input_x + 2) * SUBPIXELS;
        if room == 0 {
            return;
        }
        let mut start = 0;
        let mut cursor = frame.font.measure(&line.bytes[..line.cursor]);
        while cursor > room && start < line.cursor {
            cursor -= frame.font.glyph(line.bytes[start]).advance;
            start += 1;
        }
        frame.text(input_x, y, &line.text()[start..], palette.text, edge);
        if focused {
            frame.rect(
                input_x + cursor / SUBPIXELS,
                y + 2,
                2,
                ROW_HEIGHT - 4,
                palette.accent,
            );
        }
    }
}

fn clear_transcript(transcript:&mut Transcript) {
    transcript.count=1;transcript.scroll=0;transcript.selection=None;
    transcript.lengths[transcript.current]=0;
}

fn close_tab(index:usize, terminal:&mut Terminal, tabs:&mut [Transcript;MAX_TABS], inputs:&mut [Input;MAX_TABS], width:usize, font:&Inter) {
    if terminal.tab_count<=1 {
        tabs[0].reset();inputs[0]=Input::new();terminal.tab=0;
        tabs[0].append(b"AegisShell  |  Type help for commands.\n",width,font);
        return;
    }
    for i in index..terminal.tab_count-1 {tabs.swap(i,i+1);inputs.swap(i,i+1);}
    terminal.tab_count-=1;
    tabs[terminal.tab_count].reset();inputs[terminal.tab_count]=Input::new();
    if terminal.tab==index {terminal.tab=index.min(terminal.tab_count-1);} else if terminal.tab>index {terminal.tab-=1;}
    if terminal.tab_count<2 {terminal.split=false;}
}

fn run_command(tab: usize, width: usize, height: usize, terminal: &Terminal, tabs: &mut [Transcript;MAX_TABS], inputs: &mut [Input;MAX_TABS], prompt: &mut Prompt, output: &mut [u8], font: &Inter) {
    let input=&mut inputs[tab];
    if input.line.len==0 {return;}
    let (left,edge)=terminal.pane_bounds(width,tab);
    let wrap=edge.saturating_sub(left).max(1);
    let transcript=&mut tabs[tab];
    if matches!(input.line.text().trim_ascii(), b"clear" | b"cls") {
        clear_transcript(transcript);
        input.record();
        return;
    }
    transcript.append(prompt.text(),wrap,font);
    transcript.append(input.line.text(),wrap,font);
    transcript.append(b"\n",wrap,font);
    if input.line.text().trim_ascii().starts_with(b"sudo ") {
        input.sudo_command = Some(input.line);
        input.record();
        transcript.append(b"Aegis test password (Ctrl+C to cancel): ",wrap,font);
        return;
    }
    let size=syscall(SHELL_COMMAND,input.line.bytes.as_ptr() as u64,input.line.len as u64,output.as_mut_ptr() as u64);
    if (0..=output.len() as i64).contains(&size) {
        transcript.append(&output[..size as usize],wrap,font);
        if size==output.len() as i64 {transcript.append(b"\n[output truncated]\n",wrap,font);}
    } else {transcript.append(b"Command rejected by Purple OS.\n",wrap,font);}
    prompt.refresh();
    input.record();
    let _=height;
}

fn draw(
    frame: &mut Frame<'_>,
    terminal: &Terminal,
    tabs: &[Transcript; MAX_TABS],
    input: &Line,
    prompt: &Prompt,
) {
    let p = &COLORS;
    let (width, height) = (frame.width, frame.height);
    let edge = width.saturating_sub(INSET);
    let body_top = BAR_TOP + BAR_HEIGHT + 1;
    // Transparent pixels reveal the compositor's live blurred acrylic.
    frame.pixels.fill(0);
    // Keep shell chrome translucent so the live compositor backdrop remains visible.
    frame.rect_alpha(
        INSET,
        BAR_TOP,
        edge.saturating_sub(INSET),
        BAR_HEIGHT,
        0xD0211A2A,
    );
    let tab_width=terminal.tab_width(width);
    for tab in 0..terminal.tab_count {
        let x=INSET+tab*tab_width;
        if terminal.tab==tab {
            frame.rect_alpha(x,BAR_TOP,tab_width,BAR_HEIGHT,0xD0352748);
            frame.rect(x,BAR_TOP+BAR_HEIGHT-2,tab_width,2,p.accent);
        }
        let label:&[u8]=match tab {0=>b"Shell 1",1=>b"Shell 2",2=>b"Shell 3",3=>b"Shell 4",4=>b"Shell 5",5=>b"Shell 6",6=>b"Shell 7",_=>b"Shell 8"};
        frame.text(x+8,BAR_TOP+3,label,if terminal.tab==tab {p.text}else{p.muted},x+tab_width-CLOSE_WIDTH);
        frame.text(x+tab_width-CLOSE_WIDTH+5,BAR_TOP+3,b"x",if terminal.tab==tab {p.accent}else{p.muted},x+tab_width);
    }
    let tab_end=INSET+tab_width*terminal.tab_count;
    let new_x=tab_end+TAB_GAP;
    if terminal.tab_count<MAX_TABS {
        frame.rect_alpha(new_x,BAR_TOP+3,NEW_TAB_WIDTH,BAR_HEIGHT-6,0x90382C4A);
        frame.text(new_x+12,BAR_TOP+3,b"+",p.accent,new_x+NEW_TAB_WIDTH);
    }
    if width>=440 {
        let (button,_)=terminal.split_bounds(width);
        let color=if terminal.split {p.accent}else if terminal.tab_count>1 {p.muted}else{0xFF6D6278};
        frame.rect_alpha(button,BAR_TOP+3,SPLIT_WIDTH,BAR_HEIGHT-6,if terminal.split {0xD0352748}else{0x90382C4A});
        frame.text(button+8,BAR_TOP+3,b"Split",color,button+SPLIT_WIDTH);
    }
    if terminal.split && width >= 440 {
        frame.rect(width/2,body_top,1,height.saturating_sub(body_top+6),p.elevated);
    }
    let secondary=(terminal.tab+1)%terminal.tab_count;
    for tab in 0..terminal.tab_count {
        let active=terminal.tab==tab;
        let shown=active || (terminal.split && width>=440 && tab==secondary);
        if shown {
            let (left,right)=terminal.pane_bounds(width,tab);
            pane(frame,&tabs[tab],left,right,p,active.then_some(input),prompt,terminal.focused);
        }
    }
    if height > TRANSCRIPT_TOP + ROW_HEIGHT + 8 {
        let rule_y = height - ROW_HEIGHT - 6;
        frame.rect_alpha(INSET, rule_y, edge.saturating_sub(INSET), 1, 0x506D5A82);
        let status_y = height - ROW_HEIGHT - 2;
        frame.text(INSET + 2, status_y, b"AegisShell  /  Ready", p.muted, edge);
        let meta = b"Inter  /  UTF-8";
        let meta_width = frame.font.measure(meta).div_ceil(SUBPIXELS);
        frame.text(edge.saturating_sub(meta_width), status_y, meta, p.muted, edge);
    }
}

core::arch::global_asm!(
    ".global _start",
    "_start:",
    "and rsp, -16",
    "call purple_terminal_main",
    "ud2",
);

#[no_mangle]
pub extern "C" fn purple_terminal_main() -> ! {
    let startup = syscall(0x1101, 0, 0, 0);
    if startup <= 0 {
        exit(1);
    }
    // ABI v1 supplies the immutable, signed TTF as an app resource.
    let record = unsafe { slice::from_raw_parts(startup as *const u64, 8) };
    let window = record[1];
    if syscall(0x110c, window, 1, 0) < 0 { exit(1); }
    let length = record[5] as usize;
    if record[4] == 0 || !(1..=512 * 1024).contains(&length) {
        exit(1);
    }
    let resource = unsafe { slice::from_raw_parts(record[4] as *const u8, length) };
    let Some(font) = (unsafe { inter::initialize(resource) }) else {
        exit(1);
    };
    let pixels =
        unsafe { slice::from_raw_parts_mut((&raw mut PIXELS).cast::<u32>(), MAX_W * MAX_H) };
    let tabs = unsafe { &mut *(&raw mut TRANSCRIPTS) };
    let mut terminal = Terminal {
        tab: 0,
        tab_count: 1,
        split: false,
        focused: true,
    };
    let inputs = unsafe { &mut *(&raw mut INPUTS) };
    let mut escape = Escape::None;
    let mut output = [0u8; OUTPUT];
    let mut prompt = Prompt::new();
    let mut clipboard = [0u8; CLIPBOARD];
    let mut clipboard_len = 0;
    let mut selecting = false;
    let mut dirty = true;
    let mut pressed = false;
    let mut width = 780;
    let mut height = 500;
    tabs[0].append(b"AegisShell  |  Type help for commands.\n", width, font);
    loop {
        let mut event = Event {
            kind: 0,
            x: 0,
            y: 0,
            detail: 0,
        };
        loop {
            let res = syscall(0x1105, window, &mut event as *mut Event as u64, 0);
            if res < 0 {
                exit(0);
            }
            if res == 0 {
                break;
            }
            match event.kind {
                1 => {
                    let down = event.detail & 1 != 0;
                    if event.x >= 0 && event.y >= 0 {
                        let (x, y) = (event.x as usize, event.y as usize);
                        if down && !pressed {
                            let action=terminal.handle_click(x,y,width);
                            match action {
                                Some(TabAction::New) if terminal.tab_count<MAX_TABS => {
                                    let index=terminal.tab_count;terminal.tab_count+=1;terminal.tab=index;
                                    tabs[index].reset();inputs[index]=Input::new();
                                    tabs[index].append(b"AegisShell  |  Type help for commands.\n",width,font);
                                }
                                Some(TabAction::Close(index)) => close_tab(index,&mut terminal,tabs,inputs,width,font),
                                _=>(),
                            }
                            if y >= TRANSCRIPT_TOP {
                                if terminal.split && width >= 440 && x >= width/2 {
                                    terminal.tab=(terminal.tab+1)%terminal.tab_count;
                                }
                                let (left, edge) = terminal.pane_bounds(width, terminal.tab);
                                let tab = &mut tabs[terminal.tab];
                                tab.selection = ((left..edge).contains(&x))
                                    .then(|| tab.position(x, y, left, height, font))
                                    .flatten()
                                    .map(|point| Selection {
                                        anchor: point,
                                        end: point,
                                    });
                                selecting = tab.selection.is_some();
                            }
                            if y < TRANSCRIPT_TOP { selecting=false; }
                            dirty = true;
                        } else if pressed && selecting {
                            let (left, _) = terminal.pane_bounds(width, terminal.tab);
                            let tab = &mut tabs[terminal.tab];
                            if let Some(point) = tab.position(x, y, left, height, font) {
                                if let Some(selection) = &mut tab.selection {
                                    selection.end = point;
                                }
                                dirty = true;
                            }
                        }
                    }
                    if !down {
                        selecting = false;
                    }
                    pressed = down;
                }
                2 => {
                    let Some(key) = escape.key(event.x as u8) else {
                        continue;
                    };
                    let input = &mut inputs[terminal.tab];
                    if let Some(command) = input.sudo_command {
                        let (left, edge) = terminal.pane_bounds(width, terminal.tab);
                        let wrap = edge.saturating_sub(left).max(1);
                        let transcript = &mut tabs[terminal.tab];
                        match key {
                            3 => {
                                input.sudo_command = None;
                                transcript.append(b"\nCanceled.\n", wrap, font);
                            }
                            b'\r' | b'\n' => {
                                input.sudo_command = None;
                                let command = command.text().trim_ascii()
                                    .strip_prefix(b"sudo ").unwrap_or_default();
                                let mut request = [0u8; INPUT + 5];
                                let result = if input.sudo_length == input.sudo_password.len() {
                                    request[..5].copy_from_slice(&input.sudo_password);
                                    request[5..5 + command.len()].copy_from_slice(command);
                                    syscall(TEST_SUDO, request.as_ptr() as u64,
                                            (command.len() + 5) as u64, output.as_mut_ptr() as u64)
                                } else {
                                    -13
                                };
                                request.fill(0);
                                transcript.append(b"\n", wrap, font);
                                if (0..=output.len() as i64).contains(&result) {
                                    transcript.append(&output[..result as usize], wrap, font);
                                } else if result == -13 {
                                    transcript.append(b"Incorrect test password.\n", wrap, font);
                                } else {
                                    transcript.append(b"Test sudo is unavailable.\n", wrap, font);
                                }
                            }
                            8 | 127 if input.sudo_length > 0 => {
                                input.sudo_length -= 1;
                                input.sudo_password[input.sudo_length] = 0;
                            }
                            32..=126 if input.sudo_length < input.sudo_password.len() => {
                                input.sudo_password[input.sudo_length] = key;
                                input.sudo_length += 1;
                            }
                            32..=126 => {
                                input.sudo_command = None;
                                transcript.append(b"\nIncorrect test password.\n", wrap, font);
                            }
                            _ => {}
                        }
                        if input.sudo_command.is_none() {
                            input.sudo_password.fill(0);
                            input.sudo_length = 0;
                        }
                        dirty = true;
                        continue;
                    }
                    match key {
                        b'\r' | b'\n' if input.line.len > 0 => {
                            run_command(terminal.tab,width,height,&terminal,tabs,inputs,&mut prompt,&mut output,font);
                        }
                        1 => input.line.cursor = 0,
                        2 => input.line.cursor = input.line.cursor.saturating_sub(1),
                        3 => {
                            if let Some(length) = tabs[terminal.tab].copy_selection(&mut clipboard)
                            {
                                clipboard_len = length;
                            }
                        }
                        4 => input.line.delete(),
                        5 => input.line.cursor = input.line.len,
                        6 => input.line.cursor = (input.line.cursor + 1).min(input.line.len),
                        8 | 127 if input.line.cursor > 0 => {
                            input.line.cursor -= 1;
                            input.line.delete();
                        }
                        12 => clear_transcript(&mut tabs[terminal.tab]),
                        14 => input.recall(false),
                        16 => input.recall(true),
                        21 => input.line = Line::EMPTY,
                        22 => {
                            for &byte in &clipboard[..clipboard_len] {
                                input.line.insert(if byte.is_ascii_graphic() {
                                    byte
                                } else {
                                    b' '
                                });
                            }
                        }
                        128 => tabs[terminal.tab].page(true, height),
                        129 => tabs[terminal.tab].page(false, height),
                        32..=126 => input.line.insert(key),
                        _ => (),
                    }
                    dirty = true;
                }
                3 => {
                    terminal.focused = event.x != 0;
                    escape = Escape::None;
                    selecting = false;
                    pressed = false;
                    dirty = true;
                }
                4 => {
                    syscall(0x1103, window, 0, 0);
                    exit(0);
                }
                5 | 6 => dirty = true,
                _ => (),
            }
        }
        if dirty {
            let size = syscall(0x1106, window, 0, 0) as u64;
            width = (size >> 32) as usize;
            height = size as u32 as usize;
            if (80..=MAX_W).contains(&width) && (48..=MAX_H).contains(&height) {
                let mut frame = Frame {
                    pixels: &mut pixels[..width * height],
                    font,
                    width,
                    height,
                };
                draw(
                    &mut frame,
                    &terminal,
                    tabs,
                    &inputs[terminal.tab].line,
                    &prompt,
                );
                syscall(
                    0x1104,
                    window,
                    frame.pixels.as_ptr() as u64,
                    (width * height * 4) as u64,
                );
            }
            dirty = false;
        }
        syscall(24, 0, 0, 0);
    }
}

fn exit(status: u64) -> ! {
    syscall(60, status, 0, 0);
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    exit(1)
}
