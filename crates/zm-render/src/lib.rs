use std::sync::Arc;

use winit::window::Window;
use zm_core::ZmResult;
use zm_term::ZmTerm;

mod cpu;
mod gpu;

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
/// Phase 1.3.9-B-1: GpuBackend exists but has no text draw yet, so the
/// default path stays on CpuBackend.  Set `ZM_RENDER=gpu` to opt into
/// the GPU backend for init-path testing; on init failure we still fall
/// back to CpuBackend so the user never gets a blank window.
///
/// Once 1.3.9-B-2/B-3 (text + cursor + border draw) lands, this default
/// flips to "GPU first" — the fallback chain stays the same.
pub fn create_renderer(
    window: Arc<Window>,
    font_size: f32,
    font_family: &str,
) -> ZmResult<Box<dyn Renderer>> {
    let prefer_gpu = std::env::var("ZM_RENDER")
        .map(|v| v.eq_ignore_ascii_case("gpu"))
        .unwrap_or(false);

    if prefer_gpu {
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

    let cpu = CpuBackend::new(window, font_size, font_family)?;
    Ok(Box::new(cpu))
}

#[cfg(test)]
mod tests {
    // Backend tests live in the backend module — they need a real window
    // for the surface, which a unit test cannot provide.  Smoke tests for
    // the trait shape live here once we have a non-presenting test backend.
}
