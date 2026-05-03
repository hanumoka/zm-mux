//! Synchronous, single-connection transport for the CustomPaneBackend
//! protocol over a local socket (Unix domain socket on Mac/Linux, named
//! pipe on Windows). Phase 2.1.A scope.
//!
//! The wire protocol is **NDJSON**: each frame is a complete JSON value
//! followed by `\n`. Server reads request lines, dispatches via
//! [`crate::rpc::dispatch`], writes one response line, then drains and
//! emits any buffered server→client notifications.
//!
//! Single-connection means the server accepts one client, serves it until
//! that side closes, then either exits ([`BackendServer::serve_one`]) or
//! loops to accept the next client ([`BackendServer::serve_forever`]).
//! No concurrency, no multiplexing — those land with `tokio` adoption in
//! the future full reference (docs/13 Section 4 D8–D9).
//!
//! [`Client`] is the bare-bones counterpart: it connects, sends typed
//! requests, and reads typed messages. MIN-D4's `mock_client` example is
//! built on it directly.
//!
//! Design notes:
//!
//! - Both halves of the framed connection (read / write) share the same
//!   Stream; we hold a `try_clone`'d copy of it for the writer so a
//!   `BufReader` can own its half.
//! - Parse errors on the server send no response. JSON-RPC 2.0 requires a
//!   Null-id parse-error response, but our [`crate::rpc::RequestId`] enum
//!   doesn't carry a Null variant yet (TODO for the full reference). For
//!   minimal scope, the server logs and skips — the client will time out
//!   or notice the missing response and close.

use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};

use interprocess::TryClone;
use interprocess::local_socket::{
    GenericNamespaced, ListenerOptions, Stream, ToNsName,
    traits::{Listener as _, Stream as _},
};
use serde::{Deserialize, Serialize};

use crate::mux_api::types::MuxMethod;
use crate::mux_api::{MuxHandler, dispatch_mux};
use crate::rpc::{BackendHandler, Notification, Request, Response, dispatch};

/// Server-side framing + dispatch over a local socket.
pub struct BackendServer<H: BackendHandler + Send + 'static> {
    handler: Arc<Mutex<H>>,
    socket_name: String,
}

impl<H: BackendHandler + Send + 'static> BackendServer<H> {
    pub fn new(handler: H, socket_name: impl Into<String>) -> Self {
        Self {
            handler: Arc::new(Mutex::new(handler)),
            socket_name: socket_name.into(),
        }
    }

    /// The handler shared with this server. Tests and demo code can use this
    /// to inspect or mutate state alongside the server thread.
    pub fn handler_handle(&self) -> Arc<Mutex<H>> {
        Arc::clone(&self.handler)
    }

    pub fn socket_name(&self) -> &str {
        &self.socket_name
    }

    /// Accept exactly one client connection, serve it until the client
    /// closes, then return.
    pub fn serve_one(&self) -> std::io::Result<()> {
        let listener = self.bind()?;
        let stream = listener.accept()?;
        handle_connection(stream, &self.handler)
    }

    /// Accept-and-serve loop. Blocks forever (or until an accept error).
    /// Each connection is handled inline before the next accept — no
    /// concurrency.
    pub fn serve_forever(&self) -> std::io::Result<()> {
        let listener = self.bind()?;
        loop {
            match listener.accept() {
                Ok(stream) => {
                    if let Err(e) = handle_connection(stream, &self.handler) {
                        eprintln!("zm-socket: connection ended with error: {e}");
                    }
                }
                Err(e) => {
                    eprintln!("zm-socket: accept failed: {e}");
                    return Err(e);
                }
            }
        }
    }

    fn bind(&self) -> std::io::Result<interprocess::local_socket::Listener> {
        let name = self
            .socket_name
            .as_str()
            .to_ns_name::<GenericNamespaced>()?;
        ListenerOptions::new().name(name).create_sync()
    }
}

fn handle_connection<H: BackendHandler>(
    stream: Stream,
    handler: &Arc<Mutex<H>>,
) -> std::io::Result<()> {
    // Split the duplex stream into read and write halves so a BufReader can
    // own one side without blocking writes from the other.
    let writer_half = stream;
    let reader_half = writer_half.try_clone()?;
    let mut reader = BufReader::new(reader_half);
    let mut writer = writer_half;

    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => return Ok(()), // clean EOF
            Ok(_) => {}
            Err(e) => return Err(e),
        }

        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }

        let request: Request = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                // See module-level note: spec says respond with Null-id
                // parse error; we don't have Null variant yet.
                eprintln!("zm-socket: parse error on incoming line: {e}; ignored");
                continue;
            }
        };

        // Dispatch + drain inside one lock so that any notifications
        // buffered *during* dispatch (e.g. by a future `kill` that emits
        // a context_exited) ride out together with notifications that
        // accumulated before this request arrived.
        let (response, notifications) = {
            let mut h = handler.lock().expect("zm-socket: handler mutex poisoned");
            let resp = dispatch(&mut *h, request);
            let notifs = h.drain_notifications();
            (resp, notifs)
        };

        // Wire order: notifications first, response last. The client's
        // `call()` blocks on its matching response, so emitting
        // notifications first means they're already received and queued
        // by the time `call()` returns.
        for n in notifications {
            write_message(&mut writer, &n)?;
        }
        write_message(&mut writer, &response)?;
    }
}

fn write_message<T: Serialize, W: Write>(writer: &mut W, msg: &T) -> std::io::Result<()> {
    let mut bytes = serde_json::to_vec(msg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    bytes.push(b'\n');
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}

// ---- MuxServer (Phase 2.2) --------------------------------------------------

/// Server for the `mux.*` protocol. Same NDJSON framing as
/// [`BackendServer`], but dispatches to a [`MuxHandler`] implementation.
/// No notifications in the mux protocol (read-only for now).
pub struct MuxServer<M: MuxHandler + Send + Sync + 'static> {
    handler: Arc<M>,
    socket_name: String,
}

impl<M: MuxHandler + Send + Sync + 'static> MuxServer<M> {
    pub fn new(handler: Arc<M>, socket_name: impl Into<String>) -> Self {
        Self {
            handler,
            socket_name: socket_name.into(),
        }
    }

    pub fn socket_name(&self) -> &str {
        &self.socket_name
    }

    pub fn serve_forever(&self) -> std::io::Result<()> {
        let listener = self.bind()?;
        loop {
            match listener.accept() {
                Ok(stream) => {
                    if let Err(e) = handle_mux_connection(stream, &self.handler) {
                        eprintln!("zm-socket(mux): connection ended with error: {e}");
                    }
                }
                Err(e) => {
                    eprintln!("zm-socket(mux): accept failed: {e}");
                    return Err(e);
                }
            }
        }
    }

    fn bind(&self) -> std::io::Result<interprocess::local_socket::Listener> {
        let name = self
            .socket_name
            .as_str()
            .to_ns_name::<GenericNamespaced>()?;
        ListenerOptions::new().name(name).create_sync()
    }
}

fn handle_mux_connection<M: MuxHandler>(
    stream: Stream,
    handler: &Arc<M>,
) -> std::io::Result<()> {
    let writer_half = stream;
    let reader_half = writer_half.try_clone()?;
    let mut reader = BufReader::new(reader_half);
    let mut writer = writer_half;

    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => return Ok(()),
            Ok(_) => {}
            Err(e) => return Err(e),
        }

        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }

        let request: Request = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("zm-socket(mux): parse error: {e}; ignored");
                continue;
            }
        };

        let response = if MuxMethod::is_mux_method(&request.method) {
            dispatch_mux(&**handler, request)
        } else {
            use crate::rpc::{RpcError, ResponseError};
            Response::Error(ResponseError::new(
                request.id,
                RpcError::new(
                    RpcError::METHOD_NOT_FOUND,
                    format!("mux server does not handle: {}", request.method),
                ),
            ))
        };

        write_message(&mut writer, &response)?;
    }
}

// ---- Client side ------------------------------------------------------------

/// A single message read from the server. Either a full [`Response`] to a
/// previous request (matched by `id`), or a server→client [`Notification`]
/// (no `id`, push event).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum IncomingMessage {
    Response(Response),
    Notification(Notification),
}

/// Bare client wrapper around a local-socket connection. Provides
/// line-framed send/recv on top of typed RPC values.
pub struct Client {
    reader: BufReader<Stream>,
    writer: Stream,
}

impl Client {
    pub fn connect(socket_name: &str) -> std::io::Result<Self> {
        let name = socket_name.to_ns_name::<GenericNamespaced>()?;
        let writer = Stream::connect(name)?;
        let reader = BufReader::new(writer.try_clone()?);
        Ok(Self { reader, writer })
    }

    pub fn send_request(&mut self, request: &Request) -> std::io::Result<()> {
        write_message(&mut self.writer, request)
    }

    /// Read the next NDJSON frame from the server and deserialize it as
    /// either a Response or a Notification. Returns `Ok(None)` on clean
    /// EOF.
    pub fn recv_message(&mut self) -> std::io::Result<Option<IncomingMessage>> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => Ok(None),
            Ok(_) => {
                let trimmed = line.trim_end();
                if trimmed.is_empty() {
                    return Ok(None);
                }
                let msg: IncomingMessage = serde_json::from_str(trimmed).map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
                })?;
                Ok(Some(msg))
            }
            Err(e) => Err(e),
        }
    }

    /// Send a request and read responses/notifications until a Response with
    /// the matching `id` arrives. Notifications encountered along the way
    /// are returned in order.
    pub fn call(
        &mut self,
        request: &Request,
    ) -> std::io::Result<(Response, Vec<Notification>)> {
        self.send_request(request)?;
        let target_id = request.id.clone();
        let mut notifications = Vec::new();
        loop {
            match self.recv_message()? {
                Some(IncomingMessage::Response(resp)) => {
                    let resp_id = match &resp {
                        Response::Success(s) => &s.id,
                        Response::Error(e) => &e.id,
                    };
                    if resp_id == &target_id {
                        return Ok((resp, notifications));
                    } else {
                        // Out-of-order response; for minimal single-flight
                        // client this is a protocol violation. Surface as
                        // an error.
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!(
                                "received response for unexpected id (got {resp_id:?}, expected {target_id:?})"
                            ),
                        ));
                    }
                }
                Some(IncomingMessage::Notification(n)) => notifications.push(n),
                None => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "server closed connection before responding",
                    ));
                }
            }
        }
    }
}
