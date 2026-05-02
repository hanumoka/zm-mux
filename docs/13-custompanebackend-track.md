# CustomPaneBackend 트랙 — Phase 2.1 신규 사양 (Branch B)

> **목적**: tmux 호환 트랙([`docs/12`](./12-istty-workaround.md)) 이 BLOCKED 됨에 따라, **CustomPaneBackend JSON-RPC 7-op 프로토콜 (#26572)** 을 zm-mux Phase 2.1 의 새 트랙으로 격상. 이 문서는 사양 + 구현 분해 + 위험 + 검증 전략을 박제.
>
> **상위 결정**: [`docs/10-critical-risks-update.md`](./10-critical-risks-update.md) 의 리스크 3 (tmux 프로토콜 안정성) 의 후속. 원래 Phase 3.3.2 (Week 41-46) 에 배치됐던 작업이 Phase 2.1 으로 격상됨.

---

## 🟡 STRATEGY UPDATE (2026-05-02 후속)

Pre-D1 sub-spike Layer 1 (claude.exe 254 MB binary 정적 검사) 결과:

| 검색어 | 매칭 |
|---|---|
| `CLAUDE_PANE_BACKEND` / `PANE_BACKEND` / `pane_backend` | **0** |
| `spawn_agent` / `context_exited` / `CustomPane` | **0** |
| `teammate_mode` (기존 CLI flag) | 6 |
| `TmuxBackend` (기존 macOS 활성 클래스) | 28 |

→ **Anthropic 이 #26572 사양을 claude 2.1.126 에 아직 안 넣음.** TmuxBackend 28건 = 기존 macOS 동작 코드는 살아있고 isTTY 게이트로 차단됐을 뿐.

### 전략 옵션 비교 (사용자 채택 = B)

| 옵션 | 분량 | 가치 | 위험 |
|---|---|---|---|
| A. Full 12일 reference | 12일 | Anthropic 채택 시 1차 reference 100% | 미채택 시 sunk cost, 즉시 사용자 가치 0 |
| **B. Minimal reference (3~4일) + Phase 2.2 self-coordination 병행** | 13~14일 | 1차 reference 영향력 + 즉시 사용자 가치 | 분량 약간 큼 |
| C. Branch B 보류, Phase 2.2 만 | 10일 | 즉시 가치만 | 1차 reference 자리 양보 |

**채택 = B**. Minimal reference 3~4일이 Anthropic 미채택 시 sunk cost 한계, Phase 2.2 가 채택 여부와 독립적으로 사용자 가치. 두 트랙 병행 = 비대칭 hedge.

→ 아래 Section 4 (12일 분해) 는 **"Anthropic 채택 후 full reference 확장 plan"** 으로 강등. 현재 진행은 **Section 4-MIN (Minimal 3~4일 분해)** 사용.

---

---

## 1. 배경

### 1.1 tmux 호환 트랙이 막힌 이유 (요약)

[`docs/12`](./12-istty-workaround.md) 의 STATUS BLOCKED 섹션 참조. 핵심:

- psmux 3.3.4 + PowerShell 7 + native Windows 환경에서 `claude` 실행 → agent team 생성 시도 → **in-process Task 도구로 fallback**, 화면 분할 0건
- Claude Code 2.1.126 에는 `/team` 류 슬래시 명령이 없음 (autocomplete 결과 0건). teammate 기능이 사용자 수준에서 invoke 가능한 entry point 가 없음
- Claude 자체가 spike 도중 *"swap to WSL+tmux"* 를 제안 — native Windows 경로에서 작동 안 함을 implicit 인정
- Anthropic 측 fix 의향 없음 (#26244, #34150 모두 closed-not-planned)

→ **tmux 호환만으로는 Claude Code 의 agent team 기능을 끌어낼 수 없음.** CustomPaneBackend 가 사실상 유일한 viable path.

### 1.2 CustomPaneBackend (#26572) 의 핵심 가치

@Wirasm (KILD 개발자) 가 제안한 공식 프로토콜. **tmux CLI 의존을 JSON-RPC 2.0 + 환경변수 hook 로 대체**:

- `CLAUDE_PANE_BACKEND=/path/to/binary` (on-demand spawn) 또는
- `CLAUDE_PANE_BACKEND_SOCKET=/path/to/server.sock` (pre-running 서버)

→ Claude Code 가 위 env var 를 감지하면 tmux CLI 호출 대신 JSON-RPC 로 zm-mux 와 직접 통신. **isTTY 게이트와 무관** (env var 가 명시 신호이므로).

### 1.3 zm-mux 가 1차 reference 구현이 되는 의미

이슈 #26572 는 **OPEN** 상태 + Anthropic 측 응답 미확인. zm-mux 가 1차 구현체로 선점하면:
1. **Anthropic 측 채택 가속** — 동작하는 reference 가 있으면 명세 채택 결정이 빨라짐
2. **zm-mux 가 사양 미세조정 영향력 확보** — 7-op 정의를 우리 사용 사례에 맞게 다듬을 기회
3. **Windows 사용자에게 즉시 가치** — WSL 의존 없이 agent team 동작
4. **다른 터미널 (Ghostty, WezTerm, Zellij 등) 의 후속 채택 가능성** — 우리가 검증한 사양이 ecosystem 표준이 됨

---

## 2. 프로토콜 사양 (#26572 본문 + docs/10 합성)

### 2.1 전송 layer

- **포맷**: JSON-RPC 2.0 over **NDJSON** (newline-delimited JSON)
- **연결 방식 2종**:
  - On-demand spawn: `CLAUDE_PANE_BACKEND=/path/to/zm-mux-backend` 환경변수 → Claude Code 가 자식 프로세스로 spawn, stdio 로 JSON-RPC
  - Pre-running socket: `CLAUDE_PANE_BACKEND_SOCKET=/path/to/server.sock` (Unix domain socket) 또는 Windows named pipe → Claude Code 가 connect 후 JSON-RPC

zm-mux 는 **두 모드 모두 지원** 권장 — Pre-running 이 메인 (zm-mux 가 살아있는 동안), on-demand 는 fallback (zm-mux 안 떠있어도 Claude Code 가 autospawn).

### 2.2 7개 필수 오퍼레이션

| 메서드 | 대체하는 tmux 명령 | 설명 | 파라미터 (요약) |
|---|---|---|---|
| `initialize` | `display-message -p "#{pane_id}"` | 핸드셰이크. zm-mux 가 `self_context_id` 반환 (현재 pane 식별자) | `{ protocol_version: "1.0" }` → `{ self_context_id: "<id>" }` |
| `spawn_agent` | `split-window` | agent 프로세스 시작. **`argv[]` 직접 전달** (셸 문자열 escape 위험 회피) | `{ argv: ["claude", "--role", "..."], env: {...}, cwd, name }` → `{ context_id: "<new-id>" }` |
| `write` | `send-keys` | 특정 context 의 stdin 으로 데이터 전송 (base64 encoded) | `{ context_id, data: "<b64>" }` |
| `capture` | `capture-pane` | 스크롤백 읽기 (선택적). agent 출력 검사용 | `{ context_id, lines: <n> }` → `{ data: "<text>" }` |
| `kill` | `kill-pane` | context 종료 | `{ context_id }` |
| `list` | `list-panes` | 활성 context 열거 | `{}` → `{ contexts: [{id, name, status, ...}, ...] }` |
| `context_exited` | (없음) | **푸시 이벤트** — context 종료 시 Claude Code 측에 통지 | `{ context_id, exit_code }` (server → client 단방향) |

### 2.3 해결되는 기존 이슈

- **#23615 레이스 컨디션**: tmux `split-window` + `send-keys` 비동기 → 영속 백엔드가 직렬화
- **#23572 사일런트 폴백**: 명시적 `CLAUDE_PANE_BACKEND` 환경변수 → 사용자가 의도적 활성/비활성 구분
- **#24189 (Ghostty), #24122 (Zellij), #23574 (WezTerm)**: 터미널별 차단 → 클린 프로토콜 (모든 터미널이 동일 인터페이스)
- **#26244 (Windows isTTY)**: env var 자체가 명시 신호이므로 isTTY 와 무관

---

## 3. zm-mux 적용 결정

### 결정 1: `zm-socket` 크레이트가 주 작업장

기존 8 크레이트 중:
- ❌ `zm-agent` (docs/12 의 원래 후보) — tmux 호환이 BLOCKED 이라 거의 빈 채로 둠. 향후 agent 감지/리밸런싱 (Phase 2.4) 에서 활용
- ✅ `zm-socket` — 원래 Phase 2.2 (Socket API 골격) 용으로 만든 크레이트. CustomPaneBackend 도 동일 socket 추상 위에 올라감 → 동일 크레이트 안에서 자연스럽게 통합

**이점**: Phase 2.1 (CustomPaneBackend) 와 Phase 2.2 (zm-mux 자체 Socket API) 가 같은 크레이트 안에 있으면 transport / serialization / connection state 추상 재사용. 두 작업의 시너지가 큼.

### 결정 2: 두 모드 모두 지원, pre-running socket 우선

- **pre-running socket 모드** (Phase 2.1.A, ~9일): zm-mux 가 살아있는 동안 socket listener 떠있음. Claude Code 가 `CLAUDE_PANE_BACKEND_SOCKET=<path>` 로 connect.
- **on-demand spawn 모드** (Phase 2.1.B, ~3일): 별도 small binary `zm-mux-backend` 가 stdio 로 JSON-RPC. Claude Code 가 `CLAUDE_PANE_BACKEND=zm-mux-backend` 로 spawn.

순서: A 가 먼저, B 는 follow-up. A 만으로도 zm-mux 가 살아있는 normal use case 다 cover. B 는 zm-mux 안 떠있는 사용자가 `claude` 직접 실행했을 때의 fallback (낮은 우선순위).

### 결정 3: 동기 prototype → 비동기 production 의 2단계 진화

**프로토타입 (Phase 2.1.A 의 D1~D3)**: 동기 (blocking) 구현 — `std::os::unix::net::UnixListener` (Mac) + `std::os::windows::pipe::*` (Win). 멀티 클라이언트 동시 처리 안 함, 단일 connection 만. 기능 검증 우선.

**production (Phase 2.1.A 의 D8~D9)**: `tokio` 도입. `tokio::net::UnixListener` + `tokio::net::windows::named_pipe::NamedPipeServer`. 멀티 connection 동시 처리, async I/O.

**근거**: tokio 가 zm-mux workspace 에 첫 도입되는 시점. 동기 prototype 으로 프로토콜 자체부터 검증한 뒤 비동기로 옮기면 디버깅 용이 + tokio 의 lifetime 압박을 처음 7일에 회피. CustomPaneBackend 의 protocol-level bug 와 tokio-level bug 가 섞이면 격리 어려움.

### 결정 4: JSON-RPC 라이브러리 채택

- **`serde_json`** (이미 docs/10 확정 stack) — JSON 직렬화
- **`jsonrpc-core` 또는 자체 구현** — JSON-RPC 2.0 envelope. 7-op 만 다루므로 자체 구현이 ~150 LOC 으로 자기완결 가능 (외부 의존 줄이고 control 확보). `jsonrpc-core` 는 server/client 양쪽 다 지원하지만 우리는 server only — 자체 구현 권장.
- **`tokio-util::codec::LinesCodec`** (production 단계) — NDJSON 라인 단위 framing

### 결정 5: 사양 vs 명세의 갭 처리 정책

이슈 #26572 는 OPEN — Anthropic 측 응답 0건. 우리 1차 구현 시 사양에 ambiguity 가 있으면:

1. **이슈 본문의 명시적 표기 우선** — 가장 신뢰
2. **psmux/cmux 의 동등 동작 모방** (예: `capture` 의 line 단위 vs character 단위 결정) — 실증된 패턴 차용
3. **둘 다 안 되면 issue 측 댓글로 질의 + 보수적 가정 (가장 strict 한 해석) 채택** — 향후 Anthropic 사양 픽스 시 우리가 더 strict 했던 게 안전

---

## 4-MIN. Minimal Reference 분해 (3~4일, 현재 진행 중)

> **목적**: Anthropic 의 #26572 채택 여부와 무관하게, 우리(zm-mux) 가 사양을 reference 구현으로 박제 + advocacy 영상/로그 확보. 아래 분량은 *진짜 zm-mux PaneTree 와의 통합 없이* 프로토콜 자체만 self-contained 하게 검증.

| Day | 작업 | 크레이트 / 모듈 | 산출물 |
|---|---|---|---|
| MIN-D1 | JSON-RPC 2.0 envelope + 7-op enum 정의 (스냅샷 단위테스트) | `zm-socket::rpc::types` | `Request`/`Response`/`Notification` + `RpcMethod` enum |
| MIN-D2 | Handler trait + in-memory minimal 구현 (initialize/list/spawn_agent stub — 진짜 PTY spawn 안 함, 가짜 ContextId 반환) | `zm-socket::rpc::handler_min` | `MinimalHandler` (메모리 ContextRegistry 만) |
| MIN-D3 | Sync transport (Unix socket Mac + named pipe Win, 단일 connection) — listener 자체만 자기완결 | `zm-socket::transport_sync` | `BackendServer::listen_blocking()` |
| MIN-D4 | Mock client (Rust binary) — 4 RPC 라운드트립 + asciinema 녹화 / 영상 박제 | `crates/zm-socket/examples/mock_client.rs` | demo 영상 + Markdown 보고서 |

### 4-MIN.1 모듈 트리 (minimal)

```
crates/zm-socket/
├── Cargo.toml         # serde_json (필수), nix (Unix), windows-sys (Win)
├── src/
│   ├── lib.rs
│   ├── rpc/
│   │   ├── mod.rs
│   │   ├── types.rs           # MIN-D1
│   │   └── handler_min.rs     # MIN-D2
│   └── transport_sync.rs      # MIN-D3
├── examples/
│   └── mock_client.rs         # MIN-D4
└── tests/
    └── rpc_roundtrip.rs       # MIN-D1 의 snapshot 테스트
```

### 4-MIN.2 Phase 2.2 와의 시너지

zm-socket 크레이트가 minimal reference (Phase 2.1.A) + zm-mux 자체 Socket API (Phase 2.2) 양쪽의 기반. transport / serialization / connection state 추상이 공유:

- `transport_sync.rs` 의 Unix socket / named pipe listener → Phase 2.2 의 zm-mux 명령 socket 도 동일 listener 위에 (`/zm-mux/{pid}.sock` 같은 별 endpoint)
- `rpc::types::RpcMethod` enum 이 7-op + zm-mux 자체 명령 (split, list-agents, send-keys, kill) 통합 enum 으로 확장

Phase 2.2 진입 시 minimal reference 의 transport 코드가 그대로 reuse, 추가 작업은 새 RpcMethod variant + handler 만.

### 4-MIN.3 Advocacy (Phase 2.1.B, 1일 외부 액션)

minimal reference 가 동작하면:

1. asciinema / OBS 로 mock client ↔ minimal handler 의 7-op 라운드트립 영상 (~30초)
2. github issue #26572 댓글 작성:
   - Layer 1 finding (claude 2.1.126 에 사양 키워드 0건)
   - 우리 spike 결과 (psmux 도 in-process fallback) 영상 링크
   - 우리 minimal reference 동작 영상 + 코드 링크
   - **1차 reference 구현자 자원 표명** + 사양 ambiguity query (capture 의 line 단위 vs character 단위, spawn_agent 의 cwd inheritance 등)
3. (선택) cmux 메인테이너 + Calyx 메인테이너에게 cross-reference

### 4-MIN.4 검증 (3~4일 통과 기준)

- MIN-D1: 7-op 각각 JSON 라운드트립 snapshot test (insta) green
- MIN-D2: MinimalHandler 가 4 op (initialize/list/spawn_agent/kill) 응답 정확. ContextRegistry lifecycle 단위테스트
- MIN-D3: `nc -U <socket>` (Mac) / `Test-NamedPipe` (Win) 으로 connect 확인
- MIN-D4: `cargo run --example mock_client` 로 4 RPC 라운드트립 정상 + 출력 영상 박제

**진짜 claude 와의 E2E (D11 격이었던 작업) 는 N/A** — Anthropic 코드 미인지 확정. 향후 채택 시 Section 4 의 full 12일로 확장.

---

## 4. 12일 구현 분해 (Phase 2.1.A — pre-running socket 모드, 향후 Anthropic 채택 시)

| Day | 작업 | 크레이트 / 모듈 | 산출물 |
|---|---|---|---|
| D1 | JSON-RPC 2.0 envelope + 7-op enum 정의 | `zm-socket::rpc::types` | `Request`/`Response`/`Notification` 타입, 7-op `RpcMethod` enum |
| D2 | `initialize` + `list` 구현 (read-only ops) | `zm-socket::rpc::handler` | `handle_initialize`, `handle_list`, `ContextRegistry` 신규 |
| D3 | `spawn_agent` 구현 + zm-mux 측 새 패인 spawn 연동 | `zm-socket::rpc::handler`, `zm-mux::Mux::spawn_pane()` 연결 | `handle_spawn_agent` |
| D4 | `write` + `capture` + `kill` 구현 | `zm-socket::rpc::handler` | 나머지 client→server ops |
| D5 | `context_exited` 푸시 이벤트 — child process 종료 감지 → notification emit | `zm-socket::rpc::events` | event channel + emitter |
| D6 | Unix domain socket listener (Mac) + named pipe (Win) — 동기 단일 connection | `zm-socket::transport` | `BackendServer::listen_blocking()` |
| D7 | 환경변수 주입: zm-mux 가 새 패인 spawn 시 `CLAUDE_PANE_BACKEND_SOCKET=<path>` env 자동 set | `zm-app::create_pane` (line 98-103 hook) | env var auto-set |
| D8 | tokio 도입 + async transport 재작성 | `zm-socket::transport_async` | `BackendServer::listen_async()` |
| D9 | 멀티 connection 처리 + connection state 격리 | `zm-socket::transport_async` | per-connection `ContextRegistry` |
| D10 | 통합 테스트: mock claude.exe (JSON-RPC 7-op 호출 시뮬레이션) | `crates/zm-socket/tests/` | mock client + 7-op 라운드트립 검증 |
| D11 | E2E 수동 검증: 진짜 `claude` 실행, agent team 생성 → 새 zm-mux 패인 spawn 확인 | 전체 | 검증 보고 + 영상 |
| D12 | github issue #26572 댓글 (영상 + 로그 첨부, 1차 구현 표명) + 미세 조정 | docs + 외부 액션 | comment posted |

### 4.1 모듈 트리

```
crates/zm-socket/
├── Cargo.toml          # serde_json, tokio (D8+), nix (Unix), windows-sys (Win)
├── src/
│   ├── lib.rs          # public API
│   ├── rpc/
│   │   ├── mod.rs
│   │   ├── types.rs    # Request, Response, Notification, ContextId
│   │   ├── handler.rs  # 7-op handle_* functions
│   │   └── events.rs   # context_exited push notifications
│   ├── transport.rs    # 동기 Unix socket + named pipe (D6)
│   ├── transport_async.rs # tokio 비동기 (D8+)
│   └── server.rs       # BackendServer 통합 진입점
└── tests/
    └── mock_client.rs  # JSON-RPC 시뮬레이션 (D10)
```

### 4.2 함수 시그니처 핵심

```rust
// types.rs (D1)
pub enum RpcMethod {
    Initialize,
    SpawnAgent,
    Write,
    Capture,
    Kill,
    List,
}

pub struct Request {
    pub jsonrpc: &'static str,  // "2.0"
    pub id: serde_json::Value,
    pub method: RpcMethod,
    pub params: serde_json::Value,
}

// handler.rs (D2)
pub trait BackendHandler {
    fn handle_initialize(&mut self, params: InitParams) -> Result<InitResult>;
    fn handle_spawn_agent(&mut self, params: SpawnParams) -> Result<SpawnResult>;
    fn handle_write(&mut self, params: WriteParams) -> Result<()>;
    fn handle_capture(&mut self, params: CaptureParams) -> Result<CaptureResult>;
    fn handle_kill(&mut self, params: KillParams) -> Result<()>;
    fn handle_list(&mut self, params: ListParams) -> Result<ListResult>;
}

pub struct ContextRegistry {
    contexts: HashMap<ContextId, ContextState>,
}

pub struct ContextState {
    pub id: ContextId,
    pub name: String,
    pub pane_id: zm_mux::PaneId,        // zm-mux 자체 PaneId 와 매핑
    pub status: ContextStatus,
    pub child_handle: Arc<Mutex<ZmPtyProcess>>,  // 직접 ref
}

// server.rs (D6/D8)
pub struct BackendServer {
    listener: TransportListener,
    registry: Arc<Mutex<ContextRegistry>>,
    mux: Arc<Mutex<MuxState>>,           // zm-mux::MuxState 참조
}

impl BackendServer {
    pub fn listen_blocking(&mut self) -> ZmResult<()>;
    #[cfg(feature = "tokio")]
    pub async fn listen_async(&mut self) -> ZmResult<()>;
}
```

### 4.3 zm-mux 통합 hook

`zm-app::create_pane()` (line 98-103) 가 패인 spawn 시 backend socket path 를 env var 로 주입:

```rust
fn create_pane(&mut self, cols: u16, rows: u16) -> PaneState {
    let mut cmd = CommandBuilder::new_default_prog();
    if let Some(socket_path) = self.backend_server.socket_path() {
        cmd.env("CLAUDE_PANE_BACKEND_SOCKET", socket_path);
    }
    let pty = zm_pty::spawn_pty(rows, cols, cmd)?;
    // ... 기존 흐름
}
```

→ 모든 zm-mux 패인 안에서 `claude` 실행 시 자동으로 backend 사용.

---

## 5. 위험 / 미해결

1. **Claude Code 가 `CLAUDE_PANE_BACKEND_SOCKET` 을 진짜로 인지하는가**: 사양 (#26572) 에는 명시되어 있지만 실제 Claude Code 2.1.126 에 코드가 들어갔는지는 미확인. **D2 직후 1시간 spike 필수** — minimal initialize-only server 로 Claude Code 가 connect 시도하는지 확인. 안 하면 Anthropic 측 구현 일정 대기 (장기) 또는 fork PR 기여.

2. **사양의 ambiguity**: #26572 본문이 7-op 의 정확한 wire format 까지 다 명시 안 함. `capture` 의 결과 포맷 (raw bytes vs UTF-8 strict vs ANSI escape preserve), `spawn_agent` 의 환경변수 inheritance 정책, `context_exited` 의 ordering guarantees 등 미정. **결정 5 의 정책으로 처리** + 우리가 #26572 댓글로 query.

3. **`zm-mux::Mux::spawn_pane()` API 변화**: 현재 zm-mux 의 split 은 사용자 단축키 (Ctrl+Shift+D/E) 트리거. CustomPaneBackend 는 **외부 (RPC) 트리거** 라 새 진입점 필요. zm-mux 의 split API 가 외부에서 호출 가능하게 노출되어야 함 (현재 internal-only 일 가능성).

4. **`spawn_agent` 의 working dir 정책**: Claude Code 가 cwd 를 명시하면 그대로, 안 하면 zm-mux 의 현재 active pane 의 cwd 상속? 또는 user home? psmux 는 active session 의 cwd 사용. zm-mux 도 같은 정책 채택 권장 (Phase 2.1 이후 user config 로 override 가능하게).

5. **socket path 의 OS-specific 위치**: Unix = `$XDG_RUNTIME_DIR/zm-mux/<pid>.sock` (없으면 `/tmp/zm-mux-<uid>/<pid>.sock`), Windows = `\\.\pipe\zm-mux-<pid>`. AppArmor / SELinux 환경 추가 검증 필요.

6. **테스트 가능성**: D10 의 mock claude.exe 는 우리가 작성. 진짜 claude 와의 E2E (D11) 가 가능할까? Anthropic 측 코드가 아직 안 들어가 있으면 우리 mock 만으로 검증 — 진짜 검증은 Anthropic 측 채택 후. **D11 가 N/A 인 시나리오 가능** → 그 경우 #26572 댓글로 우리 reference 를 제출하고 Anthropic 측 fork/PR 검토.

7. **WSL 사용자**: `CLAUDE_PANE_BACKEND_SOCKET` 의 path 가 WSL ↔ Win 변환 필요? 별도 spike — Phase 2.1 마지막 buffer day 또는 Phase 2.2 로 이월.

---

## 6. 검증 (Phase 2.1.A 통과 기준)

### D1~D5 단위테스트

- 7-op 각각의 request/response JSON 라운드트립 (serde_json snapshot, insta 권장)
- `ContextRegistry` 의 lifecycle (생성/조회/삭제/exit) 단위 테스트

### D6~D7 통합 (Mac + Windows 양쪽)

- `BackendServer::listen_blocking()` 시작 → `nc -U <socket>` (Unix) 또는 `Test-NamedPipe` (PowerShell on Win) 으로 connect 가능 확인
- env var injection: `zm-app` 실행 후 새 패인의 셸에서 `$env:CLAUDE_PANE_BACKEND_SOCKET` (Win) / `echo $CLAUDE_PANE_BACKEND_SOCKET` (Mac) 가 정확한 socket path 반환

### D10 통합테스트

- mock client 가 7-op 모두 호출 → 각각 정상 응답 + state 정합성
- 멀티 클라이언트 동시 (D8 이후): 2 connection 이 각자 별 ContextRegistry 가지는지

### D11 E2E (가능한 경우)

- 진짜 `claude` 실행 → backend socket env var 인지 → JSON-RPC connect 시도 → 우리 server 가 initialize 받는지 확인
- 로그 캡처 + 영상 박제

### D12 외부 액션

- github issue #26572 댓글: spike 결과 (psmux 동작 안 함) + 우리 reference 구현 동작 영상 + 1차 구현체 자원 표명
- 가능하면 fork PR 도 제출 검토 (Claude Code 측 코드 fork → CLAUDE_PANE_BACKEND_SOCKET 인지 코드 추가)

---

## 7. tmux 호환 트랙의 처리

`docs/12` 의 결정 1~5 는 BLOCKED 마크 후 reference 보존. 단, 일부 산출물은 **재사용 가능**:

- `set_tmux_env()` Rust 포팅 (docs/12 결정 1) — 향후 zm-mux 자체 tmux shim (Phase 2.2 또는 Post-1.0) 에서 동일 코드 재사용
- 셸 감지 로직 (docs/12 D2) — CustomPaneBackend 도 환경변수 주입 시 셸 감지 활용 (어떤 셸 안에서 spawn 되는지에 따라 quoting 다름)
- 경로 변환 (Unix↔Windows) — backend socket path 의 사용자 표시용

→ docs/12 의 코드 사양은 **dead code 아님, deferred reuse**. `docs/12` 의 STATUS BLOCKED 섹션에 명시.

---

## 8. 향후 unblock 시나리오

### 시나리오 A: Anthropic 이 isTTY 게이트 fix

이슈 #26244 가 reopen + fix → tmux 호환 트랙(docs/12) unblock. CustomPaneBackend 와 동시 지원 가능 (env var 로 사용자 선택).

### 시나리오 B: Claude Code 에 `--teammate-mode tmux` 가 진짜 코드 경로 연결

CLI flag 가 isTTY 게이트 우회까지 가져가는 코드가 추가되면 tmux 호환 트랙 단독으로 동작. CustomPaneBackend 와 양립.

### 시나리오 C: Anthropic 이 CustomPaneBackend 사양 (#26572) 채택

zm-mux 가 1차 reference → main path 로 정착. 12일 작업의 ROI 가 가장 큰 시나리오.

→ 어느 시나리오로 가도 **CustomPaneBackend 작업은 sunk cost 아님** — Mac/Linux 의 tmux 안 쓰는 사용자, 다른 터미널의 cross-pollination, future Anthropic 표준 등에 가치.

---

## 9. 참고

- [github issue #26572 — CustomPaneBackend 제안](https://github.com/anthropics/claude-code/issues/26572)
- [`docs/10-critical-risks-update.md`](./10-critical-risks-update.md) 리스크 3 (tmux 프로토콜 안정성 + CustomPaneBackend 발견)
- [`docs/12-istty-workaround.md`](./12-istty-workaround.md) — BLOCKED 트랙, spike 결과
- [`docs/11-implementation-roadmap.md`](./11-implementation-roadmap.md) — Phase 2/3 재배열
- [Claude Code issue #26244](https://github.com/anthropics/claude-code/issues/26244) — Windows isTTY (closed not planned)

---

*조사 완료: 2026-05-02 / 결정 박제: zm-mux Phase 2.1 신규 트랙*
