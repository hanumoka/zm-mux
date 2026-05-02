use std::sync::Arc;

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping};
use glyphon::{Cache, Resolution, SwashCache, TextAtlas, TextRenderer, Viewport};
use winit::window::Window;
use zm_core::{ZmError, ZmResult};

use crate::{PaneRenderInfo, Renderer};

const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");

const BG_OUTSIDE_R: f64 = 0x1a as f64 / 255.0;
const BG_OUTSIDE_G: f64 = 0x1a as f64 / 255.0;
const BG_OUTSIDE_B: f64 = 0x2e as f64 / 255.0;

/// GPU-accelerated renderer using wgpu + glyphon.
///
/// Phase 1.3.9-B-1 status: surface init, glyphon allocation, and a clear
/// pass are wired.  Text/cursor/border draw lands in 1.3.9-B-2.  Until
/// then this backend produces a blank pane background — opt-in only via
/// `ZM_RENDER=gpu` so the default user experience is still CpuBackend.
pub struct GpuBackend {
    cell_width: usize,
    cell_height: usize,
    #[allow(dead_code)]
    font_size: f32,
    #[allow(dead_code)]
    font_family: String,

    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: (u32, u32),

    #[allow(dead_code)]
    font_system: FontSystem,
    #[allow(dead_code)]
    swash_cache: SwashCache,
    viewport: Viewport,
    #[allow(dead_code)]
    atlas: TextAtlas,
    #[allow(dead_code)]
    text_renderer: TextRenderer,
}

impl GpuBackend {
    pub fn new(
        window: Arc<Window>,
        font_size: f32,
        font_family: impl Into<String>,
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

        let font_family = font_family.into();
        let line_height = (font_size * 1.4).ceil();
        let metrics = Metrics::new(font_size, line_height);
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
        })
    }
}

impl Renderer for GpuBackend {
    fn cell_size(&self) -> (usize, usize) {
        (self.cell_width, self.cell_height)
    }

    fn required_size(&self, cols: usize, rows: usize) -> (usize, usize) {
        (cols * self.cell_width, rows * self.cell_height)
    }

    fn cols_rows_for_size(&self, width: usize, height: usize) -> (u16, u16) {
        let cols = (width / self.cell_width).max(1) as u16;
        let rows = (height / self.cell_height).max(1) as u16;
        (cols, rows)
    }

    fn render(&mut self, _panes: &[PaneRenderInfo], width: u32, height: u32) -> ZmResult<()> {
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
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("zm-mux gpu clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: BG_OUTSIDE_R,
                            g: BG_OUTSIDE_G,
                            b: BG_OUTSIDE_B,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            // TODO 1.3.9-B-2: text via glyphon TextRenderer.prepare/render
            // TODO 1.3.9-B-3: cursor + pane border via solid-color rect shader
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }
}
