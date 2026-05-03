use std::sync::Arc;

use winit::window::Window;
use zm_core::{ColorsConfig, ZmResult};
use zm_term::ZmTerm;

mod cpu;
mod gpu;
mod gpu_rect;

pub use cpu::CpuBackend;
pub use gpu::GpuBackend;

/// Height of the tab bar in physical pixels.  The tab bar always occupies
/// the top stripe of the window; pane content sits below it.  Sized so a
/// 16pt font fits with 4px padding on each side.
pub const TAB_BAR_HEIGHT_PX: u32 = 24;

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
    /// Active IME composition string for the focused pane (None for all
    /// other panes and when no composition is in progress).  Backends
    /// draw it as an overlay at the cursor position so the cell grid
    /// stays untouched until the OS commits.
    pub ime_preedit: Option<&'a str>,
    /// Search-result cells to overlay with a translucent highlight
    /// (drawn on top of cell text, under the cursor outline).
    pub highlights: &'a [HighlightCell],
    /// Selection cells to overlay with a translucent blue highlight.
    pub selection_highlights: &'a [HighlightCell],
    /// Per-pane border color in sRGB, pre-computed by the caller from
    /// agent status and focus state.  Replaces the old hardcoded
    /// focused/unfocused constants.
    pub border_color_srgb: (u8, u8, u8),
}

/// One search-hit span in a pane's viewport, in cell units.
#[derive(Debug, Clone, Copy)]
pub struct HighlightCell {
    pub row: usize,
    pub col: usize,
    pub len: usize,
}

/// One tab's display label.  The renderer draws the title text inside the
/// tab cell and uses `TabBarInfo::active_index` for accent.
pub struct TabLabel<'a> {
    pub title: &'a str,
}

pub struct TabBarInfo<'a> {
    pub tabs: &'a [TabLabel<'a>],
    pub active_index: usize,
}

/// Backend-agnostic terminal renderer.
///
/// Implementations own their own presentation surface (softbuffer for CPU,
/// wgpu surface for GPU). zm-app calls `render(...)` once per frame; size
/// args reflect the latest window inner size in physical pixels.
///
/// `required_size` and `cols_rows_for_size` get default impls that
/// reserve `TAB_BAR_HEIGHT_PX` from the vertical budget so backends only
/// implement `cell_size` and `render`.
pub trait Renderer {
    fn cell_size(&self) -> (usize, usize);

    fn required_size(&self, cols: usize, rows: usize) -> (usize, usize) {
        let (cw, ch) = self.cell_size();
        (cols * cw, rows * ch + TAB_BAR_HEIGHT_PX as usize)
    }

    fn cols_rows_for_size(&self, width: usize, height: usize) -> (u16, u16) {
        let (cw, ch) = self.cell_size();
        let avail_h = height.saturating_sub(TAB_BAR_HEIGHT_PX as usize);
        ((width / cw).max(1) as u16, (avail_h / ch).max(1) as u16)
    }

    fn render(
        &mut self,
        tab_bar: &TabBarInfo,
        panes: &[PaneRenderInfo],
        width: u32,
        height: u32,
    ) -> ZmResult<()>;
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
/// Plus `ZM_RENDER_FORCE_INIT_FAIL=1` (test/CI only): bypasses the real
/// `GpuBackend::new` call when the GPU path is selected so the fallback
/// chain is exercised end-to-end without needing an environment where
/// wgpu actually fails to initialize.  This is the only way to verify
/// the fallback chain on a developer machine where DX12/Metal always
/// succeeds.  Has no effect when `ZM_RENDER=cpu` (the GPU path is not
/// even attempted).
pub fn create_renderer(
    window: Arc<Window>,
    font_size: f32,
    font_family: &str,
    colors: &ColorsConfig,
) -> ZmResult<Box<dyn Renderer>> {
    let mode = std::env::var("ZM_RENDER")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let force_cpu = mode == "cpu";
    let force_init_fail = std::env::var("ZM_RENDER_FORCE_INIT_FAIL").is_ok();

    if !force_cpu {
        if force_init_fail {
            eprintln!(
                "zm-render: GpuBackend init forced to fail (ZM_RENDER_FORCE_INIT_FAIL); falling back to CpuBackend"
            );
        } else {
            match GpuBackend::new(window.clone(), font_size, font_family, colors) {
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

    let cpu = CpuBackend::new(window, font_size, font_family, colors)?;
    eprintln!("zm-render: using CpuBackend (softbuffer + cosmic-text)");
    Ok(Box::new(cpu))
}

#[cfg(test)]
mod tests {
    // Backend tests live in the backend module — they need a real window
    // for the surface, which a unit test cannot provide.  Smoke tests for
    // the trait shape live here once we have a non-presenting test backend.
}
