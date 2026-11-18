//! Native Compositing Pipeline and Scene Graph Renderer for Aegis64.
//!
//! Pure compositor implementation:
//! 1. Root canvas background clear
//! 2. Window Z-order sorted frame & client surface pass
//! 3. Flicker-free hardware/software cursor overlay pass
//! 4. Zero-copy hardware VBE page flip / LFB presentation pass

use super::cursor::Cursor;
use super::surface::Surface;
use super::window::Window;

pub const COLOR_ROOT_CANVAS: u32 = 0xFF140824;

pub struct CompositorPipeline;

impl CompositorPipeline {
    /// Render the scene onto the target back-buffer surface
    pub fn render_scene(
        target: &mut Surface,
        windows: &[Option<Window>],
        cursor: &mut Cursor,
        _fps_counter: u64,
    ) {
        if !target.is_valid() {
            return;
        }

        // Pass 1: Clean solid root compositor canvas
        target.clear(COLOR_ROOT_CANVAS);

        // Pass 2: Render Windows in Z-Order (ascending: lowest Z first, highest on top)
        Self::render_windows(target, windows);

        // Pass 3: Render Cursor Overlay on Top
        cursor.draw(target);
    }

    /// Sort and render visible windows in ascending Z-order
    fn render_windows(target: &mut Surface, windows: &[Option<Window>]) {
        let mut indices: [usize; 16] = [0; 16];
        let mut count = 0;

        for (i, win_opt) in windows.iter().enumerate() {
            if let Some(win) = win_opt {
                if win.is_visible && !win.is_minimized {
                    indices[count] = i;
                    count += 1;
                }
            }
        }

        // Sort indices by window.z_order ascending (insertion sort)
        for i in 1..count {
            let mut j = i;
            while j > 0 {
                let idx_a = indices[j - 1];
                let idx_b = indices[j];
                let z_a = windows[idx_a].as_ref().map(|w| w.z_order).unwrap_or(0);
                let z_b = windows[idx_b].as_ref().map(|w| w.z_order).unwrap_or(0);
                if z_a > z_b {
                    indices.swap(j - 1, j);
                    j -= 1;
                } else {
                    break;
                }
            }
        }

        // Render each window from lowest to highest
        for &idx in &indices[..count] {
            if let Some(win) = &windows[idx] {
                win.render_decorations(target);
                win.render_content(target);
            }
        }
    }
}
