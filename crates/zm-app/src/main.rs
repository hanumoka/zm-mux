//! zm-app — winit 0.30 앱 엔트리. 멀티플렉서(분할/탭) + 설정 기반 단축키.
//!
//! 구조: Mux(zm-mux, 분할 트리/탭/포커스) + PaneId→PaneRuntime 맵(PTY+Term+리더 스레드).
//!  - 각 pane 리더 스레드가 PaneId 태깅해 EventLoopProxy로 출력 전달.
//!  - 활성 pane으로 키 입력 라우팅. **프리픽스 없는 직접 단축키**(config `[keybindings]`).
//!  - 셸/폰트/색/스크롤백은 config(zm-core::Config, `%APPDATA%\zm-mux\config.toml`).
//!
//! 기본 단축키: 새탭 Ctrl+T, 탭닫기 Ctrl+Shift+W, pane닫기 Ctrl+Shift+P,
//!   상하분할 Ctrl+Shift+D, 좌우분할 Ctrl+Shift+E, 다음/이전 탭 Ctrl+Tab / Ctrl+Shift+Tab,
//!   포커스 이동 Alt+방향키.

use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;
use std::thread::JoinHandle;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{UserAttentionType, Window, WindowId};

use zm_core::{CellSnapshot, Config, GridSize, Notification};
use zm_ipc::{Command as IpcCommand, IpcRequest, PaneInfo, RequestSink, Response};
use zm_mux::{CloseResult, Mux, Orientation, PaneId, Rect};
use zm_pty::{CommandBuilder, Pty, PtyChannels};
use zm_render::{PaneView, Renderer};
use zm_term::Terminal;

/// 분할 사이 간격(divider) 픽셀.
const BORDER: f32 = 6.0;

enum UserEvent {
    PtyOutput(PaneId, Vec<u8>),
    PtyExited(PaneId),
    Ipc(IpcRequest),
}

/// zm-ipc 요청을 이벤트 루프로 전달하는 sink.
#[derive(Clone)]
struct ProxySink(EventLoopProxy<UserEvent>);

impl RequestSink for ProxySink {
    fn submit(&self, req: IpcRequest) {
        let _ = self.0.send_event(UserEvent::Ipc(req));
    }
}

fn parse_dir(s: &str) -> Option<Dir> {
    match s.trim().to_ascii_lowercase().as_str() {
        "left" | "l" => Some(Dir::Left),
        "right" | "r" => Some(Dir::Right),
        "up" | "u" => Some(Dir::Up),
        "down" | "d" => Some(Dir::Down),
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum Dir {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy)]
enum Action {
    NewTab,
    CloseTab,
    ClosePane,
    SplitHorizontal, // 상하(가로 divider)
    SplitVertical,   // 좌우(세로 divider)
    NextTab,
    PrevTab,
    Focus(Dir),
    ZoomToggle,
}

#[derive(Clone, Copy)]
enum ChordKey {
    Char(char),
    Named(NamedKey),
}

#[derive(Clone, Copy)]
struct Chord {
    ctrl: bool,
    shift: bool,
    alt: bool,
    key: ChordKey,
}

impl Chord {
    fn matches(&self, ev: &KeyEvent, mods: ModifiersState) -> bool {
        if mods.control_key() != self.ctrl
            || mods.shift_key() != self.shift
            || mods.alt_key() != self.alt
        {
            return false;
        }
        match &self.key {
            ChordKey::Char(c) => {
                matches!(&ev.logical_key, Key::Character(s)
                    if s.chars().next().map(|x| x.to_ascii_lowercase()) == Some(*c))
            }
            ChordKey::Named(n) => matches!(&ev.logical_key, Key::Named(k) if k == n),
        }
    }
}

fn parse_key(s: &str) -> Option<ChordKey> {
    let lower = s.to_ascii_lowercase();
    let nk = match lower.as_str() {
        "tab" => Some(NamedKey::Tab),
        "enter" | "return" => Some(NamedKey::Enter),
        "space" => Some(NamedKey::Space),
        "esc" | "escape" => Some(NamedKey::Escape),
        "left" => Some(NamedKey::ArrowLeft),
        "right" => Some(NamedKey::ArrowRight),
        "up" => Some(NamedKey::ArrowUp),
        "down" => Some(NamedKey::ArrowDown),
        "backspace" => Some(NamedKey::Backspace),
        "delete" | "del" => Some(NamedKey::Delete),
        "home" => Some(NamedKey::Home),
        "end" => Some(NamedKey::End),
        "pageup" => Some(NamedKey::PageUp),
        "pagedown" => Some(NamedKey::PageDown),
        _ => None,
    };
    if let Some(n) = nk {
        return Some(ChordKey::Named(n));
    }
    let mut chars = lower.chars();
    let c = chars.next()?;
    if chars.next().is_none() {
        Some(ChordKey::Char(c))
    } else {
        None
    }
}

fn parse_chord(s: &str) -> Option<Chord> {
    if s.trim().is_empty() {
        return None;
    }
    let (mut ctrl, mut shift, mut alt) = (false, false, false);
    let mut key = None;
    for part in s.split('+') {
        match part.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "alt" | "option" => alt = true,
            "super" | "win" | "cmd" | "meta" => {}
            other => key = Some(parse_key(other)?),
        }
    }
    Some(Chord {
        ctrl,
        shift,
        alt,
        key: key?,
    })
}

fn build_keymap(cfg: &Config) -> Vec<(Chord, Action)> {
    let kb = &cfg.keybindings;
    let entries: [(&str, Action); 12] = [
        (kb.new_tab.as_str(), Action::NewTab),
        (kb.close_tab.as_str(), Action::CloseTab),
        (kb.close_pane.as_str(), Action::ClosePane),
        (kb.split_horizontal.as_str(), Action::SplitHorizontal),
        (kb.split_vertical.as_str(), Action::SplitVertical),
        (kb.next_tab.as_str(), Action::NextTab),
        (kb.prev_tab.as_str(), Action::PrevTab),
        (kb.focus_left.as_str(), Action::Focus(Dir::Left)),
        (kb.focus_right.as_str(), Action::Focus(Dir::Right)),
        (kb.focus_up.as_str(), Action::Focus(Dir::Up)),
        (kb.focus_down.as_str(), Action::Focus(Dir::Down)),
        (kb.zoom.as_str(), Action::ZoomToggle),
    ];
    entries
        .iter()
        .filter_map(|(s, a)| parse_chord(s).map(|c| (c, *a)))
        .collect()
}

struct PaneRuntime {
    term: Terminal,
    pty: Pty,
    writer: Box<dyn Write + Send>,
    reader_join: Option<JoinHandle<()>>,
    snapshot: CellSnapshot,
    grid: GridSize,
}

impl Drop for PaneRuntime {
    fn drop(&mut self) {
        let _ = self.pty.kill();
        if let Some(j) = self.reader_join.take() {
            let _ = j.join();
        }
    }
}

struct RunningState {
    window: Arc<Window>,
    renderer: Renderer,
    mux: Mux,
    panes: HashMap<PaneId, PaneRuntime>,
    modifiers: ModifiersState,
    keymap: Vec<(Chord, Action)>,
    config: Arc<Config>,
    socket: String,
    cursor_pos: (f32, f32),
    dragging_divider: bool,
    last_title: String,
}

impl RunningState {
    fn window_rect(&self) -> Rect {
        let s = self.window.inner_size();
        let bar = self.renderer.tab_bar_height();
        Rect {
            x: 0.0,
            y: bar,
            w: s.width.max(1) as f32,
            h: (s.height as f32 - bar).max(1.0),
        }
    }

    fn relayout(&mut self) {
        let geoms = self.mux.compute(self.window_rect(), BORDER);
        for g in geoms {
            let grid = self.renderer.pane_grid(g.rect.w, g.rect.h);
            if let Some(rt) = self.panes.get_mut(&g.id) {
                if rt.grid != grid {
                    rt.term.resize(grid);
                    let _ = rt.pty.resize(grid, 0, 0);
                    rt.grid = grid;
                }
            }
        }
    }

    /// 활성 pane의 셸 제목을 창 제목에 반영(변경 시에만).
    fn update_title(&mut self) {
        let active = self.mux.active_pane();
        let t = self
            .panes
            .get(&active)
            .and_then(|rt| rt.term.title())
            .unwrap_or_default();
        let title = if t.trim().is_empty() {
            "zm-mux".to_string()
        } else {
            format!("zm-mux — {t}")
        };
        if title != self.last_title {
            self.window.set_title(&title);
            self.last_title = title;
        }
    }

    fn render_frame(&mut self) {
        self.update_title();
        let geoms = self.mux.compute(self.window_rect(), BORDER);
        let active = self.mux.active_pane();
        for g in &geoms {
            if let Some(rt) = self.panes.get_mut(&g.id) {
                rt.term.snapshot(&mut rt.snapshot);
            }
        }
        let active_tab = self.mux.active_tab_index();
        let tab_count = self.mux.tab_count();
        let views: Vec<PaneView> = geoms
            .iter()
            .filter_map(|g| {
                self.panes.get(&g.id).map(|rt| PaneView {
                    snapshot: &rt.snapshot,
                    x: g.rect.x,
                    y: g.rect.y,
                    w: g.rect.w,
                    h: g.rect.h,
                    focused: g.id == active,
                })
            })
            .collect();
        if let Err(e) = self.renderer.render(&views, active_tab, tab_count) {
            log::error!("render 오류: {e}");
        }
    }

    fn send_to_active(&mut self, bytes: &[u8]) {
        let active = self.mux.active_pane();
        if let Some(rt) = self.panes.get_mut(&active) {
            rt.term.scroll_to_bottom();
            let _ = rt.writer.write_all(bytes);
            let _ = rt.writer.flush();
        }
    }

    /// 좌표(물리 px) 아래의 pane.
    fn pane_at(&self, x: f32, y: f32) -> Option<PaneId> {
        self.mux
            .compute(self.window_rect(), BORDER)
            .into_iter()
            .find(|g| {
                x >= g.rect.x
                    && x < g.rect.x + g.rect.w
                    && y >= g.rect.y
                    && y < g.rect.y + g.rect.h
            })
            .map(|g| g.id)
    }

    /// 방향 포커스: 활성 pane 기준 해당 방향 가장 가까운 pane.
    fn focus_dir(&mut self, dir: Dir) {
        let geoms = self.mux.compute(self.window_rect(), BORDER);
        let active = self.mux.active_pane();
        let Some(cur) = geoms.iter().find(|g| g.id == active).map(|g| g.rect) else {
            return;
        };
        let ccx = cur.x + cur.w / 2.0;
        let ccy = cur.y + cur.h / 2.0;
        let mut best: Option<(f32, PaneId)> = None;
        for g in &geoms {
            if g.id == active {
                continue;
            }
            let r = g.rect;
            let gcx = r.x + r.w / 2.0;
            let gcy = r.y + r.h / 2.0;
            let overlap_v = r.y < cur.y + cur.h && r.y + r.h > cur.y;
            let overlap_h = r.x < cur.x + cur.w && r.x + r.w > cur.x;
            let (ok, dist) = match dir {
                Dir::Left => (gcx < ccx && overlap_v, ccx - gcx),
                Dir::Right => (gcx > ccx && overlap_v, gcx - ccx),
                Dir::Up => (gcy < ccy && overlap_h, ccy - gcy),
                Dir::Down => (gcy > ccy && overlap_h, gcy - ccy),
            };
            if ok && best.map(|(d, _)| dist < d).unwrap_or(true) {
                best = Some((dist, g.id));
            }
        }
        if let Some((_, id)) = best {
            self.mux.focus(id);
        }
    }
}

fn spawn_pane(
    proxy: &EventLoopProxy<UserEvent>,
    id: PaneId,
    grid: GridSize,
    cfg: &Config,
    socket: &str,
) -> Option<PaneRuntime> {
    let mut cmd = match cfg.shell_program() {
        Some(prog) => {
            let mut c = CommandBuilder::new(prog);
            c.args(cfg.shell_args());
            c
        }
        None => CommandBuilder::new_default_prog(),
    };
    // 자동화 소켓/현재 pane을 자식에 노출(`zm`/tmux-shim이 이 인스턴스에 연결).
    cmd.env("ZM_MUX_SOCKET", socket);
    cmd.env("ZM_MUX_PANE", id.to_string());

    // 트랙 A: tmux-shim 환경 주입(PATH 앞에 shim 디렉터리 + 가짜 TMUX 등).
    if cfg.agent.tmux_shim {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let sep = if cfg!(windows) { ';' } else { ':' };
                let old = std::env::var("PATH").unwrap_or_default();
                cmd.env("PATH", format!("{}{}{}", dir.display(), sep, old));
            }
        }
        for (k, v) in zm_agent::tmux_env(id, socket, cfg.agent.claude_agent_teams) {
            cmd.env(k, v);
        }
    }

    let (pty, chans) = Pty::spawn_cmd(cmd, grid, 0, 0).ok()?;
    let PtyChannels { reader, writer } = chans;

    let p = proxy.clone();
    let reader_join = std::thread::spawn(move || {
        let mut reader = reader;
        let mut buf = [0u8; 8192];
        loop {
            match std::io::Read::read(&mut reader, &mut buf) {
                Ok(0) => {
                    let _ = p.send_event(UserEvent::PtyExited(id));
                    break;
                }
                Ok(n) => {
                    if p.send_event(UserEvent::PtyOutput(id, buf[..n].to_vec())).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    let _ = p.send_event(UserEvent::PtyExited(id));
                    break;
                }
            }
        }
    });

    Some(PaneRuntime {
        term: Terminal::new(grid, cfg),
        pty,
        writer,
        reader_join: Some(reader_join),
        snapshot: CellSnapshot::new(grid),
        grid,
    })
}

struct App {
    proxy: EventLoopProxy<UserEvent>,
    state: Option<RunningState>,
}

impl App {
    fn do_action(&mut self, action: Action, event_loop: &ActiveEventLoop) {
        match action {
            Action::NewTab => self.cmd_new_tab(),
            Action::CloseTab => self.cmd_close_tab(event_loop),
            Action::ClosePane => self.cmd_close_pane(event_loop),
            Action::SplitHorizontal => self.cmd_split(Orientation::TopBottom),
            Action::SplitVertical => self.cmd_split(Orientation::LeftRight),
            Action::NextTab => self.cmd_tab(true),
            Action::PrevTab => self.cmd_tab(false),
            Action::Focus(d) => {
                if let Some(state) = self.state.as_mut() {
                    state.focus_dir(d);
                    state.window.request_redraw();
                }
            }
            Action::ZoomToggle => {
                if let Some(state) = self.state.as_mut() {
                    state.mux.toggle_zoom();
                    state.relayout();
                    state.window.request_redraw();
                }
            }
        }
    }

    fn cmd_split(&mut self, orient: Orientation) {
        let Some(state) = self.state.as_mut() else { return };
        let new_id = state.mux.split_active(orient);
        let cfg = state.config.clone();
        let sock = state.socket.clone();
        if let Some(rt) = spawn_pane(&self.proxy, new_id, GridSize::new(80, 24), &cfg, &sock) {
            state.panes.insert(new_id, rt);
        }
        state.relayout();
        state.window.request_redraw();
    }

    fn cmd_new_tab(&mut self) {
        let Some(state) = self.state.as_mut() else { return };
        let new_id = state.mux.new_tab();
        let cfg = state.config.clone();
        let sock = state.socket.clone();
        if let Some(rt) = spawn_pane(&self.proxy, new_id, GridSize::new(80, 24), &cfg, &sock) {
            state.panes.insert(new_id, rt);
        }
        state.relayout();
        state.window.request_redraw();
    }

    fn cmd_close_pane(&mut self, event_loop: &ActiveEventLoop) {
        let Some(state) = self.state.as_mut() else { return };
        let res = state.mux.close_active();
        let closed = match res {
            CloseResult::Pane(id) | CloseResult::Tab(id) | CloseResult::LastPane(id) => id,
        };
        if let Some(rt) = state.panes.remove(&closed) {
            drop(rt);
        }
        if matches!(res, CloseResult::LastPane(_)) {
            event_loop.exit();
            return;
        }
        state.relayout();
        state.window.request_redraw();
    }

    fn cmd_close_tab(&mut self, event_loop: &ActiveEventLoop) {
        let Some(state) = self.state.as_mut() else { return };
        let (ids, last) = state.mux.close_tab();
        for id in ids {
            if let Some(rt) = state.panes.remove(&id) {
                drop(rt);
            }
        }
        if last {
            event_loop.exit();
            return;
        }
        state.relayout();
        state.window.request_redraw();
    }

    fn cmd_tab(&mut self, next: bool) {
        if let Some(state) = self.state.as_mut() {
            if next {
                state.mux.next_tab();
            } else {
                state.mux.prev_tab();
            }
            state.relayout();
            state.window.request_redraw();
        }
    }

    /// 자동화 명령 실행(메인 스레드).
    fn exec_ipc(&mut self, cmd: &IpcCommand, event_loop: &ActiveEventLoop) -> Response {
        let proxy = self.proxy.clone();
        let Some(state) = self.state.as_mut() else {
            return Response::err("not ready");
        };
        match cmd {
            IpcCommand::ListPanes => {
                let active = state.mux.active_pane();
                let infos: Vec<PaneInfo> = state
                    .mux
                    .all_panes()
                    .iter()
                    .filter_map(|id| {
                        state.panes.get(id).map(|rt| PaneInfo {
                            id: *id,
                            active: *id == active,
                            cols: rt.grid.cols,
                            rows: rt.grid.rows,
                        })
                    })
                    .collect();
                Response::ok(serde_json::to_value(infos).unwrap_or(serde_json::Value::Null))
            }
            IpcCommand::ListTabs => Response::ok(serde_json::json!({
                "count": state.mux.tab_count(),
                "active": state.mux.active_tab_index(),
            })),
            IpcCommand::Split { vertical } => {
                let orient = if *vertical {
                    Orientation::LeftRight
                } else {
                    Orientation::TopBottom
                };
                let new_id = state.mux.split_active(orient);
                let cfg = state.config.clone();
                let sock = state.socket.clone();
                if spawn_pane(&proxy, new_id, GridSize::new(80, 24), &cfg, &sock)
                    .map(|rt| state.panes.insert(new_id, rt))
                    .is_none()
                {
                    return Response::err("pane spawn 실패");
                }
                state.relayout();
                state.window.request_redraw();
                Response::ok(serde_json::json!({ "id": new_id }))
            }
            IpcCommand::NewTab => {
                let new_id = state.mux.new_tab();
                let cfg = state.config.clone();
                let sock = state.socket.clone();
                if spawn_pane(&proxy, new_id, GridSize::new(80, 24), &cfg, &sock)
                    .map(|rt| state.panes.insert(new_id, rt))
                    .is_none()
                {
                    return Response::err("pane spawn 실패");
                }
                state.relayout();
                state.window.request_redraw();
                Response::ok(serde_json::json!({ "id": new_id }))
            }
            IpcCommand::SelectPane { id } => {
                state.mux.focus(*id);
                state.window.request_redraw();
                Response::ok_empty()
            }
            IpcCommand::Focus { dir } => match parse_dir(dir) {
                Some(d) => {
                    state.focus_dir(d);
                    state.window.request_redraw();
                    Response::ok_empty()
                }
                None => Response::err("bad dir(left/right/up/down)"),
            },
            IpcCommand::SendKeys { pane, data } => {
                let target = pane.unwrap_or(state.mux.active_pane());
                match state.panes.get_mut(&target) {
                    Some(rt) => {
                        rt.term.scroll_to_bottom();
                        let _ = rt.writer.write_all(data.as_bytes());
                        let _ = rt.writer.flush();
                        state.window.request_redraw();
                        Response::ok_empty()
                    }
                    None => Response::err("no such pane"),
                }
            }
            IpcCommand::CapturePane { pane } => {
                let target = pane.unwrap_or(state.mux.active_pane());
                match state.panes.get(&target) {
                    Some(rt) => {
                        Response::ok(serde_json::json!({ "text": rt.term.capture_text() }))
                    }
                    None => Response::err("no such pane"),
                }
            }
            IpcCommand::KillPane { id } => {
                if let Some(rt) = state.panes.remove(id) {
                    drop(rt);
                }
                let res = state.mux.close_pane(*id);
                if matches!(res, CloseResult::LastPane(_)) {
                    event_loop.exit();
                    return Response::ok_empty();
                }
                state.relayout();
                state.window.request_redraw();
                Response::ok_empty()
            }
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        event_loop.set_control_flow(ControlFlow::Wait);

        let config = Arc::new(Config::load());
        let keymap = build_keymap(&config);

        // 자동화 소켓 시작 + env 노출(자식 pane이 상속).
        let socket = zm_ipc::socket_name(std::process::id());
        std::env::set_var("ZM_MUX_SOCKET", &socket);
        if let Err(e) = zm_ipc::serve(&socket, ProxySink(self.proxy.clone())) {
            log::warn!("자동화 소켓 시작 실패: {e}");
        }

        let attrs = Window::default_attributes().with_title("zm-mux");
        let window = Arc::new(event_loop.create_window(attrs).expect("create_window 실패"));
        let renderer = match pollster::block_on(Renderer::new(window.clone(), &config)) {
            Ok(r) => r,
            Err(e) => {
                // GPU 초기화 실패: CPU 폴백 미구현 → 명확히 알리고 종료(패닉 대신).
                log::error!("GPU 렌더러 초기화 실패(CPU 폴백 미구현): {e}");
                eprintln!("zm-mux: GPU 초기화 실패 — {e}");
                event_loop.exit();
                return;
            }
        };

        let (mux, first_id) = Mux::new();
        let mut panes: HashMap<PaneId, PaneRuntime> = HashMap::new();

        let s = window.inner_size();
        let bar = renderer.tab_bar_height();
        let win = Rect {
            x: 0.0,
            y: bar,
            w: s.width.max(1) as f32,
            h: (s.height as f32 - bar).max(1.0),
        };
        let geoms = mux.compute(win, BORDER);
        let grid = geoms
            .first()
            .map(|g| renderer.pane_grid(g.rect.w, g.rect.h))
            .unwrap_or(GridSize::new(80, 24));
        if let Some(rt) = spawn_pane(&self.proxy, first_id, grid, &config, &socket) {
            panes.insert(first_id, rt);
        }

        let mut state = RunningState {
            window,
            renderer,
            mux,
            panes,
            modifiers: ModifiersState::empty(),
            keymap,
            config,
            socket,
            cursor_pos: (0.0, 0.0),
            dragging_divider: false,
            last_title: String::new(),
        };
        state.relayout();
        state.window.request_redraw();
        self.state = Some(state);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::PtyOutput(id, bytes) => {
                let Some(state) = self.state.as_mut() else { return };
                let mut notes = Vec::new();
                if let Some(rt) = state.panes.get_mut(&id) {
                    rt.term.feed(&bytes);
                    let resp = rt.term.take_pending_writes();
                    if !resp.is_empty() {
                        let _ = rt.writer.write_all(&resp);
                        let _ = rt.writer.flush();
                    }
                    notes = rt.term.take_notifications();
                    state.window.request_redraw();
                }
                if !notes.is_empty() {
                    state
                        .window
                        .request_user_attention(Some(UserAttentionType::Informational));
                    for n in &notes {
                        log::info!("알림: {} — {}", n.title, n.body);
                        fire_toast(n);
                    }
                }
            }
            UserEvent::PtyExited(id) => {
                let Some(state) = self.state.as_mut() else { return };
                if let Some(rt) = state.panes.remove(&id) {
                    drop(rt);
                }
                let res = state.mux.close_pane(id);
                if matches!(res, CloseResult::LastPane(_)) {
                    event_loop.exit();
                    return;
                }
                state.relayout();
                state.window.request_redraw();
            }
            UserEvent::Ipc(req) => {
                let resp = self.exec_ipc(&req.command, event_loop);
                req.reply(resp);
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::ModifiersChanged(mods) => {
                if let Some(state) = self.state.as_mut() {
                    state.modifiers = mods.state();
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                let mods = self.state.as_ref().map(|s| s.modifiers).unwrap_or_default();

                // 설정 단축키 우선.
                let action = self.state.as_ref().and_then(|s| {
                    s.keymap
                        .iter()
                        .find(|(c, _)| c.matches(&event, mods))
                        .map(|(_, a)| *a)
                });
                if let Some(a) = action {
                    self.do_action(a, event_loop);
                    return;
                }

                // 일반 입력 → 활성 pane(Kitty 키보드면 CSI-u 인코딩).
                let kitty = self
                    .state
                    .as_ref()
                    .and_then(|s| {
                        s.panes
                            .get(&s.mux.active_pane())
                            .map(|rt| rt.term.kitty_keyboard())
                    })
                    .unwrap_or(false);
                if let Some(bytes) = key_to_bytes(&event, mods, kitty) {
                    if let Some(state) = self.state.as_mut() {
                        state.send_to_active(&bytes);
                        state.window.request_redraw();
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                if let Some(state) = self.state.as_mut() {
                    let new = (position.x as f32, position.y as f32);
                    let old = state.cursor_pos;
                    state.cursor_pos = new;
                    if state.dragging_divider {
                        let win = state.window_rect();
                        let delta = (new.0 - old.0, new.1 - old.1);
                        if state.mux.resize_split(win, BORDER, new, delta) {
                            state.relayout();
                            state.window.request_redraw();
                        }
                    }
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(state) = self.state.as_mut() {
                    let (x, y) = state.cursor_pos;
                    match state.pane_at(x, y) {
                        Some(id) => {
                            state.mux.focus(id);
                            state.window.request_redraw();
                        }
                        // pane이 아니면(=divider/gap, 탭바 아래) 드래그 리사이즈 시작.
                        None if y >= state.renderer.tab_bar_height() => {
                            state.dragging_divider = true;
                        }
                        None => {}
                    }
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(state) = self.state.as_mut() {
                    state.dragging_divider = false;
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(state) = self.state.as_mut() {
                    let lines = match delta {
                        MouseScrollDelta::LineDelta(_, y) => (y * 3.0).round() as i32,
                        MouseScrollDelta::PixelDelta(p) => (p.y / 24.0).round() as i32,
                    };
                    if lines != 0 {
                        let active = state.mux.active_pane();
                        if let Some(rt) = state.panes.get_mut(&active) {
                            rt.term.scroll(lines);
                        }
                        state.window.request_redraw();
                    }
                }
            }

            WindowEvent::Resized(size) => {
                if let Some(state) = self.state.as_mut() {
                    state.renderer.resize(size.width, size.height);
                    state.relayout();
                    state.window.request_redraw();
                }
            }

            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(state) = self.state.as_mut() {
                    let size = state.window.inner_size();
                    state.renderer.resize(size.width, size.height);
                    state.relayout();
                    state.window.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => {
                if let Some(state) = self.state.as_mut() {
                    state.render_frame();
                }
            }

            _ => {}
        }
    }
}

/// 데스크톱 토스트(best-effort, 별도 스레드). Windows는 AUMID 미등록 시 표시 안 될 수 있음.
fn fire_toast(n: &Notification) {
    let (title, body) = (n.title.clone(), n.body.clone());
    std::thread::spawn(move || {
        let _ = notify_rust::Notification::new()
            .summary(&title)
            .body(&body)
            .show();
    });
}

/// winit KeyEvent → PTY 바이트. kitty=true면 수식 Enter를 CSI-u로(Shift+Enter 등).
fn key_to_bytes(event: &KeyEvent, mods: ModifiersState, kitty: bool) -> Option<Vec<u8>> {
    let ctrl = mods.control_key();
    let alt = mods.alt_key();
    let shift = mods.shift_key();

    match &event.logical_key {
        Key::Named(nk) => {
            // Kitty 키보드 활성 시 수식 Enter → CSI-u (Claude Code Shift+Enter 등).
            if *nk == NamedKey::Enter && kitty && (shift || ctrl || alt) {
                let m = 1 + (shift as u32) + (alt as u32) * 2 + (ctrl as u32) * 4;
                return Some(format!("\x1b[13;{m}u").into_bytes());
            }
            if ctrl && *nk == NamedKey::Space {
                return Some(vec![0]);
            }
            let b: &[u8] = match nk {
                NamedKey::Enter => b"\r",
                NamedKey::Backspace => b"\x7f",
                NamedKey::Tab => b"\t",
                NamedKey::Escape => b"\x1b",
                NamedKey::ArrowUp => b"\x1b[A",
                NamedKey::ArrowDown => b"\x1b[B",
                NamedKey::ArrowRight => b"\x1b[C",
                NamedKey::ArrowLeft => b"\x1b[D",
                NamedKey::Home => b"\x1b[H",
                NamedKey::End => b"\x1b[F",
                NamedKey::Delete => b"\x1b[3~",
                NamedKey::PageUp => b"\x1b[5~",
                NamedKey::PageDown => b"\x1b[6~",
                NamedKey::Space => b" ",
                _ => return None,
            };
            Some(b.to_vec())
        }
        Key::Character(s) => {
            if ctrl {
                let c = s.chars().next()?;
                let lower = c.to_ascii_lowercase();
                if lower.is_ascii_alphabetic() {
                    return Some(vec![lower as u8 - b'a' + 1]);
                }
                match c {
                    '[' => return Some(vec![0x1b]),
                    '\\' => return Some(vec![0x1c]),
                    ']' => return Some(vec![0x1d]),
                    '^' => return Some(vec![0x1e]),
                    '_' => return Some(vec![0x1f]),
                    _ => {}
                }
            }
            let mut v = Vec::new();
            if alt {
                v.push(0x1b);
            }
            match &event.text {
                Some(t) => v.extend_from_slice(t.as_bytes()),
                None => v.extend_from_slice(s.as_bytes()),
            }
            Some(v)
        }
        _ => event.text.as_ref().map(|t| t.as_bytes().to_vec()),
    }
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .expect("EventLoop 생성 실패");
    let proxy = event_loop.create_proxy();
    let mut app = App { proxy, state: None };
    event_loop.run_app(&mut app).expect("run_app 실패");
}
