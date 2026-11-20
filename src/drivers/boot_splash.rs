//! Short framebuffer splash shown after device initialization and before the desktop.
//! The upper firmware framebuffer is left intact for any OEM logo already present.
use super::video;

const LOGO_WIDTH: usize = 600;
const LOGO_HEIGHT: usize = 200;
const LOGO: &[u8; LOGO_WIDTH * LOGO_HEIGHT * 4] =
    include_bytes!("../../assets/branding/purple-os-logo.rgba");
const PANEL: u32 = 0xFF160D25;
const ACCENT: u32 = 0xFF9B69F5;

pub fn show() {
    let Some((width, height, pitch, _, address)) = video::get_fb_info() else {
        return;
    };
    if width < 160 || height < 160 {
        return;
    }
    let fb = address as *mut u32;
    let panel_height = (height * 34 / 100).clamp(150, height);
    let panel_top = height - panel_height;
    draw_panel(fb, width, height, pitch, panel_top);

    let padding = 24usize.min(width / 8);
    let logo_max_height = panel_height.saturating_sub(82).max(1);
    let logo_width = LOGO_WIDTH
        .min(width.saturating_sub(padding * 2))
        .min(logo_max_height.saturating_mul(3));
    let logo_height = (logo_width * LOGO_HEIGHT / LOGO_WIDTH).max(1);
    let logo_x = (width - logo_width) / 2;
    let logo_y = panel_top + 18;
    draw_logo(
        fb,
        width,
        height,
        pitch,
        logo_x,
        logo_y,
        logo_width,
        logo_height,
    );

    let spinner_x = width / 2;
    let spinner_y = logo_y + logo_height + 12 + 18;
    for phase in 0..18 {
        draw_spinner(fb, width, height, pitch, spinner_x, spinner_y, phase);
        super::pit::delay_us_long(70_000);
    }
}

fn draw_panel(fb: *mut u32, width: usize, height: usize, pitch: usize, top: usize) {
    let radius = 18usize.min(width / 2).min(height.saturating_sub(top));
    fill_rect(
        fb,
        width,
        height,
        pitch,
        0,
        top + radius,
        width,
        height - top - radius,
        PANEL,
    );
    fill_rect(
        fb,
        width,
        height,
        pitch,
        radius,
        top,
        width.saturating_sub(radius * 2),
        radius,
        PANEL,
    );
    fill_circle(
        fb,
        width,
        height,
        pitch,
        radius,
        top + radius,
        radius,
        PANEL,
    );
    fill_circle(
        fb,
        width,
        height,
        pitch,
        width.saturating_sub(radius + 1),
        top + radius,
        radius,
        PANEL,
    );
    fill_rect(fb, width, height, pitch, 0, top, width, 1, ACCENT);
}

fn draw_logo(
    fb: *mut u32,
    width: usize,
    height: usize,
    pitch: usize,
    x: usize,
    y: usize,
    draw_width: usize,
    draw_height: usize,
) {
    for dy in 0..draw_height {
        let sy = dy * LOGO_HEIGHT / draw_height;
        for dx in 0..draw_width {
            let sx = dx * LOGO_WIDTH / draw_width;
            let i = (sy * LOGO_WIDTH + sx) * 4;
            let alpha = LOGO[i + 3] as u32;
            if alpha == 0 {
                continue;
            }
            let color =
                (u32::from(LOGO[i]) << 16) | (u32::from(LOGO[i + 1]) << 8) | u32::from(LOGO[i + 2]);
            blend_pixel(fb, width, height, pitch, x + dx, y + dy, color, alpha);
        }
    }
}

fn draw_spinner(
    fb: *mut u32,
    width: usize,
    height: usize,
    pitch: usize,
    cx: usize,
    cy: usize,
    phase: usize,
) {
    let offsets: [(isize, isize); 12] = [
        (0, -16),
        (8, -14),
        (14, -8),
        (16, 0),
        (14, 8),
        (8, 14),
        (0, 16),
        (-8, 14),
        (-14, 8),
        (-16, 0),
        (-14, -8),
        (-8, -14),
    ];
    fill_rect(
        fb,
        width,
        height,
        pitch,
        cx.saturating_sub(22),
        cy.saturating_sub(22),
        44,
        44,
        PANEL,
    );
    for i in 0..offsets.len() {
        let (ox, oy) = offsets[i];
        let alpha = 255usize.saturating_sub(((i + phase) % offsets.len()) * 17) as u32;
        let x = (cx as isize + ox).max(0) as usize;
        let y = (cy as isize + oy).max(0) as usize;
        fill_circle(fb, width, height, pitch, x, y, 2, with_alpha(ACCENT, alpha));
    }
}

fn with_alpha(color: u32, alpha: u32) -> u32 {
    let inv = 255 - alpha;
    let channel = |shift: u32| {
        ((((color >> shift) & 255u32) * alpha + ((PANEL >> shift) & 255u32) * inv + 127) / 255)
            << shift
    };
    channel(16) | channel(8) | channel(0)
}

fn fill_rect(
    fb: *mut u32,
    width: usize,
    height: usize,
    pitch: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: u32,
) {
    let right = x.saturating_add(w).min(width);
    let bottom = y.saturating_add(h).min(height);
    for py in y.min(height)..bottom {
        for px in x.min(width)..right {
            unsafe { fb.add(py * pitch + px).write_volatile(color) };
        }
    }
}

fn fill_circle(
    fb: *mut u32,
    width: usize,
    height: usize,
    pitch: usize,
    cx: usize,
    cy: usize,
    radius: usize,
    color: u32,
) {
    let r2 = (radius * radius) as isize;
    for dy in -(radius as isize)..=(radius as isize) {
        for dx in -(radius as isize)..=(radius as isize) {
            if dx * dx + dy * dy > r2 {
                continue;
            }
            let x = cx as isize + dx;
            let y = cy as isize + dy;
            if x >= 0 && y >= 0 && (x as usize) < width && (y as usize) < height {
                unsafe {
                    fb.add(y as usize * pitch + x as usize)
                        .write_volatile(color)
                };
            }
        }
    }
}

fn blend_pixel(
    fb: *mut u32,
    width: usize,
    height: usize,
    pitch: usize,
    x: usize,
    y: usize,
    fg: u32,
    alpha: u32,
) {
    if x >= width || y >= height || alpha == 0 {
        return;
    }
    let dst = unsafe { fb.add(y * pitch + x).read_volatile() };
    let inv = 255 - alpha;
    let blend = |shift: u32| {
        ((((fg >> shift) & 255u32) * alpha + ((dst >> shift) & 255u32) * inv + 127) / 255) << shift
    };
    unsafe {
        fb.add(y * pitch + x)
            .write_volatile(0xFF00_0000 | blend(16) | blend(8) | blend(0));
    }
}
