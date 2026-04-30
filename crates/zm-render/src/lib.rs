use fontdue::{Font, FontSettings};
use zm_core::ZmResult;
use zm_term::{CellColor, ZmTerm};

pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

pub struct PaneRenderInfo<'a> {
    pub term: &'a ZmTerm,
    pub rect: Rect,
    pub focused: bool,
}

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

    pub fn required_size(&self, cols: usize, rows: usize) -> (usize, usize) {
        (cols * self.cell_width, rows * self.cell_height)
    }

    pub fn cols_rows_for_size(&self, width: usize, height: usize) -> (u16, u16) {
        let cols = (width / self.cell_width).max(1) as u16;
        let rows = (height / self.cell_height).max(1) as u16;
        (cols, rows)
    }

    pub fn render_panes(
        &self,
        panes: &[PaneRenderInfo],
        buf: &mut [u32],
        width: usize,
        height: usize,
    ) {
        // Clear to dark background
        for pixel in buf.iter_mut() {
            *pixel = 0x00_1a1a2e;
        }

        for pane in panes {
            self.render_single_pane(pane, buf, width, height);
        }
    }

    fn render_single_pane(
        &self,
        pane: &PaneRenderInfo,
        buf: &mut [u32],
        buf_width: usize,
        buf_height: usize,
    ) {
        let font_size = self.cell_height as f32 / 1.4;
        let baseline = (self.cell_height as f32 * 0.75) as usize;
        let term = pane.term;
        let r = &pane.rect;

        // Fill pane background (black)
        for dy in 0..r.height {
            for dx in 0..r.width {
                let px = r.x + dx;
                let py = r.y + dy;
                if px < buf_width && py < buf_height {
                    buf[py * buf_width + px] = 0x00_000000;
                }
            }
        }

        // Render cells
        for row in 0..term.rows() {
            for col in 0..term.cols() {
                let cell = term.render_cell(row, col);
                let x0 = r.x + col * self.cell_width;
                let y0 = r.y + row * self.cell_height;

                // Cell background
                if cell.bg.r > 0 || cell.bg.g > 0 || cell.bg.b > 0 {
                    let bg = color_to_u32(&cell.bg);
                    for dy in 0..self.cell_height {
                        for dx in 0..self.cell_width {
                            let px = x0 + dx;
                            let py = y0 + dy;
                            if px < buf_width && py < buf_height {
                                buf[py * buf_width + px] = bg;
                            }
                        }
                    }
                }

                // Glyph
                if cell.c != ' ' && cell.c != '\0' {
                    let (metrics, bitmap) = self.font.rasterize(cell.c, font_size);
                    let gx = x0;
                    let ymin_offset = if metrics.ymin >= 0 {
                        metrics.ymin as usize
                    } else {
                        0
                    };
                    let gy = y0 + baseline.saturating_sub(metrics.height + ymin_offset);

                    for dy in 0..metrics.height {
                        for dx in 0..metrics.width {
                            let alpha = bitmap[dy * metrics.width + dx];
                            if alpha == 0 {
                                continue;
                            }
                            let px = gx + dx;
                            let py = gy + dy;
                            if px < buf_width && py < buf_height {
                                let a = alpha as u32;
                                let existing = buf[py * buf_width + px];
                                let er = (existing >> 16) & 0xFF;
                                let eg = (existing >> 8) & 0xFF;
                                let eb = existing & 0xFF;
                                let fr = cell.fg.r as u32;
                                let fg = cell.fg.g as u32;
                                let fb = cell.fg.b as u32;
                                let nr = (fr * a + er * (255 - a)) / 255;
                                let ng = (fg * a + eg * (255 - a)) / 255;
                                let nb = (fb * a + eb * (255 - a)) / 255;
                                buf[py * buf_width + px] = (nr << 16) | (ng << 8) | nb;
                            }
                        }
                    }
                }
            }
        }

        // Cursor
        let (crow, ccol) = term.cursor_position();
        let cx = r.x + ccol * self.cell_width;
        let cy = r.y + crow * self.cell_height;
        let cursor_color = 0x00CCCCCC_u32;
        for dy in 0..self.cell_height {
            for dx in 0..self.cell_width {
                let px = cx + dx;
                let py = cy + dy;
                if px < buf_width
                    && py < buf_height
                    && (dy == 0
                        || dy == self.cell_height - 1
                        || dx == 0
                        || dx == self.cell_width - 1)
                {
                    buf[py * buf_width + px] = cursor_color;
                }
            }
        }

        // Pane border
        let border_color = if pane.focused {
            0x00_4488FF // blue for focused
        } else {
            0x00_444444 // gray for unfocused
        };

        // Top/bottom border
        for dx in 0..r.width {
            let px = r.x + dx;
            if px < buf_width {
                if r.y > 0 && (r.y - 1) < buf_height {
                    buf[(r.y - 1) * buf_width + px] = border_color;
                }
                let bot = r.y + r.height;
                if bot < buf_height {
                    buf[bot * buf_width + px] = border_color;
                }
            }
        }
        // Left/right border
        for dy in 0..r.height {
            let py = r.y + dy;
            if py < buf_height {
                if r.x > 0 && (r.x - 1) < buf_width {
                    buf[py * buf_width + (r.x - 1)] = border_color;
                }
                let right = r.x + r.width;
                if right < buf_width {
                    buf[py * buf_width + right] = border_color;
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
        assert!(r.is_ok());
        let r = r.unwrap();
        let (w, h) = r.cell_size();
        assert!(w > 0 && h > 0);
    }

    #[test]
    fn render_single_pane() {
        let r = CpuRenderer::new(16.0).unwrap();
        let mut term = ZmTerm::new(10, 3).unwrap();
        term.feed_bytes(b"Hi");

        let (w, h) = r.required_size(10, 3);
        let mut buf = vec![0u32; w * h];
        let panes = vec![PaneRenderInfo {
            term: &term,
            rect: Rect {
                x: 0,
                y: 0,
                width: w,
                height: h,
            },
            focused: true,
        }];
        r.render_panes(&panes, &mut buf, w, h);

        let non_bg = buf.iter().filter(|&&p| p != 0x001a1a2e && p != 0).count();
        assert!(non_bg > 0, "Should render text pixels");
    }

    #[test]
    fn render_two_panes() {
        let r = CpuRenderer::new(16.0).unwrap();
        let mut t1 = ZmTerm::new(10, 3).unwrap();
        let mut t2 = ZmTerm::new(10, 3).unwrap();
        t1.feed_bytes(b"Left");
        t2.feed_bytes(b"Right");

        let total_w = 400;
        let total_h = 200;
        let mut buf = vec![0u32; total_w * total_h];

        let panes = vec![
            PaneRenderInfo {
                term: &t1,
                rect: Rect {
                    x: 0,
                    y: 0,
                    width: 199,
                    height: 200,
                },
                focused: true,
            },
            PaneRenderInfo {
                term: &t2,
                rect: Rect {
                    x: 201,
                    y: 0,
                    width: 199,
                    height: 200,
                },
                focused: false,
            },
        ];
        r.render_panes(&panes, &mut buf, total_w, total_h);

        // Check left half has text
        let left_pixels: u32 = buf[..total_w * total_h / 2]
            .iter()
            .filter(|&&p| p != 0x001a1a2e && p != 0)
            .count() as u32;
        // Check right half has text
        let right_pixels: u32 = buf[total_w * total_h / 2..]
            .iter()
            .filter(|&&p| p != 0x001a1a2e && p != 0)
            .count() as u32;

        assert!(left_pixels > 0, "Left pane should have content");
        assert!(right_pixels > 0, "Right pane should have content");
    }
}
