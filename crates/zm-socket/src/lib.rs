// zm-socket: IPC and protocol layer
// Phase 2.1.A (MIN-D1) — CustomPaneBackend JSON-RPC types
// Phase 2.1.A (MIN-D2) — MinimalHandler + dispatch routing
// Phase 2.1.A (MIN-D3) — sync transport (Unix socket + named pipe)
// Phase 2.2 — Socket API (zm-mux self-coordination on the same transport)
// Phase 3.3 — MCP server (rmcp)

pub mod rpc;
pub mod transport_sync;
