//! Native Purple Desktop UI Shell Subsystem for Aegis64.
//!
//! Provides the refined Purple Bar (top floating panel) and Purple Dock
//! (bottom floating shelf with Looking Glass-inspired floor reflection).
//! Focuses exclusively on the desktop shell bars and visual refinement.

use super::blend::{frost_backdrop, interpolate_color, scale_alpha};
use super::cursor::Cursor;
use super::surface::{Rect, Surface};
use super::window::Window;
use crate::drivers::font::{self, FontWeight, DESKTOP_HEIGHT};

pub const COLOR_GLASS_BASE: u32 = 0xA9120724; // Sleek frosted acrylic glass
pub const COLOR_GLASS_BORDER: u32 = 0x568C67C8; // Translucent neon violet border
pub const COLOR_GLASS_HIGHLIGHT: u32 = 0x34FFFFFF; // Top specular sheen line
pub const COLOR_AMETHYST_ACCENT: u32 = 0xFFA855F7; // Primary vibrant amethyst
pub const COLOR_NEON_VIOLET: u32 = 0xFFC084FC; // Highlight / active indicator
pub const COLOR_CYAN_ACCENT: u32 = 0xFF38BDF8; // Electric cyan
pub const COLOR_TEXT_PRIMARY: u32 = 0xFFF8FAFC;
pub const COLOR_TEXT_MUTED: u32 = 0xFF94A3B8;

pub const DOCK_NUM_ICONS: usize = 5;
pub const DOCK_ICON_SIZE: usize = 40;
pub const DOCK_ICON_SPACING: usize = 12;
pub const DOCK_PADDING_H: usize = 18;
pub const DOCK_HEIGHT: usize = 62;
pub const BAR_HEIGHT: usize = 38;
pub const ANIMATION_HZ: u64 = 60;
pub const DOCK_LIFT: u8 = 8;
const EDGE_MARGIN: usize = 20;
const BAR_REST_Y: i32 = 12;
const BAR_ENTRANCE_TICKS: u64 = ANIMATION_HZ / 3;
const LOGO_SIZE: usize = 28;
const LOGO_INSET: i32 = 8;
const TRAY_WIDTH: usize = 132;
const REFLECTION_HEIGHT: usize = 18;
const DOCK_WIDTH: usize =
    DOCK_PADDING_H * 2 + DOCK_NUM_ICONS * DOCK_ICON_SIZE + (DOCK_NUM_ICONS - 1) * DOCK_ICON_SPACING;

fn bar_rect(width: usize, tick: u64) -> Rect {
    let margin = EDGE_MARGIN.min(width / 10);
    let remaining = BAR_ENTRANCE_TICKS.saturating_sub(tick);
    let offset = remaining.pow(2) * BAR_REST_Y as u64 / BAR_ENTRANCE_TICKS.pow(2);
    Rect::new(
        margin as i32,
        BAR_REST_Y - offset as i32,
        width - margin * 2,
        BAR_HEIGHT,
    )
}

fn logo_rect(bar: Rect) -> Rect {
    Rect::new(
        bar.x + LOGO_INSET,
        bar.y + (bar.h - LOGO_SIZE) as i32 / 2,
        LOGO_SIZE,
        LOGO_SIZE,
    )
}

fn dock_rect(width: usize, height: usize) -> Option<Rect> {
    if width < DOCK_WIDTH + EDGE_MARGIN * 2
        || height < BAR_HEIGHT + DOCK_HEIGHT + REFLECTION_HEIGHT + EDGE_MARGIN * 2
    {
        return None;
    }
    Some(Rect::new(
        ((width - DOCK_WIDTH) / 2) as i32,
        (height - DOCK_HEIGHT - REFLECTION_HEIGHT) as i32,
        DOCK_WIDTH,
        DOCK_HEIGHT,
    ))
}

fn dock_icon(dock: Rect, index: usize, lift: u8) -> Rect {
    let x = DOCK_PADDING_H + index * (DOCK_ICON_SIZE + DOCK_ICON_SPACING);
    let y = (dock.h - DOCK_ICON_SIZE) / 2;
    Rect::new(
        dock.x + x as i32,
        dock.y + y as i32 - lift as i32,
        DOCK_ICON_SIZE,
        DOCK_ICON_SIZE,
    )
}

/// Areas that can change when only desktop chrome is visible.
pub fn animated_regions(width: usize, height: usize) -> [Rect; 2] {
    let bar = bar_rect(width, 0);
    let top = Rect::new(bar.x, 0, bar.w, BAR_HEIGHT + BAR_REST_Y as usize);
    let bottom = dock_rect(width, height).map_or(Rect::new(0, 0, 0, 0), |dock| {
        let margin = 2;
        let label_margin = 72;
        let above = DESKTOP_HEIGHT + DOCK_LIFT as usize + 12;
        Rect::new(
            dock.x - label_margin,
            dock.y - above as i32,
            dock.w + label_margin as usize * 2,
            dock.h + REFLECTION_HEIGHT + above + margin,
        )
    });
    [top, bottom]
}

pub fn update_dock_lift(
    width: usize,
    height: usize,
    cursor: &Cursor,
    lift: &mut [u8; DOCK_NUM_ICONS],
    elapsed: u8,
) {
    let Some(dock) = dock_rect(width, height) else {
        return;
    };
    for (index, offset) in lift.iter_mut().enumerate() {
        let base = dock_icon(dock, index, 0);
        let hover_area = Rect::new(
            base.x,
            base.y - DOCK_LIFT as i32,
            base.w,
            base.h + DOCK_LIFT as usize,
        );
        *offset = if cursor.visible && hover_area.contains(cursor.x, cursor.y) {
            offset.saturating_add(elapsed).min(DOCK_LIFT)
        } else {
            offset.saturating_sub(elapsed)
        };
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DesktopAction {
    None,
    PurpleLogoClick,
    TrayClick,
    DockIconClick(usize),
    ShowPanel(Panel),
    Power(crate::kernel::power::Action),
    LaunchApp(crate::kernel::app_runtime::AppId),
}

#[inline(always)]
fn isqrt(val: usize) -> usize {
    let mut x = 0;
    while (x + 1) * (x + 1) <= val {
        x += 1;
    }
    x
}

/// Render an anti-aliased rounded rectangle filled with translucent glass and bordered with a subtle highlight.
pub fn render_rounded_glass_rect(
    target: &mut Surface,
    x: i32,
    y: i32,
    w: usize,
    h: usize,
    radius: usize,
    fill_color: u32,
    border_color: u32,
) {
    if !target.is_valid() || w == 0 || h == 0 {
        return;
    }

    let r = radius.min(w / 2).min(h / 2);
    let r_sq = r * r;
    let top_fill = interpolate_color(fill_color, 0xB8AD85D2, 1, 3);
    let bottom_fill = interpolate_color(fill_color, 0xDC080413, 1, 3);

    for row in 0..h {
        let py = y + row as i32;
        if py < 0 || py as usize >= target.height {
            continue;
        }

        let edge_distance = row.min(h - 1 - row);
        let dy = r.saturating_sub(edge_distance + 1);
        let cut = r.saturating_sub(isqrt(r_sq - dy * dy));
        let fill = interpolate_color(top_fill, bottom_fill, row, h - 1);
        let rim = interpolate_color(COLOR_GLASS_HIGHLIGHT, border_color, row, h - 1);
        let start = cut.max(x.saturating_neg().max(0) as usize);
        let end = (w - cut).min((target.width as i32 - x).max(0) as usize);
        for col in start..end {
            let px = (x + col as i32) as usize;
            let on_side = col == cut || col + 1 == w - cut;
            let body = if on_side && dy != 0 {
                scale_alpha(fill, 156)
            } else {
                fill
            };
            target.blend_pixel(px, py as usize, body);
            if on_side || edge_distance == 0 {
                target.blend_pixel(px, py as usize, rim);
            }
        }
    }
}

/// Inverted alpha-decaying reflection of the dock onto the wallpaper floor (Looking Glass heritage)
fn render_dock_reflection(target: &mut Surface, dock_rect: Rect) {
    let dock_bottom = dock_rect.bottom();
    let reflection_h = REFLECTION_HEIGHT.min(target.height.saturating_sub(dock_bottom as usize));

    for row in 0..reflection_h {
        let src_y = dock_bottom - 1 - (row * dock_rect.h / reflection_h) as i32;
        let dst_y = dock_bottom + row as i32;
        let remaining = reflection_h - row;
        let opacity = (105 * remaining * remaining / (reflection_h * reflection_h)) as u8;
        let inset = row * dock_rect.w / (reflection_h * 16);
        let width = dock_rect.w - 2 * inset;
        for col in 0..width {
            let src_x = dock_rect.x as usize + col * dock_rect.w / width;
            let dst_x = dock_rect.x as usize + inset + col;
            let source = target.get_pixel(src_x, src_y as usize);
            target.blend_pixel(dst_x, dst_y as usize, scale_alpha(source, opacity));
        }
    }
}

/// Render the refined Purple Bar across the top of the display with slick animations
fn render_purple_bar(target: &mut Surface, anim_tick: u64) {
    if !target.is_valid()
        || target.width < LOGO_SIZE + EDGE_MARGIN * 2
        || target.height < BAR_HEIGHT + BAR_REST_Y as usize
    {
        return;
    }

    let bar = bar_rect(target.width, anim_tick);
    let (bar_x, bar_y, bar_w, bar_h) = (bar.x, bar.y, bar.w, bar.h);

    // 2. Frosted glass panel with specular top edge and subtle border
    frost_backdrop(target, bar, 12);
    render_rounded_glass_rect(
        target,
        bar_x,
        bar_y,
        bar_w,
        bar_h,
        12,
        COLOR_GLASS_BASE,
        COLOR_GLASS_BORDER,
    );

    let logo = logo_rect(bar);
    render_rounded_glass_rect(
        target, logo.x, logo.y, logo.w, logo.h, 7, 0xEE201132, 0x807C59AE,
    );
    let mark_inset = (logo.w as i32 - super::icons::PURPLE_MARK_SIZE as i32) / 2;
    super::icons::render_purple_mark(target, logo.x + mark_inset, logo.y + mark_inset);

    // 5. Center Region: Clean Centered Digital Clock Capsule (No green dot)
    let time = crate::drivers::rtc::get_time();
    let dow = day_of_week(time.year, time.month, time.day);
    let mon = month_name(time.month);

    let mut clock_buf = [0u8; 32];
    let mut len = 0;

    for b in dow.bytes() {
        clock_buf[len] = b;
        len += 1;
    }
    clock_buf[len] = b' ';
    len += 1;
    for b in mon.bytes() {
        clock_buf[len] = b;
        len += 1;
    }
    clock_buf[len] = b' ';
    len += 1;

    if time.day >= 10 {
        clock_buf[len] = b'0' + (time.day / 10);
        len += 1;
    }
    clock_buf[len] = b'0' + (time.day % 10);
    len += 1;
    clock_buf[len] = b' ';
    len += 1;
    clock_buf[len] = b' ';
    len += 1;

    let colon_idx = len + 2;
    clock_buf[len] = b'0' + (time.hour / 10) % 10;
    len += 1;
    clock_buf[len] = b'0' + (time.hour % 10);
    len += 1;
    clock_buf[len] = b':';
    len += 1;
    clock_buf[len] = b'0' + (time.minute / 10) % 10;
    len += 1;
    clock_buf[len] = b'0' + (time.minute % 10);
    len += 1;

    let full_text = core::str::from_utf8(&clock_buf[..len]).unwrap_or("");
    let text_w = font::text_width(full_text, FontWeight::Bold);
    let pill_pad = EDGE_MARGIN;
    let pill_w = text_w + pill_pad * 2;
    let pill_h = DESKTOP_HEIGHT + 10;
    let pill_x = bar_x + (bar_w as i32 - pill_w as i32) / 2;
    let pill_y = bar_y + (bar_h - pill_h) as i32 / 2;
    let tray = tray_rect(bar);
    let show_tray = tray.x >= logo.right() + LOGO_INSET;

    if pill_x >= logo.right() + LOGO_INSET
        && (!show_tray || pill_x + pill_w as i32 + LOGO_INSET <= tray.x)
    {
        render_rounded_glass_rect(
            target,
            pill_x,
            pill_y,
            pill_w,
            pill_h,
            pill_h / 2,
            0x90110722,
            0x40A855F7,
        );
        let clock_text_x = pill_x as usize + pill_pad;
        let clock_text_y = pill_y as usize + (pill_h - DESKTOP_HEIGHT) / 2;
        let blink = (anim_tick / (ANIMATION_HZ / 2)) % 2 != 0;
        if blink && !full_text.is_empty() {
            let prefix = &full_text[..colon_idx];
            let colon_end = font::text_width(&full_text[..colon_idx + 1], FontWeight::Bold);
            target.draw_text_bold(
                clock_text_x,
                clock_text_y,
                prefix,
                COLOR_TEXT_PRIMARY,
                0,
                true,
            );
            target.draw_text_bold(
                clock_text_x + colon_end,
                clock_text_y,
                &full_text[colon_idx + 1..],
                COLOR_TEXT_PRIMARY,
                0,
                true,
            );
        } else {
            target.draw_text_bold(
                clock_text_x,
                clock_text_y,
                full_text,
                COLOR_TEXT_PRIMARY,
                0,
                true,
            );
        }
    }

    if show_tray {
        tile(target, tray, false, 0xB821162E);
        let network = if network_ready() {
            0xFF89BAFF
        } else {
            COLOR_TEXT_MUTED
        };
        for (icon, offset, color) in [
            (ShellIcon::Wifi, 9, network),
            (ShellIcon::Bluetooth, 39, COLOR_TEXT_MUTED),
            (ShellIcon::Power, 69, COLOR_TEXT_PRIMARY),
            (ShellIcon::Chevron, 101, COLOR_TEXT_MUTED),
        ] {
            shell_icon(target, icon, tray.x + offset, tray.y + 6, 18, color);
        }
    }
}

/// Render the refined Purple Dock at the bottom of the screen
fn render_purple_dock(
    target: &mut Surface,
    lift: &[u8; DOCK_NUM_ICONS],
    open_apps: &[bool; DOCK_NUM_ICONS],
    cursor: &Cursor,
) {
    if !target.is_valid() {
        return;
    }
    let Some(dock) = dock_rect(target.width, target.height) else {
        return;
    };
    frost_backdrop(target, dock, 22);
    render_rounded_glass_rect(
        target,
        dock.x,
        dock.y,
        dock.w,
        dock.h,
        22,
        COLOR_GLASS_BASE,
        COLOR_GLASS_BORDER,
    );
    for (i, offset) in lift.iter().copied().enumerate() {
        let icon = dock_icon(dock, i, offset);
        let (ix, iy) = (icon.x, icon.y);
        if offset != 0 {
            let glow = scale_alpha(COLOR_NEON_VIOLET, offset * 12);
            render_rounded_glass_rect(
                target,
                ix - 2,
                iy - 2,
                icon.w + 4,
                icon.h + 4,
                12,
                scale_alpha(glow, 64),
                glow,
            );
        }

        let (_, glyph, accent) = APPS[i];
        let base = interpolate_color(0xFF191120, accent, 1, 5);
        render_rounded_glass_rect(
            target,
            ix,
            iy,
            icon.w,
            icon.h,
            11,
            base,
            scale_alpha(accent, 120),
        );
        let inset = (DOCK_ICON_SIZE as i32 - 24) / 2;
        shell_icon(target, glyph, ix + inset, iy + inset, 24, accent);
        if open_apps[i] {
            let indicator_width = 16usize;
            let indicator_height = 4usize;
            let indicator_x = ix + (icon.w as i32 - indicator_width as i32) / 2;
            let indicator_y = dock.bottom() - indicator_height as i32 - 3;
            render_rounded_glass_rect(
                target,
                indicator_x,
                indicator_y,
                indicator_width,
                indicator_height,
                2,
                accent,
                accent,
            );
        }
    }

    // 4. Inverted mirror reflection pass below dock (Looking Glass heritage)
    render_dock_reflection(target, dock);

    if let Some((index, icon)) = lift
        .iter()
        .copied()
        .enumerate()
        .map(|(index, offset)| (index, dock_icon(dock, index, offset)))
        .find(|(_, icon)| icon.contains(cursor.x, cursor.y))
    {
        let label = APPS[index].0;
        let label_width = font::text_width(label, FontWeight::Regular);
        let width = label_width + 20;
        let height = DESKTOP_HEIGHT + 6;
        let x = (icon.x + (icon.w as i32 - width as i32) / 2)
            .clamp(0, target.width.saturating_sub(width) as i32);
        let y = icon.y - height as i32 - 5;
        tile(target, Rect::new(x, y, width, height), false, 0xF51D142B);
        text(target, x + 10, y + 3, label, COLOR_TEXT_PRIMARY, false);
    }
}

/// Render the complete desktop shell overlay in correct scene pass order
pub fn dock_open_state(windows: &[Option<Window>]) -> [bool; DOCK_NUM_ICONS] {
    const KEYS: [&[u8]; DOCK_NUM_ICONS] = [b"terminal", b"files", b"monitor", b"code", b"settings"];
    let mut open = [false; DOCK_NUM_ICONS];
    for window in windows
        .iter()
        .flatten()
        .filter(|window| window.owner.is_some())
    {
        let title = window.title_str().as_bytes();
        for (index, key) in KEYS.iter().enumerate() {
            if title
                .windows(key.len())
                .any(|part| part.eq_ignore_ascii_case(key))
            {
                open[index] = true;
            }
        }
    }
    open
}

pub fn render_desktop_shell(
    target: &mut Surface,
    tick: u64,
    lift: &[u8; DOCK_NUM_ICONS],
    open_apps: &[bool; DOCK_NUM_ICONS],
    cursor: &Cursor,
) {
    render_purple_bar(target, tick);
    render_purple_dock(target, lift, open_apps, cursor);
}

/// Decode pointer input using the geometry shown in the last presented frame.
pub fn hit_test(
    x: i32,
    y: i32,
    width: usize,
    height: usize,
    tick: u64,
    lift: &[u8; DOCK_NUM_ICONS],
) -> DesktopAction {
    if width >= LOGO_SIZE + EDGE_MARGIN * 2
        && height >= BAR_HEIGHT + BAR_REST_Y as usize
        && logo_rect(bar_rect(width, tick)).contains(x, y)
    {
        return DesktopAction::PurpleLogoClick;
    }
    let bar = bar_rect(width, tick);
    let tray = tray_rect(bar);
    if tray.x >= logo_rect(bar).right() + LOGO_INSET && tray.contains(x, y) {
        return DesktopAction::TrayClick;
    }
    if let Some(dock) = dock_rect(width, height) {
        for (index, offset) in lift.iter().copied().enumerate() {
            if dock_icon(dock, index, offset).contains(x, y) {
                return DesktopAction::DockIconClick(index);
            }
        }
    }

    DesktopAction::None
}

/// Compute 3-letter day of week for Gregorian date (Sakamoto algorithm)
fn day_of_week(year: u16, month: u8, day: u8) -> &'static str {
    let m = month as i32;
    let mut y = year as i32;
    let d = day as i32;
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    if m < 3 {
        y -= 1;
    }
    let idx = if m >= 1 && m <= 12 {
        (m - 1) as usize
    } else {
        0
    };
    let dow = ((y + y / 4 - y / 100 + y / 400 + t[idx] + d) % 7 + 7) % 7;
    match dow {
        0 => "Sun",
        1 => "Mon",
        2 => "Tue",
        3 => "Wed",
        4 => "Thu",
        5 => "Fri",
        _ => "Sat",
    }
}

/// Convert 1-based month integer to 3-letter month abbreviation
fn month_name(month: u8) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "---",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Closed,
    Applications(usize),
    AppError(&'static str),
    QuickSettings,
    Detail(QuickItem),
    DockInfo(usize),
    Power,
    PowerBusy,
    PowerError(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickItem {
    Wifi,
    Bluetooth,
    Power,
    Settings,
    Resources,
}

use super::icons::{shell_icon, ShellIcon};

const APPS: [(&str, ShellIcon, u32); DOCK_NUM_ICONS] = [
    ("Terminal", ShellIcon::Terminal, 0xFF63DFC9),
    ("Files", ShellIcon::Files, 0xFFFFC078),
    ("System Monitor", ShellIcon::Resources, 0xFF83B8FF),
    ("Code Studio", ShellIcon::Code, 0xFFFFA5D4),
    ("Settings", ShellIcon::Settings, 0xFFCBB0FF),
];

const QUICK_ITEMS: [(QuickItem, &str, ShellIcon, &str, u32); 5] = [
    (
        QuickItem::Wifi,
        "Wi-Fi",
        ShellIcon::Wifi,
        "Unavailable",
        0xFF89BAFF,
    ),
    (
        QuickItem::Bluetooth,
        "Bluetooth",
        ShellIcon::Bluetooth,
        "Unavailable",
        0xFFB8A3FF,
    ),
    (
        QuickItem::Power,
        "Power",
        ShellIcon::Power,
        "Shut down",
        0xFFFFB39E,
    ),
    (
        QuickItem::Settings,
        "Settings",
        ShellIcon::Settings,
        "Display info",
        0xFFCBB0FF,
    ),
    (
        QuickItem::Resources,
        "System Monitor",
        ShellIcon::Resources,
        "Memory usage",
        0xFF63DFC9,
    ),
];

fn network_ready() -> bool {
    crate::drivers::e1000::is_ready() || crate::drivers::rtl8139::is_ready()
}

fn tray_rect(bar: Rect) -> Rect {
    Rect::new(
        bar.right() - TRAY_WIDTH as i32 - LOGO_INSET,
        bar.y + 4,
        TRAY_WIDTH,
        30,
    )
}

pub fn panel_region(width: usize, height: usize, panel: Panel) -> Rect {
    let bar = bar_rect(width, BAR_ENTRANCE_TICKS);
    let y = bar.bottom() + 9;
    let (w, h) = match panel {
        Panel::Closed => return Rect::new(0, 0, 0, 0),
        Panel::Applications(_) => (300, 314),
        Panel::AppError(_) => (376, 88),
        Panel::QuickSettings => (342, 270),
        Panel::Detail(_) => (342, 174),
        Panel::Power | Panel::PowerBusy | Panel::PowerError(_) => {
            let w = 340;
            let h = 196;
            return if width >= w && height >= h {
                Rect::new(((width - w) / 2) as i32, ((height - h) / 2) as i32, w, h)
            } else {
                Rect::new(0, 0, 0, 0)
            };
        }
        Panel::DockInfo(_) => (224, 76),
    };
    if bar.w < w || height < y as usize + h {
        return Rect::new(0, 0, 0, 0);
    }
    match panel {
        Panel::Applications(_) | Panel::AppError(_) => Rect::new(bar.x, y, w, h),
        Panel::DockInfo(_) => {
            let bottom = DOCK_HEIGHT + REFLECTION_HEIGHT + h + 14;
            if height < y as usize + bottom {
                return Rect::new(0, 0, 0, 0);
            }
            Rect::new(((width - w) / 2) as i32, (height - bottom) as i32, w, h)
        }
        _ => Rect::new(bar.right() - w as i32, y, w, h),
    }
}

fn tile(target: &mut Surface, rect: Rect, hover: bool, fill: u32) {
    render_rounded_glass_rect(
        target,
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        13,
        if hover { 0xD6473364 } else { fill },
        0x705E4B79,
    );
}

fn text(target: &mut Surface, x: i32, y: i32, label: &str, color: u32, bold: bool) {
    if bold {
        target.draw_text_bold(x as usize, y as usize, label, color, 0, true);
    } else {
        target.draw_text(x as usize, y as usize, label, color, 0, true);
    }
}

fn app_row(panel: Rect, index: usize) -> Rect {
    Rect::new(
        panel.x + 12,
        panel.y + 47 + index as i32 * 46,
        panel.w - 24,
        41,
    )
}

fn app_pages(panel: Rect) -> [Rect; 2] {
    let y = panel.bottom() - 35;
    [
        Rect::new(panel.x + 12, y, 88, 25),
        Rect::new(panel.right() - 100, y, 88, 25),
    ]
}

fn quick_tile(panel: Rect, index: usize) -> Rect {
    let column = (panel.w - 43) / 2;
    let x = panel.x + 16 + (index % 2 * (column + 11)) as i32;
    Rect::new(
        x,
        panel.y + 66 + (index / 2) as i32 * 64,
        if index == 4 { panel.w - 32 } else { column },
        55,
    )
}

fn detail_back(panel: Rect) -> Rect {
    Rect::new(panel.x + 16, panel.bottom() - 48, 92, 32)
}

impl Panel {
    pub fn is_power(self) -> bool {
        matches!(self, Self::Power | Self::PowerBusy | Self::PowerError(_))
    }
}

fn power_buttons(panel: Rect) -> [Rect; 2] {
    let width = (panel.w - 52) / 2;
    [
        Rect::new(panel.x + 20, panel.y + 57, width, 76),
        Rect::new(panel.x + 32 + width as i32, panel.y + 57, width, 76),
    ]
}

fn power_cancel(panel: Rect) -> Rect {
    Rect::new(panel.right() - 112, panel.bottom() - 46, 92, 28)
}

fn dim_desktop(target: &mut Surface) {
    for y in 0..target.height {
        for x in 0..target.width {
            target.blend_pixel(x, y, 0x7205010B);
        }
    }
}

fn render_power(target: &mut Surface, panel: Panel, area: Rect, cursor: &Cursor) {
    dim_desktop(target);
    frost_backdrop(target, area, 20);
    render_rounded_glass_rect(
        target, area.x, area.y, area.w, area.h, 20, 0xE51B1028, 0xA87857A1,
    );
    text(target, area.x + 20, area.y + 16, "Power", 0xFFF6F6F6, true);

    let buttons = power_buttons(area);
    for (i, (label, icon, color, fill)) in [
        ("Shutdown", ShellIcon::Power, 0xFFFFB8A8, 0xFF662D36),
        ("Restart", ShellIcon::Restart, 0xFFB9EAB8, 0xFF285342),
    ]
    .into_iter()
    .enumerate()
    {
        let button = buttons[i];
        let fill = if panel != Panel::PowerBusy && button.contains(cursor.x, cursor.y) {
            interpolate_color(fill, color, 1, 4)
        } else {
            fill
        };
        tile(target, button, false, fill);
        shell_icon(
            target,
            icon,
            button.x + (button.w as i32 - 24) / 2,
            button.y + 10,
            24,
            color,
        );
        text(
            target,
            button.x + (button.w as i32 - font::text_width(label, FontWeight::Bold) as i32) / 2,
            button.y + 45,
            label,
            COLOR_TEXT_PRIMARY,
            true,
        );
    }

    match panel {
        Panel::PowerBusy => text(
            target,
            area.x + 20,
            area.bottom() - 37,
            "Syncing system disk...",
            0xFFE1E1E1,
            false,
        ),
        _ => {
            if let Panel::PowerError(message) = panel {
                text(
                    target,
                    area.x + 20,
                    area.y + 136,
                    "Power action cancelled",
                    0xFFFFFFFF,
                    true,
                );
                let label = if message.len() > 24 {
                    "Check serial log"
                } else {
                    message
                };
                text(
                    target,
                    area.x + 20,
                    area.bottom() - 31,
                    label,
                    0xFFE1E1E1,
                    false,
                );
            }
            let cancel = power_cancel(area);
            tile(
                target,
                cancel,
                cancel.contains(cursor.x, cursor.y),
                0xFF393939,
            );
            text(
                target,
                cancel.x + 20,
                cancel.y + 6,
                "Cancel",
                0xFFF6F6F6,
                false,
            );
        }
    }
}

pub fn render_panel(target: &mut Surface, panel: Panel, cursor: &Cursor) {
    let area = panel_region(target.width, target.height, panel);
    if area.w == 0 {
        return;
    }
    if panel.is_power() {
        render_power(target, panel, area, cursor);
        return;
    }
    let hovered = |rect: Rect| rect.contains(cursor.x, cursor.y);
    frost_backdrop(target, area, 18);
    render_rounded_glass_rect(
        target, area.x, area.y, area.w, area.h, 18, 0xC021162E, 0xA4705692,
    );
    let x = area.x + 16;
    let y = area.y + 14;
    match panel {
        Panel::Closed => {}
        Panel::Applications(page) => {
            text(target, x, y, "Applications", COLOR_TEXT_PRIMARY, true);
            let (entries, more) = crate::kernel::app_runtime::launcher_page::<5>(page);
            if entries[0].is_none() {
                text(
                    target,
                    x,
                    y + 36,
                    "No installed applications",
                    COLOR_TEXT_MUTED,
                    false,
                );
            }
            for (index, entry) in entries.iter().enumerate() {
                if let Some(entry) = entry {
                    let row = app_row(area, index);
                    tile(target, row, hovered(row), 0xFF261B36);
                    shell_icon(
                        target,
                        ShellIcon::Files,
                        row.x + 9,
                        row.y + 10,
                        21,
                        COLOR_NEON_VIOLET,
                    );
                    text(
                        target,
                        row.x + 40,
                        row.y + 12,
                        entry.as_str(),
                        COLOR_TEXT_PRIMARY,
                        true,
                    );
                }
            }
            let pages = app_pages(area);
            for (button, enabled, label) in
                [(pages[0], page > 0, "Previous"), (pages[1], more, "Next")]
            {
                if enabled {
                    tile(target, button, hovered(button), 0xFF261B36);
                    text(
                        target,
                        button.x + 9,
                        button.y + 5,
                        label,
                        COLOR_TEXT_PRIMARY,
                        false,
                    );
                }
            }
        }
        Panel::AppError(error) => {
            text(
                target,
                x,
                y,
                "Application could not start",
                COLOR_TEXT_PRIMARY,
                true,
            );
            text(target, x, y + 28, error, COLOR_TEXT_MUTED, false);
        }
        Panel::QuickSettings => {
            text(target, x, y, "Quick Settings", COLOR_TEXT_PRIMARY, true);
            let status = if network_ready() {
                "Wired adapter detected"
            } else {
                "No network adapter"
            };
            text(target, x, y + 21, status, COLOR_TEXT_MUTED, false);
            for (index, (_, label, icon, subtitle, accent)) in
                QUICK_ITEMS.iter().copied().enumerate()
            {
                let row = quick_tile(area, index);
                tile(target, row, hovered(row), 0xFF2D203F);
                shell_icon(target, icon, row.x + 10, row.y + 16, 22, accent);
                text(
                    target,
                    row.x + 40,
                    row.y + 8,
                    label,
                    COLOR_TEXT_PRIMARY,
                    true,
                );
                text(
                    target,
                    row.x + 40,
                    row.y + 28,
                    subtitle,
                    COLOR_TEXT_MUTED,
                    false,
                );
            }
        }
        Panel::Detail(item) => {
            let (_, title, icon, _, accent) = QUICK_ITEMS
                .iter()
                .copied()
                .find(|entry| entry.0 == item)
                .unwrap();
            text(target, x, y, title, COLOR_TEXT_PRIMARY, true);
            shell_icon(target, icon, area.right() - 41, y, 24, accent);
            match item {
                QuickItem::Wifi => {
                    text(
                        target,
                        x,
                        y + 34,
                        "Wi-Fi controls unavailable",
                        COLOR_TEXT_MUTED,
                        false,
                    );
                    let wired = if network_ready() {
                        "Wired adapter detected"
                    } else {
                        "No wired adapter detected"
                    };
                    text(target, x, y + 55, wired, COLOR_TEXT_MUTED, false);
                }
                QuickItem::Bluetooth => text(
                    target,
                    x,
                    y + 34,
                    "Bluetooth controls unavailable",
                    COLOR_TEXT_MUTED,
                    false,
                ),
                QuickItem::Power => {}
                QuickItem::Settings => {
                    text(
                        target,
                        x,
                        y + 34,
                        "Display: linear framebuffer",
                        COLOR_TEXT_MUTED,
                        false,
                    );
                    text(
                        target,
                        x,
                        y + 55,
                        "Pointer: PS/2 mouse",
                        COLOR_TEXT_MUTED,
                        false,
                    );
                }
                QuickItem::Resources => {
                    let total = crate::mm::pmm::total_frames_count();
                    let used = total.saturating_sub(crate::mm::pmm::free_frames_count());
                    let mut number = [0; 20];
                    let len = crate::fs::vfs::format_u64((used / 256) as u64, &mut number);
                    text(
                        target,
                        x,
                        y + 33,
                        "Memory used (MiB)",
                        COLOR_TEXT_MUTED,
                        false,
                    );
                    if let Ok(value) = core::str::from_utf8(&number[..len]) {
                        text(target, x + 158, y + 33, value, COLOR_TEXT_PRIMARY, true);
                    }
                    let meter = area.w - 32;
                    target.fill_rect(Rect::new(x, y + 65, meter, 6), 0xFF463854);
                    let filled = used.saturating_mul(meter).checked_div(total).unwrap_or(0);
                    target.fill_rect(Rect::new(x, y + 65, filled, 6), accent);
                }
            }
            let back = detail_back(area);
            tile(target, back, hovered(back), 0xFF312342);
            text(
                target,
                back.x + 13,
                back.y + 8,
                "Back",
                COLOR_TEXT_PRIMARY,
                false,
            );
        }
        Panel::DockInfo(index) => {
            let (label, icon, accent) = APPS[index];
            shell_icon(target, icon, x, y + 9, 24, accent);
            text(target, x + 37, y + 3, label, COLOR_TEXT_PRIMARY, true);
            text(
                target,
                x + 37,
                y + 25,
                "Not installed",
                COLOR_TEXT_MUTED,
                false,
            );
        }
        Panel::Power | Panel::PowerBusy | Panel::PowerError(_) => {}
    }
}

pub fn app_panel(index: usize) -> Panel {
    match index {
        2 => Panel::Detail(QuickItem::Resources),
        4 => Panel::Detail(QuickItem::Settings),
        _ => Panel::DockInfo(index),
    }
}

/// A click inside a popup is always consumed, including its background.
pub fn panel_action(panel: Panel, area: Rect, x: i32, y: i32) -> DesktopAction {
    match panel {
        Panel::Applications(page) => {
            let (entries, more) = crate::kernel::app_runtime::launcher_page::<5>(page);
            for (index, entry) in entries.into_iter().enumerate() {
                if let Some(entry) = entry.filter(|_| app_row(area, index).contains(x, y)) {
                    return DesktopAction::LaunchApp(entry);
                }
            }
            let buttons = app_pages(area);
            if page > 0 && buttons[0].contains(x, y) {
                return DesktopAction::ShowPanel(Panel::Applications(page - 1));
            }
            if more && buttons[1].contains(x, y) {
                return DesktopAction::ShowPanel(Panel::Applications(page + 1));
            }
        }
        Panel::QuickSettings => {
            if let Some(index) =
                (0..QUICK_ITEMS.len()).find(|&i| quick_tile(area, i).contains(x, y))
            {
                let item = QUICK_ITEMS[index].0;
                return DesktopAction::ShowPanel(if item == QuickItem::Power {
                    Panel::Power
                } else {
                    Panel::Detail(item)
                });
            }
        }
        Panel::Detail(_) => {
            if detail_back(area).contains(x, y) {
                return DesktopAction::ShowPanel(Panel::QuickSettings);
            }
        }
        Panel::Power | Panel::PowerError(_) => {
            use crate::kernel::power::Action;
            if power_cancel(area).contains(x, y) {
                return DesktopAction::ShowPanel(Panel::QuickSettings);
            }
            for (button, action) in power_buttons(area)
                .into_iter()
                .zip([Action::Shutdown, Action::Restart])
            {
                if button.contains(x, y) {
                    return DesktopAction::Power(action);
                }
            }
        }
        _ => {}
    }
    DesktopAction::ShowPanel(panel)
}
