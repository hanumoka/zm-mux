//! End-to-end integration tests for the synchronous local-socket
//! transport. Spawns a `BackendServer` in a background thread, connects
//! a `Client` from the test thread, and exercises the full RPC surface
//! over a real local socket.
//!
//! Each test uses a unique socket name (pid + nanos) so concurrent test
//! runs and parallel cargo test invocations don't collide.

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use zm_socket::rpc::{
    BackendHandler, ContextId, InitParams, InitResult, KillParams, ListParams, ListResult,
    MinimalHandler, Notification, Request, RequestId, Response, RpcMethod, SpawnAgentParams,
    SpawnAgentResult,
};
use zm_socket::transport_sync::{BackendServer, Client};

fn unique_socket_name(tag: &str) -> String {
    let pid = std::process::id();
    let nano = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("zm-mux-{tag}-{pid}-{nano}")
}

fn unwrap_success_value<T: serde::de::DeserializeOwned>(resp: Response) -> T {
    match resp {
        Response::Success(s) => serde_json::from_value(s.result).expect("decode result"),
        Response::Error(e) => panic!(
            "expected success response, got error: {} ({})",
            e.error.message, e.error.code
        ),
    }
}

#[test]
fn server_client_full_lifecycle() {
    let socket_name = unique_socket_name("lifecycle");

    let server_socket = socket_name.clone();
    let server_thread = thread::spawn(move || {
        let server = BackendServer::new(MinimalHandler::new(), server_socket);
        server.serve_one().expect("serve_one should not error")
    });

    // Give the server a moment to bind before the client connects. 100 ms is
    // generous on a developer machine; CI may need more, but minimal scope.
    thread::sleep(Duration::from_millis(100));

    let mut client = Client::connect(&socket_name).expect("client should connect");

    // initialize
    let init_req = Request::new(
        RequestId::Num(1),
        RpcMethod::Initialize,
        InitParams {
            protocol_version: "1.0".to_string(),
        },
    )
    .unwrap();
    let (resp, notifs) = client.call(&init_req).expect("initialize call");
    assert!(notifs.is_empty(), "no notifications expected at this point");
    let init_result: InitResult = unwrap_success_value(resp);
    assert_eq!(init_result.self_context_id, ContextId::new("ctx-self"));

    // spawn_agent
    let spawn_req = Request::new(
        RequestId::Num(2),
        RpcMethod::SpawnAgent,
        SpawnAgentParams {
            argv: vec!["claude".to_string()],
            env: HashMap::new(),
            cwd: None,
            name: Some("e2e-test".to_string()),
        },
    )
    .unwrap();
    let (resp, _) = client.call(&spawn_req).expect("spawn_agent call");
    let spawn_result: SpawnAgentResult = unwrap_success_value(resp);
    assert_eq!(spawn_result.context_id, ContextId::new("ctx-1"));

    // list shows the spawned agent
    let list_req = Request::new(RequestId::Num(3), RpcMethod::List, ListParams::default()).unwrap();
    let (resp, _) = client.call(&list_req).expect("list call");
    let list_result: ListResult = unwrap_success_value(resp);
    assert_eq!(list_result.contexts.len(), 1);
    assert_eq!(list_result.contexts[0].id, ContextId::new("ctx-1"));

    // kill
    let kill_req = Request::new(
        RequestId::Num(4),
        RpcMethod::Kill,
        KillParams {
            context_id: spawn_result.context_id,
        },
    )
    .unwrap();
    let (resp, _) = client.call(&kill_req).expect("kill call");
    match resp {
        Response::Success(s) => assert_eq!(s.result, serde_json::Value::Null),
        Response::Error(e) => panic!("kill failed: {} ({})", e.error.message, e.error.code),
    }

    // list now empty
    let list_req2 = Request::new(RequestId::Num(5), RpcMethod::List, ListParams::default()).unwrap();
    let (resp, _) = client.call(&list_req2).expect("list call 2");
    let list_result: ListResult = unwrap_success_value(resp);
    assert!(list_result.contexts.is_empty());

    drop(client);
    server_thread.join().expect("server thread should not panic");
}

#[test]
fn simulate_exit_pushes_notification_alongside_response() {
    let socket_name = unique_socket_name("notif");

    let handler = MinimalHandler::new();
    let server = BackendServer::new(handler, socket_name.clone());
    let handler_handle = server.handler_handle();

    let server_thread = thread::spawn(move || {
        server.serve_one().expect("serve_one should not error")
    });
    thread::sleep(Duration::from_millis(100));

    let mut client = Client::connect(&socket_name).expect("client should connect");

    // spawn an agent
    let spawn_req = Request::new(
        RequestId::Num(1),
        RpcMethod::SpawnAgent,
        SpawnAgentParams {
            argv: vec!["claude".to_string()],
            env: HashMap::new(),
            cwd: None,
            name: None,
        },
    )
    .unwrap();
    let (resp, notifs) = client.call(&spawn_req).expect("spawn call");
    assert!(notifs.is_empty());
    let spawn_result: SpawnAgentResult = unwrap_success_value(resp);

    // From the test thread (i.e. "outside" the server's dispatch path),
    // simulate the child exiting. This buffers a context_exited
    // notification that the server will drain after its next dispatch.
    {
        let mut h = handler_handle.lock().unwrap();
        h.simulate_exit(&spawn_result.context_id, 0)
            .expect("simulate_exit");
    }

    // Now make any RPC call. The server runs dispatch, writes the
    // response, then drains and writes the buffered notification.
    let list_req =
        Request::new(RequestId::Num(2), RpcMethod::List, ListParams::default()).unwrap();
    let (resp, notifs) = client.call(&list_req).expect("list call");
    let list_result: ListResult = unwrap_success_value(resp);
    // entry still present (kill not sent), status now Exited
    assert_eq!(list_result.contexts.len(), 1);

    // and the notification arrived after the list response
    assert_eq!(notifs.len(), 1);
    assert_eq!(notifs[0].method, Notification::CONTEXT_EXITED);
    let payload = notifs[0].params.as_object().expect("notif params object");
    assert_eq!(
        payload.get("context_id").and_then(|v| v.as_str()),
        Some("ctx-1")
    );
    assert_eq!(payload.get("exit_code").and_then(|v| v.as_i64()), Some(0));

    // ensure no further pending notifications in the handler
    {
        let mut h = handler_handle.lock().unwrap();
        let drained = h.drain_notifications();
        assert!(drained.is_empty(), "handler should be drained");
    }

    drop(client);
    server_thread.join().expect("server thread");
    let _ = Arc::strong_count(&handler_handle);
}
