use serde::{Deserialize, Serialize};

use crate::{PaneNode, PaneTree, PaneId, tabs::{Tab, TabId, TabSet}};

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub name: String,
    pub timestamp: String,
    pub tabs: Vec<TabSnapshot>,
    pub active_tab: u32,
    pub next_tab_id: u32,
    pub next_pane_id: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TabSnapshot {
    pub id: u32,
    pub title: Option<String>,
    pub focused_pane: u32,
    pub tree: PaneNode,
}

impl SessionSnapshot {
    pub fn from_tab_set(tabs: &TabSet, name: &str) -> Self {
        let timestamp = {
            let d = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            format!("{}", d.as_secs())
        };

        let tab_snapshots: Vec<TabSnapshot> = tabs
            .tabs()
            .iter()
            .map(|tab| TabSnapshot {
                id: tab.id.0,
                title: tab.title.clone(),
                focused_pane: tab.focused_pane.0,
                tree: clone_node(tab.tree.root_node()),
            })
            .collect();

        Self {
            name: name.to_string(),
            timestamp,
            tabs: tab_snapshots,
            active_tab: tabs.active_id().0,
            next_tab_id: tabs.next_tab_id(),
            next_pane_id: tabs.next_pane_id(),
        }
    }

    pub fn to_tab_set(&self) -> TabSet {
        let tabs: Vec<Tab> = self
            .tabs
            .iter()
            .map(|snap| Tab {
                id: TabId(snap.id),
                tree: PaneTree::from_node(clone_node(&snap.tree)),
                focused_pane: PaneId(snap.focused_pane),
                title: snap.title.clone(),
            })
            .collect();

        TabSet::restore(tabs, TabId(self.active_tab), self.next_tab_id, self.next_pane_id)
    }

    pub fn save_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }

    pub fn load_from_file(path: &std::path::Path) -> std::io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

fn clone_node(node: &PaneNode) -> PaneNode {
    match node {
        PaneNode::Leaf(id) => PaneNode::Leaf(*id),
        PaneNode::Split { direction, ratio, first, second } => PaneNode::Split {
            direction: *direction,
            ratio: *ratio,
            first: Box::new(clone_node(first)),
            second: Box::new(clone_node(second)),
        },
    }
}

pub fn sessions_dir() -> std::path::PathBuf {
    dirs_next().join("sessions")
}

fn dirs_next() -> std::path::PathBuf {
    #[cfg(windows)]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return std::path::PathBuf::from(appdata).join("zm-mux");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return std::path::PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("zm-mux");
        }
    }
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        return std::path::PathBuf::from(xdg).join("zm-mux");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return std::path::PathBuf::from(home).join(".zm-mux");
    }
    std::path::PathBuf::from(".zm-mux")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SplitDirection;

    #[test]
    fn snapshot_roundtrip() {
        let (tabs, _) = TabSet::new();
        let snap = SessionSnapshot::from_tab_set(&tabs, "test");
        let json = serde_json::to_string_pretty(&snap).unwrap();
        let loaded: SessionSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.name, "test");
        assert_eq!(loaded.tabs.len(), 1);
        assert_eq!(loaded.active_tab, 0);
    }

    #[test]
    fn snapshot_with_splits() {
        let (mut tabs, initial) = TabSet::new();
        let new_id = tabs.alloc_pane_id();
        tabs.active_mut().tree.split(initial, SplitDirection::Horizontal, new_id);

        let snap = SessionSnapshot::from_tab_set(&tabs, "split-test");
        let json = serde_json::to_string(&snap).unwrap();
        let loaded: SessionSnapshot = serde_json::from_str(&json).unwrap();

        let restored = loaded.to_tab_set();
        assert_eq!(restored.active().tree.pane_count(), 2);
        assert_eq!(restored.next_pane_id(), tabs.next_pane_id());
        assert_eq!(restored.active().tree.pane_ids(), tabs.active().tree.pane_ids());
    }

    #[test]
    fn snapshot_multi_tab() {
        let (mut tabs, _) = TabSet::new();
        let _ = tabs.create_tab();
        let _ = tabs.create_tab();

        let snap = SessionSnapshot::from_tab_set(&tabs, "multi");
        let restored = snap.to_tab_set();
        assert_eq!(restored.tab_count(), 3);
    }
}
