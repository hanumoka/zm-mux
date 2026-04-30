use fontdue::{Font, FontSettings};
use zm_core::ZmResult;
use zm_term::{CellColor, ZmTerm};

pub struct CpuRenderer {
    font: Font,
    cell_width: usize,
    cell_height: usize,
}

impl CpuRenderer {
    pub fn new(font_size: f32) -> ZmResult<Self> {
        let font_data = include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
        let font = Font::from_bytes(font_data.as_slice(), FontSettings::default())
            .map_err(|e| zm_core::ZmError::Render(e.to_string()))?;

        let metrics = font.metrics('M', font_size);
        let cell_width = metrics.advance_width.ceil() as usize;
        let cell_height = (font_size * 1.4).ceil() as usize;

        Ok(Self {
            font,
            cell_width,
            cell_height,
        })
    }

    pub fn cell_size(&self) -> (usize, usize) {
        (self.cell_width, self.cell_height)
    }

    pub fn required_size(&self, term: &ZmTerm) -> (usize, usize) {
        let width = term.cols() * self.cell_width;
        let height = term.rows() * self.cell_height;
        (width, height)
    }

    pub fn render_to_buffer(&self, term: &ZmTerm, buf: &mut [u32], width: usize, height: usize) {
        let font_size = self.cell_height as f32 / 1.4;
        let baseline = (self.cell_height as f32 * 0.75) as usize;

        // Clear background
        for pixel in buf.iter_mut() {
            *pixel = color_to_u32(&CellColor::BLACK);
        }

        for row in 0..term.rows() {
            for col in 0..term.cols() {
                let cell = term.render_cell(row, col);
                let x0 = col * self.cell_width;
                let y0 = row * self.cell_height;

                // Draw cell background if not black
                if cell.bg.r > 0 || cell.bg.g > 0 || cell.bg.b > 0 {
                    let bg = color_to_u32(&cell.bg);
                    for dy in 0..self.cell_height {
                        for dx in 0..self.cell_width {
                            let px = x0 + dx;
                            let py = y0 + dy;
                            if px < width && py < height {
                                buf[py * width + px] = bg;
                            }
                        }
                    }
                }

                // Draw glyph
                if cell.c != ' ' && cell.c != '\0' {
                    let (metrics, bitmap) = self.font.rasterize(cell.c, font_size);
                    let gx = x0;
                    let gy = y0 + baseline.saturating_sub(metrics.height + metrics.ymin as usize);

                    for dy in 0..metrics.height {
                        for dx in 0..metrics.width {
                            let alpha = bitmap[dy * metrics.width + dx];
                            if alpha == 0 {
                                continue;
                            }
                            let px = gx + dx;
                            let py = gy + dy;
                            if px < width && py < height {
                                let fg = &cell.fg;
                                let a = alpha as u32;
                                let existing = buf[py * width + px];
                                let er = (existing >> 16) & 0xFF;
                                let eg = (existing >> 8) & 0xFF;
                                let eb = existing & 0xFF;
                                let r = (fg.r as u32 * a + er * (255 - a)) / 255;
                                let g = (fg.g as u32 * a + eg * (255 - a)) / 255;
                                let b = (fg.b as u32 * a + eb * (255 - a)) / 255;
                                buf[py * width + px] = (r << 16) | (g << 8) | b;
                            }
                        }
                    }
                }
            }
        }

        // Draw cursor
        let (crow, ccol) = term.cursor_position();
        let cx = ccol * self.cell_width;
        let cy = crow * self.cell_height;
        let cursor_color = 0x00CCCCCC_u32;
        for dy in 0..self.cell_height {
            for dx in 0..self.cell_width {
                let px = cx + dx;
                let py = cy + dy;
                if px < width
                    && py < height
                    && (dy == 0
                        || dy == self.cell_height - 1
                        || dx == 0
                        || dx == self.cell_width - 1)
                {
                    buf[py * width + px] = cursor_color;
                }
            }
        }
    }
}

fn color_to_u32(c: &CellColor) -> u32 {
    (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_renderer() {
        let r = CpuRenderer::new(16.0);
        assert!(r.is_ok(), "Renderer should initialize");
        let r = r.unwrap();
        let (w, h) = r.cell_size();
        assert!(w > 0 && h > 0, "Cell size should be positive: {}x{}", w, h);
    }

    #[test]
    fn render_text_to_buffer() {
        let r = CpuRenderer::new(16.0).unwrap();
        let mut term = ZmTerm::new(10, 3).unwrap();
        term.feed_bytes(b"Hi");

        let (w, h) = r.required_size(&term);
        let mut buf = vec![0u32; w * h];
        r.render_to_buffer(&term, &mut buf, w, h);

        // Should have some non-black pixels (text was rendered)
        let non_black = buf.iter().filter(|&&p| p != 0).count();
        assert!(non_black > 0, "Should have rendered pixels for 'Hi'");
    }
}
