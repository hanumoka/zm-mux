use crate::{PaneId, PaneTree};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub u32);

#[derive(Debug)]
pub struct Tab {
    pub id: TabId,
    pub tree: PaneTree,
    pub focused_pane: PaneId,
    pub title: Option<String>,
}

#[derive(Debug)]
pub struct TabSet {
    tabs: Vec<Tab>,
    active: TabId,
    next_tab_id: u32,
    next_pane_id: u32,
}

impl TabSet {
    pub fn new() -> (Self, PaneId) {
        let initial_pane = PaneId(0);
        let initial_tab_id = TabId(0);
        let tab = Tab {
            id: initial_tab_id,
            tree: PaneTree::with_initial_pane(initial_pane),
            focused_pane: initial_pane,
            title: None,
        };
        let set = Self {
            tabs: vec![tab],
            active: initial_tab_id,
            next_tab_id: 1,
            next_pane_id: 1,
        };
        (set, initial_pane)
    }

    pub fn alloc_pane_id(&mut self) -> PaneId {
        let id = PaneId(self.next_pane_id);
        self.next_pane_id += 1;
        id
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn active_id(&self) -> TabId {
        self.active
    }

    pub fn active(&self) -> &Tab {
        self.tabs
            .iter()
            .find(|t| t.id == self.active)
            .expect("active tab id must always be present in tabs")
    }

    pub fn active_mut(&mut self) -> &mut Tab {
        let active = self.active;
        self.tabs
            .iter_mut()
            .find(|t| t.id == active)
            .expect("active tab id must always be present in tabs")
    }

    pub fn active_index(&self) -> usize {
        self.tabs
            .iter()
            .position(|t| t.id == self.active)
            .unwrap_or(0)
    }

    /// Create a new tab with one initial pane and switch focus to it.
    /// Returns the new TabId and the PaneId of its first pane so the
    /// caller can spawn a PTY for it.
    pub fn create_tab(&mut self) -> (TabId, PaneId) {
        let new_tab_id = TabId(self.next_tab_id);
        self.next_tab_id += 1;
        let initial_pane = self.alloc_pane_id();
        let tab = Tab {
            id: new_tab_id,
            tree: PaneTree::with_initial_pane(initial_pane),
            focused_pane: initial_pane,
            title: None,
        };
        self.tabs.push(tab);
        self.active = new_tab_id;
        (new_tab_id, initial_pane)
    }

    /// Close the active tab and return the list of PaneIds that lived in
    /// it so the caller can kill their PTYs.  No-op (returns empty Vec)
    /// when only one tab remains — a single-tab close would leave the
    /// app with no focusable surface.
    pub fn close_active(&mut self) -> Vec<PaneId> {
        if self.tabs.len() <= 1 {
            return Vec::new();
        }
        let idx = self.active_index();
        let removed = self.tabs.remove(idx);
        let new_idx = idx.min(self.tabs.len() - 1);
        self.active = self.tabs[new_idx].id;
        removed.tree.pane_ids()
    }

    pub fn switch_to(&mut self, target: TabId) -> bool {
        if self.tabs.iter().any(|t| t.id == target) {
            self.active = target;
            true
        } else {
            false
        }
    }

    pub fn switch_next(&mut self) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        let next = (self.active_index() + 1) % self.tabs.len();
        self.active = self.tabs[next].id;
        true
    }

    pub fn switch_prev(&mut self) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        let cur = self.active_index();
        let prev = if cur == 0 {
            self.tabs.len() - 1
        } else {
            cur - 1
        };
        self.active = self.tabs[prev].id;
        true
    }

    /// Switch to tab by 0-based index.  Returns false if no such tab.
    /// Caller is responsible for any 1-based key mapping (Ctrl+1 → 0).
    pub fn switch_to_index(&mut self, idx: usize) -> bool {
        if let Some(tab) = self.tabs.get(idx) {
            self.active = tab.id;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SplitDirection;

    #[test]
    fn new_has_one_tab_with_initial_pane() {
        let (set, initial) = TabSet::new();
        assert_eq!(set.tab_count(), 1);
        assert_eq!(initial, PaneId(0));
        assert_eq!(set.active().focused_pane, PaneId(0));
        assert_eq!(set.active().tree.pane_count(), 1);
    }

    #[test]
    fn alloc_pane_id_is_global_and_monotonic() {
        let (mut set, _) = TabSet::new();
        let a = set.alloc_pane_id();
        let b = set.alloc_pane_id();
        assert_ne!(a, b);
        assert_eq!(a.0 + 1, b.0);
    }

    #[test]
    fn create_tab_auto_switches_and_uses_global_pane_id() {
        let (mut set, initial) = TabSet::new();
        assert_eq!(initial, PaneId(0));

        let (tab_id, new_pane) = set.create_tab();
        assert_eq!(set.tab_count(), 2);
        assert_eq!(set.active_id(), tab_id);
        // PaneId is global across tabs — second tab's first pane is not 0.
        assert_ne!(new_pane, PaneId(0));
    }

    #[test]
    fn close_active_returns_panes_and_switches_focus() {
        let (mut set, _initial) = TabSet::new();
        let (tab1, _) = set.create_tab();
        let (tab2, _) = set.create_tab(); // active = tab2

        // Split a pane in the active tab so close_active returns 2 ids.
        let new_pane_id = set.alloc_pane_id();
        let focused = set.active().focused_pane;
        assert!(set
            .active_mut()
            .tree
            .split(focused, SplitDirection::Horizontal, new_pane_id));

        let killed = set.close_active();
        assert_eq!(killed.len(), 2);
        assert_eq!(set.tab_count(), 2);
        assert_ne!(set.active_id(), tab2); // removed tab is no longer active
        // Closing the rightmost tab falls back to its predecessor.
        assert_eq!(set.active_id(), tab1);
    }

    #[test]
    fn close_active_noop_when_one_tab() {
        let (mut set, _) = TabSet::new();
        assert_eq!(set.close_active(), Vec::<PaneId>::new());
        assert_eq!(set.tab_count(), 1);
    }

    #[test]
    fn switch_next_prev_wraps() {
        let (mut set, _) = TabSet::new();
        let (t1, _) = set.create_tab();
        let (t2, _) = set.create_tab();
        // active = t2 now
        assert!(set.switch_next()); // wraps to first
        assert_eq!(set.active_id(), TabId(0));
        assert!(set.switch_prev()); // wraps to t2
        assert_eq!(set.active_id(), t2);
        assert!(set.switch_to(t1));
        assert_eq!(set.active_id(), t1);
    }

    #[test]
    fn switch_to_index_bounds() {
        let (mut set, _) = TabSet::new();
        let _ = set.create_tab();
        let _ = set.create_tab(); // 3 tabs total

        assert!(set.switch_to_index(0));
        assert_eq!(set.active_id(), TabId(0));
        assert!(set.switch_to_index(2));
        assert_eq!(set.active_id(), TabId(2));
        assert!(!set.switch_to_index(99));
    }

    #[test]
    fn switch_unknown_tab_returns_false() {
        let (mut set, _) = TabSet::new();
        assert!(!set.switch_to(TabId(99)));
    }
}
