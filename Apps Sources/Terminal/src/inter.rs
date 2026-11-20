#[path = "../../../sdk/gui/inter.rs"]
mod rasterizer;

pub use rasterizer::{Glyph, Inter, HEIGHT, SUBPIXELS};

static mut FONT: Inter = Inter::EMPTY;

/// Called once by the app before publishing the font.
pub unsafe fn initialize(bytes: &[u8]) -> Option<&'static Inter> {
    let font = &mut *(&raw mut FONT);
    font.load(bytes)?;
    Some(font)
}
