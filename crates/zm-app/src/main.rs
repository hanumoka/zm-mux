use portable_pty::CommandBuilder;
use softbuffer::Surface;
use std::io::Read;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use zm_pty::ZmPtyProcess;
use zm_render::CpuRenderer;
use zm_term::ZmTerm;

const FONT_SIZE: f32 = 16.0;
const INITIAL_COLS: u16 = 80;
const INITIAL_ROWS: u16 = 24;

struct TermState {
    term: ZmTerm,
    pty: ZmPtyProcess,
    dirty: bool,
}

struct App {
    window: Option<Arc<Window>>,
    context: Option<softbuffer::Context<Arc<Window>>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    renderer: CpuRenderer,
    state: Arc<Mutex<TermState>>,
}

impl App {
    fn new(renderer: CpuRenderer, state: Arc<Mutex<TermState>>) -> Self {
        Self {
            window: None,
            context: None,
            surface: None,
            renderer,
            state,
        }
    }

    fn redraw(&mut self) {
        let Some(window) = &self.window else { return };
        let Some(surface) = &mut self.surface else {
            return;
        };

        let state = self.state.lock().unwrap();
        let size = window.inner_size();
        let width = size.width as usize;
        let height = size.height as usize;

        if width == 0 || height == 0 {
            return;
        }

        let _ = surface.resize(
            NonZeroU32::new(size.width).unwrap(),
            NonZeroU32::new(size.height).unwrap(),
        );

        let mut buffer = surface.buffer_mut().unwrap();
        let buf_slice: &mut [u32] = &mut buffer;
        self.renderer
            .render_to_buffer(&state.term, buf_slice, width, height);
        let _ = buffer.present();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let (req_w, req_h) = {
            let state = self.state.lock().unwrap();
            self.renderer.required_size(&state.term)
        };

        let attrs = Window::default_attributes()
            .with_title("zm-mux")
            .with_inner_size(winit::dpi::LogicalSize::new(req_w as u32, req_h as u32));

        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let context = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        self.context = Some(context);
        let surface = Surface::new(self.context.as_ref().unwrap(), window.clone())
            .expect("softbuffer surface");

        self.window = Some(window);
        self.surface = Some(surface);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
            }
            WindowEvent::Resized(size) => {
                let (cw, ch) = self.renderer.cell_size();
                if cw > 0 && ch > 0 {
                    let new_cols = (size.width as usize / cw).max(1) as u16;
                    let new_rows = (size.height as usize / ch).max(1) as u16;
                    let mut state = self.state.lock().unwrap();
                    state.term.resize(new_cols, new_rows);
                    let _ = state.pty.resize(new_rows, new_cols);
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
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
                let _ = state.pty.write_input(&bytes);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Always request redraw to keep terminal output flowing
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

fn main() {
    let renderer = CpuRenderer::new(FONT_SIZE).expect("renderer init");

    let cmd = CommandBuilder::new_default_prog();
    let mut pty = zm_pty::spawn_pty(INITIAL_ROWS, INITIAL_COLS, cmd).expect("PTY spawn");
    let term = ZmTerm::new(INITIAL_COLS, INITIAL_ROWS).expect("term init");

    let pty_reader = pty.take_reader().expect("PTY reader");

    let state = Arc::new(Mutex::new(TermState {
        term,
        pty,
        dirty: false,
    }));

    // Background thread: read PTY output → feed to terminal
    let state_clone = state.clone();
    std::thread::spawn(move || {
        let mut reader = pty_reader;
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let mut s = state_clone.lock().unwrap();
                    s.term.feed_bytes(&buf[..n]);
                    s.dirty = true;
                }
                Err(_) => break,
            }
        }
    });

    // Periodic redraw trigger
    let state_redraw = state.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(16));
            let mut s = state_redraw.lock().unwrap();
            if s.dirty {
                s.dirty = false;
                // The about_to_wait handler will request redraw
            }
            drop(s);
        }
    });

    let event_loop = EventLoop::new().expect("event loop");
    let mut app = App::new(renderer, state);
    let _ = event_loop.run_app(&mut app);
}
