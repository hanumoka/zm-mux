//! zm-term — alacritty_terminal 0.26 위의 VT 모델 래퍼.
//!
//! - `feed(&[u8])`로 PTY 바이트를 VT 파서에 투입(`vte::ansi::Processor::advance`).
//! - `snapshot(&mut CellSnapshot)`로 렌더 가능한 그리드를 256색 팔레트로 해석해 채움.
//! - 터미널이 생성하는 응답(DSR 커서위치 등 `Event::PtyWrite`)은 `take_pending_writes()`로
//!   회수해 호출자가 PTY writer로 되돌려야 한다(ConPTY `ESC[6n` 함정, docs/research/07 §4).

use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Processor};

use zm_core::{CellSnapshot, CursorKind, CursorSnapshot, GridSize, Notification, Rgba};

/// alacritty `Dimensions` 어댑터. (총 라인=화면 라인=rows; 스크롤백은 Config로)
struct TermDims(GridSize);

impl Dimensions for TermDims {
    fn total_lines(&self) -> usize {
        self.0.rows as usize
    }
    fn screen_lines(&self) -> usize {
        self.0.rows as usize
    }
    fn columns(&self) -> usize {
        self.0.cols as usize
    }
}

/// 터미널이 PTY로 되돌려야 할 바이트(DSR/DA 응답 등)를 모으는 EventListener.
///
/// `send_event(&self, ...)`가 불변참조이므로 내부 가변성(Arc<Mutex>) 사용.
#[derive(Clone, Default)]
pub struct WriteCollector {
    writes: Arc<Mutex<Vec<u8>>>,
    title: Arc<Mutex<Option<String>>>,
}

impl EventListener for WriteCollector {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(text) => {
                if let Ok(mut buf) = self.writes.lock() {
                    buf.extend_from_slice(text.as_bytes());
                }
            }
            Event::Title(t) => {
                if let Ok(mut title) = self.title.lock() {
                    *title = Some(t);
                }
            }
            Event::ResetTitle => {
                if let Ok(mut title) = self.title.lock() {
                    *title = None;
                }
            }
            _ => {} // Bell/Clipboard 등은 무시.
        }
    }
}

impl WriteCollector {
    fn take(&self) -> Vec<u8> {
        match self.writes.lock() {
            Ok(mut buf) => std::mem::take(&mut *buf),
            Err(_) => Vec::new(),
        }
    }
    fn title(&self) -> Option<String> {
        self.title.lock().ok().and_then(|t| t.clone())
    }
}

/// VT 터미널 모델.
pub struct Terminal {
    term: Term<WriteCollector>,
    parser: Processor,
    writes: WriteCollector,
    palette: Palette,
    size: GridSize,
    /// OSC 알림 스캔용 캐리(미완 시퀀스 보관, 상한 있음).
    osc_carry: Vec<u8>,
    notifications: Vec<Notification>,
}

impl Terminal {
    pub fn new(size: GridSize, cfg: &zm_core::Config) -> Self {
        let writes = WriteCollector::default();
        let mut aconfig = Config::default();
        aconfig.scrolling_history = cfg.scrollback_lines();
        let term = Term::new(aconfig, &TermDims(size), writes.clone());
        Self {
            term,
            parser: Processor::new(),
            writes,
            palette: Palette::from_config(cfg),
            size,
            osc_carry: Vec::new(),
            notifications: Vec::new(),
        }
    }

    /// PTY 출력 바이트를 VT 파서에 투입(그리드 갱신) + OSC 알림 스캔.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
        self.scan_notifications(bytes);
    }

    /// 데스크톱 알림(OSC 9/777) 회수.
    pub fn take_notifications(&mut self) -> Vec<Notification> {
        std::mem::take(&mut self.notifications)
    }

    /// Kitty 키보드 프로토콜 활성 여부(CSI-u 인코딩 게이트).
    pub fn kitty_keyboard(&self) -> bool {
        self.term.mode().contains(TermMode::DISAMBIGUATE_ESC_CODES)
    }

    /// 셸이 설정한 창 제목(OSC 0/2). 없으면 None.
    pub fn title(&self) -> Option<String> {
        self.writes.title()
    }

    /// 바이트에서 OSC 9 / OSC 777 알림을 추출(미완 시퀀스는 캐리에 보관).
    fn scan_notifications(&mut self, chunk: &[u8]) {
        self.osc_carry.extend_from_slice(chunk);
        loop {
            // OSC 시작(ESC ]) 찾기.
            let Some(start) = find_sub(&self.osc_carry, b"\x1b]") else {
                // OSC 시작 없음 → 마지막 ESC 가능성만 남기고 버림.
                let keep = self.osc_carry.last() == Some(&0x1b);
                self.osc_carry.clear();
                if keep {
                    self.osc_carry.push(0x1b);
                }
                break;
            };
            if start > 0 {
                self.osc_carry.drain(..start);
            }
            // 종료자(BEL=0x07 또는 ST=ESC '\') 찾기.
            let body = &self.osc_carry[2..];
            let (term_at, term_len) = match find_terminator(body) {
                Some(x) => x,
                None => {
                    // 미완 → 상한 초과 시 버림.
                    if self.osc_carry.len() > 8192 {
                        self.osc_carry.clear();
                    }
                    break;
                }
            };
            let osc = body[..term_at].to_vec();
            if let Some(n) = parse_osc_notification(&osc) {
                self.notifications.push(n);
            }
            let consumed = 2 + term_at + term_len;
            self.osc_carry.drain(..consumed);
        }
    }

    /// 터미널이 생성한 PTY 응답 바이트를 회수(호출자가 PTY writer로 전송).
    pub fn take_pending_writes(&mut self) -> Vec<u8> {
        self.writes.take()
    }

    pub fn resize(&mut self, size: GridSize) {
        if size == self.size {
            return;
        }
        self.term.resize(TermDims(size));
        self.size = size;
    }

    pub fn size(&self) -> GridSize {
        self.size
    }

    /// 스크롤백 이동(+: 위로 히스토리, -: 아래로). 휠 등에 사용.
    pub fn scroll(&mut self, delta_lines: i32) {
        self.term.scroll_display(Scroll::Delta(delta_lines));
    }

    /// 최하단(라이브)으로 스냅(입력 시).
    pub fn scroll_to_bottom(&mut self) {
        self.term.scroll_display(Scroll::Bottom);
    }

    /// 현재 화면을 텍스트로 캡처(자동화 capture-pane). 행별 우측 공백 제거.
    pub fn capture_text(&self) -> String {
        let cols = self.size.cols as usize;
        let rows = self.size.rows as usize;
        let content = self.term.renderable_content();
        let offset = content.display_offset as i32;
        let mut grid = vec![vec![' '; cols]; rows];
        for indexed in content.display_iter {
            let line = indexed.point.line.0 + offset;
            let col = indexed.point.column.0;
            if line >= 0 && (line as usize) < rows && col < cols {
                let c = indexed.cell.c;
                grid[line as usize][col] = if c == '\0' { ' ' } else { c };
            }
        }
        grid.into_iter()
            .map(|row| row.into_iter().collect::<String>().trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 현재 화면(display_offset=0)을 RGBA로 해석해 스냅샷에 채운다.
    pub fn snapshot(&self, out: &mut CellSnapshot) {
        if out.size != self.size {
            out.resize(self.size);
        }
        out.default_fg = self.palette.fg;
        out.default_bg = self.palette.bg;

        // 기본 셀(공백+기본색)으로 초기화.
        let blank = zm_core::RenderCell {
            c: ' ',
            fg: self.palette.fg,
            bg: self.palette.bg,
            bold: false,
            italic: false,
            inverse: false,
        };
        for cell in out.cells.iter_mut() {
            *cell = blank;
        }

        let content = self.term.renderable_content();
        let colors = content.colors;
        let cursor = content.cursor;
        let display_offset = content.display_offset as i32;

        for indexed in content.display_iter {
            // 화면 행 = 그리드 라인 + display_offset(스크롤백 반영).
            let line = indexed.point.line.0 + display_offset;
            let col = indexed.point.column.0;
            if line < 0 || line as usize >= self.size.rows as usize || col >= self.size.cols as usize
            {
                continue;
            }
            let cell = indexed.cell;
            let flags = cell.flags;
            // 와이드문자 스페이서는 건너뜀(앞 셀이 그림).
            if flags.contains(Flags::WIDE_CHAR_SPACER)
                || flags.contains(Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }

            let mut fg = self.palette.resolve(cell.fg, colors);
            let mut bg = self.palette.resolve(cell.bg, colors);
            let bold = flags.contains(Flags::BOLD);
            let italic = flags.contains(Flags::ITALIC);
            if flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            // HIDDEN: 전경=배경 처리.
            if flags.contains(Flags::HIDDEN) {
                fg = bg;
            }

            if let Some(idx) = out.index(col as u16, line as u16) {
                out.cells[idx] = zm_core::RenderCell {
                    c: cell.c,
                    fg,
                    bg,
                    bold,
                    italic,
                    inverse: false,
                };
            }
        }

        // 커서(스크롤백 시 뷰포트 밖이면 숨김).
        let crow = cursor.point.line.0 + display_offset;
        let ccol = cursor.point.column.0;
        let visible = !matches!(cursor.shape, CursorShape::Hidden)
            && crow >= 0
            && (crow as usize) < self.size.rows as usize
            && ccol < self.size.cols as usize;
        let kind = match cursor.shape {
            CursorShape::Block | CursorShape::Hidden => CursorKind::Block,
            CursorShape::Underline => CursorKind::Underline,
            CursorShape::Beam => CursorKind::Beam,
            CursorShape::HollowBlock => CursorKind::Hollow,
        };
        let col = ccol.min(u16::MAX as usize) as u16;
        let row = crow.max(0).min(u16::MAX as i32) as u16;

        // 블록/할로 커서는 셀 색 반전으로 베이크(글리프가 배경색으로 보임).
        // 빔/언더라인은 zm-render가 얇은 rect로 그린다.
        if visible && matches!(kind, CursorKind::Block | CursorKind::Hollow) {
            if let Some(idx) = out.index(col, row) {
                let orig_bg = out.cells[idx].bg;
                out.cells[idx].bg = self.palette.cursor;
                out.cells[idx].fg = orig_bg;
            }
        }

        out.cursor = CursorSnapshot {
            col,
            row,
            kind,
            visible,
            color: self.palette.cursor,
        };
    }
}

/// 256색 팔레트 + 기본 fg/bg/cursor (Windows Terminal Campbell 유사).
struct Palette {
    table: [Rgba; 256],
    fg: Rgba,
    bg: Rgba,
    cursor: Rgba,
}

impl Default for Palette {
    fn default() -> Self {
        let mut table = [Rgba::BLACK; 256];

        // 0..16 표준 ANSI (Campbell)
        const ANSI: [(u8, u8, u8); 16] = [
            (0x0C, 0x0C, 0x0C), // 0 black
            (0xC5, 0x0F, 0x1F), // 1 red
            (0x13, 0xA1, 0x0E), // 2 green
            (0xC1, 0x9C, 0x00), // 3 yellow
            (0x00, 0x37, 0xDA), // 4 blue
            (0x88, 0x17, 0x98), // 5 magenta
            (0x3A, 0x96, 0xDD), // 6 cyan
            (0xCC, 0xCC, 0xCC), // 7 white
            (0x76, 0x76, 0x76), // 8 bright black
            (0xE7, 0x48, 0x56), // 9 bright red
            (0x16, 0xC6, 0x0C), // 10 bright green
            (0xF9, 0xF1, 0xA5), // 11 bright yellow
            (0x3B, 0x78, 0xFF), // 12 bright blue
            (0xB4, 0x00, 0x9E), // 13 bright magenta
            (0x61, 0xD6, 0xD6), // 14 bright cyan
            (0xF2, 0xF2, 0xF2), // 15 bright white
        ];
        for (i, &(r, g, b)) in ANSI.iter().enumerate() {
            table[i] = Rgba::rgb(r, g, b);
        }

        // 16..232: 6x6x6 색 큐브
        const STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];
        let mut idx = 16;
        for r in 0..6 {
            for g in 0..6 {
                for b in 0..6 {
                    table[idx] = Rgba::rgb(STEPS[r], STEPS[g], STEPS[b]);
                    idx += 1;
                }
            }
        }

        // 232..256: 그레이스케일 램프
        for i in 0..24 {
            let v = 8 + 10 * i as u8;
            table[232 + i] = Rgba::rgb(v, v, v);
        }

        Self {
            table,
            fg: Rgba::rgb(0xCC, 0xCC, 0xCC),
            bg: Rgba::rgb(0x0C, 0x0C, 0x0C),
            cursor: Rgba::rgb(0xCC, 0xCC, 0xCC),
        }
    }
}

impl Palette {
    /// 기본 256색 테이블 + 설정의 fg/bg/cursor.
    fn from_config(cfg: &zm_core::Config) -> Self {
        let mut p = Palette::default();
        p.fg = cfg.fg();
        p.bg = cfg.bg();
        p.cursor = cfg.cursor_color();
        p
    }

    /// alacritty `Color` → RGBA. `colors` 오버라이드 우선, 없으면 내장 팔레트.
    fn resolve(&self, color: Color, colors: &Colors) -> Rgba {
        match color {
            Color::Spec(rgb) => Rgba::rgb(rgb.r, rgb.g, rgb.b),
            Color::Indexed(i) => self.override_or(colors[i as usize], self.table[i as usize]),
            Color::Named(named) => {
                let ov = colors[named];
                self.override_or(ov, self.named_default(named))
            }
        }
    }

    fn override_or(&self, ov: Option<alacritty_terminal::vte::ansi::Rgb>, fallback: Rgba) -> Rgba {
        match ov {
            Some(rgb) => Rgba::rgb(rgb.r, rgb.g, rgb.b),
            None => fallback,
        }
    }

    fn named_default(&self, n: NamedColor) -> Rgba {
        use NamedColor::*;
        match n {
            Black => self.table[0],
            Red => self.table[1],
            Green => self.table[2],
            Yellow => self.table[3],
            Blue => self.table[4],
            Magenta => self.table[5],
            Cyan => self.table[6],
            White => self.table[7],
            BrightBlack => self.table[8],
            BrightRed => self.table[9],
            BrightGreen => self.table[10],
            BrightYellow => self.table[11],
            BrightBlue => self.table[12],
            BrightMagenta => self.table[13],
            BrightCyan => self.table[14],
            BrightWhite => self.table[15],
            Foreground | BrightForeground | DimForeground => self.fg,
            Background => self.bg,
            Cursor => self.cursor,
            DimBlack => self.table[0],
            DimRed => self.table[1],
            DimGreen => self.table[2],
            DimYellow => self.table[3],
            DimBlue => self.table[4],
            DimMagenta => self.table[5],
            DimCyan => self.table[6],
            DimWhite => self.table[7],
        }
    }
}

fn find_sub(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

/// 종료자 위치와 길이: BEL=(i,1), ST(ESC '\')=(i,2).
fn find_terminator(body: &[u8]) -> Option<(usize, usize)> {
    let mut i = 0;
    while i < body.len() {
        if body[i] == 0x07 {
            return Some((i, 1));
        }
        if body[i] == 0x1b && body.get(i + 1) == Some(&b'\\') {
            return Some((i, 2));
        }
        i += 1;
    }
    None
}

/// OSC 본문 → 알림(OSC 9: 단일 메시지, OSC 777;notify;title;body).
fn parse_osc_notification(osc: &[u8]) -> Option<Notification> {
    let text = String::from_utf8_lossy(osc);
    let mut parts = text.splitn(2, ';');
    let code = parts.next()?;
    match code {
        "9" => {
            let body = parts.next().unwrap_or("").to_string();
            if body.is_empty() {
                None
            } else {
                Some(Notification {
                    title: "zm-mux".to_string(),
                    body,
                })
            }
        }
        "777" => {
            // 777;notify;<title>;<body>
            let rest = parts.next()?;
            let mut f = rest.splitn(3, ';');
            let kind = f.next()?;
            if kind != "notify" {
                return None;
            }
            let title = f.next().unwrap_or("zm-mux").to_string();
            let body = f.next().unwrap_or("").to_string();
            Some(Notification { title, body })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_and_snapshot_plain_text() {
        let size = GridSize::new(20, 5);
        let mut term = Terminal::new(size, &zm_core::Config::default());
        term.feed(b"hi");
        let mut snap = CellSnapshot::new(size);
        term.snapshot(&mut snap);
        assert_eq!(snap.cells[snap.index(0, 0).unwrap()].c, 'h');
        assert_eq!(snap.cells[snap.index(1, 0).unwrap()].c, 'i');
    }

    #[test]
    fn dsr_query_produces_pty_write() {
        let mut term = Terminal::new(GridSize::new(20, 5), &zm_core::Config::default());
        // ESC[6n → 터미널이 커서 위치 보고를 PtyWrite로 생성.
        term.feed(b"\x1b[6n");
        let resp = term.take_pending_writes();
        assert!(!resp.is_empty(), "DSR 응답이 비어있음");
        assert!(resp.starts_with(b"\x1b["), "CPR 형식 아님: {resp:?}");
    }

    #[test]
    fn osc9_notification() {
        let mut term = Terminal::new(GridSize::new(40, 5), &zm_core::Config::default());
        term.feed(b"\x1b]9;Build done\x07");
        let n = term.take_notifications();
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].body, "Build done");
    }

    #[test]
    fn osc777_notification_split_chunks() {
        let mut term = Terminal::new(GridSize::new(40, 5), &zm_core::Config::default());
        // 청크 경계로 쪼개져도 캐리로 이어붙여 파싱.
        term.feed(b"\x1b]777;notify;Title");
        term.feed(b";Body text\x1b\\");
        let n = term.take_notifications();
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].title, "Title");
        assert_eq!(n[0].body, "Body text");
    }

    #[test]
    fn sgr_color_sets_fg() {
        let size = GridSize::new(20, 3);
        let mut term = Terminal::new(size, &zm_core::Config::default());
        term.feed(b"\x1b[31mR"); // red 'R'
        let mut snap = CellSnapshot::new(size);
        term.snapshot(&mut snap);
        let cell = snap.cells[snap.index(0, 0).unwrap()];
        assert_eq!(cell.c, 'R');
        assert_eq!(cell.fg, Rgba::rgb(0xC5, 0x0F, 0x1F));
    }
}
