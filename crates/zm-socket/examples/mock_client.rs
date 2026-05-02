//! Mock client demonstration of the CustomPaneBackend protocol
//! (issue #26572). Phase 2.1.A MIN-D4.
//!
//! Runs a self-contained server + client roundtrip and prints every
//! frame on the wire as pretty-printed JSON. Designed to be the
//! asciinema / OBS recording target for the github issue #26572
//! advocacy comment.
//!
//! Run with:
//!     cargo run -p zm-socket --example mock_client
//!
//! The demo exercises all 6 client→server methods plus the
//! `context_exited` server→client notification:
//!
//!   1. initialize     — handshake
//!   2. spawn_agent    — register a teammate context
//!   3. write          — send stdin (base64) to the context
//!   4. capture        — read scrollback
//!   5. list           — enumerate active contexts
//!   6. (simulate_exit triggered out-of-band on the handler)
//!   7. list           — server emits the buffered notification
//!                       before the response, so the client receives
//!                       the context_exited push attached to this call
//!   8. kill           — evict the context

use std::collections::HashMap;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use zm_socket::rpc::{
    CaptureParams, InitParams, KillParams, ListParams, MinimalHandler, Request, RequestId,
    Response, RpcMethod, SpawnAgentParams, SpawnAgentResult, WriteParams,
};
use zm_socket::transport_sync::{BackendServer, Client};

fn main() -> std::io::Result<()> {
    let socket_name = unique_name();

    println!("============================================================");
    println!(" zm-mux CustomPaneBackend reference  (Phase 2.1.A MIN-D4)");
    println!(" issue #26572 advocacy demo");
    println!("============================================================");
    println!();
    println!("[setup] socket name: {socket_name}");
    println!();

    // Spawn the server in a background thread. handler_handle gives us a
    // cloneable Arc<Mutex<MinimalHandler>> so the demo can poke
    // simulate_exit between RPCs without going through the wire.
    let server = BackendServer::new(MinimalHandler::new(), socket_name.clone());
    let handler = server.handler_handle();
    let server_thread = thread::spawn(move || server.serve_one());

    // Generous bind window — minimal sync scope, no oneshot signal.
    thread::sleep(Duration::from_millis(150));
    println!("[server] listening, accepting one connection");

    let mut client = Client::connect(&socket_name)?;
    println!("[client] connected");
    println!();

    // ---- 1. initialize ------------------------------------------------------
    let init = Request::new(
        RequestId::Num(1),
        RpcMethod::Initialize,
        InitParams {
            protocol_version: "1.0".to_string(),
        },
    )
    .unwrap();
    print_roundtrip(&mut client, &init, "initialize")?;

    // ---- 2. spawn_agent -----------------------------------------------------
    let mut env = HashMap::new();
    env.insert("ZM_MUX_AGENT_ROLE".to_string(), "reviewer".to_string());
    let spawn = Request::new(
        RequestId::Num(2),
        RpcMethod::SpawnAgent,
        SpawnAgentParams {
            argv: vec![
                "claude".to_string(),
                "--role".to_string(),
                "reviewer".to_string(),
            ],
            env,
            cwd: None,
            name: Some("reviewer-1".to_string()),
        },
    )
    .unwrap();
    let spawn_resp = print_roundtrip(&mut client, &spawn, "spawn_agent")?;
    let spawn_result: SpawnAgentResult = match spawn_resp {
        Response::Success(s) => serde_json::from_value(s.result).unwrap(),
        Response::Error(e) => {
            eprintln!("spawn_agent failed: {} ({})", e.error.message, e.error.code);
            return Ok(());
        }
    };
    let context_id = spawn_result.context_id;

    // ---- 3. write -----------------------------------------------------------
    let write = Request::new(
        RequestId::Num(3),
        RpcMethod::Write,
        WriteParams {
            context_id: context_id.clone(),
            data: "aGVsbG8gd29ybGQK".to_string(), // base64("hello world\n")
        },
    )
    .unwrap();
    print_roundtrip(&mut client, &write, "write")?;

    // ---- 4. capture ---------------------------------------------------------
    let capture = Request::new(
        RequestId::Num(4),
        RpcMethod::Capture,
        CaptureParams {
            context_id: context_id.clone(),
            lines: 10,
        },
    )
    .unwrap();
    print_roundtrip(&mut client, &capture, "capture")?;

    // ---- 5. list (before exit) ---------------------------------------------
    let list1 = Request::new(RequestId::Num(5), RpcMethod::List, ListParams::default()).unwrap();
    print_roundtrip(&mut client, &list1, "list (running)")?;

    // ---- 6. simulate exit (out-of-band, no wire traffic) -------------------
    println!("--- [demo] triggering MinimalHandler::simulate_exit out of band");
    {
        let mut h = handler.lock().unwrap();
        h.simulate_exit(&context_id, 0).unwrap();
    }
    println!("--- [demo] handler now has 1 buffered context_exited notification");
    println!();

    // ---- 7. list (after exit) — server flushes the notification BEFORE
    //         the response per our wire order so the client picks it up
    //         attached to this call.
    let list2 = Request::new(RequestId::Num(6), RpcMethod::List, ListParams::default()).unwrap();
    print_roundtrip(&mut client, &list2, "list (after exit)")?;

    // ---- 8. kill ------------------------------------------------------------
    let kill = Request::new(
        RequestId::Num(7),
        RpcMethod::Kill,
        KillParams {
            context_id: context_id.clone(),
        },
    )
    .unwrap();
    print_roundtrip(&mut client, &kill, "kill")?;

    // ---- final list (empty) ------------------------------------------------
    let list3 = Request::new(RequestId::Num(8), RpcMethod::List, ListParams::default()).unwrap();
    print_roundtrip(&mut client, &list3, "list (empty)")?;

    drop(client);
    match server_thread.join() {
        Ok(Ok(())) => {}
        Ok(Err(e)) => eprintln!("[server] returned error: {e}"),
        Err(_) => eprintln!("[server] panicked"),
    }

    println!("============================================================");
    println!(" Demo complete. 8 RPCs + 1 notification round-tripped over a");
    println!(" {} local socket.",
        if cfg!(windows) { "Windows named pipe" } else { "Unix domain" });
    println!("============================================================");

    Ok(())
}

fn print_roundtrip(
    client: &mut Client,
    req: &Request,
    label: &str,
) -> std::io::Result<Response> {
    println!("--- {label} ---");
    println!(">>> request:");
    println!("{}", serde_json::to_string_pretty(req).unwrap());
    let (resp, notifs) = client.call(req)?;
    for n in &notifs {
        println!("<<< notification (server→client push):");
        println!("{}", serde_json::to_string_pretty(n).unwrap());
    }
    println!("<<< response:");
    println!("{}", serde_json::to_string_pretty(&resp).unwrap());
    println!();
    Ok(resp)
}

fn unique_name() -> String {
    let pid = std::process::id();
    let nano = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("zm-mux-demo-{pid}-{nano}")
}
