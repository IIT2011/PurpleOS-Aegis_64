// SPDX-License-Identifier: LGPL-2.1-or-later
//! VLC Media Player UI Renderer in Pure Rust with Hardware Acceleration Badge.

pub struct VlcUi {
    pub width: u32,
    pub height: u32,
}

impl VlcUi {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn draw_rect(&self, buf: &mut [u32], x: u32, y: u32, w: u32, h: u32, color: u32) {
        let x_end = (x + w).min(self.width);
        let y_end = (y + h).min(self.height);
        for cy in y..y_end {
            let row = (cy * self.width) as usize;
            for cx in x..x_end {
                buf[row + cx as usize] = color;
            }
        }
    }

    pub fn draw_char_3x5(&self, buf: &mut [u32], x: u32, y: u32, ch: char, color: u32) {
        let rows: [u8; 5] = match ch.to_ascii_uppercase() {
            'A' => [0b111, 0b101, 0b111, 0b101, 0b101],
            'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
            'C' => [0b111, 0b100, 0b100, 0b100, 0b111],
            'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
            'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
            'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
            'G' => [0b111, 0b100, 0b101, 0b101, 0b111],
            'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
            'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
            'J' => [0b001, 0b001, 0b001, 0b101, 0b111],
            'K' => [0b101, 0b110, 0b100, 0b110, 0b101],
            'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
            'M' => [0b101, 0b111, 0b101, 0b101, 0b101],
            'N' => [0b110, 0b101, 0b101, 0b101, 0b101],
            'O' => [0b111, 0b101, 0b101, 0b101, 0b111],
            'P' => [0b111, 0b101, 0b111, 0b100, 0b100],
            'Q' => [0b111, 0b101, 0b101, 0b111, 0b001],
            'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
            'S' => [0b111, 0b100, 0b111, 0b001, 0b111],
            'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
            'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
            'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
            'W' => [0b101, 0b101, 0b101, 0b111, 0b101],
            'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
            'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
            'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
            '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
            '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
            '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
            '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
            '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
            '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
            '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
            '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
            '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
            '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
            ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
            '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
            '[' => [0b110, 0b100, 0b100, 0b100, 0b110],
            ']' => [0b011, 0b001, 0b001, 0b001, 0b011],
            '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
            _ => [0b000, 0b000, 0b000, 0b000, 0b000],
        };

        for (ry, &row_bits) in rows.iter().enumerate() {
            let py = y + ry as u32;
            if py >= self.height {
                continue;
            }
            for rx in 0..3 {
                let px = x + rx as u32;
                if px >= self.width {
                    continue;
                }
                if (row_bits & (1 << (2 - rx))) != 0 {
                    buf[(py * self.width + px) as usize] = color;
                }
            }
        }
    }

    pub fn draw_text(&self, buf: &mut [u32], mut x: u32, y: u32, text: &str, color: u32) {
        for ch in text.chars() {
            if ch == ' ' {
                x += 3;
            } else {
                self.draw_char_3x5(buf, x, y, ch, color);
                x += 4;
            }
            if x >= self.width {
                break;
            }
        }
    }

    pub fn draw_cone(&self, buf: &mut [u32], cx: i32, cy: i32, scale: i32) {
        let orange = 0xFFF07000u32;
        let white = 0xFFF8F8F8u32;
        let base_dark = 0xFF884000u32;

        // Base ring
        for y in -6..=6 {
            for x in -30..=30 {
                let dx = x * 10 / 30;
                let dy = y * 10 / 6;
                if dx * dx + dy * dy <= 100 {
                    let px = (cx + x * scale / 10) as u32;
                    let py = (cy + 45 * scale / 10 + y * scale / 10) as u32;
                    if px < self.width && py < self.height {
                        buf[(py * self.width + px) as usize] = base_dark;
                    }
                }
            }
        }

        // Cone body (rings from bottom to top)
        for h in 0..(80 * scale / 10) {
            let rel_y = (80 * scale / 10) - h;
            let half_w = (rel_y * 32) / (80 * scale / 10);
            let py = cy - 40 * scale / 10 + h;
            if py < 0 || py >= self.height as i32 {
                continue;
            }

            // White stripes vs orange sections
            let color = if (h >= (22 * scale / 10) && h <= (34 * scale / 10))
                || (h >= (48 * scale / 10) && h <= (60 * scale / 10))
            {
                white
            } else {
                orange
            };

            for x in -half_w..=half_w {
                let px = cx + x;
                if px >= 0 && px < self.width as i32 {
                    buf[(py as u32 * self.width + px as u32) as usize] = color;
                }
            }
        }
    }

    pub fn draw_spectrum(&self, buf: &mut [u32], pcm: &[i16], base_y: u32, max_h: u32) {
        if pcm.is_empty() {
            return;
        }

        let bars = 64;
        let bar_width = 8u32;
        let gap = 2u32;
        let total_w = bars * (bar_width + gap);
        let start_x = (self.width.saturating_sub(total_w)) / 2;

        let chunk_size = (pcm.len() / bars as usize).max(1);

        for (i, chunk) in pcm.chunks(chunk_size).take(bars as usize).enumerate() {
            let mut sum: u64 = 0;
            for &s in chunk {
                sum += s.abs() as u64;
            }
            let avg = (sum / chunk.len() as u64) as u32;
            let h = ((avg * max_h) / 32768).min(max_h).max(3);

            let bx = start_x + (i as u32) * (bar_width + gap);
            let by = base_y - h;

            // Gradient bar
            for y in 0..h {
                let ratio = y as f32 / max_h as f32;
                let r = (240.0 * (1.0 - ratio) + 255.0 * ratio) as u32;
                let g = (112.0 * (1.0 - ratio) + 180.0 * ratio) as u32;
                let b = (20.0 * ratio) as u32;
                let col = 0xFF000000 | (r << 16) | (g << 8) | b;

                for x in 0..bar_width {
                    let px = bx + x;
                    let py = by + y;
                    if px < self.width && py < self.height {
                        buf[(py * self.width + px) as usize] = col;
                    }
                }
            }
        }
    }

    pub fn render_frame(
        &self,
        buf: &mut [u32],
        is_playing: bool,
        position: f64,
        volume: f32,
        _time_ms: i64,
        pcm: &[i16],
        is_hw: bool,
        hw_desc: &str,
    ) {
        // Acrylic dark background
        buf.fill(0xFF14141Eu32);

        // Titlebar
        self.draw_rect(buf, 0, 0, self.width, 36, 0xFF1E1E2Eu32);

        // Mini cone in titlebar
        self.draw_cone(buf, 20, 18, 2);

        // Title text
        self.draw_text(buf, 38, 15, "VLC MEDIA PLAYER", 0xFFE0E0F0);

        // Hardware Acceleration Status Badge in Titlebar
        let badge_w = 175u32;
        let badge_x = self.width.saturating_sub(badge_w + 12);
        if is_hw {
            // Neon cyan / teal hardware acceleration pill
            self.draw_rect(buf, badge_x, 9, badge_w, 18, 0xFF0D3248u32);
            self.draw_rect(buf, badge_x, 9, 3, 18, 0xFF00E5FFu32); // Glowing edge
            self.draw_text(buf, badge_x + 8, 15, "HW ACCEL: ", 0xFF00E5FFu32);
            self.draw_text(buf, badge_x + 60, 15, hw_desc, 0xFFFFFFFFu32);
        } else {
            // Subtle gray software fallback pill
            self.draw_rect(buf, badge_x, 9, badge_w, 18, 0xFF282838u32);
            self.draw_text(buf, badge_x + 8, 15, "HW ACCEL: OFF", 0xFF888899u32);
        }

        // Control Toolbar
        let bar_y = self.height - 60;
        self.draw_rect(buf, 0, bar_y, self.width, 60, 0xFF1A1A28u32);

        // Progress bar track
        self.draw_rect(buf, 16, bar_y + 10, self.width - 32, 6, 0xFF2E2E3Eu32);
        let fill_w = (((self.width - 32) as f64) * position.clamp(0.0, 1.0)) as u32;
        self.draw_rect(buf, 16, bar_y + 10, fill_w, 6, 0xFFF07000u32);

        // Play/Pause button
        let play_x = 30u32;
        let play_y = bar_y + 24;
        self.draw_rect(buf, play_x, play_y, 28, 28, 0xFF2D2D44u32);

        if is_playing {
            // Draw Pause bars ||
            self.draw_rect(buf, play_x + 8, play_y + 6, 4, 16, 0xFFFFFFFF);
            self.draw_rect(buf, play_x + 16, play_y + 6, 4, 16, 0xFFFFFFFF);
        } else {
            // Draw Play triangle >
            for y in 0..16 {
                let w = if y <= 8 { y } else { 16 - y };
                self.draw_rect(buf, play_x + 9, play_y + 6 + y, w + 1, 1, 0xFFF07000);
            }
        }

        // Volume bar
        let vol_x = self.width - 150;
        let vol_y = bar_y + 36;
        self.draw_rect(buf, vol_x, vol_y, 100, 4, 0xFF2E2E3Eu32);
        let vol_fill = ((100.0 * (volume / 2.0).clamp(0.0, 1.0))) as u32;
        self.draw_rect(buf, vol_x, vol_y, vol_fill, 4, 0xFFF07000u32);
        self.draw_text(buf, vol_x - 30, bar_y + 35, "VOL", 0xFF9999AA);

        // Big VLC Cone in center
        let center_x = (self.width / 2) as i32;
        let center_y = ((bar_y + 36) / 2) as i32;
        self.draw_cone(buf, center_x, center_y, 14);

        // Spectrum visualizer at bottom of viewport
        self.draw_spectrum(buf, pcm, bar_y - 10, 60);
    }
}
