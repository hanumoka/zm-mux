use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::schemars;
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};

use zm_socket::mux_api::types::{
    ListPanesResult, MuxMethod, SendKeysParams,
};
use zm_socket::rpc::{JSONRPC_VERSION, Request, RequestId, Response};
use zm_socket::transport_sync::Client;

fn call_socket_sync(
    socket: &str,
    method: MuxMethod,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let mut client =
        Client::connect(socket).map_err(|e| format!("cannot connect to zm-mux: {e}"))?;
    let req = Request {
        jsonrpc: JSONRPC_VERSION.to_string(),
        id: RequestId::Num(1),
        method: method.as_str().to_string(),
        params,
    };
    let (resp, _) = client.call(&req).map_err(|e| format!("socket call failed: {e}"))?;
    match resp {
        Response::Success(s) => Ok(s.result),
        Response::Error(e) => Err(format!("{} (code {})", e.error.message, e.error.code)),
    }
}

#[derive(Debug, serde::Deserialize, rmcp::schemars::JsonSchema)]
struct SendMessageParams {
    #[schemars(description = "Pane ID of the target agent")]
    pane_id: u32,
    #[schemars(description = "Text message to send to the agent's terminal")]
    message: String,
}

struct ZmMuxMcp {
    socket_name: String,
    tool_router: ToolRouter<Self>,
}

impl ZmMuxMcp {
    fn new(socket_name: String) -> Self {
        Self {
            socket_name,
            tool_router: Self::tool_router(),
        }
    }

    fn call(&self, method: MuxMethod, params: serde_json::Value) -> Result<serde_json::Value, rmcp::ErrorData> {
        call_socket_sync(&self.socket_name, method, params)
            .map_err(|e| rmcp::ErrorData::internal_error(e, None))
    }
}

#[tool_router]
impl ZmMuxMcp {
    /// Get zm-mux workspace status: version, process ID, pane count, tab count
    #[tool]
    fn get_status(&self) -> Result<String, rmcp::ErrorData> {
        let val = self.call(MuxMethod::GetStatus, serde_json::json!({}))?;
        Ok(serde_json::to_string_pretty(&val).unwrap_or_default())
    }

    /// List all AI agents running in zm-mux. Returns only panes with a registered agent.
    #[tool]
    fn list_agents(&self) -> Result<String, rmcp::ErrorData> {
        let val = self.call(MuxMethod::ListPanes, serde_json::json!({}))?;
        let result: ListPanesResult = serde_json::from_value(val)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
        let agents: Vec<_> = result.panes.into_iter().filter(|p| p.agent_type != "unknown").collect();
        Ok(serde_json::to_string_pretty(&agents).unwrap_or_else(|_| "[]".into()))
    }

    /// Send a text message to an agent in a zm-mux pane via its terminal stdin
    #[tool]
    fn send_message(&self, Parameters(params): Parameters<SendMessageParams>) -> Result<String, rmcp::ErrorData> {
        let socket_params = SendKeysParams {
            pane_id: params.pane_id,
            data: params.message,
        };
        self.call(MuxMethod::SendKeys, serde_json::to_value(socket_params).unwrap())?;
        Ok("message sent".into())
    }

    /// Discover all peers in the zm-mux workspace: workspace info plus all panes and their agent status
    #[tool]
    fn peer_discover(&self) -> Result<String, rmcp::ErrorData> {
        let status = self.call(MuxMethod::GetStatus, serde_json::json!({}))?;
        let panes = self.call(MuxMethod::ListPanes, serde_json::json!({}))?;
        let combined = serde_json::json!({ "workspace": status, "panes": panes });
        Ok(serde_json::to_string_pretty(&combined).unwrap_or_default())
    }
}

#[tool_handler]
impl ServerHandler for ZmMuxMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("zm-mux MCP server — query and control the AI agent terminal multiplexer".into()),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability { list_changed: None }),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let socket_name = std::env::var("ZM_MUX_SOCKET_PATH").unwrap_or_else(|_| {
        eprintln!("error: ZM_MUX_SOCKET_PATH not set. Run inside a zm-mux pane.");
        std::process::exit(1);
    });
    eprintln!("zm-mux-mcp: connecting to {socket_name}");

    let mcp = ZmMuxMcp::new(socket_name);
    let service = mcp.serve(rmcp::transport::io::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
