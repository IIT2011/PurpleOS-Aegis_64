//! Background-only Purple nebula wallpaper with dynamic display scaling.

use super::surface::{Rect, Surface};
use crate::sync::Spinlock;

const RAW_MAGIC: &[u8; 8] = b"PWRAW001";
const RAW_HEADER_SIZE: usize = 16;
const BYTES_PER_PIXEL: usize = 3;

static WALLPAPER_ASSET: &[u8] = include_bytes!("../../assets/wallpapers/purple_nebula.pwraw");
static WALLPAPER_CACHE: Spinlock<Option<Surface>> = Spinlock::new(None);

pub struct Wallpaper;

impl Wallpaper {
    /// Render the wallpaper at the target surface's active display dimensions.
    pub fn render_to_surface(target: &mut Surface) {
        if !target.is_valid() {
            return;
        }

        let screen = Rect::new(0, 0, target.width, target.height);
        Self::restore_regions(target, &[screen]);
    }

    /// Restore the wallpaper beneath damaged desktop regions.
    pub fn restore_regions(target: &mut Surface, regions: &[Rect]) {
        let mut cache = WALLPAPER_CACHE.lock();
        let needs_redraw = match *cache {
            Some(ref surface) => surface.width != target.width || surface.height != target.height,
            None => true,
        };

        if needs_redraw {
            if let Some(mut old_surface) = cache.take() {
                old_surface.destroy();
            }

            if let Some(mut surface) = Surface::new(target.width, target.height) {
                if Self::render_scaled(&mut surface) {
                    *cache = Some(surface);
                } else {
                    surface.destroy();
                }
            }
        }

        if let Some(ref cached_surface) = *cache {
            let screen = Rect::new(0, 0, target.width, target.height);
            for rect in regions {
                if let Some(rect) = rect.intersection(&screen) {
                    let x = rect.x as usize;
                    for y in rect.y as usize..rect.bottom() as usize {
                        unsafe {
                            let src = cached_surface.pixels.add(y * cached_surface.stride + x);
                            let dst = target.pixels.add(y * target.stride + x);
                            core::ptr::copy_nonoverlapping(src, dst, rect.w);
                        }
                    }
                }
            }
        } else if !Self::render_scaled(target) {
            target.clear(super::pipeline::COLOR_ROOT_CANVAS);
        }
    }

    /// Scale with a center crop, preserving the wallpaper's aspect ratio.
    /// The target dimensions come from the active framebuffer, never a preset mode.
    pub fn render_scaled(target: &mut Surface) -> bool {
        if !target.is_valid() {
            return false;
        }

        let Some((source_width, source_height, pixels)) = Self::asset() else {
            return false;
        };
        if source_width == 0 || source_height == 0 {
            return false;
        }

        let target_width = target.width;
        let target_height = target.height;
        let (crop_x, crop_y, crop_width, crop_height) =
            Self::center_crop(source_width, source_height, target_width, target_height);
        let max_x = source_width - 1;
        let max_y = source_height - 1;

        for y in 0..target_height {
            let source_y_fp = Self::map_axis(y, target_height, crop_y, crop_height);
            let y0 = ((source_y_fp >> 16) as usize).min(max_y);
            let y1 = (y0 + 1).min(max_y);
            let fy = ((source_y_fp & 0xFFFF) >> 8) as u32;
            let inv_fy = 256 - fy;

            for x in 0..target_width {
                let source_x_fp = Self::map_axis(x, target_width, crop_x, crop_width);
                let x0 = ((source_x_fp >> 16) as usize).min(max_x);
                let x1 = (x0 + 1).min(max_x);
                let fx = ((source_x_fp & 0xFFFF) >> 8) as u32;
                let inv_fx = 256 - fx;

                let w00 = inv_fx * inv_fy;
                let w10 = fx * inv_fy;
                let w01 = inv_fx * fy;
                let w11 = fx * fy;
                let i00 = (y0 * source_width + x0) * BYTES_PER_PIXEL;
                let i10 = (y0 * source_width + x1) * BYTES_PER_PIXEL;
                let i01 = (y1 * source_width + x0) * BYTES_PER_PIXEL;
                let i11 = (y1 * source_width + x1) * BYTES_PER_PIXEL;

                let mut color = 0xFF00_0000;
                for channel in 0..BYTES_PER_PIXEL {
                    let value = (pixels[i00 + channel] as u32 * w00
                        + pixels[i10 + channel] as u32 * w10
                        + pixels[i01 + channel] as u32 * w01
                        + pixels[i11 + channel] as u32 * w11)
                        >> 16;
                    let shift = match channel {
                        0 => 16,
                        1 => 8,
                        _ => 0,
                    };
                    color |= value << shift;
                }
                target.set_pixel(x, y, color);
            }
        }

        true
    }

    fn asset() -> Option<(usize, usize, &'static [u8])> {
        if WALLPAPER_ASSET.len() < RAW_HEADER_SIZE
            || &WALLPAPER_ASSET[..RAW_MAGIC.len()] != RAW_MAGIC
        {
            return None;
        }

        let width = u32::from_le_bytes(WALLPAPER_ASSET[8..12].try_into().ok()?) as usize;
        let height = u32::from_le_bytes(WALLPAPER_ASSET[12..16].try_into().ok()?) as usize;
        let pixel_bytes = width.checked_mul(height)?.checked_mul(BYTES_PER_PIXEL)?;
        if width == 0 || height == 0 || WALLPAPER_ASSET.len() != RAW_HEADER_SIZE + pixel_bytes {
            return None;
        }

        Some((width, height, &WALLPAPER_ASSET[RAW_HEADER_SIZE..]))
    }

    fn center_crop(
        source_width: usize,
        source_height: usize,
        target_width: usize,
        target_height: usize,
    ) -> (usize, usize, usize, usize) {
        let source_wide = (source_width as u128) * (target_height as u128);
        let target_wide = (target_width as u128) * (source_height as u128);

        if target_wide > source_wide {
            let crop_height = ((source_width as u128) * (target_height as u128)
                / (target_width as u128)) as usize;
            let crop_height = crop_height.clamp(1, source_height);
            (
                0,
                (source_height - crop_height) / 2,
                source_width,
                crop_height,
            )
        } else {
            let crop_width = ((source_height as u128) * (target_width as u128)
                / (target_height as u128)) as usize;
            let crop_width = crop_width.clamp(1, source_width);
            (
                (source_width - crop_width) / 2,
                0,
                crop_width,
                source_height,
            )
        }
    }

    fn map_axis(
        position: usize,
        target_length: usize,
        crop_start: usize,
        crop_length: usize,
    ) -> u64 {
        if target_length <= 1 || crop_length <= 1 {
            return ((crop_start + crop_length / 2) as u64) << 16;
        }

        let offset_fp = (position as u128) * ((crop_length - 1) as u128) * 65536
            / ((target_length - 1) as u128);
        ((crop_start as u128 * 65536 + offset_fp).min(u64::MAX as u128)) as u64
    }
}
