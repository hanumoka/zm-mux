pub mod worktree;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    Claude,
    Codex,
    Gemini,
    Unknown,
}

impl AgentType {
    pub fn parse(s: &str) -> Self {
        match s {
            "claude" => Self::Claude,
            "codex" => Self::Codex,
            "gemini" => Self::Gemini,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Unknown,
    Waiting,
    Active,
    Complete,
    Error,
}

impl AgentStatus {
    pub fn parse(s: &str) -> Self {
        match s {
            "waiting" => Self::Waiting,
            "active" => Self::Active,
            "complete" => Self::Complete,
            "error" => Self::Error,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Waiting => "waiting",
            Self::Active => "active",
            Self::Complete => "complete",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub agent_type: AgentType,
    pub status: AgentStatus,
}

impl Default for AgentInfo {
    fn default() -> Self {
        Self {
            agent_type: AgentType::Unknown,
            status: AgentStatus::Unknown,
        }
    }
}

const BORDER_FOCUSED: (u8, u8, u8) = (0x44, 0x88, 0xFF);
const BORDER_UNFOCUSED: (u8, u8, u8) = (0x44, 0x44, 0x44);
const BORDER_WAITING: (u8, u8, u8) = (0x44, 0x88, 0xFF);
const BORDER_ACTIVE: (u8, u8, u8) = (0x44, 0xCC, 0x44);
const BORDER_COMPLETE: (u8, u8, u8) = (0x88, 0x88, 0x88);
const BORDER_ERROR: (u8, u8, u8) = (0xFF, 0x44, 0x44);

impl AgentInfo {
    pub fn border_color_srgb(&self, focused: bool) -> (u8, u8, u8) {
        if self.agent_type == AgentType::Unknown {
            return if focused { BORDER_FOCUSED } else { BORDER_UNFOCUSED };
        }
        match self.status {
            AgentStatus::Waiting => BORDER_WAITING,
            AgentStatus::Active => BORDER_ACTIVE,
            AgentStatus::Complete => BORDER_COMPLETE,
            AgentStatus::Error => BORDER_ERROR,
            AgentStatus::Unknown => {
                if focused { BORDER_FOCUSED } else { BORDER_UNFOCUSED }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_agent_info() {
        let info = AgentInfo::default();
        assert_eq!(info.agent_type, AgentType::Unknown);
        assert_eq!(info.status, AgentStatus::Unknown);
    }

    #[test]
    fn agent_type_parse_roundtrip() {
        for t in [AgentType::Claude, AgentType::Codex, AgentType::Gemini, AgentType::Unknown] {
            assert_eq!(AgentType::parse(t.as_str()), t);
        }
    }

    #[test]
    fn agent_status_parse_roundtrip() {
        for s in [
            AgentStatus::Unknown, AgentStatus::Waiting, AgentStatus::Active,
            AgentStatus::Complete, AgentStatus::Error,
        ] {
            assert_eq!(AgentStatus::parse(s.as_str()), s);
        }
    }

    #[test]
    fn agent_type_serde_roundtrip() {
        let t = AgentType::Claude;
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, "\"claude\"");
        let back: AgentType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn border_color_unknown_agent_uses_focused() {
        let info = AgentInfo::default();
        assert_eq!(info.border_color_srgb(true), BORDER_FOCUSED);
        assert_eq!(info.border_color_srgb(false), BORDER_UNFOCUSED);
    }

    #[test]
    fn border_color_active_agent() {
        let info = AgentInfo {
            agent_type: AgentType::Claude,
            status: AgentStatus::Active,
        };
        assert_eq!(info.border_color_srgb(true), BORDER_ACTIVE);
        assert_eq!(info.border_color_srgb(false), BORDER_ACTIVE);
    }

    #[test]
    fn border_color_error_agent() {
        let info = AgentInfo {
            agent_type: AgentType::Codex,
            status: AgentStatus::Error,
        };
        assert_eq!(info.border_color_srgb(true), BORDER_ERROR);
    }

    #[test]
    fn border_color_known_agent_unknown_status_falls_back() {
        let info = AgentInfo {
            agent_type: AgentType::Gemini,
            status: AgentStatus::Unknown,
        };
        assert_eq!(info.border_color_srgb(true), BORDER_FOCUSED);
        assert_eq!(info.border_color_srgb(false), BORDER_UNFOCUSED);
    }
}
