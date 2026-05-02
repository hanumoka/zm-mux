use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const JSONRPC_VERSION: &str = "2.0";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContextId(pub String);

impl ContextId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RpcMethod {
    Initialize,
    SpawnAgent,
    Write,
    Capture,
    Kill,
    List,
}

impl RpcMethod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Initialize => "initialize",
            Self::SpawnAgent => "spawn_agent",
            Self::Write => "write",
            Self::Capture => "capture",
            Self::Kill => "kill",
            Self::List => "list",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Num(i64),
    Str(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    pub params: Value,
}

impl Request {
    pub fn new<P: Serialize>(
        id: RequestId,
        method: RpcMethod,
        params: P,
    ) -> serde_json::Result<Self> {
        Ok(Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            method: method.as_str().to_string(),
            params: serde_json::to_value(params)?,
        })
    }

    pub fn parse_method(&self) -> Option<RpcMethod> {
        serde_json::from_value(Value::String(self.method.clone())).ok()
    }

    pub fn parse_params<P: serde::de::DeserializeOwned>(&self) -> serde_json::Result<P> {
        serde_json::from_value(self.params.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseSuccess {
    pub jsonrpc: String,
    pub id: RequestId,
    pub result: Value,
}

impl ResponseSuccess {
    pub fn new<R: Serialize>(id: RequestId, result: R) -> serde_json::Result<Self> {
        Ok(Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: serde_json::to_value(result)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseError {
    pub jsonrpc: String,
    pub id: RequestId,
    pub error: RpcError,
}

impl ResponseError {
    pub fn new(id: RequestId, error: RpcError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            error,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Response {
    Success(ResponseSuccess),
    Error(ResponseError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
}

/// Server→client push notification. Identified by `method` only — no `id`,
/// no response expected. Currently the only notification in the protocol is
/// `context_exited`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
}

impl Notification {
    pub const CONTEXT_EXITED: &'static str = "context_exited";

    pub fn context_exited(params: ContextExitedParams) -> serde_json::Result<Self> {
        Ok(Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: Self::CONTEXT_EXITED.to_string(),
            params: serde_json::to_value(params)?,
        })
    }
}

// ---- Per-method param / result types ----------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitParams {
    pub protocol_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitResult {
    pub self_context_id: ContextId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnAgentParams {
    pub argv: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnAgentResult {
    pub context_id: ContextId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteParams {
    pub context_id: ContextId,
    /// Base64-encoded bytes to write to the context's stdin.
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureParams {
    pub context_id: ContextId,
    pub lines: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureResult {
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillParams {
    pub context_id: ContextId,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListParams {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResult {
    pub contexts: Vec<ContextInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextInfo {
    pub id: ContextId,
    pub name: String,
    pub status: ContextStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextStatus {
    Running,
    Exited,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextExitedParams {
    pub context_id: ContextId,
    pub exit_code: i32,
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn rpc_method_str() {
        assert_eq!(RpcMethod::Initialize.as_str(), "initialize");
        assert_eq!(RpcMethod::SpawnAgent.as_str(), "spawn_agent");
        assert_eq!(RpcMethod::Write.as_str(), "write");
        assert_eq!(RpcMethod::Capture.as_str(), "capture");
        assert_eq!(RpcMethod::Kill.as_str(), "kill");
        assert_eq!(RpcMethod::List.as_str(), "list");
    }

    #[test]
    fn rpc_method_serde_roundtrip() {
        for m in [
            RpcMethod::Initialize,
            RpcMethod::SpawnAgent,
            RpcMethod::Write,
            RpcMethod::Capture,
            RpcMethod::Kill,
            RpcMethod::List,
        ] {
            let s = serde_json::to_value(m).unwrap();
            let m2: RpcMethod = serde_json::from_value(s).unwrap();
            assert_eq!(m, m2);
        }
    }

    #[test]
    fn request_id_num_roundtrip() {
        let id = RequestId::Num(42);
        let v = serde_json::to_value(&id).unwrap();
        assert_eq!(v, serde_json::json!(42));
        let id2: RequestId = serde_json::from_value(v).unwrap();
        assert_eq!(id, id2);
    }

    #[test]
    fn request_id_str_roundtrip() {
        let id = RequestId::Str("abc-123".to_string());
        let v = serde_json::to_value(&id).unwrap();
        assert_eq!(v, serde_json::json!("abc-123"));
        let id2: RequestId = serde_json::from_value(v).unwrap();
        assert_eq!(id, id2);
    }

    #[test]
    fn rpc_error_skip_data_when_none() {
        let err = RpcError::new(RpcError::INVALID_PARAMS, "bad params");
        let v = serde_json::to_value(&err).unwrap();
        let obj = v.as_object().unwrap();
        assert!(!obj.contains_key("data"), "data should be skipped when None");
        assert_eq!(obj.get("code"), Some(&serde_json::json!(-32602)));
    }

    #[test]
    fn parse_method_round_trips() {
        let req = Request::new(
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
        assert_eq!(req.parse_method(), Some(RpcMethod::SpawnAgent));
        let parsed: SpawnAgentParams = req.parse_params().unwrap();
        assert_eq!(parsed.argv, vec!["claude".to_string()]);
    }
}
