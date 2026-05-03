pub mod types;

use serde::{Serialize, de::DeserializeOwned};

use crate::rpc::{
    JSONRPC_VERSION, Request, RequestId, Response, ResponseError, ResponseSuccess, RpcError,
};

use types::{
    ClosePaneParams, ClosePaneResult, CloseTabParams, CloseTabResult, CreateTabParams,
    CreateTabResult, FocusPaneParams, FocusPaneResult, GetStatusParams, GetStatusResult,
    ListPanesParams, ListPanesResult, MuxMethod, SendKeysParams, SendKeysResult,
    SetAgentInfoParams, SetAgentInfoResult, SplitPaneParams, SplitPaneResult,
};

pub trait MuxHandler {
    fn handle_list_panes(&self, params: ListPanesParams) -> Result<ListPanesResult, RpcError>;
    fn handle_get_status(&self, params: GetStatusParams) -> Result<GetStatusResult, RpcError>;
    fn handle_send_keys(&self, params: SendKeysParams) -> Result<SendKeysResult, RpcError>;
    fn handle_focus_pane(&self, params: FocusPaneParams) -> Result<FocusPaneResult, RpcError>;
    fn handle_split_pane(&self, params: SplitPaneParams) -> Result<SplitPaneResult, RpcError>;
    fn handle_close_pane(&self, params: ClosePaneParams) -> Result<ClosePaneResult, RpcError>;
    fn handle_create_tab(&self, params: CreateTabParams) -> Result<CreateTabResult, RpcError>;
    fn handle_close_tab(&self, params: CloseTabParams) -> Result<CloseTabResult, RpcError>;
    fn handle_set_agent_info(&self, params: SetAgentInfoParams) -> Result<SetAgentInfoResult, RpcError>;
}

pub fn dispatch_mux<H: MuxHandler>(handler: &H, request: Request) -> Response {
    let id = request.id.clone();

    let method = match MuxMethod::parse_method(&request.method) {
        Some(m) => m,
        None => {
            return error_response(
                id,
                RpcError::METHOD_NOT_FOUND,
                format!("unknown mux method: {}", request.method),
            );
        }
    };

    match method {
        MuxMethod::ListPanes => dispatch_op(id, &request, |p| handler.handle_list_panes(p)),
        MuxMethod::GetStatus => dispatch_op(id, &request, |p| handler.handle_get_status(p)),
        MuxMethod::SendKeys => dispatch_op(id, &request, |p| handler.handle_send_keys(p)),
        MuxMethod::FocusPane => dispatch_op(id, &request, |p| handler.handle_focus_pane(p)),
        MuxMethod::SplitPane => dispatch_op(id, &request, |p| handler.handle_split_pane(p)),
        MuxMethod::ClosePane => dispatch_op(id, &request, |p| handler.handle_close_pane(p)),
        MuxMethod::CreateTab => dispatch_op(id, &request, |p| handler.handle_create_tab(p)),
        MuxMethod::CloseTab => dispatch_op(id, &request, |p| handler.handle_close_tab(p)),
        MuxMethod::SetAgentInfo => dispatch_op(id, &request, |p| handler.handle_set_agent_info(p)),
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
    use types::{PaneInfo, PANE_NOT_FOUND, TAB_NOT_FOUND, CANNOT_CLOSE_LAST_TAB};

    struct MockHandler;

    impl MuxHandler for MockHandler {
        fn handle_list_panes(&self, _p: ListPanesParams) -> Result<ListPanesResult, RpcError> {
            Ok(ListPanesResult {
                panes: vec![
                    PaneInfo { pane_id: 0, tab_id: 0, focused: true, cols: 80, rows: 24, title: "pwsh".into(), agent_type: "unknown".into(), agent_status: "unknown".into() },
                    PaneInfo { pane_id: 1, tab_id: 0, focused: false, cols: 80, rows: 24, title: "pwsh".into(), agent_type: "unknown".into(), agent_status: "unknown".into() },
                ],
            })
        }

        fn handle_get_status(&self, _p: GetStatusParams) -> Result<GetStatusResult, RpcError> {
            Ok(GetStatusResult {
                workspace_id: "ws-test".into(), pid: 1234, version: "0.1.0".into(),
                active_tab: 0, pane_count: 2, tab_count: 1, socket_path: "zm-mux-1234".into(),
            })
        }

        fn handle_send_keys(&self, p: SendKeysParams) -> Result<SendKeysResult, RpcError> {
            if p.pane_id == 99 {
                return Err(RpcError::new(PANE_NOT_FOUND, "pane not found"));
            }
            Ok(SendKeysResult {})
        }

        fn handle_focus_pane(&self, p: FocusPaneParams) -> Result<FocusPaneResult, RpcError> {
            if p.pane_id == 99 {
                return Err(RpcError::new(PANE_NOT_FOUND, "pane not found"));
            }
            Ok(FocusPaneResult {})
        }

        fn handle_split_pane(&self, p: SplitPaneParams) -> Result<SplitPaneResult, RpcError> {
            if p.pane_id == 99 {
                return Err(RpcError::new(PANE_NOT_FOUND, "pane not found"));
            }
            Ok(SplitPaneResult { new_pane_id: 2 })
        }

        fn handle_close_pane(&self, p: ClosePaneParams) -> Result<ClosePaneResult, RpcError> {
            if p.pane_id == 99 {
                return Err(RpcError::new(PANE_NOT_FOUND, "pane not found"));
            }
            Ok(ClosePaneResult {})
        }

        fn handle_create_tab(&self, _p: CreateTabParams) -> Result<CreateTabResult, RpcError> {
            Ok(CreateTabResult { tab_id: 1, pane_id: 3 })
        }

        fn handle_close_tab(&self, p: CloseTabParams) -> Result<CloseTabResult, RpcError> {
            if p.tab_id == 99 {
                return Err(RpcError::new(TAB_NOT_FOUND, "tab not found"));
            }
            if p.tab_id == 0 {
                return Err(RpcError::new(CANNOT_CLOSE_LAST_TAB, "cannot close last tab"));
            }
            Ok(CloseTabResult {})
        }

        fn handle_set_agent_info(&self, p: SetAgentInfoParams) -> Result<SetAgentInfoResult, RpcError> {
            if p.pane_id == 99 {
                return Err(RpcError::new(PANE_NOT_FOUND, "pane not found"));
            }
            Ok(SetAgentInfoResult {})
        }
    }

    fn mux_req<P: Serialize>(id: i64, method: MuxMethod, params: P) -> Request {
        Request {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: RequestId::Num(id),
            method: method.as_str().to_string(),
            params: serde_json::to_value(params).unwrap(),
        }
    }

    fn unwrap_success(resp: &Response) -> &ResponseSuccess {
        match resp {
            Response::Success(s) => s,
            Response::Error(e) => panic!("expected success, got error: {:?}", e.error),
        }
    }

    fn unwrap_error(resp: &Response) -> &ResponseError {
        match resp {
            Response::Error(e) => e,
            Response::Success(s) => panic!("expected error, got success: {:?}", s.result),
        }
    }

    #[test]
    fn dispatch_list_panes() {
        let handler = MockHandler;
        let req = mux_req(1, MuxMethod::ListPanes, ListPanesParams {});
        let resp = dispatch_mux(&handler, req);
        let s = unwrap_success(&resp);
        let result: ListPanesResult = serde_json::from_value(s.result.clone()).unwrap();
        assert_eq!(result.panes.len(), 2);
        assert_eq!(result.panes[0].pane_id, 0);
        assert!(result.panes[0].focused);
    }

    #[test]
    fn dispatch_get_status() {
        let handler = MockHandler;
        let req = mux_req(2, MuxMethod::GetStatus, GetStatusParams {});
        let resp = dispatch_mux(&handler, req);
        let s = unwrap_success(&resp);
        let result: GetStatusResult = serde_json::from_value(s.result.clone()).unwrap();
        assert_eq!(result.pid, 1234);
        assert_eq!(result.pane_count, 2);
    }

    #[test]
    fn dispatch_unknown_mux_method() {
        let handler = MockHandler;
        let req = Request {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: RequestId::Num(3),
            method: "mux.nonexistent".to_string(),
            params: serde_json::json!({}),
        };
        let resp = dispatch_mux(&handler, req);
        let e = unwrap_error(&resp);
        assert_eq!(e.error.code, RpcError::METHOD_NOT_FOUND);
    }

    #[test]
    fn dispatch_invalid_params() {
        let handler = MockHandler;
        let req = Request {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: RequestId::Num(4),
            method: "mux.list_panes".to_string(),
            params: serde_json::json!("not an object"),
        };
        let resp = dispatch_mux(&handler, req);
        let e = unwrap_error(&resp);
        assert_eq!(e.error.code, RpcError::INVALID_PARAMS);
    }

    #[test]
    fn dispatch_send_keys() {
        let handler = MockHandler;
        let req = mux_req(5, MuxMethod::SendKeys, SendKeysParams { pane_id: 0, data: "ls\r\n".into() });
        let resp = dispatch_mux(&handler, req);
        unwrap_success(&resp);
    }

    #[test]
    fn dispatch_send_keys_pane_not_found() {
        let handler = MockHandler;
        let req = mux_req(6, MuxMethod::SendKeys, SendKeysParams { pane_id: 99, data: "x".into() });
        let resp = dispatch_mux(&handler, req);
        let e = unwrap_error(&resp);
        assert_eq!(e.error.code, PANE_NOT_FOUND);
    }

    #[test]
    fn dispatch_focus_pane() {
        let handler = MockHandler;
        let req = mux_req(7, MuxMethod::FocusPane, FocusPaneParams { pane_id: 1 });
        let resp = dispatch_mux(&handler, req);
        unwrap_success(&resp);
    }

    #[test]
    fn dispatch_split_pane() {
        let handler = MockHandler;
        let req = mux_req(8, MuxMethod::SplitPane, SplitPaneParams { pane_id: 0, direction: "horizontal".into() });
        let resp = dispatch_mux(&handler, req);
        let s = unwrap_success(&resp);
        let result: SplitPaneResult = serde_json::from_value(s.result.clone()).unwrap();
        assert_eq!(result.new_pane_id, 2);
    }

    #[test]
    fn dispatch_close_pane() {
        let handler = MockHandler;
        let req = mux_req(9, MuxMethod::ClosePane, ClosePaneParams { pane_id: 1 });
        let resp = dispatch_mux(&handler, req);
        unwrap_success(&resp);
    }

    #[test]
    fn dispatch_create_tab() {
        let handler = MockHandler;
        let req = mux_req(10, MuxMethod::CreateTab, CreateTabParams {});
        let resp = dispatch_mux(&handler, req);
        let s = unwrap_success(&resp);
        let result: CreateTabResult = serde_json::from_value(s.result.clone()).unwrap();
        assert_eq!(result.tab_id, 1);
        assert_eq!(result.pane_id, 3);
    }

    #[test]
    fn dispatch_close_tab() {
        let handler = MockHandler;
        let req = mux_req(11, MuxMethod::CloseTab, CloseTabParams { tab_id: 1 });
        let resp = dispatch_mux(&handler, req);
        unwrap_success(&resp);
    }

    #[test]
    fn dispatch_close_tab_last_tab_error() {
        let handler = MockHandler;
        let req = mux_req(12, MuxMethod::CloseTab, CloseTabParams { tab_id: 0 });
        let resp = dispatch_mux(&handler, req);
        let e = unwrap_error(&resp);
        assert_eq!(e.error.code, CANNOT_CLOSE_LAST_TAB);
    }

    #[test]
    fn dispatch_set_agent_info() {
        let handler = MockHandler;
        let req = mux_req(13, MuxMethod::SetAgentInfo, SetAgentInfoParams {
            pane_id: 0,
            agent_type: Some("claude".into()),
            agent_status: Some("active".into()),
        });
        let resp = dispatch_mux(&handler, req);
        unwrap_success(&resp);
    }

    #[test]
    fn dispatch_set_agent_info_pane_not_found() {
        let handler = MockHandler;
        let req = mux_req(14, MuxMethod::SetAgentInfo, SetAgentInfoParams {
            pane_id: 99,
            agent_type: None,
            agent_status: Some("error".into()),
        });
        let resp = dispatch_mux(&handler, req);
        let e = unwrap_error(&resp);
        assert_eq!(e.error.code, PANE_NOT_FOUND);
    }
}
