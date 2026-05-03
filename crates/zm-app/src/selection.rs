use zm_mux::PaneId;
use zm_render::HighlightCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellCoord {
    pub row: usize,
    pub col: usize,
}

pub struct SelectionState {
    pub pane: Option<PaneId>,
    pub anchor: CellCoord,
    pub extent: CellCoord,
    pub dragging: bool,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            pane: None,
            anchor: CellCoord { row: 0, col: 0 },
            extent: CellCoord { row: 0, col: 0 },
            dragging: false,
        }
    }
}

impl SelectionState {
    pub fn is_active(&self) -> bool {
        self.pane.is_some()
    }

    pub fn clear(&mut self) {
        self.pane = None;
        self.dragging = false;
    }

    pub fn ordered(&self) -> (CellCoord, CellCoord) {
        if (self.anchor.row, self.anchor.col) <= (self.extent.row, self.extent.col) {
            (self.anchor, self.extent)
        } else {
            (self.extent, self.anchor)
        }
    }

    pub fn to_highlights(&self, total_cols: usize) -> Vec<HighlightCell> {
        if !self.is_active() {
            return Vec::new();
        }
        let (start, end) = self.ordered();
        let mut out = Vec::new();
        for row in start.row..=end.row {
            let col_start = if row == start.row { start.col } else { 0 };
            let col_end = if row == end.row {
                end.col
            } else {
                total_cols.saturating_sub(1)
            };
            if col_end >= col_start {
                out.push(HighlightCell {
                    row,
                    col: col_start,
                    len: col_end - col_start + 1,
                });
            }
        }
        out
    }

    pub fn start(&mut self, pane: PaneId, coord: CellCoord) {
        self.pane = Some(pane);
        self.anchor = coord;
        self.extent = coord;
        self.dragging = true;
    }

    pub fn extend(&mut self, coord: CellCoord) {
        if self.dragging {
            self.extent = coord;
        }
    }

    pub fn finalize(&mut self) {
        self.dragging = false;
    }

    pub fn select_word<F>(
        &mut self,
        pane: PaneId,
        coord: CellCoord,
        total_cols: usize,
        char_at: F,
    ) where
        F: Fn(usize, usize) -> char,
    {
        let row = coord.row;
        let c = char_at(row, coord.col);
        if c.is_whitespace() || c == '\0' {
            self.pane = Some(pane);
            self.anchor = coord;
            self.extent = coord;
            self.dragging = false;
            return;
        }
        let mut left = coord.col;
        while left > 0 {
            let prev = char_at(row, left - 1);
            if prev.is_whitespace() || prev == '\0' {
                break;
            }
            left -= 1;
        }
        let mut right = coord.col;
        while right + 1 < total_cols {
            let next = char_at(row, right + 1);
            if next.is_whitespace() || next == '\0' {
                break;
            }
            right += 1;
        }
        self.pane = Some(pane);
        self.anchor = CellCoord { row, col: left };
        self.extent = CellCoord { row, col: right };
        self.dragging = false;
    }

    pub fn select_line(&mut self, pane: PaneId, row: usize, total_cols: usize) {
        self.pane = Some(pane);
        self.anchor = CellCoord { row, col: 0 };
        self.extent = CellCoord {
            row,
            col: total_cols.saturating_sub(1),
        };
        self.dragging = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_and_extend() {
        let mut sel = SelectionState::default();
        assert!(!sel.is_active());
        sel.start(PaneId(0), CellCoord { row: 0, col: 5 });
        assert!(sel.is_active());
        assert!(sel.dragging);
        sel.extend(CellCoord { row: 2, col: 10 });
        sel.finalize();
        assert!(!sel.dragging);
        let (start, end) = sel.ordered();
        assert_eq!(start, CellCoord { row: 0, col: 5 });
        assert_eq!(end, CellCoord { row: 2, col: 10 });
    }

    #[test]
    fn reverse_selection_normalizes() {
        let mut sel = SelectionState::default();
        sel.start(PaneId(0), CellCoord { row: 5, col: 10 });
        sel.extend(CellCoord { row: 2, col: 3 });
        let (start, end) = sel.ordered();
        assert_eq!(start, CellCoord { row: 2, col: 3 });
        assert_eq!(end, CellCoord { row: 5, col: 10 });
    }

    #[test]
    fn to_highlights_single_row() {
        let mut sel = SelectionState::default();
        sel.start(PaneId(0), CellCoord { row: 3, col: 2 });
        sel.extend(CellCoord { row: 3, col: 8 });
        let hl = sel.to_highlights(80);
        assert_eq!(hl.len(), 1);
        assert_eq!(hl[0].row, 3);
        assert_eq!(hl[0].col, 2);
        assert_eq!(hl[0].len, 7);
    }

    #[test]
    fn to_highlights_multi_row() {
        let mut sel = SelectionState::default();
        sel.start(PaneId(0), CellCoord { row: 1, col: 5 });
        sel.extend(CellCoord { row: 3, col: 10 });
        let hl = sel.to_highlights(80);
        assert_eq!(hl.len(), 3);
        assert_eq!(hl[0].col, 5);
        assert_eq!(hl[1].col, 0);
        assert_eq!(hl[2].col, 0);
        assert_eq!(hl[2].len, 11);
    }

    #[test]
    fn clear_deactivates() {
        let mut sel = SelectionState::default();
        sel.start(PaneId(0), CellCoord { row: 0, col: 0 });
        sel.clear();
        assert!(!sel.is_active());
    }

    #[test]
    fn select_word_finds_boundaries() {
        let mut sel = SelectionState::default();
        let text = "hello world test";
        let chars: Vec<char> = text.chars().collect();
        sel.select_word(
            PaneId(0),
            CellCoord { row: 0, col: 7 },
            text.len(),
            |_, c| chars.get(c).copied().unwrap_or(' '),
        );
        let (start, end) = sel.ordered();
        assert_eq!(start.col, 6);
        assert_eq!(end.col, 10);
    }

    #[test]
    fn select_line_full_width() {
        let mut sel = SelectionState::default();
        sel.select_line(PaneId(0), 5, 80);
        let (start, end) = sel.ordered();
        assert_eq!(start, CellCoord { row: 5, col: 0 });
        assert_eq!(end, CellCoord { row: 5, col: 79 });
    }
}
