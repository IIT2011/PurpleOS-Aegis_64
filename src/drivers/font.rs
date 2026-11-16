use super::font8x16::FONT8X16;

pub const FONT_WIDTH: usize = 8;
pub const FONT_HEIGHT: usize = 16;

pub fn draw_char(
    fb: &mut [u32],
    fb_width: usize,
    fb_height: usize,
    x: usize,
    y: usize,
    c: u8,
    fg: u32,
    bg: u32,
) {
    if x + FONT_WIDTH > fb_width || y + FONT_HEIGHT > fb_height {
        return;
    }

    let glyph = FONT8X16[c as usize];
    for row in 0..16 {
        let bits = glyph[row];
        let py = y + row;
        for col in 0..8 {
            let px = x + col;
            let color = if (bits & (0x80 >> col)) != 0 { fg } else { bg };
            if color != 0 {
                fb[py * fb_width + px] = color;
            }
        }
    }
}
