use portable_pty::CommandBuilder;
use softbuffer::Surface;
use std::collections::HashMap;
use std::io::Read;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use zm_core::Config;
use zm_mux::{PaneId, PaneTree, SplitDirection};
use zm_pty::ZmPtyProcess;
use zm_render::{CpuRenderer, PaneRenderInfo, Rect};
use zm_term::ZmTerm;

const INITIAL_COLS: u16 = 80;
const INITIAL_ROWS: u16 = 24;

struct PaneState {
    term: ZmTerm,
    pty: ZmPtyProcess,
}

struct MuxState {
    tree: PaneTree,
    panes: HashMap<PaneId, PaneState>,
    focused: PaneId,
    dirty: bool,
}

impl MuxState {
    fn new() -> Self {
        let tree = PaneTree::new();
        let root = tree.root_pane();
        let pane = create_pane(INITIAL_COLS, INITIAL_ROWS);
        let mut panes = HashMap::new();
        panes.insert(root, pane);

        Self {
            tree,
            panes,
            focused: root,
            dirty: true,
        }
    }

    fn split(
        &mut self,
        direction: SplitDirection,
        renderer: &CpuRenderer,
        win_width: usize,
        win_height: usize,
    ) {
        if let Some(new_id) = self.tree.split(self.focused, direction) {
            let layouts = self.tree.layout(win_width, win_height);
            if let Some((_, rect)) = layouts.iter().find(|(id, _)| *id == new_id) {
                let (cols, rows) = renderer.cols_rows_for_size(rect.width, rect.height);
                let pane = create_pane(cols, rows);
                self.panes.insert(new_id, pane);
                self.resize_all_panes(renderer, win_width, win_height);
                self.dirty = true;
            }
        }
    }

    fn close_focused(&mut self) {
        if self.tree.pane_count() <= 1 {
            return;
        }
        let to_remove = self.focused;
        let ids = self.tree.pane_ids();
        let next_focus = ids.iter().find(|id| **id != to_remove).copied().unwrap();

        if self.tree.remove(to_remove) {
            if let Some(mut ps) = self.panes.remove(&to_remove) {
                ps.pty.kill().ok();
            }
            self.focused = next_focus;
            self.dirty = true;
        }
    }

    fn resize_all_panes(&mut self, renderer: &CpuRenderer, win_width: usize, win_height: usize) {
        let layouts = self.tree.layout(win_width, win_height);
        for (id, rect) in &layouts {
            if let Some(ps) = self.panes.get_mut(id) {
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
    context: Option<softbuffer::Context<Arc<Window>>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    renderer: CpuRenderer,
    state: Arc<Mutex<MuxState>>,
    modifiers: ModifiersState,
}

impl App {
    fn new(renderer: CpuRenderer, state: Arc<Mutex<MuxState>>) -> Self {
        Self {
            window: None,
            context: None,
            surface: None,
            renderer,
            state,
            modifiers: ModifiersState::empty(),
        }
    }

    fn redraw(&mut self) {
        let Some(window) = &self.window else { return };
        let Some(surface) = &mut self.surface else {
            return;
        };

        let mut state = self.state.lock().unwrap();
        let size = window.inner_size();
        let width = size.width as usize;
        let height = size.height as usize;

        if width == 0 || height == 0 {
            return;
        }

        if !state.dirty {
            return;
        }

        let _ = surface.resize(
            NonZeroU32::new(size.width).unwrap(),
            NonZeroU32::new(size.height).unwrap(),
        );

        let layouts = state.tree.layout(width, height);
        let pane_infos: Vec<PaneRenderInfo> = layouts
            .iter()
            .filter_map(|(id, mux_rect)| {
                state.panes.get(id).map(|ps| PaneRenderInfo {
                    term: &ps.term,
                    rect: Rect {
                        x: mux_rect.x,
                        y: mux_rect.y,
                        width: mux_rect.width,
                        height: mux_rect.height,
                    },
                    focused: *id == state.focused,
                })
            })
            .collect();

        let mut buffer = surface.buffer_mut().unwrap();
        let buf_slice: &mut [u32] = &mut buffer;
        self.renderer
            .render_panes(&pane_infos, buf_slice, width, height);
        let _ = buffer.present();

        state.dirty = false;
    }

    fn win_size(&self) -> (usize, usize) {
        self.window
            .as_ref()
            .map(|w| {
                let s = w.inner_size();
                (s.width as usize, s.height as usize)
            })
            .unwrap_or((800, 600))
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let (req_w, req_h) = self
            .renderer
            .required_size(INITIAL_COLS as usize, INITIAL_ROWS as usize);

        let attrs = Window::default_attributes()
            .with_title("zm-mux")
            .with_inner_size(winit::dpi::PhysicalSize::new(req_w as u32, req_h as u32));

        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let context = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        self.context = Some(context);
        let surface = Surface::new(self.context.as_ref().unwrap(), window.clone())
            .expect("softbuffer surface");

        self.window = Some(window);
        self.surface = Some(surface);

        // Start reader for initial pane
        let root = self.state.lock().unwrap().tree.root_pane();
        start_reader(root, &self.state);
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
                let focused = state.focused;
                if let Some(ps) = state.panes.get_mut(&focused) {
                    ps.term.scroll_lines(lines);
                    state.dirty = true;
                }
            }
            WindowEvent::Resized(_) => {
                let (w, h) = self.win_size();
                let mut state = self.state.lock().unwrap();
                state.resize_all_panes(&self.renderer, w, h);
                state.dirty = true;
                drop(state);
                if let Some(win) = &self.window {
                    win.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }

                let ctrl_shift = self.modifiers.contains(ModifiersState::CONTROL)
                    && self.modifiers.contains(ModifiersState::SHIFT);
                let alt = self.modifiers.contains(ModifiersState::ALT);
                let shift_only = self.modifiers.contains(ModifiersState::SHIFT)
                    && !self.modifiers.contains(ModifiersState::CONTROL)
                    && !alt;

                // Scrollback navigation (Shift+PgUp/PgDn/Home/End)
                if shift_only {
                    let mut state = self.state.lock().unwrap();
                    let focused = state.focused;
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

                // Split pane shortcuts
                if ctrl_shift {
                    match &event.logical_key {
                        Key::Character(s) if s.as_str() == "D" || s.as_str() == "d" => {
                            let (w, h) = self.win_size();
                            let mut state = self.state.lock().unwrap();
                            state.split(SplitDirection::Horizontal, &self.renderer, w, h);
                            // Find new pane and start reader
                            let new_ids: Vec<PaneId> = state.tree.pane_ids();
                            drop(state);
                            for id in new_ids {
                                let has_reader = self
                                    .state
                                    .lock()
                                    .unwrap()
                                    .panes
                                    .get(&id)
                                    .map(|ps| !ps.pty.has_reader())
                                    .unwrap_or(true);
                                if !has_reader {
                                    start_reader(id, &self.state);
                                }
                            }
                            return;
                        }
                        Key::Character(s) if s.as_str() == "E" || s.as_str() == "e" => {
                            let (w, h) = self.win_size();
                            let mut state = self.state.lock().unwrap();
                            state.split(SplitDirection::Vertical, &self.renderer, w, h);
                            let new_ids: Vec<PaneId> = state.tree.pane_ids();
                            drop(state);
                            for id in new_ids {
                                let has_reader = self
                                    .state
                                    .lock()
                                    .unwrap()
                                    .panes
                                    .get(&id)
                                    .map(|ps| !ps.pty.has_reader())
                                    .unwrap_or(true);
                                if !has_reader {
                                    start_reader(id, &self.state);
                                }
                            }
                            return;
                        }
                        Key::Character(s) if s.as_str() == "W" || s.as_str() == "w" => {
                            let mut state = self.state.lock().unwrap();
                            state.close_focused();
                            return;
                        }
                        _ => {}
                    }
                }

                // Focus navigation
                if alt {
                    let (w, h) = self.win_size();
                    let mut state = self.state.lock().unwrap();
                    let moved = match &event.logical_key {
                        Key::Named(NamedKey::ArrowRight) => state.tree.find_adjacent(
                            state.focused,
                            SplitDirection::Horizontal,
                            true,
                            w,
                            h,
                        ),
                        Key::Named(NamedKey::ArrowLeft) => state.tree.find_adjacent(
                            state.focused,
                            SplitDirection::Horizontal,
                            false,
                            w,
                            h,
                        ),
                        Key::Named(NamedKey::ArrowDown) => state.tree.find_adjacent(
                            state.focused,
                            SplitDirection::Vertical,
                            true,
                            w,
                            h,
                        ),
                        Key::Named(NamedKey::ArrowUp) => state.tree.find_adjacent(
                            state.focused,
                            SplitDirection::Vertical,
                            false,
                            w,
                            h,
                        ),
                        _ => None,
                    };
                    if let Some(new_focus) = moved {
                        state.focused = new_focus;
                        state.dirty = true;
                    }
                    return;
                }

                // Normal key input → focused pane
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
                let focused = state.focused;
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
        if needs_redraw {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
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
    let renderer =
        CpuRenderer::new(config.font.size, &config.font.family).expect("renderer init");
    let state = Arc::new(Mutex::new(MuxState::new()));

    let event_loop = EventLoop::new().expect("event loop");
    let mut app = App::new(renderer, state);
    let _ = event_loop.run_app(&mut app);
}
