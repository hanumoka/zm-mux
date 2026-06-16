//! zm-rawtest — portable-pty 를 whoami.rs 예제와 동일하게 직접 사용해 ConPTY 캡처를 격리 진단.

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::sync::mpsc::channel;

fn run(label: &str, mut cmd_build: impl FnMut() -> CommandBuilder) {
    let pty_system = NativePtySystem::default();
    let pair = pty_system
        .openpty(PtySize {
            rows: 50,
            cols: 200,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();

    let cmd = cmd_build();
    let mut child = pair.slave.spawn_command(cmd).unwrap();
    drop(pair.slave);

    // reader 스레드가 writer(conin)도 소유: conin을 자식 수명 동안 열어두어
    // close 이벤트(0xC000013A)를 막고, ConPTY의 ESC[6n(커서 위치 질의)에 CPR로 응답해
    // hang을 막는다.
    let (tx, rx) = channel();
    let mut reader = pair.master.try_clone_reader().unwrap();
    let mut writer = pair.master.take_writer().unwrap();
    use std::io::{Read, Write};
    std::thread::spawn(move || {
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

    let status = child.wait().unwrap();
    drop(pair.master); // conout 닫힘 → reader EOF (reader가 conin 보유해 [6n 응답 가능)
    let output = rx.recv().unwrap();

    let escaped: String = output.escape_debug().collect();
    println!(
        "[{label}] status={status:?} len={} bytes\n  out={escaped}\n",
        output.len()
    );
}

fn main() {
    // 1) 콘솔 프로그램(cmd echo)
    run("cmd echo", || {
        let mut c = CommandBuilder::new("cmd.exe");
        c.args(["/c", "echo", "HELLO123"]);
        for (k, v) in std::env::vars() {
            c.env(k, v);
        }
        c
    });

    // 2) 우리 Rust 프로브 exe(직접)
    let exe = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join(format!("zm-probe{}", std::env::consts::EXE_SUFFIX));
    if exe.exists() {
        run("rust probe", || {
            let mut c = CommandBuilder::new(exe.to_string_lossy().to_string());
            for (k, v) in std::env::vars() {
                c.env(k, v);
            }
            c
        });
    } else {
        println!("[rust probe] SKIP (exe missing: {})", exe.display());
    }
}
