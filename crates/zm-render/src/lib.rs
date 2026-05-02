use std::sync::Arc;

use winit::window::Window;
use zm_core::ZmResult;
use zm_term::ZmTerm;

mod cpu;
mod gpu;
mod gpu_rect;

pub use cpu::CpuBackend;
pub use gpu::GpuBackend;

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

/// Backend-agnostic terminal renderer.
///
/// Implementations own their own presentation surface (softbuffer for CPU,
/// wgpu surface for GPU). zm-app calls `render(...)` once per frame; size
/// args reflect the latest window inner size in physical pixels.
pub trait Renderer {
    fn cell_size(&self) -> (usize, usize);
    fn required_size(&self, cols: usize, rows: usize) -> (usize, usize);
    fn cols_rows_for_size(&self, width: usize, height: usize) -> (u16, u16);
    fn render(&mut self, panes: &[PaneRenderInfo], width: u32, height: u32) -> ZmResult<()>;
}

/// Try to construct the most capable renderer available, falling back to CPU.
///
/// Env-var semantics (Phase 1.3.9-B-4):
///
/// | `ZM_RENDER` | Behavior |
/// |---|---|
/// | unset / anything other than `cpu` | Default = try GpuBackend, fall back to CpuBackend on init failure |
/// | `cpu` | Force CpuBackend even if GPU is available |
///
/// Plus `ZM_RENDER_FORCE_INIT_FAIL=1` (test/CI only): when combined with
/// `ZM_RENDER=gpu`, bypasses the real `GpuBackend::new` call so the
/// fallback path is exercised end-to-end without needing an environment
/// where wgpu actually fails to initialize.  This is the only way to
/// verify the fallback chain on a developer machine where DX12/Metal
/// always succeeds.
pub fn create_renderer(
    window: Arc<Window>,
    font_size: f32,
    font_family: &str,
) -> ZmResult<Box<dyn Renderer>> {
    let mode = std::env::var("ZM_RENDER")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let force_cpu = mode == "cpu";
    let prefer_gpu = !force_cpu;
    let force_init_fail = std::env::var("ZM_RENDER_FORCE_INIT_FAIL").is_ok();

    if !force_cpu && prefer_gpu {
        if force_init_fail {
            eprintln!(
                "zm-render: GpuBackend init forced to fail (ZM_RENDER_FORCE_INIT_FAIL); falling back to CpuBackend"
            );
        } else {
            match GpuBackend::new(window.clone(), font_size, font_family) {
                Ok(gpu) => {
                    eprintln!("zm-render: using GpuBackend (wgpu + glyphon)");
                    return Ok(Box::new(gpu));
                }
                Err(e) => {
                    eprintln!(
                        "zm-render: GpuBackend init failed ({e}); falling back to CpuBackend"
                    );
                }
            }
        }
    }

    let cpu = CpuBackend::new(window, font_size, font_family)?;
    eprintln!("zm-render: using CpuBackend (softbuffer + cosmic-text)");
    Ok(Box::new(cpu))
}

#[cfg(test)]
mod tests {
    // Backend tests live in the backend module — they need a real window
    // for the surface, which a unit test cannot provide.  Smoke tests for
    // the trait shape live here once we have a non-presenting test backend.
}
