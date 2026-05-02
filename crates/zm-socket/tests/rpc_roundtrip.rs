//! Wire-format snapshot tests for the CustomPaneBackend JSON-RPC types.
//!
//! Each test serializes a representative request / response / notification
//! to JSON and compares against a stored snapshot under `tests/snapshots/`.
//! On first run, snapshots are created as `*.snap.new` and must be reviewed
//! and accepted via `cargo insta review`.

use std::collections::HashMap;

use zm_socket::rpc::{
    CaptureParams, CaptureResult, ContextExitedParams, ContextId, ContextInfo, ContextStatus,
    InitParams, InitResult, KillParams, ListParams, ListResult, Notification, Request, RequestId,
    ResponseError, ResponseSuccess, RpcError, RpcMethod, SpawnAgentParams, SpawnAgentResult,
    WriteParams,
};

// ---- initialize -------------------------------------------------------------

#[test]
fn initialize_request() {
    let req = Request::new(
        RequestId::Num(1),
        RpcMethod::Initialize,
        InitParams {
            protocol_version: "1.0".to_string(),
        },
    )
    .unwrap();
    insta::assert_json_snapshot!(req);
}

#[test]
fn initialize_response_success() {
    let resp = ResponseSuccess::new(
        RequestId::Num(1),
        InitResult {
            self_context_id: ContextId::new("ctx-main"),
        },
    )
    .unwrap();
    insta::assert_json_snapshot!(resp);
}

// ---- spawn_agent ------------------------------------------------------------

#[test]
fn spawn_agent_request_full() {
    let mut env = HashMap::new();
    env.insert("ZM_MUX_AGENT_ROLE".to_string(), "reviewer".to_string());
    let req = Request::new(
        RequestId::Num(2),
        RpcMethod::SpawnAgent,
        SpawnAgentParams {
            argv: vec!["claude".to_string(), "--role".to_string(), "reviewer".to_string()],
            env,
            cwd: Some("/home/user/project".to_string()),
            name: Some("reviewer-1".to_string()),
        },
    )
    .unwrap();
    insta::assert_json_snapshot!(req);
}

#[test]
fn spawn_agent_request_minimal() {
    let req = Request::new(
        RequestId::Str("req-spawn-min".to_string()),
        RpcMethod::SpawnAgent,
        SpawnAgentParams {
            argv: vec!["claude".to_string()],
            env: HashMap::new(),
            cwd: None,
            name: None,
        },
    )
    .unwrap();
    insta::assert_json_snapshot!(req);
}

#[test]
fn spawn_agent_response_success() {
    let resp = ResponseSuccess::new(
        RequestId::Num(2),
        SpawnAgentResult {
            context_id: ContextId::new("ctx-2"),
        },
    )
    .unwrap();
    insta::assert_json_snapshot!(resp);
}

// ---- write ------------------------------------------------------------------

#[test]
fn write_request() {
    let req = Request::new(
        RequestId::Num(3),
        RpcMethod::Write,
        WriteParams {
            context_id: ContextId::new("ctx-2"),
            data: "aGVsbG8gd29ybGQK".to_string(), // base64 "hello world\n"
        },
    )
    .unwrap();
    insta::assert_json_snapshot!(req);
}

// ---- capture ----------------------------------------------------------------

#[test]
fn capture_request() {
    let req = Request::new(
        RequestId::Num(4),
        RpcMethod::Capture,
        CaptureParams {
            context_id: ContextId::new("ctx-2"),
            lines: 50,
        },
    )
    .unwrap();
    insta::assert_json_snapshot!(req);
}

#[test]
fn capture_response_success() {
    let resp = ResponseSuccess::new(
        RequestId::Num(4),
        CaptureResult {
            data: "line1\nline2\nline3\n".to_string(),
        },
    )
    .unwrap();
    insta::assert_json_snapshot!(resp);
}

// ---- kill -------------------------------------------------------------------

#[test]
fn kill_request() {
    let req = Request::new(
        RequestId::Num(5),
        RpcMethod::Kill,
        KillParams {
            context_id: ContextId::new("ctx-2"),
        },
    )
    .unwrap();
    insta::assert_json_snapshot!(req);
}

// ---- list -------------------------------------------------------------------

#[test]
fn list_request() {
    let req = Request::new(
        RequestId::Num(6),
        RpcMethod::List,
        ListParams::default(),
    )
    .unwrap();
    insta::assert_json_snapshot!(req);
}

#[test]
fn list_response_success() {
    let resp = ResponseSuccess::new(
        RequestId::Num(6),
        ListResult {
            contexts: vec![
                ContextInfo {
                    id: ContextId::new("ctx-main"),
                    name: "main".to_string(),
                    status: ContextStatus::Running,
                },
                ContextInfo {
                    id: ContextId::new("ctx-2"),
                    name: "reviewer-1".to_string(),
                    status: ContextStatus::Exited,
                },
            ],
        },
    )
    .unwrap();
    insta::assert_json_snapshot!(resp);
}

// ---- context_exited (notification, server→client) ---------------------------

#[test]
fn context_exited_notification() {
    let n = Notification::context_exited(ContextExitedParams {
        context_id: ContextId::new("ctx-2"),
        exit_code: 0,
    })
    .unwrap();
    insta::assert_json_snapshot!(n);
}

// ---- error response (covers any method) -------------------------------------

#[test]
fn response_error_invalid_params() {
    let resp = ResponseError::new(
        RequestId::Num(2),
        RpcError::new(RpcError::INVALID_PARAMS, "missing argv"),
    );
    insta::assert_json_snapshot!(resp);
}

#[test]
fn response_error_method_not_found() {
    let resp = ResponseError::new(
        RequestId::Str("unknown-id".to_string()),
        RpcError::new(RpcError::METHOD_NOT_FOUND, "no such method: foo"),
    );
    insta::assert_json_snapshot!(resp);
}
