//! zm-render — wgpu 29 + glyphon 0.11 GPU 렌더러 (멀티 pane).
//!
//! 파이프라인:
//!  1) solid-rect(인스턴스 quad): 창 배경(gap/divider) → pane별 배경 + 셀 배경 + 빔/언더라인 커서
//!     + 포커스 pane 프레임. (블록/할로 커서는 zm-term이 셀 색 반전으로 베이크)
//!  2) glyphon 텍스트(pane별 origin 오프셋, 셀별 전경색).
//!
//! 색공간: sRGB 서피스 + rect는 CPU에서 sRGB→linear 변환, glyphon은 ColorMode::Accurate.
//! 좌표는 전부 물리 픽셀. 폰트는 시스템 모노스페이스. CPU 폴백은 후속.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glyphon::{
    Attrs, Buffer, Cache, Color as GColor, ColorMode, Family, FontSystem, Metrics, Resolution,
    Shaping, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport, Wrap,
};
use wgpu::util::DeviceExt;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, BlendState, BufferBindingType, BufferUsages, ColorTargetState,
    ColorWrites, CommandEncoderDescriptor, CompositeAlphaMode, DeviceDescriptor, FragmentState,
    Instance, InstanceDescriptor, LoadOp, MultisampleState, Operations, PipelineCompilationOptions,
    PipelineLayoutDescriptor, PowerPreference, PresentMode, PrimitiveState, RenderPassColorAttachment,
    RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, RequestAdapterOptions,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp, Surface, SurfaceConfiguration,
    TextureUsages, TextureViewDescriptor, VertexState, VertexStepMode,
};
use winit::window::Window;

use zm_core::{CellSnapshot, CursorKind, GridSize, Rgba, ZmError};

const LINE_HEIGHT_SCALE: f32 = 1.3;
/// pane 내부 여백(물리 px, 논리). 포커스 프레임이 글리프를 가리지 않도록 FOCUS_THICK보다 크게.
const INNER_PAD: f32 = 5.0;
const FOCUS_THICK: f32 = 2.0;
/// pane 사이/창 배경 divider 색(sRGB).
const GAP_COLOR: Rgba = Rgba::rgb(0x30, 0x30, 0x30);
const FOCUS_COLOR: Rgba = Rgba::rgb(0x3B, 0x78, 0xFF);
const BAR_BG: Rgba = Rgba::rgb(0x20, 0x20, 0x20);
const TAB_ACTIVE_BG: Rgba = Rgba::rgb(0x0A, 0x4A, 0x6E);
const TAB_W: f32 = 46.0;

/// 렌더할 pane 한 개(전체 pane 사각형 = 물리 px).
pub struct PaneView<'a> {
    pub snapshot: &'a CellSnapshot,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub focused: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RectInstance {
    rect: [f32; 4],
    color: [f32; 4],
}

#[derive(Debug, Clone, Copy)]
pub struct CellMetrics {
    pub cell_w: f32,
    pub cell_h: f32,
    pub scale: f32,
    pub font_px: f32,
}

pub struct Renderer {
    surface: Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: SurfaceConfiguration,

    font_system: FontSystem,
    swash_cache: SwashCache,
    _cache: Cache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,

    rect_pipeline: RenderPipeline,
    rect_bind_group: BindGroup,
    screen_uniform: wgpu::Buffer,
    rect_buffer: wgpu::Buffer,
    rect_capacity: usize,
    rect_cpu: Vec<RectInstance>,

    font_family: Option<String>,
    metrics: CellMetrics,
    /// 행 버퍼 풀(모든 pane 행을 프레임마다 순차 할당).
    buffer_pool: Vec<Buffer>,
}

impl Renderer {
    pub async fn new(window: Arc<Window>, cfg: &zm_core::Config) -> Result<Self, ZmError> {
        let size = window.inner_size();
        let scale = window.scale_factor() as f32;
        let (w, h) = (size.width.max(1), size.height.max(1));

        let instance = Instance::new(InstanceDescriptor::new_without_display_handle());
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| ZmError::Render(format!("create_surface: {e}")))?;

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .map_err(|e| ZmError::Render(format!("request_adapter: {e}")))?;

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: Some("zm-render device"),
                ..Default::default()
            })
            .await
            .map_err(|e| ZmError::Render(format!("request_device: {e}")))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width: w,
            height: h,
            present_mode: PresentMode::Fifo,
            alpha_mode: CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let mut font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache = Cache::new(&device);
        let mut viewport = Viewport::new(&device, &cache);
        viewport.update(&queue, Resolution { width: w, height: h });
        let mut atlas =
            TextAtlas::with_color_mode(&device, &queue, &cache, format, ColorMode::Accurate);
        let text_renderer =
            TextRenderer::new(&mut atlas, &device, MultisampleState::default(), None);

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("zm rect shader"),
            source: ShaderSource::Wgsl(include_str!("rect.wgsl").into()),
        });
        let screen_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("zm screen uniform"),
            contents: bytemuck::cast_slice(&[w as f32, h as f32, 0.0, 0.0]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        let bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("zm rect bgl"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let rect_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("zm rect bg"),
            layout: &bgl,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: screen_uniform.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("zm rect layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let attrs = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4];
        let rect_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("zm rect pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<RectInstance>() as u64,
                    step_mode: VertexStepMode::Instance,
                    attributes: &attrs,
                }],
            },
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(ColorTargetState {
                    format,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let rect_capacity = 2048usize;
        let rect_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("zm rect instances"),
            size: (rect_capacity * std::mem::size_of::<RectInstance>()) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let font_family = cfg
            .font_family()
            .filter(|name| font_exists(&font_system, name))
            .or_else(|| pick_monospace(&font_system));
        let font_px = (cfg.font_size() * scale).max(6.0);
        let metrics = measure_metrics(&mut font_system, font_family.as_deref(), font_px, scale);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            font_system,
            swash_cache,
            _cache: cache,
            viewport,
            atlas,
            text_renderer,
            rect_pipeline,
            rect_bind_group,
            screen_uniform,
            rect_buffer,
            rect_capacity,
            rect_cpu: Vec::new(),
            font_family,
            metrics,
            buffer_pool: Vec::new(),
        })
    }

    pub fn metrics(&self) -> CellMetrics {
        self.metrics
    }

    /// 상단 탭바 높이(물리 px). zm-app은 pane 레이아웃을 이만큼 아래로 내린다.
    pub fn tab_bar_height(&self) -> f32 {
        (self.metrics.cell_h + 8.0).round()
    }

    /// pane 픽셀 크기 → grid 크기(내부 여백 차감).
    pub fn pane_grid(&self, pane_w: f32, pane_h: f32) -> GridSize {
        let usable_w = (pane_w - 2.0 * INNER_PAD).max(1.0);
        let usable_h = (pane_h - 2.0 * INNER_PAD).max(1.0);
        let cols = (usable_w / self.metrics.cell_w).floor().max(1.0) as u16;
        let rows = (usable_h / self.metrics.cell_h).floor().max(1.0) as u16;
        GridSize::new(cols, rows)
    }

    pub fn resize(&mut self, phys_w: u32, phys_h: u32) {
        let (w, h) = (phys_w.max(1), phys_h.max(1));
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        self.viewport.update(&self.queue, Resolution { width: w, height: h });
        self.queue.write_buffer(
            &self.screen_uniform,
            0,
            bytemuck::cast_slice(&[w as f32, h as f32, 0.0, 0.0]),
        );
    }

    /// 여러 pane을 한 프레임에 렌더. 상단에 탭바(active_tab/tab_count).
    pub fn render(
        &mut self,
        panes: &[PaneView],
        active_tab: usize,
        tab_count: usize,
    ) -> Result<(), ZmError> {
        let metrics = self.metrics;
        let mono = self.font_family.clone();
        let cw = metrics.cell_w;
        let ch = metrics.cell_h;
        let bar_h = self.tab_bar_height();

        // 행 버퍼 풀 확보(총 행 수 + 탭 라벨).
        let total_rows: usize = panes.iter().map(|p| p.snapshot.size.rows as usize).sum();
        let pool_need = total_rows + tab_count;
        while self.buffer_pool.len() < pool_need {
            let mut b = Buffer::new(&mut self.font_system, Metrics::new(metrics.font_px, ch));
            b.set_wrap(&mut self.font_system, Wrap::None);
            self.buffer_pool.push(b);
        }

        // 1) 텍스트 버퍼 갱신(pane → 행). buf_idx로 풀 순차 사용.
        let mut buf_idx = 0usize;
        for pane in panes {
            let snap = pane.snapshot;
            let width = snap.size.cols as f32 * cw;
            for row in 0..snap.size.rows {
                let buf = &mut self.buffer_pool[buf_idx];
                buf_idx += 1;
                buf.set_size(&mut self.font_system, Some(width), Some(ch));

                let mut line = String::with_capacity(snap.size.cols as usize);
                let mut spans: Vec<(std::ops::Range<usize>, Rgba, bool, bool)> = Vec::new();
                for col in 0..snap.size.cols {
                    let cell = snap
                        .index(col, row)
                        .map(|i| snap.cells[i])
                        .unwrap_or_default();
                    let cstr = if cell.c == '\0' || cell.c.is_control() {
                        ' '
                    } else {
                        cell.c
                    };
                    let start = line.len();
                    line.push(cstr);
                    let end = line.len();
                    match spans.last_mut() {
                        Some((range, fg, b, i))
                            if *fg == cell.fg && *b == cell.bold && *i == cell.italic =>
                        {
                            range.end = end;
                        }
                        _ => spans.push((start..end, cell.fg, cell.bold, cell.italic)),
                    }
                }
                let default_attrs = mono_attrs(mono.as_deref());
                let rich = spans.iter().map(|(range, fg, bold, italic)| {
                    let mut a = mono_attrs(mono.as_deref()).color(rgba_to_gcolor(*fg));
                    if *bold {
                        a = a.weight(glyphon::Weight::BOLD);
                    }
                    if *italic {
                        a = a.style(glyphon::Style::Italic);
                    }
                    (&line[range.clone()], a)
                });
                buf.set_rich_text(&mut self.font_system, rich, &default_attrs, Shaping::Advanced, None);
                buf.shape_until_scroll(&mut self.font_system, false);
            }
        }

        // 탭 라벨 버퍼.
        for t in 0..tab_count {
            let buf = &mut self.buffer_pool[total_rows + t];
            buf.set_size(&mut self.font_system, Some(TAB_W), Some(ch));
            let label = format!(" {}", t + 1);
            let color = if t == active_tab {
                Rgba::WHITE
            } else {
                Rgba::rgb(0xAA, 0xAA, 0xAA)
            };
            let attrs = mono_attrs(mono.as_deref()).color(rgba_to_gcolor(color));
            buf.set_text(&mut self.font_system, &label, &attrs, Shaping::Advanced, None);
            buf.shape_until_scroll(&mut self.font_system, false);
        }

        // 2) rect 인스턴스: gap → 탭바 → pane 배경/셀/커서/포커스.
        self.rect_cpu.clear();
        self.rect_cpu.push(RectInstance {
            rect: [0.0, 0.0, self.config.width as f32, self.config.height as f32],
            color: rgba_to_linear(GAP_COLOR),
        });
        // 탭바 배경 + 활성 탭 하이라이트.
        self.rect_cpu.push(RectInstance {
            rect: [0.0, 0.0, self.config.width as f32, bar_h],
            color: rgba_to_linear(BAR_BG),
        });
        for t in 0..tab_count {
            if t == active_tab {
                self.rect_cpu.push(RectInstance {
                    rect: [4.0 + t as f32 * TAB_W, 2.0, TAB_W - 2.0, bar_h - 4.0],
                    color: rgba_to_linear(TAB_ACTIVE_BG),
                });
            }
        }
        for pane in panes {
            let snap = pane.snapshot;
            // pane 전체 배경.
            self.rect_cpu.push(RectInstance {
                rect: [pane.x, pane.y, pane.w, pane.h],
                color: rgba_to_linear(snap.default_bg),
            });
            let ox = pane.x + INNER_PAD;
            let oy = pane.y + INNER_PAD;
            // 셀 배경(행 단위 런 병합).
            for row in 0..snap.size.rows {
                let mut col = 0u16;
                while col < snap.size.cols {
                    let bg = snap
                        .index(col, row)
                        .map(|i| snap.cells[i].bg)
                        .unwrap_or(snap.default_bg);
                    if bg == snap.default_bg {
                        col += 1;
                        continue;
                    }
                    let start = col;
                    while col < snap.size.cols
                        && snap.index(col, row).map(|i| snap.cells[i].bg) == Some(bg)
                    {
                        col += 1;
                    }
                    let run = (col - start) as f32;
                    self.rect_cpu.push(RectInstance {
                        rect: [ox + start as f32 * cw, oy + row as f32 * ch, run * cw, ch],
                        color: rgba_to_linear(bg),
                    });
                }
            }
            // 빔/언더라인 커서.
            if snap.cursor.visible {
                let thick = (2.0 * metrics.scale).round().max(1.0);
                let ccx = ox + snap.cursor.col as f32 * cw;
                let ccy = oy + snap.cursor.row as f32 * ch;
                match snap.cursor.kind {
                    CursorKind::Underline => self.rect_cpu.push(RectInstance {
                        rect: [ccx, ccy + ch - thick, cw, thick],
                        color: rgba_to_linear(snap.cursor.color),
                    }),
                    CursorKind::Beam => self.rect_cpu.push(RectInstance {
                        rect: [ccx, ccy, thick, ch],
                        color: rgba_to_linear(snap.cursor.color),
                    }),
                    CursorKind::Block | CursorKind::Hollow => {}
                }
            }
            // 포커스 프레임(4변).
            if pane.focused {
                let t = FOCUS_THICK;
                let fc = rgba_to_linear(FOCUS_COLOR);
                self.rect_cpu.push(RectInstance { rect: [pane.x, pane.y, pane.w, t], color: fc });
                self.rect_cpu.push(RectInstance {
                    rect: [pane.x, pane.y + pane.h - t, pane.w, t],
                    color: fc,
                });
                self.rect_cpu.push(RectInstance { rect: [pane.x, pane.y, t, pane.h], color: fc });
                self.rect_cpu.push(RectInstance {
                    rect: [pane.x + pane.w - t, pane.y, t, pane.h],
                    color: fc,
                });
            }
        }
        if self.rect_cpu.len() > self.rect_capacity {
            self.rect_capacity = self.rect_cpu.len().next_power_of_two();
            self.rect_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("zm rect instances"),
                size: (self.rect_capacity * std::mem::size_of::<RectInstance>()) as u64,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.queue
            .write_buffer(&self.rect_buffer, 0, bytemuck::cast_slice(&self.rect_cpu));
        let rect_count = self.rect_cpu.len() as u32;

        // 3) 텍스트 prepare(TextArea: pane origin 오프셋 + pane rect로 클립).
        let mut areas: Vec<TextArea> = Vec::with_capacity(total_rows);
        let mut bi = 0usize;
        for pane in panes {
            let snap = pane.snapshot;
            let ox = pane.x + INNER_PAD;
            let oy = pane.y + INNER_PAD;
            let bounds = TextBounds {
                left: pane.x as i32,
                top: pane.y as i32,
                right: (pane.x + pane.w) as i32,
                bottom: (pane.y + pane.h) as i32,
            };
            for row in 0..snap.size.rows {
                let buf = &self.buffer_pool[bi];
                bi += 1;
                areas.push(TextArea {
                    buffer: buf,
                    left: ox,
                    top: oy + row as f32 * ch,
                    scale: 1.0,
                    bounds,
                    default_color: rgba_to_gcolor(snap.default_fg),
                    custom_glyphs: &[],
                });
            }
        }
        // 탭 라벨 TextArea.
        let bar_bounds = TextBounds {
            left: 0,
            top: 0,
            right: self.config.width as i32,
            bottom: bar_h as i32,
        };
        for t in 0..tab_count {
            let buf = &self.buffer_pool[total_rows + t];
            areas.push(TextArea {
                buffer: buf,
                left: 4.0 + t as f32 * TAB_W + 4.0,
                top: 4.0,
                scale: 1.0,
                bounds: bar_bounds,
                default_color: rgba_to_gcolor(Rgba::WHITE),
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
                areas,
                &mut self.swash_cache,
            )
            .map_err(|e| ZmError::Render(format!("prepare: {e}")))?;

        // 4) 서피스 + 렌더 패스.
        use wgpu::CurrentSurfaceTexture::*;
        let frame = match self.surface.get_current_texture() {
            Success(t) | Suboptimal(t) => t,
            Outdated | Lost => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            Timeout | Occluded => return Ok(()),
            Validation => return Err(ZmError::Render("surface validation".into())),
        };
        let view = frame.texture.create_view(&TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: Some("zm enc") });
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("zm pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Clear(wgpu::Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.rect_pipeline);
            pass.set_bind_group(0, &self.rect_bind_group, &[]);
            pass.set_vertex_buffer(0, self.rect_buffer.slice(..));
            pass.draw(0..6, 0..rect_count);
            self.text_renderer
                .render(&self.atlas, &self.viewport, &mut pass)
                .map_err(|e| ZmError::Render(format!("render: {e}")))?;
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        self.atlas.trim();
        Ok(())
    }
}

fn mono_attrs(family: Option<&str>) -> Attrs<'_> {
    let a = Attrs::new();
    match family {
        Some(name) => a.family(Family::Name(name)),
        None => a.family(Family::Monospace),
    }
}

fn font_exists(fs: &FontSystem, name: &str) -> bool {
    fs.db()
        .faces()
        .any(|f| f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)))
}

fn pick_monospace(fs: &FontSystem) -> Option<String> {
    let prefs: &[&str] = if cfg!(target_os = "windows") {
        &["Cascadia Mono", "Cascadia Code", "Consolas", "Lucida Console"]
    } else if cfg!(target_os = "macos") {
        &["SF Mono", "Menlo", "Monaco"]
    } else {
        &["DejaVu Sans Mono", "Liberation Mono", "Noto Sans Mono"]
    };
    let db = fs.db();
    for pref in prefs {
        for face in db.faces() {
            if face
                .families
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(pref))
            {
                return Some((*pref).to_string());
            }
        }
    }
    None
}

fn measure_metrics(
    fs: &mut FontSystem,
    family: Option<&str>,
    font_px: f32,
    scale: f32,
) -> CellMetrics {
    let line_height = (font_px * LINE_HEIGHT_SCALE).round().max(1.0);
    let mut buf = Buffer::new(fs, Metrics::new(font_px, line_height));
    buf.set_wrap(fs, Wrap::None);
    buf.set_size(fs, None, None);
    let attrs = mono_attrs(family);
    buf.set_text(fs, "MMMMMMMMMM", &attrs, Shaping::Advanced, None);
    buf.shape_until_scroll(fs, false);
    let measured = buf
        .layout_runs()
        .next()
        .map(|run| {
            let w: f32 = run.glyphs.iter().map(|g| g.w).sum();
            w / 10.0
        })
        .filter(|w| *w > 0.0)
        .unwrap_or(font_px * 0.6);
    CellMetrics {
        cell_w: measured.ceil().max(1.0),
        cell_h: line_height,
        scale,
        font_px,
    }
}

fn rgba_to_gcolor(c: Rgba) -> GColor {
    GColor::rgba(c.r, c.g, c.b, c.a)
}

fn srgb_to_linear(c: u8) -> f32 {
    let s = c as f32 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

fn rgba_to_linear(c: Rgba) -> [f32; 4] {
    [
        srgb_to_linear(c.r),
        srgb_to_linear(c.g),
        srgb_to_linear(c.b),
        c.a as f32 / 255.0,
    ]
}
