//! JSON-RPC 2.0 types for the CustomPaneBackend protocol (issue #26572).
//!
//! Wire format: NDJSON (newline-delimited JSON) over a Unix domain socket
//! (Mac/Linux) or Windows named pipe. Each line is a complete JSON object
//! conforming to JSON-RPC 2.0.
//!
//! Protocol surface = 6 client→server methods + 1 server→client notification:
//!
//! | Direction | Method | Purpose |
//! |---|---|---|
//! | client→server | `initialize` | Handshake, returns `self_context_id` |
//! | client→server | `spawn_agent` | Spawn a teammate process in a new pane |
//! | client→server | `write` | Send stdin bytes to a context (base64) |
//! | client→server | `capture` | Read scrollback from a context |
//! | client→server | `kill` | Terminate a context |
//! | client→server | `list` | Enumerate active contexts |
//! | server→client | `context_exited` | Push notification when a context exits |
//!
//! Phase 2.1.A scope: types only. Handler / transport / server come in
//! MIN-D2 / MIN-D3.

mod handler_min;
mod types;

pub use handler_min::{
    BackendHandler, CONTEXT_NOT_FOUND, ContextEntry, ContextRegistry, MinimalHandler, dispatch,
};
pub use types::{
    CaptureParams, CaptureResult, ContextExitedParams, ContextId, ContextInfo, ContextStatus,
    InitParams, InitResult, JSONRPC_VERSION, KillParams, ListParams, ListResult, Notification,
    Request, RequestId, Response, ResponseError, ResponseSuccess, RpcError, RpcMethod,
    SpawnAgentParams, SpawnAgentResult, WriteParams,
};
