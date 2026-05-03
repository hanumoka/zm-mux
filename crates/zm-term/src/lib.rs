use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::term::Config as TermConfig;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi;
use std::sync::{Arc, Mutex};
use vte::{Parser as VteParser, Perform as VtePerform};
use zm_core::ZmResult;

#[derive(Clone)]
pub struct EventCollector {
    events: Arc<Mutex<Vec<Event>>>,
}

impl EventCollector {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn shared(&self) -> Arc<Mutex<Vec<Event>>> {
        self.events.clone()
    }
}

impl Default for EventCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl EventListener for EventCollector {
    fn send_event(&self, event: Event) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
}

/// OSC events we sniff out of the byte stream because alacritty_terminal
/// does not surface them through its EventListener.  Right now this is the
/// notification family (OSC 9 — iTerm style, OSC 777 — urxvt style).  OSC
/// 99 (KDE D-Bus) and OSC 8 (hyperlinks) live elsewhere and are deferred
/// to follow-up tasks.
#[derive(Debug, Clone)]
pub enum OscEventKind {
    Notify { title: String, body: String },
}

#[derive(Debug, Clone)]
pub struct OscEvent {
    pub kind: OscEventKind,
}

/// One regex hit in a viewport row.  Coordinates are *cell-relative* —
/// `row` and `col` are 0-based indices into the visible grid, `len` is
/// the cell count the match occupies (wide chars count as 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub row: usize,
    pub col: usize,
    pub len: usize,
}

/// Sniffs OSC 9 / 777 sequences out of a parallel `vte::Parser` running
/// alongside alacritty's parser.  All other OSC codes are silently
/// ignored — alacritty handles the ones it cares about (title, color,
/// clipboard).  Cost: a second pass over the same bytes; for terminal
/// throughput this is negligible (byte-level state machine).
pub struct OscDispatcher {
    events: Arc<Mutex<Vec<OscEvent>>>,
}

impl OscDispatcher {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn shared(&self) -> Arc<Mutex<Vec<OscEvent>>> {
        self.events.clone()
    }

    fn push(&self, ev: OscEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(ev);
        }
    }
}

impl Default for OscDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl VtePerform for OscDispatcher {
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.is_empty() {
            return;
        }
        let code = match std::str::from_utf8(params[0]) {
            Ok(s) => s,
            Err(_) => return,
        };
        match code {
            // OSC 9;<body>BEL  (iTerm2 / Terminal.app growl-style notify)
            "9" => {
                if let Some(body_bytes) = params.get(1) {
                    let body = String::from_utf8_lossy(body_bytes).into_owned();
                    if !body.is_empty() {
                        self.push(OscEvent {
                            kind: OscEventKind::Notify {
                                title: "zm-mux".to_string(),
                                body,
                            },
                        });
                    }
                }
            }
            // OSC 777;notify;<title>;<body>BEL  (urxvt-style)
            "777" if params.len() >= 4 && params[1] == b"notify" => {
                let title = String::from_utf8_lossy(params[2]).into_owned();
                let body = String::from_utf8_lossy(params[3]).into_owned();
                self.push(OscEvent {
                    kind: OscEventKind::Notify { title, body },
                });
            }
            _ => {}
        }
    }
}

pub struct TermSize {
    pub cols: u16,
    pub rows: u16,
}

impl Dimensions for TermSize {
    fn columns(&self) -> usize {
        self.cols as usize
    }

    fn screen_lines(&self) -> usize {
        self.rows as usize
    }

    fn total_lines(&self) -> usize {
        self.rows as usize
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CellColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl CellColor {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const WHITE: Self = Self::new(255, 255, 255);
    pub const BLACK: Self = Self::new(0, 0, 0);
}

#[derive(Debug, Clone)]
pub struct RenderCell {
    pub c: char,
    pub fg: CellColor,
    pub bg: CellColor,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl Default for RenderCell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: CellColor::WHITE,
            bg: CellColor::BLACK,
            bold: false,
            italic: false,
            underline: false,
        }
    }
}

pub struct ZmTerm {
    term: Term<EventCollector>,
    parser: ansi::Processor,
    osc_parser: VteParser,
    osc_dispatcher: OscDispatcher,
    events: Arc<Mutex<Vec<Event>>>,
    osc_events: Arc<Mutex<Vec<OscEvent>>>,
    default_fg: CellColor,
    default_bg: CellColor,
}

impl ZmTerm {
    pub fn new(
        cols: u16,
        rows: u16,
        scrollback_lines: usize,
        default_fg: CellColor,
        default_bg: CellColor,
    ) -> ZmResult<Self> {
        let size = TermSize { cols, rows };
        let config = TermConfig {
            scrolling_history: scrollback_lines,
            ..TermConfig::default()
        };
        let event_collector = EventCollector::new();
        let events = event_collector.shared();
        let term = Term::new(config, &size, event_collector);
        let parser = ansi::Processor::new();
        let osc_dispatcher = OscDispatcher::new();
        let osc_events = osc_dispatcher.shared();
        let osc_parser = VteParser::new();

        Ok(Self {
            term,
            parser,
            osc_parser,
            osc_dispatcher,
            events,
            osc_events,
            default_fg,
            default_bg,
        })
    }

    pub fn feed_bytes(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
        // Run the same bytes through our OSC sniffer.  Side effects feed
        // into self.osc_events; alacritty's grid state is unaffected.
        self.osc_parser.advance(&mut self.osc_dispatcher, bytes);
    }

    /// Drain bytes that the terminal needs to send back to the PTY in response
    /// to queries like DSR (`\x1b[6n`), color requests, etc.
    /// Caller must write these bytes to the PTY master.
    pub fn drain_pty_writes(&self) -> Vec<Vec<u8>> {
        let mut events = match self.events.lock() {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };
        let mut out = Vec::new();
        let mut keep = Vec::with_capacity(events.len());
        for ev in events.drain(..) {
            match ev {
                Event::PtyWrite(s) => out.push(s.into_bytes()),
                other => keep.push(other),
            }
        }
        *events = keep;
        out
    }

    /// Drain OSC notification events accumulated since the last call.  Caller
    /// is expected to throttle / dedup before forwarding to the OS notifier.
    pub fn drain_osc_events(&self) -> Vec<OscEvent> {
        let mut events = match self.osc_events.lock() {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };
        events.drain(..).collect()
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        let size = TermSize { cols, rows };
        self.term.resize(size);
    }

    pub fn cols(&self) -> usize {
        self.term.columns()
    }

    pub fn rows(&self) -> usize {
        self.term.screen_lines()
    }

    pub fn render_cell(&self, row: usize, col: usize) -> RenderCell {
        use alacritty_terminal::index::{Column, Line};
        use alacritty_terminal::term::cell::Flags;
        use alacritty_terminal::vte::ansi::Color;

        let grid = self.term.grid();
        let line = Line(row as i32);
        let column = Column(col);

        if col >= self.cols() || row >= self.rows() {
            return RenderCell::default();
        }

        let cell = &grid[line][column];

        use alacritty_terminal::vte::ansi::NamedColor;
        let fg = match cell.fg {
            Color::Spec(rgb) => CellColor::new(rgb.r, rgb.g, rgb.b),
            Color::Named(NamedColor::Foreground) => self.default_fg,
            Color::Named(NamedColor::Background) => self.default_bg,
            Color::Named(name) => named_color_to_rgb(name),
            Color::Indexed(idx) => indexed_color_to_rgb(idx),
        };

        let bg = match cell.bg {
            Color::Spec(rgb) => CellColor::new(rgb.r, rgb.g, rgb.b),
            Color::Named(NamedColor::Foreground) => self.default_fg,
            Color::Named(NamedColor::Background) => self.default_bg,
            Color::Named(name) => named_color_to_rgb(name),
            Color::Indexed(idx) => indexed_color_to_rgb(idx),
        };

        RenderCell {
            c: cell.c,
            fg,
            bg,
            bold: cell.flags.contains(Flags::BOLD),
            italic: cell.flags.contains(Flags::ITALIC),
            underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
        }
    }

    pub fn render_row(&self, row: usize) -> Vec<RenderCell> {
        (0..self.cols())
            .map(|col| self.render_cell(row, col))
            .collect()
    }

    pub fn row_text(&self, row: usize) -> String {
        (0..self.cols())
            .map(|col| {
                let cell = self.render_cell(row, col);
                cell.c
            })
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    pub fn cursor_position(&self) -> (usize, usize) {
        let cursor = self.term.grid().cursor.point;
        (cursor.line.0 as usize, cursor.column.0)
    }

    /// Search the visible viewport for a regex pattern.  Returns one match
    /// per occurrence with viewport-relative `(row, col, len)` in *cell
    /// units* — wide CJK glyphs count as 2 cells (matching how the grid
    /// itself stores them, so renderers can paint highlight rects without
    /// a width recompute).
    ///
    /// Empty pattern returns no matches.  Invalid regex returns no matches
    /// (caller can validate the pattern separately for error UI).
    /// Scrollback search is a follow-up: the alacritty grid line model
    /// would need negative-index iteration here.
    pub fn search(&self, pattern: &str) -> Vec<SearchMatch> {
        if pattern.is_empty() {
            return Vec::new();
        }
        let re = match regex::Regex::new(pattern) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let cols = self.cols();
        let rows = self.rows();
        let mut out = Vec::new();
        for row in 0..rows {
            // Materialize the row as a String so regex can scan it.
            // Wide-char spacers contribute their counterpart cell's char,
            // but we skip duplicates by matching against the leading char
            // and counting the *cell* span separately during emit.
            let mut text = String::with_capacity(cols);
            let mut col_for_byte: Vec<usize> = Vec::with_capacity(cols);
            for col in 0..cols {
                if self.is_wide_spacer(row, col) {
                    continue;
                }
                let cell = self.render_cell(row, col);
                let c = if cell.c == '\0' { ' ' } else { cell.c };
                let byte_start = text.len();
                text.push(c);
                for _ in byte_start..text.len() {
                    col_for_byte.push(col);
                }
            }
            for m in re.find_iter(&text) {
                let start_col = col_for_byte
                    .get(m.start())
                    .copied()
                    .unwrap_or(0);
                // Width in cells: count chars in match, count wide chars twice.
                let mut width_cells = 0usize;
                let mut byte = m.start();
                for ch in m.as_str().chars() {
                    let col = col_for_byte.get(byte).copied().unwrap_or(start_col);
                    width_cells += if self.is_wide_char(row, col) { 2 } else { 1 };
                    byte += ch.len_utf8();
                }
                if width_cells > 0 {
                    out.push(SearchMatch {
                        row,
                        col: start_col,
                        len: width_cells,
                    });
                }
            }
        }
        out
    }

    pub fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    /// Scroll the viewport. Positive `delta` scrolls up (older lines into view),
    /// negative scrolls down (toward live content).
    pub fn scroll_lines(&mut self, delta: i32) {
        self.term.scroll_display(Scroll::Delta(delta));
    }

    pub fn scroll_page_up(&mut self) {
        self.term.scroll_display(Scroll::PageUp);
    }

    pub fn scroll_page_down(&mut self) {
        self.term.scroll_display(Scroll::PageDown);
    }

    pub fn scroll_to_top(&mut self) {
        self.term.scroll_display(Scroll::Top);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.term.scroll_display(Scroll::Bottom);
    }

    /// True if the cell at (row, col) is the right half of a wide CJK glyph.
    /// Renderers should skip drawing this cell.
    pub fn is_wide_spacer(&self, row: usize, col: usize) -> bool {
        use alacritty_terminal::index::{Column, Line};
        use alacritty_terminal::term::cell::Flags;
        if col >= self.cols() || row >= self.rows() {
            return false;
        }
        let cell = &self.term.grid()[Line(row as i32)][Column(col)];
        cell.flags
            .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
    }

    /// True if the cell at (row, col) is the left half of a wide CJK glyph.
    /// Renderer should advance 2 cells of horizontal space.
    pub fn is_wide_char(&self, row: usize, col: usize) -> bool {
        use alacritty_terminal::index::{Column, Line};
        use alacritty_terminal::term::cell::Flags;
        if col >= self.cols() || row >= self.rows() {
            return false;
        }
        let cell = &self.term.grid()[Line(row as i32)][Column(col)];
        cell.flags.contains(Flags::WIDE_CHAR)
    }
}

fn named_color_to_rgb(name: alacritty_terminal::vte::ansi::NamedColor) -> CellColor {
    use alacritty_terminal::vte::ansi::NamedColor::*;
    match name {
        Black => CellColor::new(0, 0, 0),
        Red => CellColor::new(204, 0, 0),
        Green => CellColor::new(78, 154, 6),
        Yellow => CellColor::new(196, 160, 0),
        Blue => CellColor::new(52, 101, 164),
        Magenta => CellColor::new(117, 80, 123),
        Cyan => CellColor::new(6, 152, 154),
        White => CellColor::new(211, 215, 207),
        BrightBlack => CellColor::new(85, 87, 83),
        BrightRed => CellColor::new(239, 41, 41),
        BrightGreen => CellColor::new(138, 226, 52),
        BrightYellow => CellColor::new(252, 233, 79),
        BrightBlue => CellColor::new(114, 159, 207),
        BrightMagenta => CellColor::new(173, 127, 168),
        BrightCyan => CellColor::new(52, 226, 226),
        BrightWhite => CellColor::new(238, 238, 236),
        Foreground => CellColor::WHITE,
        Background => CellColor::BLACK,
        _ => CellColor::WHITE,
    }
}

fn indexed_color_to_rgb(idx: u8) -> CellColor {
    if idx < 16 {
        use alacritty_terminal::vte::ansi::NamedColor::*;
        let named = match idx {
            0 => Black,
            1 => Red,
            2 => Green,
            3 => Yellow,
            4 => Blue,
            5 => Magenta,
            6 => Cyan,
            7 => White,
            8 => BrightBlack,
            9 => BrightRed,
            10 => BrightGreen,
            11 => BrightYellow,
            12 => BrightBlue,
            13 => BrightMagenta,
            14 => BrightCyan,
            15 => BrightWhite,
            _ => Foreground,
        };
        return named_color_to_rgb(named);
    }
    if idx < 232 {
        let idx = idx - 16;
        let r = (idx / 36) * 51;
        let g = ((idx % 36) / 6) * 51;
        let b = (idx % 6) * 51;
        return CellColor::new(r, g, b);
    }
    let gray = 8 + (idx - 232) * 10;
    CellColor::new(gray, gray, gray)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_term() {
        let term = ZmTerm::new(80, 24, 10_000, CellColor::WHITE, CellColor::BLACK);
        assert!(term.is_ok());
        let term = term.unwrap();
        assert_eq!(term.cols(), 80);
        assert_eq!(term.rows(), 24);
    }

    #[test]
    fn feed_plain_text() {
        let mut term = ZmTerm::new(80, 24, 10_000, CellColor::WHITE, CellColor::BLACK).unwrap();
        term.feed_bytes(b"Hello, zm-mux!");
        let row = term.row_text(0);
        assert!(row.contains("Hello, zm-mux!"), "got: '{}'", row);
    }

    #[test]
    fn feed_ansi_color() {
        let mut term = ZmTerm::new(80, 24, 10_000, CellColor::WHITE, CellColor::BLACK).unwrap();
        term.feed_bytes(b"\x1b[31mERR\x1b[0m OK");
        let row = term.row_text(0);
        assert!(row.contains("ERR") && row.contains("OK"), "got: '{}'", row);

        let cell = term.render_cell(0, 0);
        assert_eq!(cell.c, 'E');
        assert!(
            cell.fg.r > 150,
            "Red text should have high red: {}",
            cell.fg.r
        );
    }

    #[test]
    fn feed_newline() {
        let mut term = ZmTerm::new(80, 24, 10_000, CellColor::WHITE, CellColor::BLACK).unwrap();
        term.feed_bytes(b"line1\r\nline2");
        assert!(term.row_text(0).contains("line1"));
        assert!(term.row_text(1).contains("line2"));
    }

    #[test]
    fn cursor_position() {
        let mut term = ZmTerm::new(80, 24, 10_000, CellColor::WHITE, CellColor::BLACK).unwrap();
        term.feed_bytes(b"ABC");
        let (row, col) = term.cursor_position();
        assert_eq!(row, 0);
        assert_eq!(col, 3);
    }

    #[test]
    fn resize_term() {
        let mut term = ZmTerm::new(80, 24, 10_000, CellColor::WHITE, CellColor::BLACK).unwrap();
        term.resize(40, 12);
        assert_eq!(term.cols(), 40);
        assert_eq!(term.rows(), 12);
    }

    #[test]
    fn bold_text() {
        let mut term = ZmTerm::new(80, 24, 10_000, CellColor::WHITE, CellColor::BLACK).unwrap();
        term.feed_bytes(b"\x1b[1mBOLD\x1b[0m");
        let cell = term.render_cell(0, 0);
        assert_eq!(cell.c, 'B');
        assert!(cell.bold, "Should be bold");
    }

    #[test]
    fn osc_9_emits_notify_event() {
        let mut term = ZmTerm::new(80, 24, 10_000, CellColor::WHITE, CellColor::BLACK).unwrap();
        term.feed_bytes(b"\x1b]9;Hello world\x07");
        let events = term.drain_osc_events();
        assert_eq!(events.len(), 1);
        match &events[0].kind {
            OscEventKind::Notify { title, body } => {
                assert_eq!(title, "zm-mux");
                assert_eq!(body, "Hello world");
            }
        }
    }

    #[test]
    fn osc_777_notify_emits_event() {
        let mut term = ZmTerm::new(80, 24, 10_000, CellColor::WHITE, CellColor::BLACK).unwrap();
        term.feed_bytes(b"\x1b]777;notify;ZM Title;Hello\x07");
        let events = term.drain_osc_events();
        assert_eq!(events.len(), 1);
        match &events[0].kind {
            OscEventKind::Notify { title, body } => {
                assert_eq!(title, "ZM Title");
                assert_eq!(body, "Hello");
            }
        }
    }

    #[test]
    fn osc_unrelated_codes_ignored() {
        let mut term = ZmTerm::new(80, 24, 10_000, CellColor::WHITE, CellColor::BLACK).unwrap();
        // OSC 0 = title set; not notify, must not surface as OscEvent.
        term.feed_bytes(b"\x1b]0;set title\x07");
        let events = term.drain_osc_events();
        assert!(events.is_empty(), "got: {events:?}");
    }

    #[test]
    fn osc_9_empty_body_ignored() {
        let mut term = ZmTerm::new(80, 24, 10_000, CellColor::WHITE, CellColor::BLACK).unwrap();
        term.feed_bytes(b"\x1b]9;\x07");
        let events = term.drain_osc_events();
        assert!(events.is_empty(), "empty OSC 9 should not emit");
    }

    #[test]
    fn drain_clears_buffer() {
        let mut term = ZmTerm::new(80, 24, 10_000, CellColor::WHITE, CellColor::BLACK).unwrap();
        term.feed_bytes(b"\x1b]9;a\x07\x1b]9;b\x07");
        let events1 = term.drain_osc_events();
        assert_eq!(events1.len(), 2);
        let events2 = term.drain_osc_events();
        assert!(events2.is_empty());
    }

    #[test]
    fn osc_777_short_form_ignored() {
        let mut term = ZmTerm::new(80, 24, 10_000, CellColor::WHITE, CellColor::BLACK).unwrap();
        // urxvt OSC 777 with non-"notify" subtype — out of our scope.
        term.feed_bytes(b"\x1b]777;set;something\x07");
        let events = term.drain_osc_events();
        assert!(events.is_empty());
    }

    #[test]
    fn search_matches_literal() {
        let mut term = ZmTerm::new(80, 24, 10_000, CellColor::WHITE, CellColor::BLACK).unwrap();
        term.feed_bytes(b"hello TODO world\r\nanother TODO line");
        let m = term.search("TODO");
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].row, 0);
        assert_eq!(m[0].col, 6);
        assert_eq!(m[0].len, 4);
        assert_eq!(m[1].row, 1);
        assert_eq!(m[1].col, 8);
    }

    #[test]
    fn search_supports_regex() {
        let mut term = ZmTerm::new(80, 24, 10_000, CellColor::WHITE, CellColor::BLACK).unwrap();
        term.feed_bytes(b"err 42, err 7, info, err 100");
        let m = term.search(r"err \d+");
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn search_empty_pattern_returns_empty() {
        let mut term = ZmTerm::new(80, 24, 10_000, CellColor::WHITE, CellColor::BLACK).unwrap();
        term.feed_bytes(b"some text here");
        assert!(term.search("").is_empty());
    }

    #[test]
    fn search_invalid_regex_returns_empty() {
        let mut term = ZmTerm::new(80, 24, 10_000, CellColor::WHITE, CellColor::BLACK).unwrap();
        term.feed_bytes(b"some text");
        assert!(term.search("[unclosed").is_empty());
    }

    #[test]
    fn search_no_match_returns_empty() {
        let mut term = ZmTerm::new(80, 24, 10_000, CellColor::WHITE, CellColor::BLACK).unwrap();
        term.feed_bytes(b"abcdef");
        assert!(term.search("zzz").is_empty());
    }
}
