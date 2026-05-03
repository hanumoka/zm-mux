mod notify;
mod selection;

use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Mutex};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CursorIcon, Window, WindowId};

use selection::{CellCoord, SelectionState};

use zm_core::{
    Config, KeyBindingsConfig, KeyDef, ModBits, ParsedKeyBindings, ShellConfig,
};
use zm_agent::{AgentInfo, AgentStatus, AgentType};
use zm_mux::{BorderHit, PaneId, SplitDirection, TabId, TabSet};
use zm_pty::ZmPtyProcess;
use zm_render::{
    HighlightCell, PaneRenderInfo, Rect, Renderer, TAB_BAR_HEIGHT_PX, TabBarInfo, TabLabel,
    create_renderer,
};
use zm_socket::mux_api::MuxHandler;
use zm_socket::mux_api::types::{
    ClosePaneParams, ClosePaneResult, CloseTabParams, CloseTabResult, CreateTabParams,
    CreateTabResult, FocusPaneParams, FocusPaneResult, GetStatusParams, GetStatusResult,
    ListPanesParams, ListPanesResult, MuxMethod, PaneInfo, SendKeysParams, SendKeysResult,
    SetAgentInfoParams, SetAgentInfoResult, SplitPaneParams, SplitPaneResult, PANE_NOT_FOUND,
};
use zm_socket::rpc::RpcError;
use zm_term::{CellColor, OscEvent, ZmTerm};

use crate::notify::NotifyDispatcher;

const INITIAL_COLS: u16 = 80;
const INITIAL_ROWS: u16 = 24;

struct PaneState {
    term: ZmTerm,
    pty: ZmPtyProcess,
    agent_info: AgentInfo,
    worktree_path: Option<String>,
}

struct MuxState {
    tabs: TabSet,
    panes: HashMap<PaneId, PaneState>,
    dirty: bool,
    shell_cfg: ShellConfig,
    scrollback_lines: usize,
    default_fg: CellColor,
    default_bg: CellColor,
    cell_width: usize,
    cell_height: usize,
    pane_area_width: usize,
    pane_area_height: usize,
    workspace_id: String,
    socket_name: String,
}

impl MuxState {
    fn new(
        shell_cfg: ShellConfig,
        scrollback_lines: usize,
        default_fg: CellColor,
        default_bg: CellColor,
        workspace_id: String,
        socket_name: String,
    ) -> Self {
        let (tabs, initial_pane) = TabSet::new();
        let mut panes = HashMap::new();
        let env_vars = Self::pane_env_vars_owned(&workspace_id, initial_pane, &socket_name);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let pane = make_pane(
            INITIAL_COLS,
            INITIAL_ROWS,
            &shell_cfg,
            scrollback_lines,
            default_fg,
            default_bg,
            &env_refs,
            None,
        );
        panes.insert(initial_pane, pane);

        Self {
            tabs,
            panes,
            dirty: true,
            shell_cfg,
            scrollback_lines,
            default_fg,
            default_bg,
            cell_width: 0,
            cell_height: 0,
            pane_area_width: 0,
            pane_area_height: 0,
            workspace_id,
            socket_name,
        }
    }

    fn pane_env_vars_owned(workspace_id: &str, pane_id: PaneId, socket_name: &str) -> Vec<(String, String)> {
        vec![
            ("ZM_MUX_WORKSPACE_ID".into(), workspace_id.into()),
            ("ZM_MUX_SURFACE_ID".into(), pane_id.0.to_string()),
            ("ZM_MUX_SOCKET_PATH".into(), socket_name.into()),
        ]
    }

    fn make_pane(&self, cols: u16, rows: u16, pane_id: PaneId, cwd: Option<&str>) -> PaneState {
        let env_vars = Self::pane_env_vars_owned(&self.workspace_id, pane_id, &self.socket_name);
        let env_refs: Vec<(&str, &str)> = env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        make_pane(
            cols,
            rows,
            &self.shell_cfg,
            self.scrollback_lines,
            self.default_fg,
            self.default_bg,
            &env_refs,
            cwd,
        )
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
            let pane = self.make_pane(cols, rows, new_id, None);
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

    fn create_new_tab(
        &mut self,
        renderer: &dyn Renderer,
        win_width: usize,
        win_height: usize,
    ) -> PaneId {
        let (_tab_id, initial_pane) = self.tabs.create_tab();
        let (cols, rows) = renderer.cols_rows_for_size(win_width, win_height);
        let pane = self.make_pane(cols, rows, initial_pane, None);
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

    fn cached_cols_rows(&self, width: usize, height: usize) -> (u16, u16) {
        if self.cell_width == 0 || self.cell_height == 0 {
            return (INITIAL_COLS, INITIAL_ROWS);
        }
        let avail_h = height.saturating_sub(TAB_BAR_HEIGHT_PX as usize);
        (
            (width / self.cell_width).max(1) as u16,
            (avail_h / self.cell_height).max(1) as u16,
        )
    }

    fn resize_all_panes_cached(&mut self) {
        let cw = self.cell_width;
        let ch = self.cell_height;
        if cw == 0 || ch == 0 {
            return;
        }
        let w = self.pane_area_width;
        let h = self.pane_area_height;
        let all_layouts: Vec<(PaneId, zm_mux::Rect)> = self
            .tabs
            .tabs()
            .iter()
            .flat_map(|tab| tab.tree.layout(w, h))
            .collect();
        for (id, rect) in all_layouts {
            if let Some(ps) = self.panes.get_mut(&id) {
                let avail_h = rect.height.saturating_sub(TAB_BAR_HEIGHT_PX as usize);
                let cols = (rect.width / cw).max(1) as u16;
                let rows = (avail_h / ch).max(1) as u16;
                if cols > 0 && rows > 0 {
                    ps.term.resize(cols, rows);
                    ps.pty.resize(rows, cols).ok();
                }
            }
        }
    }

    fn split_pane_by_id(
        &mut self,
        target: PaneId,
        direction: SplitDirection,
        cwd: Option<&str>,
    ) -> Option<PaneId> {
        let new_id = self.tabs.alloc_pane_id();
        let tab = self.tabs.tab_containing_pane_mut(target)?;
        if !tab.tree.split(target, direction, new_id) {
            return None;
        }
        let w = self.pane_area_width;
        let h = self.pane_area_height;
        let layout = tab.tree.layout(w, h);
        let (_, rect) = layout.iter().find(|(id, _)| *id == new_id)?;
        let (cols, rows) = self.cached_cols_rows(rect.width, rect.height);
        let pane = self.make_pane(cols, rows, new_id, cwd);
        self.panes.insert(new_id, pane);
        self.resize_all_panes_cached();
        self.dirty = true;
        Some(new_id)
    }

    fn close_pane_by_id(&mut self, target: PaneId) -> bool {
        let tab = match self.tabs.tab_containing_pane_mut(target) {
            Some(t) => t,
            None => return false,
        };
        if tab.tree.pane_count() <= 1 {
            return false;
        }
        if tab.focused_pane == target {
            let ids = tab.tree.pane_ids();
            tab.focused_pane = ids
                .iter()
                .find(|id| **id != target)
                .copied()
                .unwrap_or(target);
        }
        if !tab.tree.remove(target) {
            return false;
        }
        if let Some(mut ps) = self.panes.remove(&target) {
            ps.pty.kill().ok();
            if let Some(wt_path) = &ps.worktree_path {
                let wt = std::path::PathBuf::from(wt_path);
                if let Some(root) = wt.parent().and_then(|p| p.parent()) {
                    let _ = zm_agent::worktree::remove_worktree(root, &wt);
                }
            }
        }
        self.dirty = true;
        true
    }

    fn create_tab_cached(&mut self) -> (zm_mux::TabId, PaneId) {
        let (tab_id, initial_pane) = self.tabs.create_tab();
        let w = self.pane_area_width;
        let h = self.pane_area_height;
        let (cols, rows) = self.cached_cols_rows(w, h);
        let pane = self.make_pane(cols, rows, initial_pane, None);
        self.panes.insert(initial_pane, pane);
        self.dirty = true;
        (tab_id, initial_pane)
    }

    fn close_tab_by_id(&mut self, target: zm_mux::TabId) -> bool {
        let removed = self.tabs.close_by_id(target);
        if removed.is_empty() {
            return false;
        }
        for id in removed {
            if let Some(mut ps) = self.panes.remove(&id) {
                ps.pty.kill().ok();
            }
        }
        self.dirty = true;
        true
    }
}

fn make_pane(
    cols: u16,
    rows: u16,
    shell_cfg: &ShellConfig,
    scrollback_lines: usize,
    default_fg: CellColor,
    default_bg: CellColor,
    env_vars: &[(&str, &str)],
    cwd: Option<&str>,
) -> PaneState {
    let pty = zm_pty::spawn_pty(rows, cols, shell_cfg, env_vars, cwd).expect("PTY spawn");
    let term = ZmTerm::new(cols, rows, scrollback_lines, default_fg, default_bg)
        .expect("term init");
    PaneState { term, pty, agent_info: AgentInfo::default(), worktree_path: cwd.map(String::from) }
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
            let mut s = state_clone.lock().unwrap();
            if let Some(ps) = s.panes.get_mut(&pane_id) {
                if ps.agent_info.agent_type != AgentType::Unknown {
                    ps.agent_info.status = AgentStatus::Complete;
                    s.dirty = true;
                }
            }
        });
    }
}

// ---- Socket API handler (Phase 2.2) ----------------------------------------

struct AppMuxHandler {
    state: Arc<Mutex<MuxState>>,
    socket_name: String,
}

impl MuxHandler for AppMuxHandler {
    fn handle_list_panes(&self, _params: ListPanesParams) -> Result<ListPanesResult, RpcError> {
        let state = self.state.lock().expect("MuxState lock");
        let mut panes = Vec::new();
        for tab in state.tabs.tabs() {
            let focused = tab.focused_pane;
            for pane_id in tab.tree.pane_ids() {
                let ps = state.panes.get(&pane_id);
                let (cols, rows) = ps
                    .map(|p| (p.term.cols() as u16, p.term.rows() as u16))
                    .unwrap_or((0, 0));
                let (at, as_) = ps
                    .map(|p| (p.agent_info.agent_type.as_str(), p.agent_info.status.as_str()))
                    .unwrap_or(("unknown", "unknown"));
                panes.push(PaneInfo {
                    pane_id: pane_id.0,
                    tab_id: tab.id.0,
                    focused: pane_id == focused && tab.id == state.tabs.active_id(),
                    cols,
                    rows,
                    title: tab.title.clone().unwrap_or_default(),
                    agent_type: at.to_string(),
                    agent_status: as_.to_string(),
                    worktree_path: ps.and_then(|p| p.worktree_path.clone()),
                });
            }
        }
        Ok(ListPanesResult { panes })
    }

    fn handle_get_status(&self, _params: GetStatusParams) -> Result<GetStatusResult, RpcError> {
        let state = self.state.lock().expect("MuxState lock");
        Ok(GetStatusResult {
            workspace_id: state.workspace_id.clone(),
            pid: std::process::id(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            active_tab: state.tabs.active_id().0,
            pane_count: state.panes.len(),
            tab_count: state.tabs.tab_count(),
            socket_path: self.socket_name.clone(),
        })
    }

    fn handle_send_keys(&self, params: SendKeysParams) -> Result<SendKeysResult, RpcError> {
        let mut state = self.state.lock().expect("MuxState lock");
        let target = PaneId(params.pane_id);
        let ps = state.panes.get_mut(&target).ok_or_else(|| {
            RpcError::new(PANE_NOT_FOUND, format!("pane {} not found", params.pane_id))
        })?;
        ps.pty
            .write_input(params.data.as_bytes())
            .map_err(|e| RpcError::new(RpcError::INTERNAL_ERROR, e.to_string()))?;
        state.dirty = true;
        Ok(SendKeysResult {})
    }

    fn handle_focus_pane(&self, params: FocusPaneParams) -> Result<FocusPaneResult, RpcError> {
        let mut state = self.state.lock().expect("MuxState lock");
        let target = PaneId(params.pane_id);
        if !state.panes.contains_key(&target) {
            return Err(RpcError::new(PANE_NOT_FOUND, format!("pane {} not found", params.pane_id)));
        }
        let tab = state.tabs.tabs().iter().find(|t| t.tree.pane_ids().contains(&target));
        let tab_id = match tab {
            Some(t) => t.id,
            None => return Err(RpcError::new(PANE_NOT_FOUND, "pane not in any tab")),
        };
        state.tabs.switch_to(tab_id);
        state.tabs.active_mut().focused_pane = target;
        state.dirty = true;
        Ok(FocusPaneResult {})
    }

    fn handle_split_pane(&self, params: SplitPaneParams) -> Result<SplitPaneResult, RpcError> {
        use zm_socket::mux_api::types::SPLIT_FAILED;
        let direction = match params.direction.as_str() {
            "horizontal" => SplitDirection::Horizontal,
            "vertical" => SplitDirection::Vertical,
            _ => return Err(RpcError::new(
                RpcError::INVALID_PARAMS,
                "direction must be \"horizontal\" or \"vertical\"",
            )),
        };
        let new_id = {
            let mut state = self.state.lock().expect("MuxState lock");
            state
                .split_pane_by_id(PaneId(params.pane_id), direction, params.cwd.as_deref())
                .ok_or_else(|| RpcError::new(SPLIT_FAILED, "split failed"))?
        };
        start_reader(new_id, &self.state);
        Ok(SplitPaneResult { new_pane_id: new_id.0 })
    }

    fn handle_close_pane(&self, params: ClosePaneParams) -> Result<ClosePaneResult, RpcError> {
        use zm_socket::mux_api::types::CANNOT_CLOSE_LAST_PANE;
        let mut state = self.state.lock().expect("MuxState lock");
        if !state.panes.contains_key(&PaneId(params.pane_id)) {
            return Err(RpcError::new(PANE_NOT_FOUND, format!("pane {} not found", params.pane_id)));
        }
        if !state.close_pane_by_id(PaneId(params.pane_id)) {
            return Err(RpcError::new(CANNOT_CLOSE_LAST_PANE, "cannot close the last pane in a tab"));
        }
        Ok(ClosePaneResult {})
    }

    fn handle_create_tab(&self, _params: CreateTabParams) -> Result<CreateTabResult, RpcError> {
        let (tab_id, pane_id) = {
            let mut state = self.state.lock().expect("MuxState lock");
            state.create_tab_cached()
        };
        start_reader(pane_id, &self.state);
        Ok(CreateTabResult { tab_id: tab_id.0, pane_id: pane_id.0 })
    }

    fn handle_close_tab(&self, params: CloseTabParams) -> Result<CloseTabResult, RpcError> {
        use zm_socket::mux_api::types::{TAB_NOT_FOUND, CANNOT_CLOSE_LAST_TAB};
        let mut state = self.state.lock().expect("MuxState lock");
        let target = TabId(params.tab_id);
        if !state.tabs.tabs().iter().any(|t| t.id == target) {
            return Err(RpcError::new(TAB_NOT_FOUND, format!("tab {} not found", params.tab_id)));
        }
        if !state.close_tab_by_id(target) {
            return Err(RpcError::new(CANNOT_CLOSE_LAST_TAB, "cannot close the last tab"));
        }
        Ok(CloseTabResult {})
    }

    fn handle_set_agent_info(&self, params: SetAgentInfoParams) -> Result<SetAgentInfoResult, RpcError> {
        let mut state = self.state.lock().expect("MuxState lock");
        let target = PaneId(params.pane_id);
        let ps = state.panes.get_mut(&target).ok_or_else(|| {
            RpcError::new(PANE_NOT_FOUND, format!("pane {} not found", params.pane_id))
        })?;
        if let Some(t) = &params.agent_type {
            ps.agent_info.agent_type = AgentType::parse(t);
        }
        if let Some(s) = &params.agent_status {
            ps.agent_info.status = AgentStatus::parse(s);
        }
        state.dirty = true;
        Ok(SetAgentInfoResult {})
    }
}

fn socket_name_for_pid() -> String {
    format!("zm-mux-{}", std::process::id())
}

fn start_socket_server(state: Arc<Mutex<MuxState>>, socket_name: String) {
    let handler = Arc::new(AppMuxHandler {
        state,
        socket_name: socket_name.clone(),
    });
    let server = zm_socket::transport_sync::MuxServer::new(handler, &socket_name);
    std::thread::spawn(move || {
        if let Err(e) = server.serve_forever() {
            eprintln!("zm-mux: socket server error: {e}");
        }
    });
}

/// Convert winit modifier flags into zm-core's backend-agnostic ModBits so
/// keybinding matching stays winit-free.
fn winit_mods_to_bits(m: ModifiersState) -> ModBits {
    let mut b = ModBits::empty();
    if m.contains(ModifiersState::CONTROL) {
        b |= ModBits::CTRL;
    }
    if m.contains(ModifiersState::SHIFT) {
        b |= ModBits::SHIFT;
    }
    if m.contains(ModifiersState::ALT) {
        b |= ModBits::ALT;
    }
    if m.contains(ModifiersState::SUPER) {
        b |= ModBits::SUPER;
    }
    b
}

/// Encode `ModBits` as a Kitty keyboard-protocol modifier mask (CSI u).
/// Spec: shift=1, alt=2, ctrl=4, super=8; the wire value is `mask + 1`
/// (the +1 distinguishes "no modifiers" = 1 from an omitted field).
///
/// portable-pty 0.9 already sets `PSEUDOCONSOLE_WIN32_INPUT_MODE` on
/// every ConPTY it creates, so on Windows the kernel side is ready to
/// receive these sequences without any extra setup on our end.
fn kitty_modifier_mask(mods: ModBits) -> u8 {
    let mut k = 0u8;
    if mods.contains(ModBits::SHIFT) {
        k |= 1;
    }
    if mods.contains(ModBits::ALT) {
        k |= 2;
    }
    if mods.contains(ModBits::CTRL) {
        k |= 4;
    }
    if mods.contains(ModBits::SUPER) {
        k |= 8;
    }
    k + 1
}

/// Map a winit logical key to the keybinding-friendly subset.  Returns None
/// for keys we never bind (function keys, media keys, dead keys, etc.).
fn winit_key_to_def(key: &Key) -> Option<KeyDef> {
    match key {
        Key::Character(s) => s.chars().next().map(KeyDef::Char),
        Key::Named(n) => Some(match n {
            NamedKey::Tab => KeyDef::Tab,
            NamedKey::Enter => KeyDef::Enter,
            NamedKey::Escape => KeyDef::Escape,
            NamedKey::Backspace => KeyDef::Backspace,
            NamedKey::Space => KeyDef::Space,
            NamedKey::PageUp => KeyDef::PageUp,
            NamedKey::PageDown => KeyDef::PageDown,
            NamedKey::Home => KeyDef::Home,
            NamedKey::End => KeyDef::End,
            NamedKey::ArrowUp => KeyDef::ArrowUp,
            NamedKey::ArrowDown => KeyDef::ArrowDown,
            NamedKey::ArrowLeft => KeyDef::ArrowLeft,
            NamedKey::ArrowRight => KeyDef::ArrowRight,
            _ => return None,
        }),
        // Unidentified / Dead keys (IME composition state, raw scancodes
        // we don't recognize) cannot drive bindings.
        _ => None,
    }
}

/// Search mode state.  Active = the focused pane's input is hijacked by
/// the search dispatcher (Esc exits, n/N walk matches, printable chars
/// extend the query, Backspace shrinks it).  `matches` is recomputed
/// every time the query changes; it is purely a viewport overlay (no
/// scrollback search yet).  `current` is the highlighted match index;
/// for now we render every match the same color and rely on the user
/// to scan visually -- per-match emphasis is a follow-up.
#[derive(Default)]
struct SearchState {
    active: bool,
    query: String,
    matches: Vec<HighlightCell>,
    current: usize,
}

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Box<dyn Renderer>>,
    state: Arc<Mutex<MuxState>>,
    modifiers: ModifiersState,
    config: Config,
    keybindings: ParsedKeyBindings,
    notifier: NotifyDispatcher,
    notify_gc_counter: u32,
    /// Active IME composition string for the focused pane.  None when no
    /// IME composition is in progress.  Rendered as an overlay at the
    /// cursor position so cell grid stays clean — only Commit touches PTY.
    ime_preedit: Option<String>,
    search: SearchState,
    selection: SelectionState,
    cursor_position: (f64, f64),
    last_click_time: std::time::Instant,
    click_count: u8,
    resize_drag: Option<ResizeDrag>,
    left_button_down: bool,
    last_mouse_cell: Option<(PaneId, usize, usize)>,
}

struct ResizeDrag {
    direction: SplitDirection,
    start_x: usize,
    start_y: usize,
}

impl App {
    fn new(
        state: Arc<Mutex<MuxState>>,
        config: Config,
        keybindings: ParsedKeyBindings,
    ) -> Self {
        Self {
            window: None,
            renderer: None,
            state,
            modifiers: ModifiersState::empty(),
            config,
            keybindings,
            notifier: NotifyDispatcher::new(),
            notify_gc_counter: 0,
            ime_preedit: None,
            search: SearchState::default(),
            selection: SelectionState::default(),
            cursor_position: (0.0, 0.0),
            last_click_time: std::time::Instant::now(),
            click_count: 0,
            resize_drag: None,
            left_button_down: false,
            last_mouse_cell: None,
        }
    }

    /// Re-run regex search against the focused pane's viewport and refresh
    /// `self.search.matches`.  Called whenever the query changes (char
    /// added/removed) or the user explicitly presses Enter.
    fn do_research(&mut self) {
        let raw = {
            let state = self.state.lock().unwrap();
            let focused = state.focused_pane();
            state
                .panes
                .get(&focused)
                .map(|ps| ps.term.search(&self.search.query))
                .unwrap_or_default()
        };
        self.search.matches = raw
            .into_iter()
            .map(|m| HighlightCell {
                row: m.row,
                col: m.col,
                len: m.len,
            })
            .collect();
        if self.search.matches.is_empty() || self.search.current >= self.search.matches.len() {
            self.search.current = 0;
        }
        self.state.lock().unwrap().dirty = true;
    }

    /// Cancel any in-flight IME composition.  Called whenever focus moves
    /// (pane or tab change) so a half-typed Korean syllable doesn't leak
    /// from the source pane to the destination.  The set_ime_allowed
    /// false→true cycle tells winit to release the current composing
    /// context to the OS and start a fresh one — winit emits a synthetic
    /// `Preedit("", None)` along the way which we observe as an empty
    /// preedit and clear our overlay.
    fn ime_cancel(&mut self) {
        self.ime_preedit = None;
        if let Some(w) = &self.window {
            w.set_ime_allowed(false);
            w.set_ime_allowed(true);
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
        let preedit_str = self.ime_preedit.as_deref();
        let active_highlights: &[HighlightCell] = if self.search.active {
            &self.search.matches
        } else {
            &[]
        };
        let sel_pane = self.selection.pane;
        let sel_highlights: Vec<HighlightCell> = if self.selection.is_active() {
            sel_pane
                .and_then(|pid| state.panes.get(&pid))
                .map(|ps| self.selection.to_highlights(ps.term.cols()))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let pane_infos: Vec<PaneRenderInfo> = layouts
            .iter()
            .filter_map(|(id, mux_rect)| {
                state.panes.get(id).map(|ps| {
                    let is_focused = *id == focused;
                    PaneRenderInfo {
                        term: &ps.term,
                        rect: Rect {
                            x: mux_rect.x,
                            y: mux_rect.y + TAB_BAR_HEIGHT_PX as usize,
                            width: mux_rect.width,
                            height: mux_rect.height,
                        },
                        focused: is_focused,
                        ime_preedit: if is_focused { preedit_str } else { None },
                        highlights: if is_focused { active_highlights } else { &[] },
                        selection_highlights: if Some(*id) == sel_pane {
                            &sel_highlights
                        } else {
                            &[]
                        },
                        border_color_srgb: ps.agent_info.border_color_srgb(is_focused),
                    }
                })
            })
            .collect();

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

    /// Helper to dedupe split keybinding handlers (Ctrl+Shift+D / E).
    /// Spawns a reader for any pane that newly has one.
    fn do_split(&mut self, direction: SplitDirection) {
        let (w, h) = self.pane_area_size();
        let Some(renderer) = self.renderer_ref() else {
            return;
        };
        let new_ids: Vec<PaneId> = {
            let mut state = self.state.lock().unwrap();
            state.split(direction, renderer, w, h);
            state.tabs.active().tree.pane_ids()
        };
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
    }

    /// Helper to dedupe focus-direction handlers (Alt+Arrow*).
    fn do_focus(&mut self, direction: SplitDirection, forward: bool) {
        let (w, h) = self.pane_area_size();
        let moved;
        {
            let mut state = self.state.lock().unwrap();
            let focused = state.focused_pane();
            moved = {
                let tab = state.tabs.active();
                tab.tree.find_adjacent(focused, direction, forward, w, h)
            };
            if let Some(new_focus) = moved {
                state.tabs.active_mut().focused_pane = new_focus;
                state.dirty = true;
            }
        }
        if moved.is_some() {
            self.ime_cancel();
            self.selection.clear();
        }
    }

    fn hit_test(&self, px_x: f64, px_y: f64) -> Option<(PaneId, usize, usize)> {
        let renderer = self.renderer.as_deref()?;
        let (cell_w, cell_h) = renderer.cell_size();
        if cell_w == 0 || cell_h == 0 {
            return None;
        }
        let (w, h) = self.pane_area_size();
        let state = self.state.lock().unwrap();
        let layouts = state.tabs.active().tree.layout(w, h);

        let pane_area_y = px_y - TAB_BAR_HEIGHT_PX as f64;
        if pane_area_y < 0.0 {
            return None;
        }

        for (id, mux_rect) in &layouts {
            let rx = mux_rect.x as f64;
            let ry = mux_rect.y as f64;
            let rw = mux_rect.width as f64;
            let rh = mux_rect.height as f64;
            if px_x >= rx && px_x < rx + rw && pane_area_y >= ry && pane_area_y < ry + rh {
                let col = ((px_x - rx) / cell_w as f64).floor() as usize;
                let row = ((pane_area_y - ry) / cell_h as f64).floor() as usize;
                let ps = state.panes.get(id)?;
                let col = col.min(ps.term.cols().saturating_sub(1));
                let row = row.min(ps.term.rows().saturating_sub(1));
                let col = if ps.term.is_wide_spacer(row, col) && col > 0 {
                    col - 1
                } else {
                    col
                };
                return Some((*id, row, col));
            }
        }
        None
    }

    fn border_hit_test(&self, px_x: f64, px_y: f64) -> Option<BorderHit> {
        let pane_area_y = px_y - TAB_BAR_HEIGHT_PX as f64;
        if pane_area_y < 0.0 {
            return None;
        }
        let (w, h) = self.pane_area_size();
        let state = self.state.lock().unwrap();
        state.tabs.active().tree.border_hit_test(
            px_x as usize, pane_area_y as usize, w, h, 8,
        )
    }
}

fn sgr_mouse_seq(btn: u8, col: usize, row: usize, pressed: bool) -> Vec<u8> {
    let suffix = if pressed { 'M' } else { 'm' };
    format!("\x1b[<{btn};{};{}{suffix}", col + 1, row + 1).into_bytes()
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

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
            &self.config.colors,
        )
        .expect("renderer init");

        let (req_w, req_h) = renderer.required_size(INITIAL_COLS as usize, INITIAL_ROWS as usize);
        let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(req_w as u32, req_h as u32));

        // Enable IME early so users can start typing CJK before any focus
        // event (the very first pane is born focused).
        window.set_ime_allowed(true);

        self.window = Some(window.clone());
        self.renderer = Some(renderer);

        {
            let (cw, ch) = self.renderer.as_ref().unwrap().cell_size();
            let size = window.inner_size();
            let pa_w = size.width as usize;
            let pa_h = (size.height as usize).saturating_sub(TAB_BAR_HEIGHT_PX as usize);
            let mut state = self.state.lock().unwrap();
            state.cell_width = cw;
            state.cell_height = ch;
            state.pane_area_width = pa_w;
            state.pane_area_height = pa_h;
        }

        let initial = self.state.lock().unwrap().tabs.active().focused_pane;
        start_reader(initial, &self.state);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                {
                    let state = self.state.lock().unwrap();
                    let snap = zm_mux::session::SessionSnapshot::from_tab_set(&state.tabs, "autosave");
                    let path = zm_mux::session::sessions_dir().join("autosave.json");
                    if let Err(e) = snap.save_to_file(&path) {
                        eprintln!("zm-mux: session save failed: {e}");
                    }
                }
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }
            WindowEvent::Ime(ime) => match ime {
                // Enabled / Disabled are informational; we keep IME always on
                // when the window has focus and rely on focus-change toggles
                // to cancel composition.
                Ime::Enabled | Ime::Disabled => {}
                Ime::Preedit(text, _cursor) => {
                    self.ime_preedit = if text.is_empty() { None } else { Some(text) };
                    self.state.lock().unwrap().dirty = true;
                }
                Ime::Commit(text) => {
                    // winit guarantees a synthetic `Preedit("", None)` just
                    // before this, so our overlay is already clear.  We only
                    // need to push the committed bytes to the focused PTY
                    // exactly once; alacritty parses them as plain text.
                    self.ime_preedit = None;
                    let mut state = self.state.lock().unwrap();
                    let focused = state.focused_pane();
                    let mut snap_dirty = false;
                    if let Some(ps) = state.panes.get_mut(&focused) {
                        if ps.term.display_offset() != 0 {
                            ps.term.scroll_to_bottom();
                            snap_dirty = true;
                        }
                        let _ = ps.pty.write_input(text.as_bytes());
                    }
                    if snap_dirty {
                        state.dirty = true;
                    }
                }
            },
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = (position.x, position.y);

                // Priority 1: border resize drag
                if let Some(ref drag) = self.resize_drag {
                    let pane_y = position.y - TAB_BAR_HEIGHT_PX as f64;
                    let drag_pos = match drag.direction {
                        SplitDirection::Horizontal => position.x as usize,
                        SplitDirection::Vertical => pane_y.max(0.0) as usize,
                    };
                    let (w, h) = self.pane_area_size();
                    let (cell_w, cell_h) = self.renderer.as_deref()
                        .map(|r| r.cell_size()).unwrap_or((10, 20));
                    let min_px = match drag.direction {
                        SplitDirection::Horizontal => cell_w * 2,
                        SplitDirection::Vertical => cell_h * 2,
                    };
                    let start_x = drag.start_x;
                    let start_y = drag.start_y;
                    let mut state = self.state.lock().unwrap();
                    if state.tabs.active_mut().tree.adjust_border(
                        start_x, start_y, drag_pos, w, h, 500, min_px,
                    ) {
                        if let Some(renderer) = self.renderer.as_deref() {
                            state.resize_all_panes(renderer, w, h);
                        }
                        state.dirty = true;
                    }
                    if let Some(ref mut drag) = self.resize_drag {
                        match drag.direction {
                            SplitDirection::Horizontal => drag.start_x = drag_pos,
                            SplitDirection::Vertical => drag.start_y = drag_pos,
                        }
                    }
                    return;
                }

                // Priority 2: selection drag
                if self.selection.dragging {
                    if let Some((pane_id, row, col)) = self.hit_test(position.x, position.y) {
                        if self.selection.pane == Some(pane_id) {
                            self.selection.extend(CellCoord { row, col });
                            self.state.lock().unwrap().dirty = true;
                        }
                    }
                    return;
                }

                // Priority 3: mouse motion forwarding to PTY
                if self.left_button_down || {
                    let state = self.state.lock().unwrap();
                    let focused = state.focused_pane();
                    state.panes.get(&focused).map(|ps| ps.term.is_mouse_motion()).unwrap_or(false)
                } {
                    if let Some((pane_id, row, col)) = self.hit_test(position.x, position.y) {
                        if self.last_mouse_cell != Some((pane_id, row, col)) {
                            self.last_mouse_cell = Some((pane_id, row, col));
                            let mut state = self.state.lock().unwrap();
                            let focused = state.focused_pane();
                            if pane_id == focused {
                                if let Some(ps) = state.panes.get_mut(&focused) {
                                    if ps.term.is_mouse_enabled() {
                                        let btn = if self.left_button_down { 32u8 } else { 35 };
                                        let seq = sgr_mouse_seq(btn, col, row, true);
                                        let _ = ps.pty.write_input(&seq);
                                    }
                                }
                            }
                        }
                    }
                    return;
                }

                // Priority 4: border hover cursor
                if let Some(hit) = self.border_hit_test(position.x, position.y) {
                    let icon = match hit.direction {
                        SplitDirection::Horizontal => CursorIcon::ColResize,
                        SplitDirection::Vertical => CursorIcon::RowResize,
                    };
                    if let Some(w) = &self.window {
                        w.set_cursor(icon);
                    }
                } else if let Some(w) = &self.window {
                    w.set_cursor(CursorIcon::Default);
                }
            }
            WindowEvent::MouseInput {
                state: button_state,
                button,
                ..
            } => {
                let is_left = button == MouseButton::Left;
                if is_left {
                    match button_state {
                        ElementState::Pressed => self.left_button_down = true,
                        ElementState::Released => self.left_button_down = false,
                    }
                }

                let (px_x, px_y) = self.cursor_position;

                match button_state {
                    ElementState::Pressed => {
                        // Priority 1: border resize (left button only)
                        if is_left {
                            if let Some(hit) = self.border_hit_test(px_x, px_y) {
                                let pane_y = (px_y - TAB_BAR_HEIGHT_PX as f64).max(0.0);
                                self.resize_drag = Some(ResizeDrag {
                                    direction: hit.direction,
                                    start_x: px_x as usize,
                                    start_y: pane_y.max(0.0) as usize,
                                });
                                return;
                            }
                        }

                        // Priority 2: Shift held → bypass mouse tracking, do selection
                        let shift_held = self.modifiers.contains(ModifiersState::SHIFT);

                        // Priority 3: forward to PTY if mouse tracking active
                        if !shift_held {
                            let should_forward = {
                                let state = self.state.lock().unwrap();
                                let focused = state.focused_pane();
                                state.panes.get(&focused)
                                    .map(|ps| ps.term.is_mouse_enabled())
                                    .unwrap_or(false)
                            };
                            if should_forward {
                                if let Some((pane_id, row, col)) = self.hit_test(px_x, px_y) {
                                    let mut state = self.state.lock().unwrap();
                                    let focused = state.focused_pane();
                                    if pane_id == focused {
                                        if let Some(ps) = state.panes.get_mut(&focused) {
                                            let btn = match button {
                                                MouseButton::Left => 0u8,
                                                MouseButton::Middle => 1,
                                                MouseButton::Right => 2,
                                                _ => return,
                                            };
                                            let seq = sgr_mouse_seq(btn, col, row, true);
                                            let _ = ps.pty.write_input(&seq);
                                        }
                                    }
                                }
                                return;
                            }
                        }

                        // Priority 4: selection (left button only)
                        if is_left {
                            if let Some((pane_id, row, col)) = self.hit_test(px_x, px_y) {
                                let now = std::time::Instant::now();
                                let elapsed = now.duration_since(self.last_click_time);
                                if elapsed < std::time::Duration::from_millis(500) {
                                    self.click_count = (self.click_count % 3) + 1;
                                } else {
                                    self.click_count = 1;
                                }
                                self.last_click_time = now;

                                let coord = CellCoord { row, col };
                                match self.click_count {
                                    1 => self.selection.start(pane_id, coord),
                                    2 => {
                                        let chars: Vec<char> = {
                                            let state = self.state.lock().unwrap();
                                            state.panes.get(&pane_id)
                                                .map(|ps| {
                                                    (0..ps.term.cols())
                                                        .map(|c| {
                                                            let cell = ps.term.render_cell(row, c);
                                                            if cell.c == '\0' { ' ' } else { cell.c }
                                                        })
                                                        .collect()
                                                })
                                                .unwrap_or_default()
                                        };
                                        let total_cols = chars.len();
                                        self.selection.select_word(pane_id, coord, total_cols, |_, c| {
                                            chars.get(c).copied().unwrap_or(' ')
                                        });
                                    }
                                    3 => {
                                        let total_cols = {
                                            let state = self.state.lock().unwrap();
                                            state.panes.get(&pane_id)
                                                .map(|ps| ps.term.cols())
                                                .unwrap_or(80)
                                        };
                                        self.selection.select_line(pane_id, row, total_cols);
                                    }
                                    _ => {}
                                }
                                self.state.lock().unwrap().dirty = true;
                            }
                        }
                    }
                    ElementState::Released => {
                        if self.resize_drag.is_some() {
                            self.resize_drag = None;
                            return;
                        }
                        // Forward release to PTY if mouse tracking
                        if !self.modifiers.contains(ModifiersState::SHIFT) {
                            let should_forward = {
                                let state = self.state.lock().unwrap();
                                let focused = state.focused_pane();
                                state.panes.get(&focused)
                                    .map(|ps| ps.term.is_mouse_enabled())
                                    .unwrap_or(false)
                            };
                            if should_forward {
                                if let Some((pane_id, row, col)) = self.hit_test(px_x, px_y) {
                                    let mut state = self.state.lock().unwrap();
                                    let focused = state.focused_pane();
                                    if pane_id == focused {
                                        if let Some(ps) = state.panes.get_mut(&focused) {
                                            let btn = match button {
                                                MouseButton::Left => 0u8,
                                                MouseButton::Middle => 1,
                                                MouseButton::Right => 2,
                                                _ => return,
                                            };
                                            let seq = sgr_mouse_seq(btn, col, row, false);
                                            let _ = ps.pty.write_input(&seq);
                                        }
                                    }
                                }
                                return;
                            }
                        }
                        if self.selection.dragging {
                            self.selection.finalize();
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y.round() as i32 * 3,
                    MouseScrollDelta::PixelDelta(pos) => (pos.y / 16.0).round() as i32,
                };
                if lines == 0 {
                    return;
                }

                // Determine mode BEFORE locking for write
                let hit = self.hit_test(self.cursor_position.0, self.cursor_position.1);
                let mouse_mode = {
                    let state = self.state.lock().unwrap();
                    let focused = state.focused_pane();
                    state.panes.get(&focused).map(|ps| {
                        (ps.term.is_mouse_enabled(), ps.term.is_alt_screen() && ps.term.is_alternate_scroll())
                    }).unwrap_or((false, false))
                };

                let mut state = self.state.lock().unwrap();
                let focused = state.focused_pane();
                if let Some(ps) = state.panes.get_mut(&focused) {
                    if mouse_mode.0 {
                        let btn = if lines > 0 { 64u8 } else { 65 };
                        if let Some((pane_id, row, col)) = hit {
                            if pane_id == focused {
                                let count = lines.unsigned_abs();
                                for _ in 0..count.min(10) {
                                    let seq = sgr_mouse_seq(btn, col, row, true);
                                    let _ = ps.pty.write_input(&seq);
                                }
                            }
                        }
                    } else if mouse_mode.1 {
                        let key = if lines > 0 { b"\x1b[A" as &[u8] } else { b"\x1b[B" };
                        let count = lines.unsigned_abs();
                        for _ in 0..count.min(10) {
                            let _ = ps.pty.write_input(key);
                        }
                    } else {
                        self.selection.clear();
                        ps.term.scroll_lines(lines);
                        state.dirty = true;
                    }
                }
            }
            WindowEvent::Resized(_) => {
                self.selection.clear();
                let (w, h) = self.pane_area_size();
                if let Some(renderer) = self.renderer_ref() {
                    let mut state = self.state.lock().unwrap();
                    state.pane_area_width = w;
                    state.pane_area_height = h;
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

                let mods_bits = winit_mods_to_bits(self.modifiers);
                let key_def = winit_key_to_def(&event.logical_key);

                // Search mode hijacks every keystroke until Esc.  This
                // sits before the keybinding dispatcher so Ctrl+T etc.
                // cannot accidentally fire while the user is typing a
                // query.  Matches are recomputed live as the query grows
                // — no Enter required.
                if self.search.active {
                    match &event.logical_key {
                        Key::Named(NamedKey::Escape) => {
                            self.search.active = false;
                            self.search.query.clear();
                            self.search.matches.clear();
                            self.search.current = 0;
                            self.state.lock().unwrap().dirty = true;
                        }
                        Key::Named(NamedKey::Backspace) => {
                            self.search.query.pop();
                            self.do_research();
                        }
                        Key::Named(NamedKey::Enter) => {
                            self.do_research();
                        }
                        Key::Character(s) if mods_bits == ModBits::EMPTY => {
                            self.search.query.push_str(s);
                            self.do_research();
                        }
                        _ => {}
                    }
                    return;
                }

                // Scrollback navigation (Shift+PgUp/PgDn/Home/End) — kept
                // hardcoded; tightly coupled to scroll API and unlikely to be
                // user-rebound.
                if mods_bits == ModBits::SHIFT
                    && let Some(ref kd) = key_def
                {
                    let act = match kd {
                        KeyDef::PageUp => Some(0u8),
                        KeyDef::PageDown => Some(1),
                        KeyDef::Home => Some(2),
                        KeyDef::End => Some(3),
                        _ => None,
                    };
                    if let Some(a) = act {
                        let mut state = self.state.lock().unwrap();
                        let focused = state.focused_pane();
                        if let Some(ps) = state.panes.get_mut(&focused) {
                            match a {
                                0 => ps.term.scroll_page_up(),
                                1 => ps.term.scroll_page_down(),
                                2 => ps.term.scroll_to_top(),
                                _ => ps.term.scroll_to_bottom(),
                            }
                            state.dirty = true;
                        }
                        return;
                    }
                }

                // Configurable keybindings.
                if let Some(ref kd) = key_def {
                    let kb = self.keybindings.clone();

                    if kb.new_tab.matches(mods_bits, kd) {
                        let (w, h) = self.pane_area_size();
                        let Some(renderer) = self.renderer_ref() else {
                            return;
                        };
                        let new_pane = {
                            let mut state = self.state.lock().unwrap();
                            state.create_new_tab(renderer, w, h)
                        };
                        start_reader(new_pane, &self.state);
                        self.ime_cancel();
                        self.selection.clear();
                        return;
                    }
                    if kb.close_tab.matches(mods_bits, kd) {
                        self.state.lock().unwrap().close_active_tab();
                        self.ime_cancel();
                        self.selection.clear();
                        return;
                    }
                    if kb.close_pane.matches(mods_bits, kd) {
                        self.state.lock().unwrap().close_focused_pane();
                        self.ime_cancel();
                        return;
                    }
                    if kb.split_horizontal.matches(mods_bits, kd) {
                        self.do_split(SplitDirection::Horizontal);
                        return;
                    }
                    if kb.split_vertical.matches(mods_bits, kd) {
                        self.do_split(SplitDirection::Vertical);
                        return;
                    }
                    if kb.next_tab.matches(mods_bits, kd) {
                        self.state.lock().unwrap().switch_tab_next();
                        self.ime_cancel();
                        self.selection.clear();
                        return;
                    }
                    if kb.prev_tab.matches(mods_bits, kd) {
                        self.state.lock().unwrap().switch_tab_prev();
                        self.ime_cancel();
                        self.selection.clear();
                        return;
                    }
                    if kb.focus_left.matches(mods_bits, kd) {
                        self.do_focus(SplitDirection::Horizontal, false);
                        return;
                    }
                    if kb.focus_right.matches(mods_bits, kd) {
                        self.do_focus(SplitDirection::Horizontal, true);
                        return;
                    }
                    if kb.focus_up.matches(mods_bits, kd) {
                        self.do_focus(SplitDirection::Vertical, false);
                        return;
                    }
                    if kb.focus_down.matches(mods_bits, kd) {
                        self.do_focus(SplitDirection::Vertical, true);
                        return;
                    }
                    if kb.search.matches(mods_bits, kd) {
                        // Cancel any in-flight IME composition so a half-typed
                        // syllable doesn't bleed into the search query overlay.
                        self.ime_cancel();
                        self.search.active = true;
                        self.search.query.clear();
                        self.search.matches.clear();
                        self.search.current = 0;
                        self.state.lock().unwrap().dirty = true;
                        return;
                    }
                    if kb.copy.matches(mods_bits, kd) {
                        if self.selection.is_active() {
                            if let Some(sel_pane) = self.selection.pane {
                                let (start, end) = self.selection.ordered();
                                let text = {
                                    let state = self.state.lock().unwrap();
                                    state
                                        .panes
                                        .get(&sel_pane)
                                        .map(|ps| {
                                            ps.term
                                                .extract_text(start.row, start.col, end.row, end.col)
                                        })
                                        .unwrap_or_default()
                                };
                                if !text.is_empty() {
                                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                        let _ = clipboard.set_text(&text);
                                    }
                                }
                            }
                            self.selection.clear();
                            self.state.lock().unwrap().dirty = true;
                        }
                        return;
                    }
                    if kb.paste.matches(mods_bits, kd) {
                        if let Ok(mut clipboard) = arboard::Clipboard::new() {
                            if let Ok(text) = clipboard.get_text() {
                                if !text.is_empty() {
                                    let mut state = self.state.lock().unwrap();
                                    let focused = state.focused_pane();
                                    if let Some(ps) = state.panes.get_mut(&focused) {
                                        let _ = ps.pty.write_input(b"\x1b[200~");
                                        let _ = ps.pty.write_input(text.as_bytes());
                                        let _ = ps.pty.write_input(b"\x1b[201~");
                                    }
                                }
                            }
                        }
                        self.selection.clear();
                        return;
                    }
                    if kb.select_all.matches(mods_bits, kd) {
                        let (rows, cols, focused) = {
                            let state = self.state.lock().unwrap();
                            let f = state.focused_pane();
                            state
                                .panes
                                .get(&f)
                                .map(|ps| (ps.term.rows(), ps.term.cols(), f))
                                .unwrap_or((0, 0, f))
                        };
                        if rows > 0 && cols > 0 {
                            self.selection.pane = Some(focused);
                            self.selection.anchor = CellCoord { row: 0, col: 0 };
                            self.selection.extent = CellCoord {
                                row: rows.saturating_sub(1),
                                col: cols.saturating_sub(1),
                            };
                            self.selection.dragging = false;
                            self.state.lock().unwrap().dirty = true;
                        }
                        return;
                    }

                    // Ctrl+1..9 — direct tab index, kept out of config.
                    if mods_bits == ModBits::CTRL
                        && let KeyDef::Char(c) = kd
                        && let Some(d) = c.to_digit(10)
                        && (1..=9).contains(&d)
                    {
                        self.state
                            .lock()
                            .unwrap()
                            .switch_tab_to_index((d - 1) as usize);
                        self.ime_cancel();
                        return;
                    }
                }

                self.selection.clear();

                // Plain key input → focused pane in active tab.
                let bytes: Vec<u8> = match &event.logical_key {
                    Key::Character(s) => s.as_bytes().to_vec(),
                    Key::Named(NamedKey::Enter) => {
                        // Bare Enter → plain CR (every terminal app understands it).
                        // Modifier+Enter → Kitty CSI u sequence so apps that
                        // negotiate the protocol (claude code, neovim with
                        // unicode-keyboard, modern editors) can distinguish
                        // Shift/Ctrl/Alt/Super + Enter from a plain newline.
                        // Apps that did NOT negotiate will print this as
                        // garbled text, which is the standard fallback.
                        if mods_bits == ModBits::EMPTY {
                            vec![b'\r']
                        } else {
                            let mask = kitty_modifier_mask(mods_bits);
                            format!("\x1b[13;{mask}u").into_bytes()
                        }
                    }
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
        // Drain OSC notification events from every pane and forward to the
        // throttled OS dispatcher.  Done here (the 16ms idle tick) rather
        // than inside the PTY reader thread so notify-rust is only ever
        // called from the main thread (Win toast API thread-affinity).
        let osc_events: Vec<OscEvent> = {
            let state = self.state.lock().unwrap();
            state
                .panes
                .values()
                .flat_map(|ps| ps.term.drain_osc_events())
                .collect()
        };
        for ev in &osc_events {
            self.notifier.dispatch(ev);
        }
        // Periodic dedup-table sweep — every ~16s (1000 ticks * 16ms).
        self.notify_gc_counter = self.notify_gc_counter.wrapping_add(1);
        if self.notify_gc_counter.is_multiple_of(1000) {
            self.notifier.gc();
        }

        let needs_redraw = self.state.lock().unwrap().dirty;
        if needs_redraw && let Some(w) = &self.window {
            w.request_redraw();
        }
        event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(16),
        ));
    }
}

// ---- CLI client mode --------------------------------------------------------

fn cli_main(args: &[String]) {
    let subcommand = args[1].as_str();
    let socket_name = match std::env::var("ZM_MUX_SOCKET_PATH") {
        Ok(s) => s,
        Err(_) => {
            eprintln!("error: ZM_MUX_SOCKET_PATH not set. Is zm-mux running?");
            std::process::exit(1);
        }
    };

    let mut client = match zm_socket::transport_sync::Client::connect(&socket_name) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: cannot connect to zm-mux at {socket_name}: {e}");
            std::process::exit(1);
        }
    };

    use zm_socket::rpc::{JSONRPC_VERSION, Request, RequestId, Response};

    match subcommand {
        "list" => {
            let req = Request {
                jsonrpc: JSONRPC_VERSION.to_string(),
                id: RequestId::Num(1),
                method: MuxMethod::ListPanes.as_str().to_string(),
                params: serde_json::json!({}),
            };
            match client.call(&req) {
                Ok((Response::Success(s), _)) => {
                    let result: ListPanesResult =
                        serde_json::from_value(s.result).expect("parse result");
                    println!("{:<8} {:<8} {:<8} {:<6} {:<6} TITLE",
                        "PANE_ID", "TAB_ID", "FOCUSED", "COLS", "ROWS");
                    for p in &result.panes {
                        println!("{:<8} {:<8} {:<8} {:<6} {:<6} {}",
                            p.pane_id,
                            p.tab_id,
                            if p.focused { "*" } else { "" },
                            p.cols,
                            p.rows,
                            p.title);
                    }
                }
                Ok((Response::Error(e), _)) => {
                    eprintln!("error: {} (code {})", e.error.message, e.error.code);
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
            }
        }
        "status" => {
            let req = Request {
                jsonrpc: JSONRPC_VERSION.to_string(),
                id: RequestId::Num(1),
                method: MuxMethod::GetStatus.as_str().to_string(),
                params: serde_json::json!({}),
            };
            match client.call(&req) {
                Ok((Response::Success(s), _)) => {
                    let result: GetStatusResult =
                        serde_json::from_value(s.result).expect("parse result");
                    println!("workspace:  {}", result.workspace_id);
                    println!("pid:        {}", result.pid);
                    println!("version:    {}", result.version);
                    println!("active_tab: {}", result.active_tab);
                    println!("panes:      {}", result.pane_count);
                    println!("tabs:       {}", result.tab_count);
                    println!("socket:     {}", result.socket_path);
                }
                Ok((Response::Error(e), _)) => {
                    eprintln!("error: {} (code {})", e.error.message, e.error.code);
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
            }
        }
        "send" => {
            if args.len() < 4 {
                eprintln!("usage: zm-mux send <pane_id> <text>");
                std::process::exit(1);
            }
            let pane_id: u32 = args[2].parse().unwrap_or_else(|_| {
                eprintln!("error: pane_id must be a number");
                std::process::exit(1);
            });
            let data = args[3..].join(" ");
            let req = Request {
                jsonrpc: JSONRPC_VERSION.to_string(),
                id: RequestId::Num(1),
                method: MuxMethod::SendKeys.as_str().to_string(),
                params: serde_json::to_value(SendKeysParams { pane_id, data }).unwrap(),
            };
            match client.call(&req) {
                Ok((Response::Success(_), _)) => println!("ok"),
                Ok((Response::Error(e), _)) => {
                    eprintln!("error: {} (code {})", e.error.message, e.error.code);
                    std::process::exit(1);
                }
                Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
            }
        }
        "focus" => {
            if args.len() < 3 {
                eprintln!("usage: zm-mux focus <pane_id>");
                std::process::exit(1);
            }
            let pane_id: u32 = args[2].parse().unwrap_or_else(|_| {
                eprintln!("error: pane_id must be a number");
                std::process::exit(1);
            });
            let req = Request {
                jsonrpc: JSONRPC_VERSION.to_string(),
                id: RequestId::Num(1),
                method: MuxMethod::FocusPane.as_str().to_string(),
                params: serde_json::to_value(FocusPaneParams { pane_id }).unwrap(),
            };
            match client.call(&req) {
                Ok((Response::Success(_), _)) => println!("ok"),
                Ok((Response::Error(e), _)) => {
                    eprintln!("error: {} (code {})", e.error.message, e.error.code);
                    std::process::exit(1);
                }
                Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
            }
        }
        "split" => {
            if args.len() < 4 {
                eprintln!("usage: zm-mux split <pane_id> <horizontal|vertical>");
                std::process::exit(1);
            }
            let pane_id: u32 = args[2].parse().unwrap_or_else(|_| {
                eprintln!("error: pane_id must be a number");
                std::process::exit(1);
            });
            let direction = args[3].clone();
            let req = Request {
                jsonrpc: JSONRPC_VERSION.to_string(),
                id: RequestId::Num(1),
                method: MuxMethod::SplitPane.as_str().to_string(),
                params: serde_json::to_value(SplitPaneParams { pane_id, direction, cwd: None }).unwrap(),
            };
            match client.call(&req) {
                Ok((Response::Success(s), _)) => {
                    let r: SplitPaneResult = serde_json::from_value(s.result).expect("parse");
                    println!("new_pane_id: {}", r.new_pane_id);
                }
                Ok((Response::Error(e), _)) => {
                    eprintln!("error: {} (code {})", e.error.message, e.error.code);
                    std::process::exit(1);
                }
                Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
            }
        }
        "close-pane" => {
            if args.len() < 3 {
                eprintln!("usage: zm-mux close-pane <pane_id>");
                std::process::exit(1);
            }
            let pane_id: u32 = args[2].parse().unwrap_or_else(|_| {
                eprintln!("error: pane_id must be a number");
                std::process::exit(1);
            });
            let req = Request {
                jsonrpc: JSONRPC_VERSION.to_string(),
                id: RequestId::Num(1),
                method: MuxMethod::ClosePane.as_str().to_string(),
                params: serde_json::to_value(ClosePaneParams { pane_id }).unwrap(),
            };
            match client.call(&req) {
                Ok((Response::Success(_), _)) => println!("ok"),
                Ok((Response::Error(e), _)) => {
                    eprintln!("error: {} (code {})", e.error.message, e.error.code);
                    std::process::exit(1);
                }
                Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
            }
        }
        "new-tab" => {
            let req = Request {
                jsonrpc: JSONRPC_VERSION.to_string(),
                id: RequestId::Num(1),
                method: MuxMethod::CreateTab.as_str().to_string(),
                params: serde_json::json!({}),
            };
            match client.call(&req) {
                Ok((Response::Success(s), _)) => {
                    let r: CreateTabResult = serde_json::from_value(s.result).expect("parse");
                    println!("tab_id: {}, pane_id: {}", r.tab_id, r.pane_id);
                }
                Ok((Response::Error(e), _)) => {
                    eprintln!("error: {} (code {})", e.error.message, e.error.code);
                    std::process::exit(1);
                }
                Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
            }
        }
        "close-tab" => {
            if args.len() < 3 {
                eprintln!("usage: zm-mux close-tab <tab_id>");
                std::process::exit(1);
            }
            let tab_id: u32 = args[2].parse().unwrap_or_else(|_| {
                eprintln!("error: tab_id must be a number");
                std::process::exit(1);
            });
            let req = Request {
                jsonrpc: JSONRPC_VERSION.to_string(),
                id: RequestId::Num(1),
                method: MuxMethod::CloseTab.as_str().to_string(),
                params: serde_json::to_value(CloseTabParams { tab_id }).unwrap(),
            };
            match client.call(&req) {
                Ok((Response::Success(_), _)) => println!("ok"),
                Ok((Response::Error(e), _)) => {
                    eprintln!("error: {} (code {})", e.error.message, e.error.code);
                    std::process::exit(1);
                }
                Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
            }
        }
        "agent-info" => {
            if args.len() < 3 {
                eprintln!("usage: zm-mux agent-info <pane_id> [type] [status]");
                eprintln!("  type:   claude | codex | gemini | unknown");
                eprintln!("  status: waiting | active | complete | error | unknown");
                std::process::exit(1);
            }
            let pane_id: u32 = args[2].parse().unwrap_or_else(|_| {
                eprintln!("error: pane_id must be a number");
                std::process::exit(1);
            });
            let agent_type = args.get(3).cloned();
            let agent_status = args.get(4).cloned();
            let req = Request {
                jsonrpc: JSONRPC_VERSION.to_string(),
                id: RequestId::Num(1),
                method: MuxMethod::SetAgentInfo.as_str().to_string(),
                params: serde_json::to_value(SetAgentInfoParams { pane_id, agent_type, agent_status }).unwrap(),
            };
            match client.call(&req) {
                Ok((Response::Success(_), _)) => println!("ok"),
                Ok((Response::Error(e), _)) => {
                    eprintln!("error: {} (code {})", e.error.message, e.error.code);
                    std::process::exit(1);
                }
                Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
            }
        }
        "launch" => {
            if args.len() < 3 {
                eprintln!("usage: zm-mux launch <agent1> [agent2] ...");
                eprintln!("  agents: claude, codex, gemini");
                eprintln!("  example: zm-mux launch claude codex");
                std::process::exit(1);
            }
            let agent_names: Vec<&str> = args[2..].iter().map(|s| s.as_str()).collect();
            let cwd = std::env::current_dir().unwrap_or_default();
            let git_root = zm_agent::worktree::detect_git_root(&cwd).ok();

            let mut launched = Vec::new();
            let mut last_pane: Option<u32> = None;

            for agent_name in &agent_names {
                let worktree = if let Some(ref root) = git_root {
                    match zm_agent::worktree::create_worktree(root, agent_name) {
                        Ok(wt) => Some(wt),
                        Err(e) => {
                            eprintln!("warning: worktree creation failed for {agent_name}: {e}");
                            None
                        }
                    }
                } else {
                    None
                };

                let target_pane = last_pane.unwrap_or(0);
                let cwd_str = worktree.as_ref().map(|wt| wt.path.to_string_lossy().to_string());
                let split_req = Request {
                    jsonrpc: JSONRPC_VERSION.to_string(),
                    id: RequestId::Num(1),
                    method: MuxMethod::SplitPane.as_str().to_string(),
                    params: serde_json::to_value(SplitPaneParams {
                        pane_id: target_pane,
                        direction: "horizontal".into(),
                        cwd: cwd_str.clone(),
                    }).unwrap(),
                };

                let new_pane_id = match client.call(&split_req) {
                    Ok((Response::Success(s), _)) => {
                        let r: SplitPaneResult = serde_json::from_value(s.result).expect("parse");
                        r.new_pane_id
                    }
                    Ok((Response::Error(e), _)) => {
                        eprintln!("error splitting for {agent_name}: {}", e.error.message);
                        break;
                    }
                    Err(e) => { eprintln!("error: {e}"); break; }
                };

                let info_req = Request {
                    jsonrpc: JSONRPC_VERSION.to_string(),
                    id: RequestId::Num(1),
                    method: MuxMethod::SetAgentInfo.as_str().to_string(),
                    params: serde_json::to_value(SetAgentInfoParams {
                        pane_id: new_pane_id,
                        agent_type: Some(agent_name.to_string()),
                        agent_status: Some("waiting".into()),
                    }).unwrap(),
                };
                let _ = client.call(&info_req);

                let cmd = format!("{agent_name}\r\n");
                let send_req = Request {
                    jsonrpc: JSONRPC_VERSION.to_string(),
                    id: RequestId::Num(1),
                    method: MuxMethod::SendKeys.as_str().to_string(),
                    params: serde_json::to_value(SendKeysParams {
                        pane_id: new_pane_id,
                        data: cmd,
                    }).unwrap(),
                };
                let _ = client.call(&send_req);

                launched.push((new_pane_id, *agent_name, cwd_str));
                last_pane = Some(new_pane_id);
            }

            println!("{:<8} {:<10} WORKTREE", "PANE_ID", "AGENT");
            for (pid, name, wt) in &launched {
                println!("{:<8} {:<10} {}", pid, name, wt.as_deref().unwrap_or("-"));
            }
        }
        "worktree" => {
            if args.len() < 3 {
                eprintln!("usage: zm-mux worktree <list|cleanup>");
                std::process::exit(1);
            }
            match args[2].as_str() {
                "list" => {
                    let cwd = std::env::current_dir().unwrap_or_default();
                    match zm_agent::worktree::detect_git_root(&cwd) {
                        Ok(root) => {
                            let wt_dir = root.join(".zm-worktrees");
                            if wt_dir.exists() {
                                for entry in std::fs::read_dir(&wt_dir).unwrap() {
                                    if let Ok(e) = entry {
                                        if e.path().is_dir() {
                                            println!("{}", e.path().display());
                                        }
                                    }
                                }
                            } else {
                                println!("no worktrees found");
                            }
                        }
                        Err(_) => eprintln!("not a git repository"),
                    }
                }
                "cleanup" => {
                    let cwd = std::env::current_dir().unwrap_or_default();
                    match zm_agent::worktree::detect_git_root(&cwd) {
                        Ok(root) => {
                            let wt_dir = root.join(".zm-worktrees");
                            if wt_dir.exists() {
                                let mut removed = 0;
                                for entry in std::fs::read_dir(&wt_dir).unwrap() {
                                    if let Ok(e) = entry {
                                        if e.path().is_dir() {
                                            match zm_agent::worktree::remove_worktree(&root, &e.path()) {
                                                Ok(()) => { removed += 1; }
                                                Err(err) => eprintln!("warning: {}: {err}", e.path().display()),
                                            }
                                        }
                                    }
                                }
                                println!("removed {removed} worktree(s)");
                            } else {
                                println!("no worktrees to clean");
                            }
                        }
                        Err(_) => eprintln!("not a git repository"),
                    }
                }
                other => {
                    eprintln!("unknown worktree subcommand: {other}");
                    eprintln!("usage: zm-mux worktree <list|cleanup>");
                    std::process::exit(1);
                }
            }
        }
        "save" => {
            let name = args.get(2).map(|s| s.as_str()).unwrap_or("default");
            let req = Request {
                jsonrpc: JSONRPC_VERSION.to_string(),
                id: RequestId::Num(1),
                method: MuxMethod::ListPanes.as_str().to_string(),
                params: serde_json::json!({}),
            };
            match client.call(&req) {
                Ok((Response::Success(_), _)) => {
                    use zm_mux::session::{SessionSnapshot, sessions_dir};
                    let status_req = Request {
                        jsonrpc: JSONRPC_VERSION.to_string(),
                        id: RequestId::Num(2),
                        method: MuxMethod::GetStatus.as_str().to_string(),
                        params: serde_json::json!({}),
                    };
                    if let Ok((Response::Success(status_resp), _)) = client.call(&status_req) {
                        let result: GetStatusResult =
                            serde_json::from_value(status_resp.result).expect("parse");
                        eprintln!("session save requires direct MuxState access (not yet via Socket API)");
                        eprintln!("panes: {}, tabs: {}", result.pane_count, result.tab_count);
                        eprintln!("hint: session save will be integrated into GUI mode in next iteration");
                    }
                }
                Ok((Response::Error(e), _)) => {
                    eprintln!("error: {}", e.error.message);
                    std::process::exit(1);
                }
                Err(e) => { eprintln!("error: {e}"); std::process::exit(1); }
            }
        }
        "sessions" => {
            use zm_mux::session::sessions_dir;
            let dir = sessions_dir();
            if !dir.exists() {
                println!("no saved sessions");
            } else {
                let mut found = false;
                for entry in std::fs::read_dir(&dir).unwrap() {
                    if let Ok(e) = entry {
                        if e.path().extension().is_some_and(|ext| ext == "json") {
                            let name = e.path().file_stem().unwrap().to_string_lossy().to_string();
                            if let Ok(meta) = e.metadata() {
                                let modified = meta.modified().ok()
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0);
                                println!("{:<20} (modified: {})", name, modified);
                            } else {
                                println!("{name}");
                            }
                            found = true;
                        }
                    }
                }
                if !found {
                    println!("no saved sessions");
                }
            }
        }
        "--version" | "-V" => {
            println!("zm-mux {}", env!("CARGO_PKG_VERSION"));
        }
        "--help" | "-h" | "help" => {
            println!("zm-mux — AI agent terminal multiplexer");
            println!();
            println!("USAGE:");
            println!("  zm-mux                                 Launch GUI");
            println!("  zm-mux list                            List panes");
            println!("  zm-mux status                          Show status");
            println!("  zm-mux send <pane_id> <text>           Send text to pane");
            println!("  zm-mux focus <pane_id>                 Focus pane");
            println!("  zm-mux split <pane_id> <h|v>           Split pane");
            println!("  zm-mux close-pane <pane_id>            Close pane");
            println!("  zm-mux new-tab                         Create tab");
            println!("  zm-mux close-tab <tab_id>              Close tab");
            println!("  zm-mux agent-info <id> [type] [status] Set agent info");
            println!("  zm-mux launch <agent1> [agent2] ...    Launch agents");
            println!("  zm-mux worktree list|cleanup            Manage worktrees");
            println!("  zm-mux save [name]                     Save session layout");
            println!("  zm-mux sessions                        List saved sessions");
            println!("  zm-mux --version                       Show version");
        }
        other => {
            eprintln!("unknown subcommand: {other}");
            eprintln!("run 'zm-mux --help' for usage");
            std::process::exit(1);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        cli_main(&args);
        return;
    }

    let config = Config::load();
    let parsed = match config.keybindings.parse() {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "config: keybindings parse error ({e}) — falling back to defaults"
            );
            KeyBindingsConfig::default()
                .parse()
                .expect("default keybindings always parse")
        }
    };

    let socket_name = socket_name_for_pid();
    // SAFETY: single-threaded here — no PTY or reader threads spawned yet.
    // Must be set before MuxState::new() so the initial pane's shell inherits it.
    unsafe { std::env::set_var("ZM_MUX_SOCKET_PATH", &socket_name) };

    let (fr, fg, fb) = config.colors.foreground_rgb();
    let (br, bg, bb) = config.colors.background_rgb();
    let default_fg = CellColor::new(fr, fg, fb);
    let default_bg = CellColor::new(br, bg, bb);

    let workspace_id = format!("zm-{}", std::process::id());
    let state = Arc::new(Mutex::new(MuxState::new(
        config.shell.clone(),
        config.scrollback.max_lines,
        default_fg,
        default_bg,
        workspace_id,
        socket_name.clone(),
    )));

    start_socket_server(Arc::clone(&state), socket_name.clone());
    eprintln!("zm-mux: socket API listening on {socket_name}");

    let event_loop = EventLoop::new().expect("event loop");
    let mut app = App::new(state, config, parsed);
    let _ = event_loop.run_app(&mut app);
}
