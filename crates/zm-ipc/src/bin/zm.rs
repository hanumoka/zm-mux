//! zm — zm-mux 자동화 CLI. 실행 중인 zm-mux(ZM_MUX_SOCKET)에 명령 전송.
//!
//! 사용:
//!   zm list-panes | list-tabs
//!   zm split [-v|-h]          (-v 좌우(기본), -h 상하)
//!   zm new-tab
//!   zm focus <left|right|up|down>
//!   zm select-pane <id>
//!   zm send-keys <data> [--pane <id>]
//!   zm capture-pane [--pane <id>]
//!   zm kill-pane <id>

use zm_ipc::{send_command, socket_name, Command};

fn usage() -> ! {
    eprintln!(
        "usage: zm <list-panes|list-tabs|split [-v|-h]|new-tab|focus DIR|\
select-pane ID|send-keys DATA [--pane ID]|capture-pane [--pane ID]|kill-pane ID>"
    );
    std::process::exit(2);
}

fn parse(args: &[String]) -> Option<Command> {
    let (head, rest) = args.split_first()?;
    Some(match head.as_str() {
        "list-panes" => Command::ListPanes,
        "list-tabs" => Command::ListTabs,
        "new-tab" | "new-window" => Command::NewTab,
        "split" | "split-window" => {
            let vertical = !rest.iter().any(|a| a == "-h");
            Command::Split { vertical }
        }
        "focus" | "select-direction" => Command::Focus {
            dir: rest.first()?.clone(),
        },
        "select-pane" => Command::SelectPane {
            id: rest.first()?.parse().ok()?,
        },
        "kill-pane" => Command::KillPane {
            id: rest.first()?.parse().ok()?,
        },
        "send-keys" => {
            let pane = pane_flag(rest);
            // "--pane <id>"를 제외한 나머지를 data로(순서 무관).
            let mut data_parts = Vec::new();
            let mut i = 0;
            while i < rest.len() {
                if rest[i] == "--pane" {
                    i += 2;
                    continue;
                }
                data_parts.push(rest[i].clone());
                i += 1;
            }
            Command::SendKeys {
                pane,
                data: data_parts.join(" "),
            }
        }
        "capture-pane" => Command::CapturePane {
            pane: pane_flag(rest),
        },
        _ => return None,
    })
}

fn pane_flag(args: &[String]) -> Option<u64> {
    let i = args.iter().position(|a| a == "--pane")?;
    args.get(i + 1)?.parse().ok()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = parse(&args) else { usage() };

    let sock = std::env::var("ZM_MUX_SOCKET")
        .unwrap_or_else(|_| socket_name(std::process::id()));

    match send_command(&sock, &cmd) {
        Ok(resp) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&resp).unwrap_or_else(|_| "{}".into())
            );
            if !resp.ok {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("zm: 연결 실패({sock}): {e}");
            std::process::exit(1);
        }
    }
}
