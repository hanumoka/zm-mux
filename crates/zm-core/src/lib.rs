//! zm-core — zm-mux의 OS 비의존 공유 타입.
//!
//! 렌더링 입력(`CellSnapshot`)을 **소유 데이터**로 정의해 zm-term(alacritty)·
//! zm-render(wgpu/glyphon) 간 외부 타입 누출을 차단한다. (docs/research/03 §2)

use std::fmt;

/// 터미널 그리드 크기(열·행). PtySize와 맞추기 위해 `u16`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridSize {
    pub cols: u16,
    pub rows: u16,
}

impl GridSize {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols: cols.max(1),
            rows: rows.max(1),
        }
    }

    /// 셀 개수(cols * rows).
    pub fn cell_count(self) -> usize {
        self.cols as usize * self.rows as usize
    }
}

/// 8비트 채널 RGBA 색. zm-term의 256색 팔레트로 이미 해석된 구체 색.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const BLACK: Rgba = Rgba::rgb(0, 0, 0);
    pub const WHITE: Rgba = Rgba::rgb(255, 255, 255);

    /// wgpu 클리어용 정규화 f64 (sRGB 바이트 그대로; 서피스는 sRGB 포맷 가정).
    pub fn to_wgpu_f64(self) -> [f64; 4] {
        [
            self.r as f64 / 255.0,
            self.g as f64 / 255.0,
            self.b as f64 / 255.0,
            self.a as f64 / 255.0,
        ]
    }
}

/// 한 셀의 렌더 정보. alacritty 타입과 완전 분리.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderCell {
    pub c: char,
    pub fg: Rgba,
    pub bg: Rgba,
    pub bold: bool,
    pub italic: bool,
    pub inverse: bool,
}

impl Default for RenderCell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: Rgba::WHITE,
            bg: Rgba::BLACK,
            bold: false,
            italic: false,
            inverse: false,
        }
    }
}

/// 커서 모양.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorKind {
    Block,
    Underline,
    Beam,
    Hollow,
}

/// 커서 스냅샷(그리드 좌표).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorSnapshot {
    pub col: u16,
    pub row: u16,
    pub kind: CursorKind,
    pub visible: bool,
    pub color: Rgba,
}

impl Default for CursorSnapshot {
    fn default() -> Self {
        Self {
            col: 0,
            row: 0,
            kind: CursorKind::Block,
            visible: true,
            color: Rgba::WHITE,
        }
    }
}

/// 한 프레임 분량의 렌더 상태(소유). `cells`는 row-major, 길이 == cols*rows.
#[derive(Debug, Clone)]
pub struct CellSnapshot {
    pub size: GridSize,
    pub cells: Vec<RenderCell>,
    pub cursor: CursorSnapshot,
    pub default_fg: Rgba,
    pub default_bg: Rgba,
}

impl CellSnapshot {
    pub fn new(size: GridSize) -> Self {
        Self {
            size,
            cells: vec![RenderCell::default(); size.cell_count()],
            cursor: CursorSnapshot::default(),
            default_fg: Rgba::WHITE,
            default_bg: Rgba::BLACK,
        }
    }

    /// (col,row) → cells 인덱스. 범위 밖이면 None.
    #[inline]
    pub fn index(&self, col: u16, row: u16) -> Option<usize> {
        if col >= self.size.cols || row >= self.size.rows {
            return None;
        }
        Some(row as usize * self.size.cols as usize + col as usize)
    }

    /// 그리드 크기에 맞게 버퍼를 재할당(내용 초기화).
    pub fn resize(&mut self, size: GridSize) {
        self.size = size;
        self.cells.clear();
        self.cells.resize(size.cell_count(), RenderCell::default());
    }
}

/// 데스크톱 알림(OSC 9/777에서 파싱).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub title: String,
    pub body: String,
}

/// zm-mux 공통 에러.
#[derive(thiserror::Error, Debug)]
pub enum ZmError {
    #[error("pty error: {0}")]
    Pty(String),
    #[error("render error: {0}")]
    Render(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ZmError>;

/// 사용자 설정(TOML). 중첩 섹션 스키마. 누락 필드/섹션은 기본값, 미지 필드는 무시.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub font: FontConfig,
    pub colors: ColorConfig,
    pub scrollback: ScrollbackConfig,
    pub shell: ShellConfig,
    pub keybindings: KeybindingConfig,
    pub agent: AgentConfig,
}

/// 에이전트 연동(트랙 A) 설정.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    /// pane에 tmux-shim 환경(가짜 TMUX/TMUX_PANE + shim PATH + 에이전트 팀 플래그) 주입.
    pub tmux_shim: bool,
    /// `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` 주입.
    pub claude_agent_teams: bool,
}
impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            tmux_shim: true,
            claude_agent_teams: true,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct FontConfig {
    /// 폰트 패밀리(빈 문자열이면 플랫폼 기본 모노스페이스 탐색).
    pub family: String,
    pub size: f32,
}
impl Default for FontConfig {
    fn default() -> Self {
        Self { family: String::new(), size: 14.0 }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct ColorConfig {
    pub background: String,
    pub foreground: String,
    /// 커서 색(미지정 시 전경색).
    pub cursor: String,
}
impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            background: "#0c0c0c".into(),
            foreground: "#cccccc".into(),
            cursor: String::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct ScrollbackConfig {
    pub max_lines: usize,
}
impl Default for ScrollbackConfig {
    fn default() -> Self {
        Self { max_lines: 10_000 }
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct ShellConfig {
    /// 실행할 셸(빈 문자열이면 플랫폼 기본 %COMSPEC%/$SHELL).
    pub program: String,
    pub args: Vec<String>,
}

/// 키바인딩(프리픽스 없는 직접 단축키 문자열, 예 "Ctrl+Shift+D").
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct KeybindingConfig {
    pub new_tab: String,
    pub close_tab: String,
    pub close_pane: String,
    pub split_horizontal: String,
    pub split_vertical: String,
    pub next_tab: String,
    pub prev_tab: String,
    pub focus_left: String,
    pub focus_right: String,
    pub focus_up: String,
    pub focus_down: String,
    pub zoom: String,
}
impl Default for KeybindingConfig {
    fn default() -> Self {
        Self {
            new_tab: "Ctrl+T".into(),
            close_tab: "Ctrl+Shift+W".into(),
            close_pane: "Ctrl+Shift+P".into(),
            split_horizontal: "Ctrl+Shift+D".into(),
            split_vertical: "Ctrl+Shift+E".into(),
            next_tab: "Ctrl+Tab".into(),
            prev_tab: "Ctrl+Shift+Tab".into(),
            focus_left: "Alt+Left".into(),
            focus_right: "Alt+Right".into(),
            focus_up: "Alt+Up".into(),
            focus_down: "Alt+Down".into(),
            zoom: "Ctrl+Shift+Z".into(),
        }
    }
}

impl Config {
    /// 설정 파일을 읽어 로드. 없으면 기본값, 파싱 실패 시 경고 후 기본값.
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str::<Config>(&text) {
                Ok(cfg) => cfg,
                Err(e) => {
                    eprintln!("zm-mux: 설정 파싱 실패({}): {e} — 기본값 사용", path.display());
                    Self::default()
                }
            },
            Err(_) => Self::default(), // 파일 없음 → 기본값
        }
    }

    pub fn font_family(&self) -> Option<String> {
        let f = self.font.family.trim();
        if f.is_empty() {
            None
        } else {
            Some(f.to_string())
        }
    }
    pub fn font_size(&self) -> f32 {
        if self.font.size > 0.0 {
            self.font.size
        } else {
            14.0
        }
    }
    pub fn scrollback_lines(&self) -> usize {
        self.scrollback.max_lines
    }
    pub fn shell_program(&self) -> Option<&str> {
        let p = self.shell.program.trim();
        if p.is_empty() {
            None
        } else {
            Some(p)
        }
    }
    pub fn shell_args(&self) -> &[String] {
        &self.shell.args
    }

    pub fn fg(&self) -> Rgba {
        parse_hex(&self.colors.foreground).unwrap_or(Rgba::rgb(0xCC, 0xCC, 0xCC))
    }
    pub fn bg(&self) -> Rgba {
        parse_hex(&self.colors.background).unwrap_or(Rgba::rgb(0x0C, 0x0C, 0x0C))
    }
    pub fn cursor_color(&self) -> Rgba {
        parse_hex(&self.colors.cursor).unwrap_or_else(|| self.fg())
    }
}

fn config_path() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    if let Ok(p) = std::env::var("ZM_MUX_CONFIG") {
        return Some(PathBuf::from(p));
    }
    #[cfg(windows)]
    {
        std::env::var("APPDATA")
            .ok()
            .map(|a| PathBuf::from(a).join("zm-mux").join("config.toml"))
    }
    #[cfg(not(windows))]
    {
        if let Ok(x) = std::env::var("XDG_CONFIG_HOME") {
            return Some(PathBuf::from(x).join("zm-mux").join("config.toml"));
        }
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join(".config").join("zm-mux").join("config.toml"))
    }
}

/// "#rrggbb" / "rrggbb" → Rgba.
pub fn parse_hex(s: &str) -> Option<Rgba> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&h[0..2], 16).ok()?;
    let g = u8::from_str_radix(&h[2..4], 16).ok()?;
    let b = u8::from_str_radix(&h[4..6], 16).ok()?;
    Some(Rgba::rgb(r, g, b))
}

impl fmt::Display for GridSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.cols, self.rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_min_one() {
        let g = GridSize::new(0, 0);
        assert_eq!(g.cols, 1);
        assert_eq!(g.rows, 1);
    }

    #[test]
    fn snapshot_index() {
        let s = CellSnapshot::new(GridSize::new(80, 24));
        assert_eq!(s.cells.len(), 80 * 24);
        assert_eq!(s.index(0, 0), Some(0));
        assert_eq!(s.index(79, 23), Some(23 * 80 + 79));
        assert_eq!(s.index(80, 0), None);
    }
}
