//! Minimal in-memory handler for the CustomPaneBackend protocol.
//!
//! Phase 2.1.A scope. The real handler will live alongside zm-mux's
//! PaneTree and spawn actual PTY child processes; this minimal handler
//! tracks contexts purely as in-memory metadata so that the protocol
//! itself — request routing, response shape, error code coverage,
//! notification emission — can be exercised end-to-end before the
//! transport layer (MIN-D3) lands and before the full reference
//! integrates with PTY spawning (future Section 4 D2-D5).
//!
//! Design choices:
//!
//! - `BackendHandler` trait so that the future full reference can swap
//!   in a `RealHandler` over the same `dispatch` routing without the
//!   transport layer caring which one is plugged in.
//! - `ContextRegistry` is split out so that any handler implementation
//!   can reuse the lifecycle semantics (insert / remove / list).
//! - Notifications are buffered on the handler and drained by the
//!   transport. In a real impl, an `mpsc` channel would replace the
//!   buffer, but for a synchronous minimal stub a `Vec` keeps the
//!   surface tiny.
//! - `simulate_exit` is the only non-protocol method on `MinimalHandler`
//!   — it lets tests and the mock client (MIN-D4) exercise the
//!   `context_exited` notification path without needing a real child
//!   process to die.

use std::collections::HashMap;

use serde::{Serialize, de::DeserializeOwned};

use crate::rpc::types::{
    CaptureParams, CaptureResult, ContextExitedParams, ContextId, ContextInfo, ContextStatus,
    InitParams, InitResult, JSONRPC_VERSION, KillParams, ListParams, ListResult, Notification,
    Request, RequestId, Response, ResponseError, ResponseSuccess, RpcError, RpcMethod,
    SpawnAgentParams, SpawnAgentResult, WriteParams,
};

/// Server-defined error code for "context not found". JSON-RPC 2.0 reserves
/// the range -32000..=-32099 for implementation-defined server errors;
/// -32000 is the conventional starting point.
pub const CONTEXT_NOT_FOUND: i32 = -32000;

/// Trait shared by minimal and (future) real handlers. Each method maps 1:1
/// to a JSON-RPC method on the wire. Notifications (server→client) are
/// emitted via the implementation's own surface, not through this trait.
pub trait BackendHandler {
    fn handle_initialize(&mut self, params: InitParams) -> Result<InitResult, RpcError>;
    fn handle_spawn_agent(
        &mut self,
        params: SpawnAgentParams,
    ) -> Result<SpawnAgentResult, RpcError>;
    fn handle_write(&mut self, params: WriteParams) -> Result<(), RpcError>;
    fn handle_capture(&mut self, params: CaptureParams) -> Result<CaptureResult, RpcError>;
    fn handle_kill(&mut self, params: KillParams) -> Result<(), RpcError>;
    fn handle_list(&mut self, params: ListParams) -> Result<ListResult, RpcError>;
}

#[derive(Debug, Clone)]
pub struct ContextEntry {
    pub id: ContextId,
    pub name: String,
    pub status: ContextStatus,
    pub argv: Vec<String>,
    /// In a real handler this is the rolling stdout buffer of the spawned
    /// process. The minimal stub echoes back whatever was `write`n to it
    /// so callers can exercise capture round-trips deterministically.
    pub write_log: Vec<String>,
}

#[derive(Debug, Default)]
pub struct ContextRegistry {
    entries: HashMap<ContextId, ContextEntry>,
    next_seq: u64,
}

impl ContextRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allocate_id(&mut self) -> ContextId {
        self.next_seq += 1;
        ContextId::new(format!("ctx-{}", self.next_seq))
    }

    pub fn insert(&mut self, entry: ContextEntry) {
        self.entries.insert(entry.id.clone(), entry);
    }

    pub fn get(&self, id: &ContextId) -> Option<&ContextEntry> {
        self.entries.get(id)
    }

    pub fn get_mut(&mut self, id: &ContextId) -> Option<&mut ContextEntry> {
        self.entries.get_mut(id)
    }

    pub fn remove(&mut self, id: &ContextId) -> Option<ContextEntry> {
        self.entries.remove(id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn snapshot(&self) -> Vec<ContextInfo> {
        let mut v: Vec<ContextInfo> = self
            .entries
            .values()
            .map(|e| ContextInfo {
                id: e.id.clone(),
                name: e.name.clone(),
                status: e.status,
            })
            .collect();
        // sort for deterministic ordering — important for snapshot tests and
        // for predictable client UX
        v.sort_by(|a, b| a.id.0.cmp(&b.id.0));
        v
    }
}

pub struct MinimalHandler {
    self_id: ContextId,
    registry: ContextRegistry,
    pending_notifications: Vec<Notification>,
}

impl MinimalHandler {
    pub fn new() -> Self {
        Self::with_self_id(ContextId::new("ctx-self"))
    }

    pub fn with_self_id(self_id: ContextId) -> Self {
        Self {
            self_id,
            registry: ContextRegistry::new(),
            pending_notifications: Vec::new(),
        }
    }

    pub fn registry(&self) -> &ContextRegistry {
        &self.registry
    }

    /// Drain all server→client notifications currently buffered on the
    /// handler. The transport layer (MIN-D3) will poll this once per
    /// dispatch cycle and push each notification down the wire.
    pub fn drain_notifications(&mut self) -> Vec<Notification> {
        std::mem::take(&mut self.pending_notifications)
    }

    /// Synthesize a `context_exited` notification for an already-registered
    /// context. Sets the entry's status to `Exited` but leaves it in the
    /// registry — the client is expected to send `kill` to fully evict.
    /// This is the only path that produces notifications in the minimal
    /// handler; in the full reference, the PTY child's actual exit drives
    /// it instead.
    pub fn simulate_exit(
        &mut self,
        id: &ContextId,
        exit_code: i32,
    ) -> Result<(), RpcError> {
        let entry = self.registry.get_mut(id).ok_or_else(|| {
            RpcError::new(
                CONTEXT_NOT_FOUND,
                format!("context not found: {}", id.0),
            )
        })?;
        entry.status = ContextStatus::Exited;
        let n = Notification::context_exited(ContextExitedParams {
            context_id: id.clone(),
            exit_code,
        })
        .map_err(|e| RpcError::new(RpcError::INTERNAL_ERROR, e.to_string()))?;
        self.pending_notifications.push(n);
        Ok(())
    }
}

impl Default for MinimalHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl BackendHandler for MinimalHandler {
    fn handle_initialize(&mut self, _params: InitParams) -> Result<InitResult, RpcError> {
        Ok(InitResult {
            self_context_id: self.self_id.clone(),
        })
    }

    fn handle_spawn_agent(
        &mut self,
        params: SpawnAgentParams,
    ) -> Result<SpawnAgentResult, RpcError> {
        if params.argv.is_empty() {
            return Err(RpcError::new(
                RpcError::INVALID_PARAMS,
                "argv must not be empty",
            ));
        }
        let id = self.registry.allocate_id();
        let name = params
            .name
            .unwrap_or_else(|| format!("agent-{}", id.0));
        self.registry.insert(ContextEntry {
            id: id.clone(),
            name,
            status: ContextStatus::Running,
            argv: params.argv,
            write_log: Vec::new(),
        });
        Ok(SpawnAgentResult { context_id: id })
    }

    fn handle_write(&mut self, params: WriteParams) -> Result<(), RpcError> {
        let entry = self.registry.get_mut(&params.context_id).ok_or_else(|| {
            RpcError::new(
                CONTEXT_NOT_FOUND,
                format!("context not found: {}", params.context_id.0),
            )
        })?;
        entry.write_log.push(params.data);
        Ok(())
    }

    fn handle_capture(&mut self, params: CaptureParams) -> Result<CaptureResult, RpcError> {
        let entry = self.registry.get(&params.context_id).ok_or_else(|| {
            RpcError::new(
                CONTEXT_NOT_FOUND,
                format!("context not found: {}", params.context_id.0),
            )
        })?;
        let lines = params.lines as usize;
        let total = entry.write_log.len();
        let start = total.saturating_sub(lines);
        let data = entry.write_log[start..].join("\n");
        Ok(CaptureResult { data })
    }

    fn handle_kill(&mut self, params: KillParams) -> Result<(), RpcError> {
        match self.registry.remove(&params.context_id) {
            Some(_) => Ok(()),
            None => Err(RpcError::new(
                CONTEXT_NOT_FOUND,
                format!("context not found: {}", params.context_id.0),
            )),
        }
    }

    fn handle_list(&mut self, _params: ListParams) -> Result<ListResult, RpcError> {
        Ok(ListResult {
            contexts: self.registry.snapshot(),
        })
    }
}

// ---- Dispatch ---------------------------------------------------------------

/// Route a single JSON-RPC request through a `BackendHandler`, returning the
/// fully-formed response. Wraps:
///
/// 1. method-string → `RpcMethod` (METHOD_NOT_FOUND on miss)
/// 2. `Value` → typed `Params` (INVALID_PARAMS on miss)
/// 3. handler call (any returned `RpcError` is preserved as-is)
/// 4. result → `Value` → `ResponseSuccess` (INTERNAL_ERROR on serialize miss)
pub fn dispatch<H: BackendHandler>(handler: &mut H, request: Request) -> Response {
    let id = request.id.clone();

    let method = match request.parse_method() {
        Some(m) => m,
        None => {
            return error_response(
                id,
                RpcError::METHOD_NOT_FOUND,
                format!("unknown method: {}", request.method),
            );
        }
    };

    match method {
        RpcMethod::Initialize => dispatch_op(id, &request, |p| handler.handle_initialize(p)),
        RpcMethod::SpawnAgent => dispatch_op(id, &request, |p| handler.handle_spawn_agent(p)),
        RpcMethod::Write => dispatch_op(id, &request, |p| handler.handle_write(p)),
        RpcMethod::Capture => dispatch_op(id, &request, |p| handler.handle_capture(p)),
        RpcMethod::Kill => dispatch_op(id, &request, |p| handler.handle_kill(p)),
        RpcMethod::List => dispatch_op(id, &request, |p| handler.handle_list(p)),
    }
}

fn dispatch_op<P, R, F>(id: RequestId, request: &Request, f: F) -> Response
where
    P: DeserializeOwned,
    R: Serialize,
    F: FnOnce(P) -> Result<R, RpcError>,
{
    let params: P = match request.parse_params() {
        Ok(p) => p,
        Err(e) => return error_response(id, RpcError::INVALID_PARAMS, e.to_string()),
    };
    match f(params) {
        Ok(r) => match serde_json::to_value(&r) {
            Ok(v) => Response::Success(ResponseSuccess {
                jsonrpc: JSONRPC_VERSION.to_string(),
                id,
                result: v,
            }),
            Err(e) => error_response(
                id,
                RpcError::INTERNAL_ERROR,
                format!("serialize result: {e}"),
            ),
        },
        Err(e) => Response::Error(ResponseError::new(id, e)),
    }
}

fn error_response(id: RequestId, code: i32, message: impl Into<String>) -> Response {
    Response::Error(ResponseError::new(id, RpcError::new(code, message)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn req<P: Serialize>(id: i64, method: RpcMethod, params: P) -> Request {
        Request::new(RequestId::Num(id), method, params).unwrap()
    }

    fn unwrap_success(resp: &Response) -> &ResponseSuccess {
        match resp {
            Response::Success(s) => s,
            Response::Error(e) => {
                panic!("expected success, got error: {} ({})", e.error.message, e.error.code)
            }
        }
    }

    fn unwrap_error(resp: &Response) -> &ResponseError {
        match resp {
            Response::Success(_) => panic!("expected error, got success"),
            Response::Error(e) => e,
        }
    }

    // ---- handler-level tests ------------------------------------------------

    #[test]
    fn initialize_returns_self_id() {
        let mut h = MinimalHandler::with_self_id(ContextId::new("ctx-fixed"));
        let result = h
            .handle_initialize(InitParams {
                protocol_version: "1.0".to_string(),
            })
            .unwrap();
        assert_eq!(result.self_context_id, ContextId::new("ctx-fixed"));
    }

    #[test]
    fn spawn_agent_assigns_sequential_ids() {
        let mut h = MinimalHandler::new();
        let r1 = h
            .handle_spawn_agent(SpawnAgentParams {
                argv: vec!["claude".to_string()],
                env: HashMap::new(),
                cwd: None,
                name: None,
            })
            .unwrap();
        let r2 = h
            .handle_spawn_agent(SpawnAgentParams {
                argv: vec!["claude".to_string()],
                env: HashMap::new(),
                cwd: None,
                name: None,
            })
            .unwrap();
        assert_eq!(r1.context_id, ContextId::new("ctx-1"));
        assert_eq!(r2.context_id, ContextId::new("ctx-2"));
    }

    #[test]
    fn spawn_agent_rejects_empty_argv() {
        let mut h = MinimalHandler::new();
        let err = h
            .handle_spawn_agent(SpawnAgentParams {
                argv: vec![],
                env: HashMap::new(),
                cwd: None,
                name: None,
            })
            .unwrap_err();
        assert_eq!(err.code, RpcError::INVALID_PARAMS);
    }

    #[test]
    fn list_reflects_registry_sorted() {
        let mut h = MinimalHandler::new();
        for argv in [vec!["a".to_string()], vec!["b".to_string()], vec!["c".to_string()]] {
            h.handle_spawn_agent(SpawnAgentParams {
                argv,
                env: HashMap::new(),
                cwd: None,
                name: None,
            })
            .unwrap();
        }
        let r = h.handle_list(ListParams::default()).unwrap();
        assert_eq!(r.contexts.len(), 3);
        // sorted lexicographically by id (ctx-1 < ctx-2 < ctx-3)
        assert_eq!(r.contexts[0].id, ContextId::new("ctx-1"));
        assert_eq!(r.contexts[1].id, ContextId::new("ctx-2"));
        assert_eq!(r.contexts[2].id, ContextId::new("ctx-3"));
    }

    #[test]
    fn write_then_capture_round_trips() {
        let mut h = MinimalHandler::new();
        let s = h
            .handle_spawn_agent(SpawnAgentParams {
                argv: vec!["claude".to_string()],
                env: HashMap::new(),
                cwd: None,
                name: Some("test".to_string()),
            })
            .unwrap();
        for line in ["aGVsbG8=", "d29ybGQ=", "IQ=="] {
            h.handle_write(WriteParams {
                context_id: s.context_id.clone(),
                data: line.to_string(),
            })
            .unwrap();
        }
        let cap = h
            .handle_capture(CaptureParams {
                context_id: s.context_id.clone(),
                lines: 2,
            })
            .unwrap();
        assert_eq!(cap.data, "d29ybGQ=\nIQ==");
    }

    #[test]
    fn kill_unknown_context_returns_context_not_found() {
        let mut h = MinimalHandler::new();
        let err = h
            .handle_kill(KillParams {
                context_id: ContextId::new("ctx-ghost"),
            })
            .unwrap_err();
        assert_eq!(err.code, CONTEXT_NOT_FOUND);
    }

    #[test]
    fn kill_then_list_excludes_killed() {
        let mut h = MinimalHandler::new();
        let s = h
            .handle_spawn_agent(SpawnAgentParams {
                argv: vec!["claude".to_string()],
                env: HashMap::new(),
                cwd: None,
                name: None,
            })
            .unwrap();
        h.handle_kill(KillParams {
            context_id: s.context_id.clone(),
        })
        .unwrap();
        let r = h.handle_list(ListParams::default()).unwrap();
        assert!(r.contexts.is_empty());
    }

    #[test]
    fn simulate_exit_emits_notification_and_marks_exited() {
        let mut h = MinimalHandler::new();
        let s = h
            .handle_spawn_agent(SpawnAgentParams {
                argv: vec!["claude".to_string()],
                env: HashMap::new(),
                cwd: None,
                name: None,
            })
            .unwrap();
        h.simulate_exit(&s.context_id, 0).unwrap();
        let notifs = h.drain_notifications();
        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0].method, Notification::CONTEXT_EXITED);
        // second drain is empty
        assert!(h.drain_notifications().is_empty());
        // entry still exists (kill is separate), but status is Exited
        let listed = h.handle_list(ListParams::default()).unwrap();
        assert_eq!(listed.contexts.len(), 1);
        assert_eq!(listed.contexts[0].status, ContextStatus::Exited);
    }

    // ---- dispatch routing ---------------------------------------------------

    #[test]
    fn dispatch_routes_initialize() {
        let mut h = MinimalHandler::with_self_id(ContextId::new("ctx-self-disp"));
        let r = req(
            1,
            RpcMethod::Initialize,
            InitParams {
                protocol_version: "1.0".to_string(),
            },
        );
        let resp = dispatch(&mut h, r);
        let s = unwrap_success(&resp);
        assert_eq!(s.id, RequestId::Num(1));
        let result: InitResult = serde_json::from_value(s.result.clone()).unwrap();
        assert_eq!(result.self_context_id, ContextId::new("ctx-self-disp"));
    }

    #[test]
    fn dispatch_unknown_method_returns_method_not_found() {
        let mut h = MinimalHandler::new();
        let bogus = Request {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: RequestId::Num(99),
            method: "nonsuch".to_string(),
            params: serde_json::Value::Null,
        };
        let resp = dispatch(&mut h, bogus);
        let e = unwrap_error(&resp);
        assert_eq!(e.error.code, RpcError::METHOD_NOT_FOUND);
        assert_eq!(e.id, RequestId::Num(99));
    }

    #[test]
    fn dispatch_invalid_params_returns_invalid_params() {
        let mut h = MinimalHandler::new();
        // spawn_agent with malformed params (missing required argv)
        let bad = Request {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: RequestId::Num(7),
            method: RpcMethod::SpawnAgent.as_str().to_string(),
            params: serde_json::json!({ "name": "no-argv" }),
        };
        let resp = dispatch(&mut h, bad);
        let e = unwrap_error(&resp);
        assert_eq!(e.error.code, RpcError::INVALID_PARAMS);
    }

    #[test]
    fn dispatch_handler_error_passes_through() {
        // empty argv → handler returns its own INVALID_PARAMS RpcError
        let mut h = MinimalHandler::new();
        let bad = req(
            8,
            RpcMethod::SpawnAgent,
            SpawnAgentParams {
                argv: vec![],
                env: HashMap::new(),
                cwd: None,
                name: None,
            },
        );
        let resp = dispatch(&mut h, bad);
        let e = unwrap_error(&resp);
        assert_eq!(e.error.code, RpcError::INVALID_PARAMS);
        assert!(e.error.message.contains("argv"));
    }

    #[test]
    fn dispatch_full_lifecycle_via_requests() {
        let mut h = MinimalHandler::new();

        // initialize
        let r0 = dispatch(
            &mut h,
            req(
                0,
                RpcMethod::Initialize,
                InitParams {
                    protocol_version: "1.0".to_string(),
                },
            ),
        );
        unwrap_success(&r0);

        // spawn
        let spawn_resp = dispatch(
            &mut h,
            req(
                1,
                RpcMethod::SpawnAgent,
                SpawnAgentParams {
                    argv: vec!["claude".to_string()],
                    env: HashMap::new(),
                    cwd: None,
                    name: Some("life".to_string()),
                },
            ),
        );
        let spawn_ok = unwrap_success(&spawn_resp);
        let spawn_result: SpawnAgentResult =
            serde_json::from_value(spawn_ok.result.clone()).unwrap();

        // write
        let w_resp = dispatch(
            &mut h,
            req(
                2,
                RpcMethod::Write,
                WriteParams {
                    context_id: spawn_result.context_id.clone(),
                    data: "Zm9v".to_string(), // base64 "foo"
                },
            ),
        );
        let w_ok = unwrap_success(&w_resp);
        assert_eq!(w_ok.result, serde_json::Value::Null);

        // list shows 1 entry
        let l_resp = dispatch(&mut h, req(3, RpcMethod::List, ListParams::default()));
        let l_ok = unwrap_success(&l_resp);
        let l_result: ListResult = serde_json::from_value(l_ok.result.clone()).unwrap();
        assert_eq!(l_result.contexts.len(), 1);

        // kill
        let k_resp = dispatch(
            &mut h,
            req(
                4,
                RpcMethod::Kill,
                KillParams {
                    context_id: spawn_result.context_id,
                },
            ),
        );
        unwrap_success(&k_resp);

        // list now empty
        let l2_resp = dispatch(&mut h, req(5, RpcMethod::List, ListParams::default()));
        let l2_ok = unwrap_success(&l2_resp);
        let l2_result: ListResult = serde_json::from_value(l2_ok.result.clone()).unwrap();
        assert!(l2_result.contexts.is_empty());
    }
}
