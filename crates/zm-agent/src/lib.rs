//! zm-agent — 에이전트 연동(트랙 A) 헬퍼.
//!
//! pane에 주입할 tmux-shim 환경변수 산출. zm-app이 이를 PATH 프리펜드와 함께 적용한다.
//! Claude Code 등은 `TMUX`/`TMUX_PANE` 존재로 "tmux 안"이라 판단하고, PATH 상의 `tmux`
//! shim(별도 바이너리)을 호출 → zm-ipc로 변환된다. (docs/research/04 트랙 A)

/// pane에 주입할 tmux-shim 환경변수 목록.
///
/// - `TMUX`      : 가짜 tmux 존재 신호(형식 `socket,pid,session`; shim은 내용 무시).
/// - `TMUX_PANE` : `%<id>`.
/// - `TERM`      : `xterm-256color`(호환 폭 넓음).
/// - `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` : (claude_teams=true 시) 에이전트 팀 게이트.
pub fn tmux_env(pane_id: u64, socket: &str, claude_teams: bool) -> Vec<(&'static str, String)> {
    let server_pid = std::process::id();
    let mut v = vec![
        ("TMUX", format!("{socket},{server_pid},0")),
        ("TMUX_PANE", format!("%{pane_id}")),
        ("TERM", "xterm-256color".to_string()),
    ];
    if claude_teams {
        v.push(("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS", "1".to_string()));
    }
    v
}

/// tmux 타깃 문자열("%N" 등) → pane id.
pub fn parse_target(t: &str) -> Option<u64> {
    let s = t.trim();
    let s = s.strip_prefix('%').unwrap_or(s);
    // "session:window.pane" 형식이면 마지막 숫자만.
    let last = s.rsplit(['.', ':']).next().unwrap_or(s);
    last.parse().ok()
}

/// tmux 포맷 문자열에 pane id 치환(최소 지원).
pub fn format_pane(fmt: &str, pane_id: u64) -> String {
    fmt.replace("#{pane_id}", &format!("%{pane_id}"))
        .replace("#{pane_index}", &pane_id.to_string())
        .replace("#D", &format!("%{pane_id}"))
}

/// tmux send-keys 키 토큰 → 바이트(비literal). 알 수 없으면 그대로.
pub fn translate_key(token: &str) -> String {
    match token {
        "Enter" | "C-m" | "KPEnter" => "\r".to_string(),
        "Tab" | "C-i" => "\t".to_string(),
        "Space" => " ".to_string(),
        "Escape" | "Esc" => "\x1b".to_string(),
        "BSpace" | "DC" => "\x7f".to_string(),
        "Up" => "\x1b[A".to_string(),
        "Down" => "\x1b[B".to_string(),
        "Right" => "\x1b[C".to_string(),
        "Left" => "\x1b[D".to_string(),
        _ => {
            // "C-x" → Ctrl 바이트
            if let Some(rest) = token.strip_prefix("C-") {
                if let Some(c) = rest.chars().next() {
                    let lower = c.to_ascii_lowercase();
                    if lower.is_ascii_alphabetic() {
                        return ((lower as u8 - b'a' + 1) as char).to_string();
                    }
                }
            }
            token.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets() {
        assert_eq!(parse_target("%3"), Some(3));
        assert_eq!(parse_target("sess:1.2"), Some(2));
        assert_eq!(parse_target("7"), Some(7));
        assert_eq!(parse_target("x"), None);
    }

    #[test]
    fn keys() {
        assert_eq!(translate_key("Enter"), "\r");
        assert_eq!(translate_key("C-c"), "\u{3}");
        assert_eq!(translate_key("Space"), " ");
        assert_eq!(translate_key("hello"), "hello");
    }

    #[test]
    fn format() {
        assert_eq!(format_pane("#{pane_id}", 2), "%2");
        assert_eq!(format_pane("#{pane_index}", 2), "2");
    }

    #[test]
    fn env_has_tmux() {
        let v = tmux_env(5, "sock", true);
        assert!(v.iter().any(|(k, val)| *k == "TMUX_PANE" && val == "%5"));
        assert!(v.iter().any(|(k, _)| *k == "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS"));
    }
}
