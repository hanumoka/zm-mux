//! zm-ipc — 로컬 소켓 자동화: JSON 라인 프로토콜 + 서버 브리지 + 클라이언트.
//!
//! 크로스플랫폼 로컬 소켓(interprocess): Windows 네임드 파이프 / Unix UDS.
//! 프로토콜: 요청/응답 각각 JSON 한 줄(`\n` 구분).
//!
//! 서버는 zm-app이 호스팅한다. 연결별 핸들러 스레드가 명령을 파싱해 [`RequestSink`]로
//! 메인 스레드에 넘기고, 메인 스레드가 [`IpcRequest::reply`]로 응답한다(이벤트 루프 비차단).

use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::thread;

use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{GenericNamespaced, ListenerOptions, Stream};
use serde::{Deserialize, Serialize};

/// 자동화 명령.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "kebab-case")]
pub enum Command {
    /// 모든 pane 목록.
    ListPanes,
    /// 탭 목록(개수/활성).
    ListTabs,
    /// 활성 pane 분할. vertical=true → 좌/우, false → 상/하.
    Split { vertical: bool },
    /// 새 탭.
    NewTab,
    /// 특정 pane 포커스.
    SelectPane { id: u64 },
    /// 방향 포커스(left/right/up/down).
    Focus { dir: String },
    /// pane에 키(바이트) 전송. pane 미지정 시 활성.
    SendKeys {
        #[serde(default)]
        pane: Option<u64>,
        data: String,
    },
    /// pane 내용 캡처(텍스트). pane 미지정 시 활성.
    CapturePane {
        #[serde(default)]
        pane: Option<u64>,
    },
    /// pane 종료.
    KillPane { id: u64 },
}

/// pane 메타.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneInfo {
    pub id: u64,
    pub active: bool,
    pub cols: u16,
    pub rows: u16,
}

/// 응답(JSON 한 줄).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub data: serde_json::Value,
}

impl Response {
    pub fn ok(data: serde_json::Value) -> Self {
        Self { ok: true, error: None, data }
    }
    pub fn ok_empty() -> Self {
        Self { ok: true, error: None, data: serde_json::Value::Null }
    }
    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(msg.into()),
            data: serde_json::Value::Null,
        }
    }
}

/// 서버 스레드 → 앱 메인 스레드로 넘기는 요청(응답 채널 포함).
pub struct IpcRequest {
    pub command: Command,
    resp: SyncSender<Response>,
}

impl IpcRequest {
    pub fn reply(self, r: Response) {
        let _ = self.resp.send(r);
    }
}

/// 앱이 구현: 받은 요청을 메인 스레드(이벤트 루프)로 전달.
pub trait RequestSink: Clone + Send + 'static {
    fn submit(&self, req: IpcRequest);
}

/// 소켓 이름(네임스페이스). 자식/CLI는 `ZM_MUX_SOCKET` env로 공유.
pub fn socket_name(pid: u32) -> String {
    format!("zm-mux-{pid}.sock")
}

/// 서버 시작: 백그라운드 리스너 스레드 + 연결별 핸들러 스레드.
pub fn serve<S: RequestSink>(name: &str, sink: S) -> std::io::Result<()> {
    let ns = name.to_ns_name::<GenericNamespaced>()?;
    let listener = ListenerOptions::new().name(ns).create_sync()?;
    thread::spawn(move || {
        for conn in listener.incoming() {
            match conn {
                Ok(conn) => {
                    let sink = sink.clone();
                    thread::spawn(move || handle_conn(conn, sink));
                }
                Err(_) => continue,
            }
        }
    });
    Ok(())
}

fn handle_conn<S: RequestSink>(conn: Stream, sink: S) {
    let mut reader = BufReader::new(&conn);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let resp = match serde_json::from_str::<Command>(trimmed) {
                    Ok(cmd) => {
                        let (tx, rx) = sync_channel(1);
                        sink.submit(IpcRequest { command: cmd, resp: tx });
                        rx.recv().unwrap_or_else(|_| Response::err("no response from app"))
                    }
                    Err(e) => Response::err(format!("parse error: {e}")),
                };
                let mut out = serde_json::to_string(&resp)
                    .unwrap_or_else(|_| "{\"ok\":false}".to_string());
                out.push('\n');
                if (&conn).write_all(out.as_bytes()).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

/// 클라이언트: 명령 1개 전송 후 응답 1개 수신.
pub fn send_command(name: &str, cmd: &Command) -> std::io::Result<Response> {
    let ns = name.to_ns_name::<GenericNamespaced>()?;
    let conn = Stream::connect(ns)?;
    let mut line = serde_json::to_string(cmd).unwrap_or_default();
    line.push('\n');
    (&conn).write_all(line.as_bytes())?;
    let mut reader = BufReader::new(&conn);
    let mut resp = String::new();
    reader.read_line(&mut resp)?;
    let r = serde_json::from_str::<Response>(resp.trim())
        .unwrap_or_else(|e| Response::err(format!("bad response: {e}")));
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_roundtrip() {
        let c = Command::Split { vertical: true };
        let s = serde_json::to_string(&c).unwrap();
        assert!(s.contains("\"cmd\":\"split\""));
        assert!(s.contains("\"vertical\":true"));
        let back: Command = serde_json::from_str(&s).unwrap();
        assert!(matches!(back, Command::Split { vertical: true }));
    }

    #[test]
    fn send_keys_optional_pane() {
        let s = r#"{"cmd":"send-keys","data":"ls"}"#;
        let c: Command = serde_json::from_str(s).unwrap();
        assert!(matches!(c, Command::SendKeys { pane: None, .. }));
    }

    #[test]
    fn response_ok_serializes() {
        let r = Response::ok(serde_json::json!({"id": 2}));
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"ok\":true"));
        assert!(s.contains("\"id\":2"));
        // error는 None일 때 생략.
        assert!(!s.contains("error"));
    }
}
