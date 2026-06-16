//! tmux — zm-mux의 tmux 호환 shim.
//!
//! PATH 앞에 놓여 Claude Code 등의 `tmux ...` 호출을 가로채 zm-ipc 명령으로 변환한다.
//! 트랙 A의 핵심: 에이전트가 "tmux 안"이라 믿고 분할/전송을 시도하면 zm-mux가 수행.
//!
//! 지원(부분): -V, split-window, send-keys, select-pane, list-panes, new-window,
//! kill-pane, display-message. 미지원 서브커맨드는 성공(exit 0)으로 흡수해 에이전트 흐름 유지.

use std::process::exit;

use zm_agent::{format_pane, parse_target, translate_key};
use zm_ipc::{send_command, Command, Response};

fn socket() -> String {
    std::env::var("ZM_MUX_SOCKET").unwrap_or_default()
}

fn dispatch(cmd: &Command) -> Result<Response, String> {
    let sock = socket();
    if sock.is_empty() {
        return Err("ZM_MUX_SOCKET 미설정(zm-mux 밖)".into());
    }
    send_command(&sock, cmd).map_err(|e| e.to_string())
}

/// 플래그 값(`-t %1` 형태) 추출.
fn flag_value(args: &[String], flag: &str) -> Option<String> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1).cloned()
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sub = args.first().map(|s| s.as_str()).unwrap_or("");
    let rest = if args.is_empty() { &[][..] } else { &args[1..] };

    let code = match sub {
        "-V" | "--version" => {
            println!("tmux 3.4");
            0
        }
        "split-window" | "splitw" => cmd_split(rest),
        "send-keys" | "send" => cmd_send_keys(rest),
        "select-pane" | "selectp" => cmd_select_pane(rest),
        "list-panes" | "lsp" => cmd_list_panes(rest),
        "new-window" | "neww" => cmd_new_window(rest),
        "kill-pane" | "killp" => cmd_kill_pane(rest),
        "display-message" | "display" | "displayp" => cmd_display(rest),
        // 자주 오는 무해 명령: 성공 흡수.
        "set-option" | "set" | "setw" | "set-window-option" | "set-hook" | "bind-key" | "bind"
        | "set-environment" | "setenv" | "rename-window" | "select-window" | "selectw" | "" => 0,
        _ => 0, // 미지원도 성공 흡수(에이전트 중단 방지)
    };
    exit(code);
}

fn cmd_split(args: &[String]) -> i32 {
    // tmux: -h = 좌/우(side by side), -v = 상/하. (직관과 반대)
    let vertical = if has_flag(args, "-h") {
        true
    } else if has_flag(args, "-v") {
        false
    } else {
        false // tmux 기본 = 상/하
    };
    // -t 타깃이 있으면 먼저 포커스.
    if let Some(t) = flag_value(args, "-t").and_then(|t| parse_target(&t)) {
        let _ = dispatch(&Command::SelectPane { id: t });
    }
    match dispatch(&Command::Split { vertical }) {
        Ok(resp) if resp.ok => {
            let id = resp.data.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            if has_flag(args, "-P") {
                let fmt = flag_value(args, "-F").unwrap_or_else(|| "#{pane_id}".to_string());
                println!("{}", format_pane(&fmt, id));
            }
            0
        }
        Ok(resp) => {
            eprintln!("tmux(shim): {}", resp.error.unwrap_or_default());
            1
        }
        Err(e) => {
            eprintln!("tmux(shim): {e}");
            1
        }
    }
}

fn cmd_send_keys(args: &[String]) -> i32 {
    let pane = flag_value(args, "-t").and_then(|t| parse_target(&t));
    let literal = has_flag(args, "-l");
    // 플래그/값 토큰 제거 후 키 토큰만 수집.
    let mut keys: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-t" | "-N" | "-c" => i += 2, // 값 있는 플래그 스킵
            "-l" | "-R" | "-M" | "-K" => i += 1,
            other => {
                keys.push(other.to_string());
                i += 1;
            }
        }
    }
    let data: String = if literal {
        keys.concat()
    } else {
        keys.iter().map(|k| translate_key(k)).collect()
    };
    match dispatch(&Command::SendKeys { pane, data }) {
        Ok(resp) if resp.ok => 0,
        _ => 1,
    }
}

fn cmd_select_pane(args: &[String]) -> i32 {
    let cmd = if has_flag(args, "-L") {
        Command::Focus { dir: "left".into() }
    } else if has_flag(args, "-R") {
        Command::Focus { dir: "right".into() }
    } else if has_flag(args, "-U") {
        Command::Focus { dir: "up".into() }
    } else if has_flag(args, "-D") {
        Command::Focus { dir: "down".into() }
    } else if let Some(t) = flag_value(args, "-t").and_then(|t| parse_target(&t)) {
        Command::SelectPane { id: t }
    } else {
        return 0;
    };
    match dispatch(&cmd) {
        Ok(resp) if resp.ok => 0,
        _ => 1,
    }
}

fn cmd_list_panes(args: &[String]) -> i32 {
    match dispatch(&Command::ListPanes) {
        Ok(resp) if resp.ok => {
            let fmt = flag_value(args, "-F");
            if let Some(arr) = resp.data.as_array() {
                for p in arr {
                    let id = p.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                    let active = p.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
                    let cols = p.get("cols").and_then(|v| v.as_u64()).unwrap_or(0);
                    let rows = p.get("rows").and_then(|v| v.as_u64()).unwrap_or(0);
                    match &fmt {
                        Some(f) => {
                            let line = format_pane(f, id)
                                .replace("#{pane_active}", if active { "1" } else { "0" })
                                .replace("#{pane_width}", &cols.to_string())
                                .replace("#{pane_height}", &rows.to_string());
                            println!("{line}");
                        }
                        None => println!(
                            "%{id}: [{cols}x{rows}]{}",
                            if active { " (active)" } else { "" }
                        ),
                    }
                }
            }
            0
        }
        _ => 1,
    }
}

fn cmd_new_window(args: &[String]) -> i32 {
    match dispatch(&Command::NewTab) {
        Ok(resp) if resp.ok => {
            let id = resp.data.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            if has_flag(args, "-P") {
                let fmt = flag_value(args, "-F").unwrap_or_else(|| "#{pane_id}".to_string());
                println!("{}", format_pane(&fmt, id));
            }
            0
        }
        _ => 1,
    }
}

fn cmd_kill_pane(args: &[String]) -> i32 {
    let Some(id) = flag_value(args, "-t").and_then(|t| parse_target(&t)) else {
        return 0;
    };
    match dispatch(&Command::KillPane { id }) {
        Ok(resp) if resp.ok => 0,
        _ => 1,
    }
}

fn cmd_display(args: &[String]) -> i32 {
    // display-message -p "<format>" : 현재 pane 기준 최소 치환.
    if has_flag(args, "-p") {
        let pane_env = std::env::var("TMUX_PANE").unwrap_or_else(|_| "%0".into());
        let id: u64 = pane_env.trim_start_matches('%').parse().unwrap_or(0);
        // 마지막 위치 인자를 포맷으로 간주.
        let fmt = args
            .iter()
            .rev()
            .find(|a| !a.starts_with('-'))
            .cloned()
            .unwrap_or_else(|| "#{pane_id}".to_string());
        println!("{}", format_pane(&fmt, id));
    }
    0
}
