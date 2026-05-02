use portable_pty::CommandBuilder;
use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Mutex};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use zm_core::Config;
use zm_mux::{PaneId, SplitDirection, TabSet};
use zm_pty::ZmPtyProcess;
use zm_render::{
    PaneRenderInfo, Rect, Renderer, TAB_BAR_HEIGHT_PX, TabBarInfo, TabLabel, create_renderer,
};
use zm_term::ZmTerm;

const INITIAL_COLS: u16 = 80;
const INITIAL_ROWS: u16 = 24;

struct PaneState {
    term: ZmTerm,
    pty: ZmPtyProcess,
}

struct MuxState {
    tabs: TabSet,
    panes: HashMap<PaneId, PaneState>,
    dirty: bool,
}

impl MuxState {
    fn new() -> Self {
        let (tabs, initial_pane) = TabSet::new();
        let pane = create_pane(INITIAL_COLS, INITIAL_ROWS);
        let mut panes = HashMap::new();
        panes.insert(initial_pane, pane);

        Self {
            tabs,
            panes,
            dirty: true,
        }
    }

    fn focused_pane(&self) -> PaneId {
        self.tabs.active().focused_pane
    }

    fn split(
        &mut self,
        direction: SplitDirection,
        renderer: &dyn Renderer,
        win_width: usize,
        win_height: usize,
    ) {
        let new_id = self.tabs.alloc_pane_id();
        let target = self.tabs.active().focused_pane;
        let did_split;
        let layout;
        {
            let tab = self.tabs.active_mut();
            did_split = tab.tree.split(target, direction, new_id);
            layout = if did_split {
                Some(tab.tree.layout(win_width, win_height))
            } else {
                None
            };
        }
        if !did_split {
            return;
        }
        let layout = layout.unwrap();
        if let Some((_, rect)) = layout.iter().find(|(id, _)| *id == new_id) {
            let (cols, rows) = renderer.cols_rows_for_size(rect.width, rect.height);
            let pane = create_pane(cols, rows);
            self.panes.insert(new_id, pane);
            self.resize_all_panes(renderer, win_width, win_height);
            self.dirty = true;
        }
    }

    fn close_focused_pane(&mut self) {
        let to_remove;
        {
            let tab = self.tabs.active_mut();
            if tab.tree.pane_count() <= 1 {
                return;
            }
            let focused = tab.focused_pane;
            let ids = tab.tree.pane_ids();
            let next_focus = ids
                .iter()
                .find(|id| **id != focused)
                .copied()
                .unwrap_or(focused);
            if !tab.tree.remove(focused) {
                return;
            }
            tab.focused_pane = next_focus;
            to_remove = focused;
        }
        if let Some(mut ps) = self.panes.remove(&to_remove) {
            ps.pty.kill().ok();
        }
        self.dirty = true;
    }

    fn close_active_tab(&mut self) {
        let removed = self.tabs.close_active();
        if removed.is_empty() {
            return;
        }
        for id in removed {
            if let Some(mut ps) = self.panes.remove(&id) {
                ps.pty.kill().ok();
            }
        }
        self.dirty = true;
    }

    /// Create a new tab whose initial pane occupies the full available area
    /// (caller passes the viewport size used for sizing the PTY).  Returns
    /// the PaneId of the new tab's first pane so the caller can wire a reader.
    fn create_new_tab(
        &mut self,
        renderer: &dyn Renderer,
        win_width: usize,
        win_height: usize,
    ) -> PaneId {
        let (_tab_id, initial_pane) = self.tabs.create_tab();
        let (cols, rows) = renderer.cols_rows_for_size(win_width, win_height);
        let pane = create_pane(cols, rows);
        self.panes.insert(initial_pane, pane);
        self.dirty = true;
        initial_pane
    }

    fn switch_tab_next(&mut self) {
        if self.tabs.switch_next() {
            self.dirty = true;
        }
    }

    fn switch_tab_prev(&mut self) {
        if self.tabs.switch_prev() {
            self.dirty = true;
        }
    }

    fn switch_tab_to_index(&mut self, idx: usize) {
        if self.tabs.switch_to_index(idx) {
            self.dirty = true;
        }
    }

    fn resize_all_panes(&mut self, renderer: &dyn Renderer, win_width: usize, win_height: usize) {
        // Pre-collect every pane's layout across all tabs so the borrow on
        // self.tabs ends before we touch self.panes.  All tabs share the
        // same window viewport.
        let all_layouts: Vec<(PaneId, zm_mux::Rect)> = self
            .tabs
            .tabs()
            .iter()
            .flat_map(|tab| tab.tree.layout(win_width, win_height))
            .collect();
        for (id, rect) in all_layouts {
            if let Some(ps) = self.panes.get_mut(&id) {
                let (cols, rows) = renderer.cols_rows_for_size(rect.width, rect.height);
                if cols > 0 && rows > 0 {
                    ps.term.resize(cols, rows);
                    ps.pty.resize(rows, cols).ok();
                }
            }
        }
    }
}

fn create_pane(cols: u16, rows: u16) -> PaneState {
    let cmd = CommandBuilder::new_default_prog();
    let pty = zm_pty::spawn_pty(rows, cols, cmd).expect("PTY spawn");
    let term = ZmTerm::new(cols, rows).expect("term init");
    PaneState { term, pty }
}

fn start_reader(pane_id: PaneId, state: &Arc<Mutex<MuxState>>) {
    let state_clone = state.clone();
    let reader = {
        let mut s = state.lock().unwrap();
        s.panes
            .get_mut(&pane_id)
            .and_then(|ps| ps.pty.take_reader())
    };

    if let Some(reader) = reader {
        std::thread::spawn(move || {
            let mut reader = reader;
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let mut s = state_clone.lock().unwrap();
                        if let Some(ps) = s.panes.get_mut(&pane_id) {
                            ps.term.feed_bytes(&buf[..n]);
                            // Reply to terminal queries (DSR cursor position, etc.)
                            // alacritty_terminal emits these as PtyWrite events;
                            // without forwarding them the shell stalls waiting.
                            for w in ps.term.drain_pty_writes() {
                                let _ = ps.pty.write_input(&w);
                            }
                            s.dirty = true;
                        } else {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }
}

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Box<dyn Renderer>>,
    state: Arc<Mutex<MuxState>>,
    modifiers: ModifiersState,
    config: Config,
}

impl App {
    fn new(state: Arc<Mutex<MuxState>>, config: Config) -> Self {
        Self {
            window: None,
            renderer: None,
            state,
            modifiers: ModifiersState::empty(),
            config,
        }
    }

    fn renderer_ref(&self) -> Option<&(dyn Renderer + '_)> {
        self.renderer.as_deref()
    }

    fn redraw(&mut self) {
        let Some(window) = &self.window else { return };
        let size = window.inner_size();
        let width = size.width;
        let height = size.height;

        if width == 0 || height == 0 {
            return;
        }

        let mut state = self.state.lock().unwrap();
        if !state.dirty {
            return;
        }

        let pane_area_h = (height as usize).saturating_sub(TAB_BAR_HEIGHT_PX as usize);
        let active = state.tabs.active();
        let layouts = active.tree.layout(width as usize, pane_area_h);
        let focused = active.focused_pane;
        let pane_infos: Vec<PaneRenderInfo> = layouts
            .iter()
            .filter_map(|(id, mux_rect)| {
                state.panes.get(id).map(|ps| PaneRenderInfo {
                    term: &ps.term,
                    rect: Rect {
                        x: mux_rect.x,
                        y: mux_rect.y + TAB_BAR_HEIGHT_PX as usize,
                        width: mux_rect.width,
                        height: mux_rect.height,
                    },
                    focused: *id == focused,
                })
            })
            .collect();

        // Tab bar payload — owned title strings so the active-tab borrow
        // ends before render(), then borrowed labels for the slice handoff.
        let tab_titles: Vec<String> = state
            .tabs
            .tabs()
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                tab.title
                    .clone()
                    .unwrap_or_else(|| format!("Tab {}", i + 1))
            })
            .collect();
        let active_index = state.tabs.active_index();
        let tab_labels: Vec<TabLabel> = tab_titles
            .iter()
            .map(|t| TabLabel { title: t.as_str() })
            .collect();
        let tab_bar = TabBarInfo {
            tabs: &tab_labels,
            active_index,
        };

        let Some(renderer) = self.renderer.as_deref_mut() else {
            return;
        };
        if let Err(e) = renderer.render(&tab_bar, &pane_infos, width, height) {
            eprintln!("render error: {e}");
        }

        state.dirty = false;
    }

    /// Pane viewport size — full window minus the tab bar strip.  Layouts,
    /// PTY resizes, and split allocations all use this so coordinates inside
    /// PaneTree are pane-area-relative; the redraw step adds the tab bar
    /// offset before handing rects to the renderer.
    fn pane_area_size(&self) -> (usize, usize) {
        let (w, h) = self
            .window
            .as_ref()
            .map(|w| {
                let s = w.inner_size();
                (s.width as usize, s.height as usize)
            })
            .unwrap_or((800, 600));
        (w, h.saturating_sub(TAB_BAR_HEIGHT_PX as usize))
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // Two-phase initialization: window must exist before the renderer's
        // surface can bind to it.  We open at a placeholder size, then ask
        // the platform to resize once we know the real cell metrics.
        // Approximate cell pixels for the initial open: avoids one extra
        // resize round-trip in the common case.
        let initial_cell_w = (self.config.font.size * 0.6).ceil() as u32;
        let initial_cell_h = (self.config.font.size * 1.4).ceil() as u32;
        let initial_w = initial_cell_w * INITIAL_COLS as u32;
        let initial_h = initial_cell_h * INITIAL_ROWS as u32;

        let attrs = Window::default_attributes()
            .with_title("zm-mux")
            .with_inner_size(winit::dpi::PhysicalSize::new(initial_w, initial_h));

        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));

        let renderer = create_renderer(
            window.clone(),
            self.config.font.size,
            &self.config.font.family,
        )
        .expect("renderer init");

        // Snap window to exact cell-metric size now that we have a real
        // shaper.  Platform may ignore this on Wayland; benign either way.
        let (req_w, req_h) = renderer.required_size(INITIAL_COLS as usize, INITIAL_ROWS as usize);
        let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(req_w as u32, req_h as u32));

        self.window = Some(window);
        self.renderer = Some(renderer);

        // Start reader for initial pane
        let initial = self.state.lock().unwrap().tabs.active().focused_pane;
        start_reader(initial, &self.state);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y.round() as i32 * 3,
                    MouseScrollDelta::PixelDelta(pos) => (pos.y / 16.0).round() as i32,
                };
                if lines == 0 {
                    return;
                }
                let mut state = self.state.lock().unwrap();
                let focused = state.focused_pane();
                if let Some(ps) = state.panes.get_mut(&focused) {
                    ps.term.scroll_lines(lines);
                    state.dirty = true;
                }
            }
            WindowEvent::Resized(_) => {
                let (w, h) = self.pane_area_size();
                if let Some(renderer) = self.renderer_ref() {
                    let mut state = self.state.lock().unwrap();
                    state.resize_all_panes(renderer, w, h);
                    state.dirty = true;
                }
                if let Some(win) = &self.window {
                    win.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }

                let ctrl = self.modifiers.contains(ModifiersState::CONTROL);
                let shift = self.modifiers.contains(ModifiersState::SHIFT);
                let alt = self.modifiers.contains(ModifiersState::ALT);
                let ctrl_only = ctrl && !shift && !alt;
                let ctrl_shift = ctrl && shift && !alt;
                let shift_only = shift && !ctrl && !alt;

                // Scrollback navigation (Shift+PgUp/PgDn/Home/End)
                if shift_only {
                    let mut state = self.state.lock().unwrap();
                    let focused = state.focused_pane();
                    let scrolled = if let Some(ps) = state.panes.get_mut(&focused) {
                        match &event.logical_key {
                            Key::Named(NamedKey::PageUp) => {
                                ps.term.scroll_page_up();
                                true
                            }
                            Key::Named(NamedKey::PageDown) => {
                                ps.term.scroll_page_down();
                                true
                            }
                            Key::Named(NamedKey::Home) => {
                                ps.term.scroll_to_top();
                                true
                            }
                            Key::Named(NamedKey::End) => {
                                ps.term.scroll_to_bottom();
                                true
                            }
                            _ => false,
                        }
                    } else {
                        false
                    };
                    if scrolled {
                        state.dirty = true;
                        return;
                    }
                }

                // Tab management — Ctrl-only chord
                if ctrl_only {
                    match &event.logical_key {
                        Key::Character(s) if s.as_str() == "t" || s.as_str() == "T" => {
                            let (w, h) = self.pane_area_size();
                            let Some(renderer) = self.renderer_ref() else { return };
                            let mut state = self.state.lock().unwrap();
                            let new_pane = state.create_new_tab(renderer, w, h);
                            drop(state);
                            start_reader(new_pane, &self.state);
                            return;
                        }
                        Key::Named(NamedKey::Tab) => {
                            let mut state = self.state.lock().unwrap();
                            state.switch_tab_next();
                            return;
                        }
                        Key::Character(s) => {
                            // Ctrl+1..9 → switch to tab index (0..8)
                            if let Some(c) = s.chars().next()
                                && let Some(d) = c.to_digit(10)
                                && (1..=9).contains(&d)
                            {
                                let mut state = self.state.lock().unwrap();
                                state.switch_tab_to_index((d - 1) as usize);
                                return;
                            }
                        }
                        _ => {}
                    }
                }

                // Split / close / prev-tab — Ctrl+Shift chord
                if ctrl_shift {
                    match &event.logical_key {
                        Key::Character(s) if s.as_str() == "D" || s.as_str() == "d" => {
                            let (w, h) = self.pane_area_size();
                            let Some(renderer) = self.renderer_ref() else { return };
                            let mut state = self.state.lock().unwrap();
                            state.split(SplitDirection::Horizontal, renderer, w, h);
                            let new_ids: Vec<PaneId> = state.tabs.active().tree.pane_ids();
                            drop(state);
                            for id in new_ids {
                                let needs_reader = self
                                    .state
                                    .lock()
                                    .unwrap()
                                    .panes
                                    .get(&id)
                                    .map(|ps| ps.pty.has_reader())
                                    .unwrap_or(false);
                                if needs_reader {
                                    start_reader(id, &self.state);
                                }
                            }
                            return;
                        }
                        Key::Character(s) if s.as_str() == "E" || s.as_str() == "e" => {
                            let (w, h) = self.pane_area_size();
                            let Some(renderer) = self.renderer_ref() else { return };
                            let mut state = self.state.lock().unwrap();
                            state.split(SplitDirection::Vertical, renderer, w, h);
                            let new_ids: Vec<PaneId> = state.tabs.active().tree.pane_ids();
                            drop(state);
                            for id in new_ids {
                                let needs_reader = self
                                    .state
                                    .lock()
                                    .unwrap()
                                    .panes
                                    .get(&id)
                                    .map(|ps| ps.pty.has_reader())
                                    .unwrap_or(false);
                                if needs_reader {
                                    start_reader(id, &self.state);
                                }
                            }
                            return;
                        }
                        Key::Character(s) if s.as_str() == "W" || s.as_str() == "w" => {
                            let mut state = self.state.lock().unwrap();
                            state.close_active_tab();
                            return;
                        }
                        Key::Character(s) if s.as_str() == "P" || s.as_str() == "p" => {
                            let mut state = self.state.lock().unwrap();
                            state.close_focused_pane();
                            return;
                        }
                        Key::Named(NamedKey::Tab) => {
                            let mut state = self.state.lock().unwrap();
                            state.switch_tab_prev();
                            return;
                        }
                        _ => {}
                    }
                }

                // Pane focus navigation — Alt+Arrow
                if alt {
                    let (w, h) = self.pane_area_size();
                    let mut state = self.state.lock().unwrap();
                    let focused = state.focused_pane();
                    let moved = {
                        let tab = state.tabs.active();
                        match &event.logical_key {
                            Key::Named(NamedKey::ArrowRight) => tab
                                .tree
                                .find_adjacent(focused, SplitDirection::Horizontal, true, w, h),
                            Key::Named(NamedKey::ArrowLeft) => tab
                                .tree
                                .find_adjacent(focused, SplitDirection::Horizontal, false, w, h),
                            Key::Named(NamedKey::ArrowDown) => tab
                                .tree
                                .find_adjacent(focused, SplitDirection::Vertical, true, w, h),
                            Key::Named(NamedKey::ArrowUp) => tab
                                .tree
                                .find_adjacent(focused, SplitDirection::Vertical, false, w, h),
                            _ => None,
                        }
                    };
                    if let Some(new_focus) = moved {
                        state.tabs.active_mut().focused_pane = new_focus;
                        state.dirty = true;
                    }
                    return;
                }

                // Normal key input → focused pane in active tab
                let bytes: Vec<u8> = match &event.logical_key {
                    Key::Character(s) => s.as_bytes().to_vec(),
                    Key::Named(NamedKey::Enter) => vec![b'\r'],
                    Key::Named(NamedKey::Backspace) => vec![0x7f],
                    Key::Named(NamedKey::Tab) => vec![b'\t'],
                    Key::Named(NamedKey::Escape) => vec![0x1b],
                    Key::Named(NamedKey::ArrowUp) => b"\x1b[A".to_vec(),
                    Key::Named(NamedKey::ArrowDown) => b"\x1b[B".to_vec(),
                    Key::Named(NamedKey::ArrowRight) => b"\x1b[C".to_vec(),
                    Key::Named(NamedKey::ArrowLeft) => b"\x1b[D".to_vec(),
                    _ => return,
                };

                let mut state = self.state.lock().unwrap();
                let focused = state.focused_pane();
                let mut snap_dirty = false;
                if let Some(ps) = state.panes.get_mut(&focused) {
                    // Snap viewport back to live content on user input.
                    if ps.term.display_offset() != 0 {
                        ps.term.scroll_to_bottom();
                        snap_dirty = true;
                    }
                    let _ = ps.pty.write_input(&bytes);
                }
                if snap_dirty {
                    state.dirty = true;
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let needs_redraw = self.state.lock().unwrap().dirty;
        if needs_redraw && let Some(w) = &self.window {
            w.request_redraw();
        }
        // Poll dirty flag at ~60Hz so PTY reader thread output reaches the
        // screen without an EventLoopProxy.  TODO: switch to EventLoopProxy.
        event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(16),
        ));
    }
}

fn main() {
    let config = Config::load();
    let state = Arc::new(Mutex::new(MuxState::new()));

    let event_loop = EventLoop::new().expect("event loop");
    let mut app = App::new(state, config);
    let _ = event_loop.run_app(&mut app);
}
