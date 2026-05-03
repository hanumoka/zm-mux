use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MuxMethod {
    ListPanes,
    GetStatus,
    SendKeys,
    FocusPane,
    SplitPane,
    ClosePane,
    CreateTab,
    CloseTab,
    SetAgentInfo,
}

impl MuxMethod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ListPanes => "mux.list_panes",
            Self::GetStatus => "mux.get_status",
            Self::SendKeys => "mux.send_keys",
            Self::FocusPane => "mux.focus_pane",
            Self::SplitPane => "mux.split_pane",
            Self::ClosePane => "mux.close_pane",
            Self::CreateTab => "mux.create_tab",
            Self::CloseTab => "mux.close_tab",
            Self::SetAgentInfo => "mux.set_agent_info",
        }
    }

    pub fn parse_method(s: &str) -> Option<Self> {
        match s {
            "mux.list_panes" => Some(Self::ListPanes),
            "mux.get_status" => Some(Self::GetStatus),
            "mux.send_keys" => Some(Self::SendKeys),
            "mux.focus_pane" => Some(Self::FocusPane),
            "mux.split_pane" => Some(Self::SplitPane),
            "mux.close_pane" => Some(Self::ClosePane),
            "mux.create_tab" => Some(Self::CreateTab),
            "mux.close_tab" => Some(Self::CloseTab),
            "mux.set_agent_info" => Some(Self::SetAgentInfo),
            _ => None,
        }
    }

    pub fn is_mux_method(s: &str) -> bool {
        s.starts_with("mux.")
    }
}

pub const PANE_NOT_FOUND: i32 = -32001;
pub const TAB_NOT_FOUND: i32 = -32002;
pub const CANNOT_CLOSE_LAST_PANE: i32 = -32003;
pub const CANNOT_CLOSE_LAST_TAB: i32 = -32004;
pub const SPLIT_FAILED: i32 = -32005;

// ---- mux.list_panes ---------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListPanesParams {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPanesResult {
    pub panes: Vec<PaneInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneInfo {
    pub pane_id: u32,
    pub tab_id: u32,
    pub focused: bool,
    pub cols: u16,
    pub rows: u16,
    pub title: String,
    #[serde(default)]
    pub agent_type: String,
    #[serde(default)]
    pub agent_status: String,
}

// ---- mux.send_keys ----------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendKeysParams {
    pub pane_id: u32,
    pub data: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SendKeysResult {}

// ---- mux.focus_pane ---------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusPaneParams {
    pub pane_id: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FocusPaneResult {}

// ---- mux.split_pane ---------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitPaneParams {
    pub pane_id: u32,
    pub direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitPaneResult {
    pub new_pane_id: u32,
}

// ---- mux.close_pane ---------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosePaneParams {
    pub pane_id: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClosePaneResult {}

// ---- mux.create_tab ---------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateTabParams {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTabResult {
    pub tab_id: u32,
    pub pane_id: u32,
}

// ---- mux.close_tab ----------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseTabParams {
    pub tab_id: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CloseTabResult {}

// ---- mux.set_agent_info -----------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetAgentInfoParams {
    pub pane_id: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_status: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SetAgentInfoResult {}

// ---- mux.get_status ---------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GetStatusParams {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetStatusResult {
    pub workspace_id: String,
    pub pid: u32,
    pub version: String,
    pub active_tab: u32,
    pub pane_count: usize,
    pub tab_count: usize,
    pub socket_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mux_method_str_roundtrip() {
        for m in [
            MuxMethod::ListPanes,
            MuxMethod::GetStatus,
            MuxMethod::SendKeys,
            MuxMethod::FocusPane,
            MuxMethod::SplitPane,
            MuxMethod::ClosePane,
            MuxMethod::CreateTab,
            MuxMethod::CloseTab,
            MuxMethod::SetAgentInfo,
        ] {
            let s = m.as_str();
            assert_eq!(MuxMethod::parse_method(s), Some(m));
        }
    }

    #[test]
    fn is_mux_method_prefix() {
        assert!(MuxMethod::is_mux_method("mux.list_panes"));
        assert!(MuxMethod::is_mux_method("mux.get_status"));
        assert!(MuxMethod::is_mux_method("mux.unknown_future"));
        assert!(!MuxMethod::is_mux_method("initialize"));
        assert!(!MuxMethod::is_mux_method("spawn_agent"));
    }

    #[test]
    fn unknown_method_returns_none() {
        assert_eq!(MuxMethod::parse_method("mux.nonexistent"), None);
        assert_eq!(MuxMethod::parse_method("initialize"), None);
    }

    #[test]
    fn list_panes_result_serde() {
        let result = ListPanesResult {
            panes: vec![PaneInfo {
                pane_id: 0,
                tab_id: 0,
                focused: true,
                cols: 80,
                rows: 24,
                title: "powershell".to_string(),
                agent_type: "unknown".to_string(),
                agent_status: "unknown".to_string(),
            }],
        };
        let json = serde_json::to_value(&result).unwrap();
        let back: ListPanesResult = serde_json::from_value(json).unwrap();
        assert_eq!(back.panes.len(), 1);
        assert_eq!(back.panes[0].pane_id, 0);
        assert!(back.panes[0].focused);
    }

    #[test]
    fn get_status_result_serde() {
        let result = GetStatusResult {
            workspace_id: "ws-abc123".to_string(),
            pid: 12345,
            version: "0.1.0".to_string(),
            active_tab: 0,
            pane_count: 2,
            tab_count: 1,
            socket_path: "zm-mux-12345".to_string(),
        };
        let json = serde_json::to_value(&result).unwrap();
        let back: GetStatusResult = serde_json::from_value(json).unwrap();
        assert_eq!(back.pid, 12345);
        assert_eq!(back.pane_count, 2);
    }

    #[test]
    fn list_panes_params_empty_object() {
        let json = serde_json::json!({});
        let _p: ListPanesParams = serde_json::from_value(json).unwrap();
    }

    #[test]
    fn get_status_params_empty_object() {
        let json = serde_json::json!({});
        let _p: GetStatusParams = serde_json::from_value(json).unwrap();
    }
}
