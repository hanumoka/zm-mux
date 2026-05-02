use std::num::NonZeroU32;
use std::sync::Arc;

use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight,
};
use softbuffer::{Context, Surface};
use winit::window::Window;
use zm_core::{ZmError, ZmResult};

use crate::{PaneRenderInfo, Rect, Renderer};

const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");

const BG_OUTSIDE: u32 = 0x00_1a1a2e;
const BG_PANE: u32 = 0x00_000000;
const CURSOR_COLOR: u32 = 0x00_CCCCCC;
const BORDER_FOCUSED: u32 = 0x00_4488FF;
const BORDER_UNFOCUSED: u32 = 0x00_444444;

// Shaping + rasterization state, separable from presentation surface
// so the surface borrow in render() does not block draw access to these.
struct CellShaper {
    font_system: FontSystem,
    swash_cache: SwashCache,
    cell_buffer: Buffer,
    cell_width: usize,
    cell_height: usize,
    font_family: String,
}

impl CellShaper {
    fn new(font_size: f32, font_family: String) -> ZmResult<Self> {
        let mut font_system = FontSystem::new();
        font_system
            .db_mut()
            .load_font_data(JETBRAINS_MONO_REGULAR.to_vec());

        let line_height = (font_size * 1.4).ceil();
        let metrics = Metrics::new(font_size, line_height);
        let swash_cache = SwashCache::new();

        let probe_attrs = Attrs::new().family(Family::Name(&font_family));
        let mut probe = Buffer::new(&mut font_system, metrics);
        {
            let mut bw = probe.borrow_with(&mut font_system);
            bw.set_size(None, Some(line_height));
            bw.set_text("M", &probe_attrs, Shaping::Basic, None);
            bw.shape_until_scroll(false);
        }
        let cell_width = probe
            .layout_runs()
            .next()
            .and_then(|run| run.glyphs.first().map(|g| g.w.ceil() as usize))
            .unwrap_or((font_size * 0.6).ceil() as usize)
            .max(1);
        let cell_height = (line_height as usize).max(1);

        let cell_buffer = Buffer::new(&mut font_system, metrics);

        Ok(Self {
            font_system,
            swash_cache,
            cell_buffer,
            cell_width,
            cell_height,
            font_family,
        })
    }

    fn draw_panes(
        &mut self,
        panes: &[PaneRenderInfo],
        buf: &mut [u32],
        width: usize,
        height: usize,
    ) {
        for pixel in buf.iter_mut() {
            *pixel = BG_OUTSIDE;
        }
        for pane in panes {
            self.draw_single_pane(pane, buf, width, height);
        }
    }

    fn draw_single_pane(
        &mut self,
        pane: &PaneRenderInfo,
        buf: &mut [u32],
        buf_width: usize,
        buf_height: usize,
    ) {
        let r = &pane.rect;

        for dy in 0..r.height {
            for dx in 0..r.width {
                let px = r.x + dx;
                let py = r.y + dy;
                if px < buf_width && py < buf_height {
                    buf[py * buf_width + px] = BG_PANE;
                }
            }
        }

        let term = pane.term;

        for row in 0..term.rows() {
            for col in 0..term.cols() {
                if term.is_wide_spacer(row, col) {
                    continue;
                }
                let cell = term.render_cell(row, col);
                let is_wide = term.is_wide_char(row, col);
                let cw = if is_wide {
                    self.cell_width * 2
                } else {
                    self.cell_width
                };
                let x0 = r.x + col * self.cell_width;
                let y0 = r.y + row * self.cell_height;

                if cell.bg.r > 0 || cell.bg.g > 0 || cell.bg.b > 0 {
                    let bg = ((cell.bg.r as u32) << 16)
                        | ((cell.bg.g as u32) << 8)
                        | cell.bg.b as u32;
                    fill_rect(buf, buf_width, buf_height, x0, y0, cw, self.cell_height, bg);
                }

                if cell.c == ' ' || cell.c == '\0' {
                    continue;
                }

                let mut attrs = Attrs::new().family(Family::Name(&self.font_family));
                if cell.bold {
                    attrs = attrs.weight(Weight::BOLD);
                }
                if cell.italic {
                    attrs = attrs.style(Style::Italic);
                }
                let text = cell.c.to_string();

                {
                    let mut bw = self.cell_buffer.borrow_with(&mut self.font_system);
                    bw.set_size(Some(cw as f32), Some(self.cell_height as f32));
                    bw.set_text(&text, &attrs, Shaping::Basic, None);
                    bw.shape_until_scroll(false);
                }
                let fg_color = Color::rgb(cell.fg.r, cell.fg.g, cell.fg.b);
                let cache = &mut self.swash_cache;
                let bw = &mut self.cell_buffer.borrow_with(&mut self.font_system);
                bw.draw(cache, fg_color, |dx, dy, dw, dh, px_color| {
                    let alpha = px_color.a();
                    if alpha == 0 {
                        return;
                    }
                    blend_rect(
                        buf,
                        buf_width,
                        buf_height,
                        x0 as i32 + dx,
                        y0 as i32 + dy,
                        dw as usize,
                        dh as usize,
                        px_color.r(),
                        px_color.g(),
                        px_color.b(),
                        alpha,
                    );
                });
            }
        }

        if term.display_offset() == 0 {
            let (crow, ccol) = term.cursor_position();
            if crow < term.rows() && ccol < term.cols() {
                let cx = r.x + ccol * self.cell_width;
                let cy = r.y + crow * self.cell_height;
                draw_cursor_outline(
                    buf,
                    buf_width,
                    buf_height,
                    cx,
                    cy,
                    self.cell_width,
                    self.cell_height,
                    CURSOR_COLOR,
                );
            }
        }

        let border = if pane.focused {
            BORDER_FOCUSED
        } else {
            BORDER_UNFOCUSED
        };
        draw_pane_border(buf, buf_width, buf_height, r, border);
    }
}

pub struct CpuBackend {
    shaper: CellShaper,
    _context: Context<Arc<Window>>,
    surface: Surface<Arc<Window>, Arc<Window>>,
}

impl CpuBackend {
    pub fn new(
        window: Arc<Window>,
        font_size: f32,
        font_family: impl Into<String>,
    ) -> ZmResult<Self> {
        let shaper = CellShaper::new(font_size, font_family.into())?;

        let context = Context::new(window.clone())
            .map_err(|e| ZmError::Render(format!("softbuffer context: {e}")))?;
        let surface = Surface::new(&context, window)
            .map_err(|e| ZmError::Render(format!("softbuffer surface: {e}")))?;

        Ok(Self {
            shaper,
            _context: context,
            surface,
        })
    }
}

impl Renderer for CpuBackend {
    fn cell_size(&self) -> (usize, usize) {
        (self.shaper.cell_width, self.shaper.cell_height)
    }

    fn required_size(&self, cols: usize, rows: usize) -> (usize, usize) {
        (
            cols * self.shaper.cell_width,
            rows * self.shaper.cell_height,
        )
    }

    fn cols_rows_for_size(&self, width: usize, height: usize) -> (u16, u16) {
        let cols = (width / self.shaper.cell_width).max(1) as u16;
        let rows = (height / self.shaper.cell_height).max(1) as u16;
        (cols, rows)
    }

    fn render(&mut self, panes: &[PaneRenderInfo], width: u32, height: u32) -> ZmResult<()> {
        let (Some(w), Some(h)) = (NonZeroU32::new(width), NonZeroU32::new(height)) else {
            return Ok(());
        };

        self.surface
            .resize(w, h)
            .map_err(|e| ZmError::Render(format!("surface resize: {e}")))?;

        let mut buffer = self
            .surface
            .buffer_mut()
            .map_err(|e| ZmError::Render(format!("surface buffer_mut: {e}")))?;

        let buf_slice: &mut [u32] = &mut buffer;
        self.shaper
            .draw_panes(panes, buf_slice, width as usize, height as usize);

        buffer
            .present()
            .map_err(|e| ZmError::Render(format!("surface present: {e}")))?;
        Ok(())
    }
}

fn fill_rect(
    buf: &mut [u32],
    buf_width: usize,
    buf_height: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: u32,
) {
    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px < buf_width && py < buf_height {
                buf[py * buf_width + px] = color;
            }
        }
    }
}

fn blend_rect(
    buf: &mut [u32],
    buf_width: usize,
    buf_height: usize,
    x: i32,
    y: i32,
    w: usize,
    h: usize,
    fr: u8,
    fg: u8,
    fb: u8,
    alpha: u8,
) {
    let a = alpha as u32;
    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx as i32;
            let py = y + dy as i32;
            if px < 0 || py < 0 || px >= buf_width as i32 || py >= buf_height as i32 {
                continue;
            }
            let idx = py as usize * buf_width + px as usize;
            let existing = buf[idx];
            let er = (existing >> 16) & 0xFF;
            let eg = (existing >> 8) & 0xFF;
            let eb = existing & 0xFF;
            let nr = (fr as u32 * a + er * (255 - a)) / 255;
            let ng = (fg as u32 * a + eg * (255 - a)) / 255;
            let nb = (fb as u32 * a + eb * (255 - a)) / 255;
            buf[idx] = (nr << 16) | (ng << 8) | nb;
        }
    }
}

fn draw_cursor_outline(
    buf: &mut [u32],
    buf_width: usize,
    buf_height: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: u32,
) {
    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px < buf_width
                && py < buf_height
                && (dy == 0 || dy == h - 1 || dx == 0 || dx == w - 1)
            {
                buf[py * buf_width + px] = color;
            }
        }
    }
}

fn draw_pane_border(buf: &mut [u32], buf_width: usize, buf_height: usize, r: &Rect, color: u32) {
    for dx in 0..r.width {
        let px = r.x + dx;
        if px < buf_width {
            if r.y > 0 && r.y - 1 < buf_height {
                buf[(r.y - 1) * buf_width + px] = color;
            }
            let bot = r.y + r.height;
            if bot < buf_height {
                buf[bot * buf_width + px] = color;
            }
        }
    }
    for dy in 0..r.height {
        let py = r.y + dy;
        if py < buf_height {
            if r.x > 0 && r.x - 1 < buf_width {
                buf[py * buf_width + (r.x - 1)] = color;
            }
            let right = r.x + r.width;
            if right < buf_width {
                buf[py * buf_width + right] = color;
            }
        }
    }
}
