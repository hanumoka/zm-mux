//! zm-pty — portable-pty 위의 얇은 PTY 추상화.
//!
//! Windows에서는 ConPTY, 그 외에는 POSIX PTY를 단일 trait로 다룬다.
//! 스레드 spawn과 drop 순서(ConPTY conout 데드락 회피)는 호출자(zm-app/harness)가
//! 소유한다 — 이 크레이트는 핸들 생성/리사이즈/종료만 책임진다.
//!
//! 참고(SAFE/MIT): reference/wezterm/pty/src/lib.rs, examples/bash.rs.

use std::io::{Read, Write};

use portable_pty::{native_pty_system, ExitStatus, PtySize};
use zm_core::{GridSize, Result, ZmError};

// 호출자가 임의 명령을 구성할 수 있도록 재노출(PTY 경계 일원화).
pub use portable_pty::CommandBuilder;

fn to_pty_size(size: GridSize, pixel_w: u16, pixel_h: u16) -> PtySize {
    PtySize {
        rows: size.rows,
        cols: size.cols,
        pixel_width: pixel_w,
        pixel_height: pixel_h,
    }
}

fn pty_err(e: impl std::fmt::Display) -> ZmError {
    ZmError::Pty(e.to_string())
}

/// 스폰된 PTY의 입출력 채널. reader는 별도 스레드에서 소비하고,
/// writer는 메인 스레드에서 사용하는 것을 권장한다.
pub struct PtyChannels {
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
}

/// 살아있는 PTY 마스터 + 자식 프로세스 핸들.
///
/// **Drop 순서 주의(Windows/ConPTY):** 마스터를 drop하기 전에 conout reader를
/// 끝까지 비워야 데드락이 없다. 따라서 호출자는 `kill()`/자식 종료 → reader 스레드
/// join → `Pty` drop 순서를 지켜야 한다. (reference/wezterm/pty/examples/whoami.rs)
pub struct Pty {
    master: Box<dyn portable_pty::MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    killer: Box<dyn portable_pty::ChildKiller + Send + Sync>,
}

impl Pty {
    /// 플랫폼 기본 셸(%COMSPEC% / $SHELL)을 PTY 안에서 실행.
    pub fn spawn(size: GridSize, pixel_w: u16, pixel_h: u16) -> Result<(Pty, PtyChannels)> {
        Self::spawn_cmd(CommandBuilder::new_default_prog(), size, pixel_w, pixel_h)
    }

    /// 임의 명령을 PTY 안에서 실행(프로브 하네스용).
    pub fn spawn_cmd(
        cmd: CommandBuilder,
        size: GridSize,
        pixel_w: u16,
        pixel_h: u16,
    ) -> Result<(Pty, PtyChannels)> {
        let sys = native_pty_system();
        let pair = sys
            .openpty(to_pty_size(size, pixel_w, pixel_h))
            .map_err(pty_err)?;

        let child = pair.slave.spawn_command(cmd).map_err(pty_err)?;
        // slave 핸들은 spawn 직후 즉시 해제 — 그래야 자식이 EOF를 정상 인식.
        drop(pair.slave);

        let reader = pair.master.try_clone_reader().map_err(pty_err)?;
        let writer = pair.master.take_writer().map_err(pty_err)?;
        let killer = child.clone_killer();

        Ok((
            Pty {
                master: pair.master,
                child,
                killer,
            },
            PtyChannels { reader, writer },
        ))
    }

    /// PTY 크기 변경(자식에 SIGWINCH / ConPTY resize 전달).
    pub fn resize(&self, size: GridSize, pixel_w: u16, pixel_h: u16) -> Result<()> {
        self.master
            .resize(to_pty_size(size, pixel_w, pixel_h))
            .map_err(pty_err)
    }

    /// 자식 프로세스 강제 종료.
    pub fn kill(&mut self) -> Result<()> {
        self.killer.kill().map_err(ZmError::Io)
    }

    /// 다른 스레드(워치독)에서 사용할 수 있는 killer 클론.
    pub fn killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> {
        self.killer.clone_killer()
    }

    /// 논블로킹 종료 확인.
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        self.child.try_wait().map_err(ZmError::Io)
    }

    /// 블로킹 종료 대기.
    pub fn wait(&mut self) -> Result<ExitStatus> {
        self.child.wait().map_err(ZmError::Io)
    }

    /// 자식 PID(있으면).
    pub fn process_id(&self) -> Option<u32> {
        self.child.process_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// 비대화형 명령을 PTY로 실행해 출력 바이트가 reader로 들어오는지 확인.
    #[test]
    fn spawn_cmd_emits_output() {
        let marker = "ZMPTY_OK_8421";
        let mut cmd = if cfg!(windows) {
            let mut c = CommandBuilder::new("cmd.exe");
            c.args(["/c", &format!("echo {marker}")]);
            c
        } else {
            let mut c = CommandBuilder::new("sh");
            c.args(["-c", &format!("echo {marker}")]);
            c
        };
        cmd.env("TERM", "xterm-256color");

        let (mut pty, chans) = Pty::spawn_cmd(cmd, GridSize::new(80, 24), 0, 0).expect("spawn");
        let PtyChannels { reader, writer } = chans;

        // reader 스레드가 writer(conin)도 소유: conin 유지(0xC000013A 방지) + ESC[6n에 CPR 응답(hang 방지).
        let handle = std::thread::spawn(move || {
            use std::io::{Read, Write};
            let mut reader = reader;
            let mut writer = writer;
            let mut acc: Vec<u8> = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        let s = &chunk[..n];
                        acc.extend_from_slice(s);
                        if s.windows(4).any(|w| w == b"\x1b[6n") {
                            let _ = writer.write_all(b"\x1b[1;1R");
                            let _ = writer.flush();
                        }
                    }
                    Err(_) => break,
                }
            }
            String::from_utf8_lossy(&acc).into_owned()
        });

        // 자식 종료 대기(타임아웃 → kill).
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match pty.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                _ => {
                    let _ = pty.kill();
                    break;
                }
            }
        }
        drop(pty); // 마스터 drop은 reader가 살아있는 상태에서(conout drain).
        let out = handle.join().expect("join");
        assert!(out.contains(marker), "출력에 마커 없음: {out:?}");
    }
}
