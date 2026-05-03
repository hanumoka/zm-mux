use std::sync::Arc;

use cosmic_text::{
    Attrs, Buffer, Color as CTColor, Family, FontSystem, Metrics, Shaping, Style,
    Weight, Wrap,
};
use glyphon::{Cache, Color, Resolution, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport};
use winit::window::Window;
use zm_core::{ColorsConfig, ZmError, ZmResult};

use crate::gpu_rect::{push_rect, RectPipeline, RectVertex};
use crate::{PaneRenderInfo, Renderer, TAB_BAR_HEIGHT_PX, TabBarInfo};

const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");

// Default fg used when a glyph has no explicit color (cosmic-text fallback).
const DEFAULT_FG_R: u8 = 0xCC;
const DEFAULT_FG_G: u8 = 0xCC;
const DEFAULT_FG_B: u8 = 0xCC;

// Cursor outline + pane border colors (sRGB; converted to linear at draw time).
const CURSOR_SRGB: (u8, u8, u8) = (0xCC, 0xCC, 0xCC);

// Tab bar palette (sRGB).
const TAB_BAR_BG_SRGB: (u8, u8, u8) = (0x0F, 0x0F, 0x1A);
const TAB_ACTIVE_SRGB: (u8, u8, u8) = (0x2A, 0x48, 0x80);
const TAB_INACTIVE_SRGB: (u8, u8, u8) = (0x1A, 0x1A, 0x2E);
const TAB_TEXT_FG: (u8, u8, u8) = (0xE0, 0xE0, 0xE0);

// IME composing overlay palette (sRGB).
const IME_BG_SRGB: (u8, u8, u8) = (0x40, 0x40, 0x60);
const IME_UNDERLINE_SRGB: (u8, u8, u8) = (0xFF, 0xFF, 0xFF);
const IME_FG: (u8, u8, u8) = (0xFF, 0xFF, 0xFF);

// Search highlight — yellow translucent (linear, ~0.39 alpha).  Drawn
// after cell text and before the cursor outline rect so glyphs remain
// readable through the highlight.
const HL_LINEAR: [f32; 4] = [1.0, 1.0, 0.0, 0.39];
const SEL_LINEAR: [f32; 4] = [0.0082, 0.0452, 1.0, 0.39];

/// GPU-accelerated renderer using wgpu + glyphon.
pub struct GpuBackend {
    cell_width: usize,
    cell_height: usize,
    font_size: f32,
    font_family: String,

    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: (u32, u32),

    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    rect_pipeline: RectPipeline,

    /// Window outside-pane background, pre-converted to linear space (the
    /// surface format is sRGB, so wgpu's LoadOp::Clear takes linear values).
    bg_outside_linear: wgpu::Color,
}

impl GpuBackend {
    pub fn new(
        window: Arc<Window>,
        font_size: f32,
        font_family: impl Into<String>,
        colors: &ColorsConfig,
    ) -> ZmResult<Self> {
        let win_size = window.inner_size();
        let size = (win_size.width.max(1), win_size.height.max(1));

        let instance = wgpu::Instance::new(
            wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
        );

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| ZmError::Render(format!("wgpu surface: {e}")))?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .map_err(|e| ZmError::Render(format!("wgpu request_adapter: {e:?}")))?;

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .map_err(|e| ZmError::Render(format!("wgpu request_device: {e}")))?;

        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.0,
            height: size.1,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let mut font_system = FontSystem::new();
        font_system
            .db_mut()
            .load_font_data(JETBRAINS_MONO_REGULAR.to_vec());
        let swash_cache = SwashCache::new();
        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let mut atlas = TextAtlas::new(&device, &queue, &cache, format);
        let text_renderer =
            TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);
        let rect_pipeline = RectPipeline::new(&device, format);

        let font_family = font_family.into();
        let line_height = (font_size * 1.4).ceil();
        let metrics = Metrics::new(font_size, line_height);
        let probe_attrs = Attrs::new().family(Family::Name(&font_family));
        let mut probe = Buffer::new(&mut font_system, metrics);
        {
            let mut bw = probe.borrow_with(&mut font_system);
            bw.set_size(None, Some(line_height));
            bw.set_text("M", &probe_attrs, Shaping::Advanced, None);
            bw.shape_until_scroll(false);
        }
        let cell_width = probe
            .layout_runs()
            .next()
            .and_then(|run| run.glyphs.first().map(|g| g.w.ceil() as usize))
            .unwrap_or((font_size * 0.6).ceil() as usize)
            .max(1);
        let cell_height = (line_height as usize).max(1);

        let bg_rgb = colors.background_rgb();
        let bg_outside_linear = wgpu::Color {
            r: srgb_byte_to_linear(bg_rgb.0),
            g: srgb_byte_to_linear(bg_rgb.1),
            b: srgb_byte_to_linear(bg_rgb.2),
            a: 1.0,
        };

        Ok(Self {
            cell_width,
            cell_height,
            font_size,
            font_family,
            surface,
            device,
            queue,
            config,
            size,
            font_system,
            swash_cache,
            viewport,
            atlas,
            text_renderer,
            rect_pipeline,
            bg_outside_linear,
        })
    }
}

/// sRGB-encoded value (0..=1) → linear.  IEC 61966-2-1.
fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb_byte_to_linear(b: u8) -> f64 {
    srgb_to_linear(b as f64 / 255.0)
}

fn srgb_triplet_to_linear(rgb: (u8, u8, u8)) -> [f32; 4] {
    [
        srgb_byte_to_linear(rgb.0) as f32,
        srgb_byte_to_linear(rgb.1) as f32,
        srgb_byte_to_linear(rgb.2) as f32,
        1.0,
    ]
}

impl Renderer for GpuBackend {
    fn cell_size(&self) -> (usize, usize) {
        (self.cell_width, self.cell_height)
    }

    fn render(
        &mut self,
        tab_bar: &TabBarInfo,
        panes: &[PaneRenderInfo],
        width: u32,
        height: u32,
    ) -> ZmResult<()> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        if (width, height) != self.size {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.size = (width, height);
        }
        self.viewport
            .update(&self.queue, Resolution { width, height });

        // Build per-cell Buffers for each non-empty cell.  Each cell is
        // positioned at its exact grid coordinate so CJK wide characters
        // don't shift subsequent glyphs.
        let line_height = (self.font_size * 1.4).ceil();
        let metrics = Metrics::new(self.font_size, line_height);
        let font_family = self.font_family.clone();

        struct CellBuf {
            buffer: Buffer,
            x: f32,
            y: f32,
            pane_idx: usize,
        }
        struct CellBg {
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            color: [f32; 4],
        }
        let mut cell_buffers: Vec<CellBuf> = Vec::new();
        let mut cell_bgs: Vec<CellBg> = Vec::new();

        for (pi, pane) in panes.iter().enumerate() {
            let term = pane.term;
            let rx = pane.rect.x as f32;
            let ry = pane.rect.y as f32;
            let cw = self.cell_width as f32;
            let ch = self.cell_height as f32;

            for row in 0..term.rows() {
                for col in 0..term.cols() {
                    if term.is_wide_spacer(row, col) {
                        continue;
                    }
                    let cell = term.render_cell(row, col);
                    let is_wide = term.is_wide_char(row, col);
                    let buf_w = if is_wide { cw * 2.0 } else { cw };
                    let x = rx + col as f32 * cw;
                    let y = ry + row as f32 * ch;

                    if cell.bg.r > 0 || cell.bg.g > 0 || cell.bg.b > 0 {
                        cell_bgs.push(CellBg {
                            x: x as i32,
                            y: y as i32,
                            w: buf_w as i32,
                            h: ch as i32,
                            color: [
                                srgb_byte_to_linear(cell.bg.r) as f32,
                                srgb_byte_to_linear(cell.bg.g) as f32,
                                srgb_byte_to_linear(cell.bg.b) as f32,
                                1.0,
                            ],
                        });
                    }

                    if cell.c == ' ' || cell.c == '\0' {
                        continue;
                    }

                    let mut attrs = Attrs::new()
                        .family(Family::Name(&font_family))
                        .color(CTColor::rgb(cell.fg.r, cell.fg.g, cell.fg.b));
                    if cell.bold {
                        attrs = attrs.weight(Weight::BOLD);
                    }
                    if cell.italic {
                        attrs = attrs.style(Style::Italic);
                    }
                    let text = cell.c.to_string();
                    let mut buffer = Buffer::new(&mut self.font_system, metrics);
                    {
                        let mut bw = buffer.borrow_with(&mut self.font_system);
                        bw.set_size(Some(buf_w), Some(ch));
                        bw.set_wrap(Wrap::None);
                        bw.set_text(&text, &attrs, Shaping::Advanced, None);
                        bw.shape_until_scroll(false);
                    }
                    cell_buffers.push(CellBuf {
                        buffer,
                        x,
                        y,
                        pane_idx: pi,
                    });
                }
            }
        }

        // Build per-pane IME preedit Buffer for the focused pane (at most
        // one).  Lives alongside pane_buffers so its lifetime extends
        // across prepare() and render().
        let mut ime_overlay: Option<(Buffer, f32, f32, f32, f32)> = None; // (buf, cx, cy, w, h)
        for pane in panes {
            if !pane.focused {
                continue;
            }
            let Some(preedit) = pane.ime_preedit else { continue };
            if preedit.is_empty() {
                continue;
            }
            let (crow, ccol) = pane.term.cursor_position();
            if crow >= pane.term.rows() || ccol >= pane.term.cols() {
                continue;
            }
            let cx = pane.rect.x as f32 + (ccol * self.cell_width) as f32;
            let cy = pane.rect.y as f32 + (crow * self.cell_height) as f32;
            // Char-count estimate; cosmic-text shapes the real glyph bounds
            // but we need a background-rect width.
            let est_w = (preedit.chars().count().max(1) * self.cell_width) as f32;
            let cell_h = self.cell_height as f32;

            let attrs = Attrs::new().family(Family::Name(&font_family));
            let mut buffer = Buffer::new(&mut self.font_system, metrics);
            {
                let mut bw = buffer.borrow_with(&mut self.font_system);
                bw.set_size(Some(est_w), Some(cell_h));
                bw.set_wrap(Wrap::None);
                bw.set_text(preedit, &attrs, Shaping::Advanced, None);
                bw.shape_until_scroll(false);
            }
            ime_overlay = Some((buffer, cx, cy, est_w, cell_h));
            break; // At most one focused pane has a preedit.
        }

        // Build per-tab title Buffers.  Layout = even split across the bar
        // width, one Buffer per tab cell (text-only; backgrounds are drawn
        // by the rect pipeline below).
        let bar_h = TAB_BAR_HEIGHT_PX as usize;
        let tab_count = tab_bar.tabs.len();
        // Safe even when tab_count == 0 — the tab_buffers loop below skips
        // and the rect-pipeline tab_count check guards usage.
        let tab_w_default = (width as usize) / tab_count.max(1);
        let tab_text_y = bar_h.saturating_sub(self.cell_height) / 2;

        // Each tab_buffer entry carries (Buffer, text_left_x, cell_right_x).
        let mut tab_buffers: Vec<(Buffer, usize, usize)> = Vec::with_capacity(tab_count);
        for (i, label) in tab_bar.tabs.iter().enumerate() {
            let x0 = i * tab_w_default;
            let cell_w = if i == tab_count - 1 {
                (width as usize).saturating_sub(x0).max(1)
            } else {
                tab_w_default
            };
            let text_left = x0 + 6;
            let text_right = x0 + cell_w;
            let text_w = text_right.saturating_sub(text_left).max(1) as f32;

            let attrs = Attrs::new().family(Family::Name(&font_family));
            let mut buffer = Buffer::new(&mut self.font_system, metrics);
            {
                let mut bw = buffer.borrow_with(&mut self.font_system);
                bw.set_size(Some(text_w), Some(bar_h as f32));
                bw.set_wrap(Wrap::None);
                bw.set_text(label.title, &attrs, Shaping::Advanced, None);
                bw.shape_until_scroll(false);
            }
            tab_buffers.push((buffer, text_left, text_right));
        }

        // Build TextAreas: per-cell buffers first, then tab titles.  All
        // share one prepare/render call.
        let mut text_areas: Vec<TextArea> = cell_buffers
            .iter()
            .map(|cb| {
                let pane = &panes[cb.pane_idx];
                TextArea {
                    buffer: &cb.buffer,
                    left: cb.x,
                    top: cb.y,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: pane.rect.x as i32,
                        top: pane.rect.y as i32,
                        right: (pane.rect.x + pane.rect.width) as i32,
                        bottom: (pane.rect.y + pane.rect.height) as i32,
                    },
                    default_color: Color::rgb(DEFAULT_FG_R, DEFAULT_FG_G, DEFAULT_FG_B),
                    custom_glyphs: &[],
                }
            })
            .collect();
        for (buffer, text_left, text_right) in &tab_buffers {
            text_areas.push(TextArea {
                buffer,
                left: *text_left as f32,
                top: tab_text_y as f32,
                scale: 1.0,
                bounds: TextBounds {
                    left: *text_left as i32,
                    top: 0,
                    right: *text_right as i32,
                    bottom: bar_h as i32,
                },
                default_color: Color::rgb(TAB_TEXT_FG.0, TAB_TEXT_FG.1, TAB_TEXT_FG.2),
                custom_glyphs: &[],
            });
        }
        if let Some((buffer, cx, cy, w, h)) = &ime_overlay {
            text_areas.push(TextArea {
                buffer,
                left: *cx,
                top: *cy,
                scale: 1.0,
                bounds: TextBounds {
                    left: *cx as i32,
                    top: *cy as i32,
                    right: (*cx + *w) as i32,
                    bottom: (*cy + *h) as i32,
                },
                default_color: Color::rgb(IME_FG.0, IME_FG.1, IME_FG.2),
                custom_glyphs: &[],
            });
        }

        self.text_renderer
            .prepare(
                &self.device,
                &self.queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                text_areas,
                &mut self.swash_cache,
            )
            .map_err(|e| ZmError::Render(format!("glyphon prepare: {e}")))?;

        // Linear color cache (sRGB → linear once).
        let cursor_color = srgb_triplet_to_linear(CURSOR_SRGB);
        let tab_bar_bg = srgb_triplet_to_linear(TAB_BAR_BG_SRGB);
        let tab_active_bg = srgb_triplet_to_linear(TAB_ACTIVE_SRGB);
        let tab_inactive_bg = srgb_triplet_to_linear(TAB_INACTIVE_SRGB);

        let mut rect_verts: Vec<RectVertex> = Vec::new();

        // Tab bar — full strip background, then per-tab cell, then 1px
        // separator on every cell except the last.
        if tab_count > 0 {
            push_rect(
                &mut rect_verts,
                0,
                0,
                width as i32,
                bar_h as i32,
                tab_bar_bg,
                width,
                height,
            );
            for (i, _label) in tab_bar.tabs.iter().enumerate() {
                let x0 = i * tab_w_default;
                let cell_w = if i == tab_count - 1 {
                    (width as usize).saturating_sub(x0).max(1)
                } else {
                    tab_w_default
                };
                let bg = if i == tab_bar.active_index {
                    tab_active_bg
                } else {
                    tab_inactive_bg
                };
                push_rect(
                    &mut rect_verts,
                    x0 as i32,
                    0,
                    cell_w as i32,
                    bar_h as i32,
                    bg,
                    width,
                    height,
                );
                if i != tab_count - 1 {
                    push_rect(
                        &mut rect_verts,
                        (x0 + cell_w - 1) as i32,
                        0,
                        1,
                        bar_h as i32,
                        tab_bar_bg,
                        width,
                        height,
                    );
                }
            }
        }

        // Cell backgrounds (drawn before text, borders, highlights).
        for bg in &cell_bgs {
            push_rect(&mut rect_verts, bg.x, bg.y, bg.w, bg.h, bg.color, width, height);
        }

        // Pane borders + search highlights + cursor outlines.
        for pane in panes {
            let r = &pane.rect;
            let term = pane.term;

            // Search highlights for this pane (translucent yellow).  Push
            // first so cursor outline + border draw on top.
            for h in pane.highlights {
                let hx = r.x as i32 + h.col as i32 * self.cell_width as i32;
                let hy = r.y as i32 + h.row as i32 * self.cell_height as i32;
                let hw = h.len as i32 * self.cell_width as i32;
                push_rect(
                    &mut rect_verts,
                    hx,
                    hy,
                    hw,
                    self.cell_height as i32,
                    HL_LINEAR,
                    width,
                    height,
                );
            }

            for h in pane.selection_highlights {
                let hx = r.x as i32 + h.col as i32 * self.cell_width as i32;
                let hy = r.y as i32 + h.row as i32 * self.cell_height as i32;
                let hw = h.len as i32 * self.cell_width as i32;
                push_rect(
                    &mut rect_verts,
                    hx,
                    hy,
                    hw,
                    self.cell_height as i32,
                    SEL_LINEAR,
                    width,
                    height,
                );
            }

            let border = srgb_triplet_to_linear(pane.border_color_srgb);
            push_rect(
                &mut rect_verts,
                r.x as i32,
                r.y as i32 - 1,
                r.width as i32,
                1,
                border,
                width,
                height,
            );
            push_rect(
                &mut rect_verts,
                r.x as i32,
                (r.y + r.height) as i32,
                r.width as i32,
                1,
                border,
                width,
                height,
            );
            push_rect(
                &mut rect_verts,
                r.x as i32 - 1,
                r.y as i32,
                1,
                r.height as i32,
                border,
                width,
                height,
            );
            push_rect(
                &mut rect_verts,
                (r.x + r.width) as i32,
                r.y as i32,
                1,
                r.height as i32,
                border,
                width,
                height,
            );

            if term.display_offset() == 0 {
                let (crow, ccol) = term.cursor_position();
                if crow < term.rows() && ccol < term.cols() {
                    let cx = r.x as i32 + ccol as i32 * self.cell_width as i32;
                    let cy = r.y as i32 + crow as i32 * self.cell_height as i32;
                    let cw = self.cell_width as i32;
                    let ch = self.cell_height as i32;
                    push_rect(&mut rect_verts, cx, cy, cw, 1, cursor_color, width, height);
                    push_rect(
                        &mut rect_verts,
                        cx,
                        cy + ch - 1,
                        cw,
                        1,
                        cursor_color,
                        width,
                        height,
                    );
                    push_rect(&mut rect_verts, cx, cy, 1, ch, cursor_color, width, height);
                    push_rect(
                        &mut rect_verts,
                        cx + cw - 1,
                        cy,
                        1,
                        ch,
                        cursor_color,
                        width,
                        height,
                    );
                }
            }
        }

        // IME composing overlay rect — background + bottom underline.
        // Drawn after pane borders so it sits on top.  TextArea above
        // paints the glyphs.
        if let Some((_buf, cx, cy, w, h)) = &ime_overlay {
            let bg = srgb_triplet_to_linear(IME_BG_SRGB);
            let underline = srgb_triplet_to_linear(IME_UNDERLINE_SRGB);
            push_rect(
                &mut rect_verts,
                *cx as i32,
                *cy as i32,
                *w as i32,
                *h as i32,
                bg,
                width,
                height,
            );
            push_rect(
                &mut rect_verts,
                *cx as i32,
                (*cy + *h - 1.0) as i32,
                *w as i32,
                1,
                underline,
                width,
                height,
            );
        }

        let rect_buffer = if rect_verts.is_empty() {
            None
        } else {
            Some(self.rect_pipeline.build_buffer(&self.device, &rect_verts))
        };

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            other => {
                return Err(ZmError::Render(format!(
                    "get_current_texture non-success: {other:?}"
                )));
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("zm-mux encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("zm-mux gpu clear+text+rects"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.bg_outside_linear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            // Order: rects (tab bar bg, pane borders, cursor) BEFORE text so
            // tab titles + cell glyphs sit on top of their backgrounds.
            if let Some(ref buf) = rect_buffer {
                self.rect_pipeline.draw(&mut pass, buf, rect_verts.len() as u32);
            }
            self.text_renderer
                .render(&self.atlas, &self.viewport, &mut pass)
                .map_err(|e| ZmError::Render(format!("glyphon render: {e}")))?;
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }
}

