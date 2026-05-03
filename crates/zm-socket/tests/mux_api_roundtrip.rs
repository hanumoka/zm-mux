use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use zm_socket::mux_api::MuxHandler;
use zm_socket::mux_api::types::*;
use zm_socket::rpc::{JSONRPC_VERSION, Request, RequestId, Response, RpcError};
use zm_socket::transport_sync::{Client, MuxServer};

fn unique_socket_name(tag: &str) -> String {
    let pid = std::process::id();
    let nano = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("zm-mux-{tag}-{pid}-{nano}")
}

struct FakeMuxHandler;

impl MuxHandler for FakeMuxHandler {
    fn handle_list_panes(&self, _p: ListPanesParams) -> Result<ListPanesResult, RpcError> {
        Ok(ListPanesResult {
            panes: vec![
                PaneInfo {
                    pane_id: 0,
                    tab_id: 0,
                    focused: true,
                    cols: 120,
                    rows: 40,
                    title: "bash".to_string(),
                    agent_type: "unknown".to_string(),
                    agent_status: "unknown".to_string(),
                },
                PaneInfo {
                    pane_id: 1,
                    tab_id: 0,
                    focused: false,
                    cols: 60,
                    rows: 40,
                    title: "bash".to_string(),
                    agent_type: "unknown".to_string(),
                    agent_status: "unknown".to_string(),
                },
            ],
        })
    }

    fn handle_get_status(&self, _p: GetStatusParams) -> Result<GetStatusResult, RpcError> {
        Ok(GetStatusResult {
            workspace_id: "ws-test-42".to_string(),
            pid: 42,
            version: "0.1.0".to_string(),
            active_tab: 0,
            pane_count: 2,
            tab_count: 1,
            socket_path: "test-socket".to_string(),
        })
    }

    fn handle_send_keys(&self, p: SendKeysParams) -> Result<SendKeysResult, RpcError> {
        if p.pane_id == 99 { return Err(RpcError::new(PANE_NOT_FOUND, "pane not found")); }
        Ok(SendKeysResult {})
    }

    fn handle_focus_pane(&self, p: FocusPaneParams) -> Result<FocusPaneResult, RpcError> {
        if p.pane_id == 99 { return Err(RpcError::new(PANE_NOT_FOUND, "pane not found")); }
        Ok(FocusPaneResult {})
    }

    fn handle_split_pane(&self, p: SplitPaneParams) -> Result<SplitPaneResult, RpcError> {
        if p.pane_id == 99 { return Err(RpcError::new(PANE_NOT_FOUND, "pane not found")); }
        Ok(SplitPaneResult { new_pane_id: 2 })
    }

    fn handle_close_pane(&self, p: ClosePaneParams) -> Result<ClosePaneResult, RpcError> {
        if p.pane_id == 99 { return Err(RpcError::new(PANE_NOT_FOUND, "pane not found")); }
        Ok(ClosePaneResult {})
    }

    fn handle_create_tab(&self, _p: CreateTabParams) -> Result<CreateTabResult, RpcError> {
        Ok(CreateTabResult { tab_id: 1, pane_id: 3 })
    }

    fn handle_close_tab(&self, p: CloseTabParams) -> Result<CloseTabResult, RpcError> {
        if p.tab_id == 99 { return Err(RpcError::new(TAB_NOT_FOUND, "tab not found")); }
        Ok(CloseTabResult {})
    }

    fn handle_set_agent_info(&self, p: SetAgentInfoParams) -> Result<SetAgentInfoResult, RpcError> {
        if p.pane_id == 99 { return Err(RpcError::new(PANE_NOT_FOUND, "pane not found")); }
        Ok(SetAgentInfoResult {})
    }
}

fn mux_req(id: i64, method: MuxMethod) -> Request {
    Request {
        jsonrpc: JSONRPC_VERSION.to_string(),
        id: RequestId::Num(id),
        method: method.as_str().to_string(),
        params: serde_json::json!({}),
    }
}

fn mux_req_with<P: serde::Serialize>(id: i64, method: MuxMethod, params: P) -> Request {
    Request {
        jsonrpc: JSONRPC_VERSION.to_string(),
        id: RequestId::Num(id),
        method: method.as_str().to_string(),
        params: serde_json::to_value(params).unwrap(),
    }
}

#[test]
fn mux_server_list_panes_over_socket() {
    let socket_name = unique_socket_name("mux-list");

    let server_socket = socket_name.clone();
    let handler = Arc::new(FakeMuxHandler);
    let server = MuxServer::new(handler, server_socket);

    let server_thread = thread::spawn(move || {
        // serve_forever blocks, but the client disconnects which causes
        // the read loop to return Ok(()) for that connection. Since we
        // only make one connection, we can't use serve_one (MuxServer
        // doesn't have it), so we'll just let the thread hang and detach.
        let _ = server.serve_forever();
    });

    thread::sleep(Duration::from_millis(100));

    let mut client = Client::connect(&socket_name).expect("connect");
    let req = mux_req(1, MuxMethod::ListPanes);
    let (resp, notifs) = client.call(&req).expect("list_panes call");
    assert!(notifs.is_empty());

    match resp {
        Response::Success(s) => {
            let result: ListPanesResult =
                serde_json::from_value(s.result).expect("parse");
            assert_eq!(result.panes.len(), 2);
            assert_eq!(result.panes[0].pane_id, 0);
            assert!(result.panes[0].focused);
            assert_eq!(result.panes[1].cols, 60);
        }
        Response::Error(e) => panic!("expected success: {:?}", e),
    }

    drop(client);
    drop(server_thread);
}

#[test]
fn mux_server_get_status_over_socket() {
    let socket_name = unique_socket_name("mux-status");

    let handler = Arc::new(FakeMuxHandler);
    let server = MuxServer::new(handler, socket_name.clone());

    let server_thread = thread::spawn(move || {
        let _ = server.serve_forever();
    });

    thread::sleep(Duration::from_millis(100));

    let mut client = Client::connect(&socket_name).expect("connect");
    let req = mux_req(1, MuxMethod::GetStatus);
    let (resp, _) = client.call(&req).expect("get_status call");

    match resp {
        Response::Success(s) => {
            let result: GetStatusResult =
                serde_json::from_value(s.result).expect("parse");
            assert_eq!(result.pid, 42);
            assert_eq!(result.pane_count, 2);
            assert_eq!(result.workspace_id, "ws-test-42");
        }
        Response::Error(e) => panic!("expected success: {:?}", e),
    }

    drop(client);
    drop(server_thread);
}

#[test]
fn mux_server_rejects_non_mux_method() {
    let socket_name = unique_socket_name("mux-reject");

    let handler = Arc::new(FakeMuxHandler);
    let server = MuxServer::new(handler, socket_name.clone());

    let server_thread = thread::spawn(move || {
        let _ = server.serve_forever();
    });

    thread::sleep(Duration::from_millis(100));

    let mut client = Client::connect(&socket_name).expect("connect");
    let req = Request {
        jsonrpc: JSONRPC_VERSION.to_string(),
        id: RequestId::Num(1),
        method: "initialize".to_string(),
        params: serde_json::json!({}),
    };
    let (resp, _) = client.call(&req).expect("call");

    match resp {
        Response::Error(e) => {
            assert_eq!(e.error.code, RpcError::METHOD_NOT_FOUND);
        }
        Response::Success(_) => panic!("expected error for non-mux method"),
    }

    drop(client);
    drop(server_thread);
}

fn connect_to_fake_server(tag: &str) -> (Client, thread::JoinHandle<()>) {
    let socket_name = unique_socket_name(tag);
    let handler = Arc::new(FakeMuxHandler);
    let server = MuxServer::new(handler, socket_name.clone());
    let handle = thread::spawn(move || { let _ = server.serve_forever(); });
    thread::sleep(Duration::from_millis(100));
    let client = Client::connect(&socket_name).expect("connect");
    (client, handle)
}

#[test]
fn mux_server_send_keys_over_socket() {
    let (mut client, _h) = connect_to_fake_server("mux-send");
    let req = mux_req_with(1, MuxMethod::SendKeys, SendKeysParams { pane_id: 0, data: "ls\r\n".into() });
    let (resp, _) = client.call(&req).expect("send_keys");
    assert!(matches!(resp, Response::Success(_)));
}

#[test]
fn mux_server_split_pane_over_socket() {
    let (mut client, _h) = connect_to_fake_server("mux-split");
    let req = mux_req_with(1, MuxMethod::SplitPane, SplitPaneParams { pane_id: 0, direction: "horizontal".into() });
    let (resp, _) = client.call(&req).expect("split_pane");
    match resp {
        Response::Success(s) => {
            let r: SplitPaneResult = serde_json::from_value(s.result).unwrap();
            assert_eq!(r.new_pane_id, 2);
        }
        Response::Error(e) => panic!("expected success: {:?}", e),
    }
}

#[test]
fn mux_server_create_tab_over_socket() {
    let (mut client, _h) = connect_to_fake_server("mux-newtab");
    let req = mux_req(1, MuxMethod::CreateTab);
    let (resp, _) = client.call(&req).expect("create_tab");
    match resp {
        Response::Success(s) => {
            let r: CreateTabResult = serde_json::from_value(s.result).unwrap();
            assert_eq!(r.tab_id, 1);
            assert_eq!(r.pane_id, 3);
        }
        Response::Error(e) => panic!("expected success: {:?}", e),
    }
}

#[test]
fn mux_server_pane_not_found_over_socket() {
    let (mut client, _h) = connect_to_fake_server("mux-notfound");
    let req = mux_req_with(1, MuxMethod::SendKeys, SendKeysParams { pane_id: 99, data: "x".into() });
    let (resp, _) = client.call(&req).expect("call");
    match resp {
        Response::Error(e) => assert_eq!(e.error.code, PANE_NOT_FOUND),
        Response::Success(_) => panic!("expected error"),
    }
}
