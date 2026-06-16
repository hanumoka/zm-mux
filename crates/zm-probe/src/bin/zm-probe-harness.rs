//! zm-probe-harness — R1(isTTY) 측정 드라이버.
//!
//! node / python / rust 프로브를 (직접 / cmd / pwsh / powershell) 런치 경로로
//! ConPTY(zm-pty) 안에서 실행하고, 자식이 본 `isTTY`/`isatty`를 수집해 표로 출력한다.
//!
//! 종료코드: node-direct 의 stdout.isTTY 가 true 이면 0, 아니면 1 (CI 게이트).
//!
//! 사용:  cargo build -p zm-probe   (두 바이너리 모두 빌드)
//!        cargo run -p zm-probe --bin zm-probe-harness
//!
//! 측정 원리: ConPTY 직속 자식의 isatty 는 CreatePseudoConsole 플래그가 아니라
//! PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE 핸들 속성으로 결정 → 직속 자식 isTTY=true 가
//! 강한 기대값. (docs/research/02 §4, 04 §2, 06 R1)

use std::io::{Read, Write};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use zm_core::GridSize;
use zm_pty::{CommandBuilder, Pty, PtyChannels};

const NODE_SCRIPT: &str = r#"process.stdout.write('ZMPROBE lang=node out='+(process.stdout.isTTY===true)+' in='+(process.stdin.isTTY===true)+' err='+(process.stderr.isTTY===true)+' END\n');
process.stdout.write('ZMPROBE_RAW lang=node rawout='+String(process.stdout.isTTY)+' END\n');
"#;

const PY_SCRIPT: &str = r#"import sys
def b(x): return str(bool(x)).lower()
sys.stdout.write('ZMPROBE lang=py out=%s in=%s err=%s END\n' % (b(sys.stdout.isatty()), b(sys.stdin.isatty()), b(sys.stderr.isatty())))
sys.stdout.flush()
"#;

/// 한 셀(언어 × 런치경로)의 측정 결과.
#[derive(Debug, Clone)]
struct Cell {
    lang: &'static str,
    launch: &'static str,
    out: Option<bool>,
    inp: Option<bool>,
    err: Option<bool>,
    raw_out: Option<String>, // node 의 process.stdout.isTTY 원시값 (false vs undefined 구분)
    status: &'static str,    // PASS / FAIL / TIMEOUT / NO_MARKER / SKIP / SPAWN_ERR
    note: String,
}

/// DIRECT 형태의 프로브(프로그램 + 인자).
struct Probe {
    lang: &'static str,
    prog: String,
    args: Vec<String>,
    available: bool,
}

fn squote(s: &str) -> String {
    // PowerShell 단일 인용(내부 ' → '')
    format!("'{}'", s.replace('\'', "''"))
}

/// 주어진 DIRECT 프로브를 런치경로로 감싼 최종 argv 생성.
fn argv_for(probe: &Probe, launch: &str) -> Vec<String> {
    match launch {
        "direct" => {
            let mut v = vec![probe.prog.clone()];
            v.extend(probe.args.iter().cloned());
            v
        }
        "cmd" => {
            let mut v = vec!["cmd.exe".to_string(), "/c".to_string(), probe.prog.clone()];
            v.extend(probe.args.iter().cloned());
            v
        }
        "pwsh" | "powershell" => {
            let shell = if launch == "pwsh" { "pwsh" } else { "powershell" };
            let mut cmd = format!("& {}", squote(&probe.prog));
            for a in &probe.args {
                cmd.push(' ');
                cmd.push_str(&squote(a));
            }
            vec![
                shell.to_string(),
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                cmd,
            ]
        }
        other => vec![other.to_string()],
    }
}

/// argv 를 ConPTY 자식으로 실행하고 출력을 수집(데드락·타임아웃 안전).
fn run_argv(argv: &[String]) -> (String, &'static str, String) {
    if argv.is_empty() {
        return (String::new(), "SPAWN_ERR", "empty argv".into());
    }
    let mut cmd = CommandBuilder::new(&argv[0]);
    if argv.len() > 1 {
        cmd.args(&argv[1..]);
    }
    // 부모 환경 전체 전파(PATH/COMSPEC/SystemRoot/TEMP 등 보장). Claude 관련 변수는 우리가 설정하지 않음.
    for (k, v) in std::env::vars() {
        cmd.env(k, v);
    }

    let (mut pty, chans) = match Pty::spawn_cmd(cmd, GridSize::new(200, 50), 0, 0) {
        Ok(x) => x,
        Err(e) => return (String::new(), "SPAWN_ERR", e.to_string()),
    };
    let PtyChannels { reader, writer } = chans;

    // reader 스레드가 writer(conin)도 소유:
    //  - conin을 자식 수명 동안 열어두어 ConPTY close 이벤트(STATUS_CONTROL_C_EXIT 0xC000013A) 방지
    //  - ConPTY의 ESC[6n(커서 위치 질의)에 CPR(ESC[1;1R)로 응답해 hang(docs/02 함정) 방지
    let (tx, rx) = mpsc::channel::<String>();
    let reader_handle = thread::spawn(move || {
        let mut reader = reader;
        let mut writer = writer;
        let mut acc: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    let slice = &chunk[..n];
                    acc.extend_from_slice(slice);
                    if slice.windows(4).any(|w| w == b"\x1b[6n") {
                        let _ = writer.write_all(b"\x1b[1;1R");
                        let _ = writer.flush();
                    }
                }
                Err(_) => break,
            }
        }
        let _ = tx.send(String::from_utf8_lossy(&acc).into_owned());
    });

    // 워치독: 10s 내 자식이 끝나지 않으면 kill (hang 방지). 끝나면 done 신호로 즉시 종료.
    let timed_out = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let mut killer = pty.killer();
    let timed_out2 = timed_out.clone();
    let watchdog = thread::spawn(move || {
        use mpsc::RecvTimeoutError::*;
        match done_rx.recv_timeout(Duration::from_secs(10)) {
            Err(Timeout) => {
                timed_out2.store(true, std::sync::atomic::Ordering::SeqCst);
                let _ = killer.kill();
            }
            _ => {}
        }
    });

    // 블로킹 wait(정석 whoami 패턴). 자식이 끝나면 반환.
    let _ = pty.wait();
    let _ = done_tx.send(()); // 워치독 해제

    // 자식 종료 후 master drop → conout 닫힘 → reader EOF.
    // (reader 스레드가 conin 보유 + 살아있어 [6n 응답/conout drain 모두 가능 → 안전)
    drop(pty);
    let out = rx.recv_timeout(Duration::from_secs(3)).unwrap_or_default();
    let _ = reader_handle.join();
    let _ = watchdog.join();
    let timed_out = timed_out.load(std::sync::atomic::Ordering::SeqCst);

    if std::env::var("ZM_DEBUG").is_ok() {
        eprintln!(
            "--- DEBUG argv={:?}\n    len={} bytes\n    escaped={:?}",
            argv,
            out.len(),
            out
        );
    }

    let status = if timed_out { "TIMEOUT" } else { "" };
    (out, status, String::new())
}

/// 출력에서 ZMPROBE 마커를 파싱.
fn parse_markers(buf: &str) -> (Option<bool>, Option<bool>, Option<bool>, Option<String>) {
    let mut out = None;
    let mut inp = None;
    let mut err = None;
    let mut raw = None;
    for line in buf.lines() {
        if line.contains("ZMPROBE_RAW") {
            for tok in line.split_whitespace() {
                if let Some(v) = tok.strip_prefix("rawout=") {
                    raw = Some(v.to_string());
                }
            }
        } else if line.contains("ZMPROBE ") && line.contains(" END") {
            for tok in line.split_whitespace() {
                if let Some(v) = tok.strip_prefix("out=") {
                    out = parse_bool(v);
                } else if let Some(v) = tok.strip_prefix("in=") {
                    inp = parse_bool(v);
                } else if let Some(v) = tok.strip_prefix("err=") {
                    err = parse_bool(v);
                }
            }
        }
    }
    (out, inp, err, raw)
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn run_cell(probe: &Probe, launch: &'static str) -> Cell {
    if !probe.available {
        return Cell {
            lang: probe.lang,
            launch,
            out: None,
            inp: None,
            err: None,
            raw_out: None,
            status: "SKIP",
            note: "probe unavailable".into(),
        };
    }
    let argv = argv_for(probe, launch);
    let (buf, status0, note) = run_argv(&argv);
    if status0 == "SPAWN_ERR" {
        return Cell {
            lang: probe.lang,
            launch,
            out: None,
            inp: None,
            err: None,
            raw_out: None,
            status: "SPAWN_ERR",
            note,
        };
    }
    let (out, inp, err, raw) = parse_markers(&buf);
    let status = if out.is_some() {
        "PASS"
    } else if status0 == "TIMEOUT" {
        "TIMEOUT"
    } else {
        "NO_MARKER"
    };
    let note = if status == "NO_MARKER" || status == "TIMEOUT" {
        // 진단용 출력 일부 보존
        let excerpt: String = buf.chars().take(120).collect();
        excerpt.replace(['\r', '\n'], "·")
    } else {
        String::new()
    };
    Cell {
        lang: probe.lang,
        launch,
        out,
        inp,
        err,
        raw_out: raw,
        status,
        note,
    }
}

fn b2s(b: Option<bool>) -> &'static str {
    match b {
        Some(true) => "true",
        Some(false) => "false",
        None => "-",
    }
}

fn main() {
    // 프로브 스크립트를 임시 파일로 기록(node/python). .js 직접 실행이 아니라 `node <file>` 이므로 안전.
    let tmp = std::env::temp_dir();
    let js = tmp.join("zm_probe.js");
    let py = tmp.join("zm_probe.py");
    let _ = std::fs::write(&js, NODE_SCRIPT);
    let _ = std::fs::write(&py, PY_SCRIPT);

    // 빌드된 zm-probe 바이너리 경로(같은 target 디렉터리).
    let rust_probe = std::env::current_exe().ok().and_then(|p| {
        p.parent()
            .map(|d| d.join(format!("zm-probe{}", std::env::consts::EXE_SUFFIX)))
    });
    let rust_available = rust_probe.as_ref().map(|p| p.exists()).unwrap_or(false);

    fn which(prog: &str) -> bool {
        // 간단 확인: 직접 --version 류 실행 대신 PATH 존재만 신뢰하고 셀 결과로 판정.
        // (없으면 SPAWN_ERR로 자연히 드러남) — 여기서는 항상 시도하도록 true.
        let _ = prog;
        true
    }

    let probes = vec![
        Probe {
            lang: "node",
            prog: "node".into(),
            args: vec![js.to_string_lossy().into_owned()],
            available: which("node"),
        },
        Probe {
            lang: "py",
            prog: "python".into(),
            args: vec![py.to_string_lossy().into_owned()],
            available: which("python"),
        },
        Probe {
            lang: "rust",
            prog: rust_probe
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            args: vec![],
            available: rust_available,
        },
    ];

    let launches: [&'static str; 4] = ["direct", "cmd", "pwsh", "powershell"];

    println!("=== zm-mux R1 isTTY 측정 (zm-probe-harness) ===");
    println!(
        "OS build : {}",
        std::env::var("OS").unwrap_or_default()
    );
    if !rust_available {
        println!("주의: zm-probe 바이너리 미발견 → rust 셀 SKIP. 먼저 `cargo build -p zm-probe` 실행 권장.");
    }
    println!();
    println!(
        "{:<6} {:<11} {:<6} {:<6} {:<6} {:<10} {:<10} {}",
        "lang", "launch", "out", "in", "err", "raw(node)", "status", "note"
    );
    println!("{}", "-".repeat(90));

    let mut cells: Vec<Cell> = Vec::new();
    for probe in &probes {
        for &launch in &launches {
            let cell = run_cell(probe, launch);
            println!(
                "{:<6} {:<11} {:<6} {:<6} {:<6} {:<10} {:<10} {}",
                cell.lang,
                cell.launch,
                b2s(cell.out),
                b2s(cell.inp),
                b2s(cell.err),
                cell.raw_out.clone().unwrap_or_else(|| "-".into()),
                cell.status,
                cell.note
            );
            cells.push(cell);
        }
    }

    println!();
    // 판정: node-direct 의 stdout.isTTY.
    let node_direct = cells
        .iter()
        .find(|c| c.lang == "node" && c.launch == "direct");
    let verdict = node_direct.and_then(|c| c.out);
    let raw = node_direct
        .and_then(|c| c.raw_out.clone())
        .unwrap_or_else(|| "-".into());
    println!(
        "R1 게이트 (node, direct, stdout.isTTY) = {} (raw={})",
        b2s(verdict),
        raw
    );
    match verdict {
        Some(true) => println!("→ ConPTY 직속 자식 isTTY=true 확인. 트랙 A(자체 PTY로 진짜 TTY 부여) 진행 가능."),
        Some(false) => println!("→ isTTY=false. 플래그/런치경로/NODE_OPTIONS 프리로드/트랙 B(#26572)·C 검토 필요."),
        None => println!("→ 측정 실패(마커 없음). node 설치/실행 경로 확인 후 재시도."),
    }
    println!();
    println!("docs/research/07-poc-conpty-istty-results.md 에 위 표/판정을 기록할 것.");

    std::process::exit(if verdict == Some(true) { 0 } else { 1 });
}
