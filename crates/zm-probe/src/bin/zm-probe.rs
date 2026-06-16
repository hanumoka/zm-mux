//! zm-probe — ConPTY/PTY 자식으로 실행되는 Rust isTTY 프로브.
//!
//! `std::io::IsTerminal`로 stdout/stdin/stderr가 TTY인지 판정하고 grep 가능한
//! 한 줄 마커를 출력한다. 하네스(zm-probe-harness)가 이 출력을 파싱한다.

use std::io::{IsTerminal, Write};

fn main() {
    let out = std::io::stdout().is_terminal();
    let inp = std::io::stdin().is_terminal();
    let err = std::io::stderr().is_terminal();

    let mut so = std::io::stdout();
    let _ = writeln!(so, "ZMPROBE lang=rust out={out} in={inp} err={err} END");
    let _ = so.flush();
}
