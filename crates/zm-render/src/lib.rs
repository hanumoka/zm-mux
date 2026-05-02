use std::sync::Arc;

use winit::window::Window;
use zm_core::ZmResult;
use zm_term::ZmTerm;

mod cpu;

pub use cpu::CpuBackend;

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
/// For now only the CPU backend is wired; the GPU backend (glyphon + wgpu)
/// will be added behind the same trait so this factory becomes a real
/// preference + fallback chain. See docs/11 Phase 1.3.9 / 1.3.10.
pub fn create_renderer(
    window: Arc<Window>,
    font_size: f32,
    font_family: &str,
) -> ZmResult<Box<dyn Renderer>> {
    let cpu = CpuBackend::new(window, font_size, font_family)?;
    Ok(Box::new(cpu))
}

#[cfg(test)]
mod tests {
    // Backend tests live in the backend module — they need a real window
    // for the surface, which a unit test cannot provide.  Smoke tests for
    // the trait shape live here once we have a non-presenting test backend.
}
