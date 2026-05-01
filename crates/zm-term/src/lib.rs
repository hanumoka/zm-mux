use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::term::Config as TermConfig;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi;
use std::sync::{Arc, Mutex};
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
    events: Arc<Mutex<Vec<Event>>>,
}

impl ZmTerm {
    pub fn new(cols: u16, rows: u16) -> ZmResult<Self> {
        let size = TermSize { cols, rows };
        let config = TermConfig::default();
        let event_collector = EventCollector::new();
        let events = event_collector.shared();
        let term = Term::new(config, &size, event_collector);
        let parser = ansi::Processor::new();

        Ok(Self {
            term,
            parser,
            events,
        })
    }

    pub fn feed_bytes(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
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

        let fg = match cell.fg {
            Color::Spec(rgb) => CellColor::new(rgb.r, rgb.g, rgb.b),
            Color::Named(name) => named_color_to_rgb(name),
            Color::Indexed(idx) => indexed_color_to_rgb(idx),
        };

        let bg = match cell.bg {
            Color::Spec(rgb) => CellColor::new(rgb.r, rgb.g, rgb.b),
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
        let term = ZmTerm::new(80, 24);
        assert!(term.is_ok());
        let term = term.unwrap();
        assert_eq!(term.cols(), 80);
        assert_eq!(term.rows(), 24);
    }

    #[test]
    fn feed_plain_text() {
        let mut term = ZmTerm::new(80, 24).unwrap();
        term.feed_bytes(b"Hello, zm-mux!");
        let row = term.row_text(0);
        assert!(row.contains("Hello, zm-mux!"), "got: '{}'", row);
    }

    #[test]
    fn feed_ansi_color() {
        let mut term = ZmTerm::new(80, 24).unwrap();
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
        let mut term = ZmTerm::new(80, 24).unwrap();
        term.feed_bytes(b"line1\r\nline2");
        assert!(term.row_text(0).contains("line1"));
        assert!(term.row_text(1).contains("line2"));
    }

    #[test]
    fn cursor_position() {
        let mut term = ZmTerm::new(80, 24).unwrap();
        term.feed_bytes(b"ABC");
        let (row, col) = term.cursor_position();
        assert_eq!(row, 0);
        assert_eq!(col, 3);
    }

    #[test]
    fn resize_term() {
        let mut term = ZmTerm::new(80, 24).unwrap();
        term.resize(40, 12);
        assert_eq!(term.cols(), 40);
        assert_eq!(term.rows(), 12);
    }

    #[test]
    fn bold_text() {
        let mut term = ZmTerm::new(80, 24).unwrap();
        term.feed_bytes(b"\x1b[1mBOLD\x1b[0m");
        let cell = term.render_cell(0, 0);
        assert_eq!(cell.c, 'B');
        assert!(cell.bold, "Should be bold");
    }
}
