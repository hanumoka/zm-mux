# 03 — 권장 네이티브 Rust 아키텍처 + cmux 기능 매핑

> zm-mux의 *설계 방향*(구현은 다음 플랜). clean-room: 아래 설계는 공식 사양 + SAFE 크레이트로
> 독립 도출했으며 STUDY repo의 코드/구조를 복사하지 않는다([05](05-reference-inventory.md)).
> 범례 [00](00-overview.md).

## 1. 설계 원칙

1. **server/client 분리** — mux 서버가 PTY·세션 상태를 보유, UI 클라이언트는 로컬 소켓으로 접속.
   → 세션 지속성 + 프로그램 제어(자동화 API) + cmux식 "primitive" 철학을 한 번에. (학습: WezTerm·Zellij·tmux)
2. **플랫폼 추상화는 경계에만** — PTY(ConPTY/POSIX)·IPC(named pipe/UDS)·알림(toast/notif)·렌더(GPU/CPU)
   를 각 1개 크레이트 경계로 격리. 상위 로직은 OS 비의존.
3. **GPU 우선 + CPU 폴백** — wgpu 실패 시 softbuffer+tiny-skia로 그레이스풀 다운그레이드.
4. **에이전트 우선** — tmux 호환 + 알림 + 자동화 소켓을 1급 기능으로(데코레이션 아님).

## 2. 크레이트 레이아웃 (workspace)

```
crates/
├── zm-core/    # 공유 타입·에러·설정(config)·ID. OS 비의존.
├── zm-pty/     # PTY 추상화  → portable-pty (ConPTY + POSIX)
├── zm-term/    # VT 에뮬/그리드 → alacritty_terminal + vte
├── zm-render/  # GPU: wgpu+glyphon+cosmic-text / CPU 폴백: softbuffer+tiny-skia
├── zm-mux/     # 세션/탭/pane 트리 + 분할 레이아웃 + 멀티플렉서 코어
├── zm-ipc/     # 로컬 소켓 server/client → interprocess. 자동화 API(JSON-RPC/라인 프로토콜)
├── zm-agent/   # 에이전트 감지 + tmux-compat shim + 알림(OSC 9/99/777, Windows toast) + CustomPaneBackend 트랙
└── zm-app/     # winit 앱 엔트리 + 이벤트 루프 + 배선
```

> 이 분해는 WezTerm의 *term ↔ gui ↔ mux 분리 사상*에서 배웠으나(아키텍처 아이디어, MIT/SAFE),
> 크레이트 경계·이름·코드는 zm-mux 독자 설계다.

## 3. 런타임 토폴로지

```
 [zm-app UI client] ──local socket(interprocess)──┐
 [external CLI: `zm` ]──────────────────────────── │──> [zm-mux server]
 [AI agent via tmux-shim / CustomPaneBackend ]──────┘        │ holds: PTYs(zm-pty)
                                                             │        terminal grids(zm-term)
                                                             │        session/tab/pane tree
 render: winit → wgpu surface → glyphon(text)+rect(cursor/border)   ▼ CPU fallback: softbuffer+tiny-skia
```

- **서버**: PTY 스폰/IO, VT 파싱→그리드, 세션/레이아웃 상태, 자동화 명령 처리.
- **클라이언트(UI)**: 윈도우·입력·렌더. 서버 상태 구독.
- **자동화/에이전트**: 동일 소켓으로 `new-window/split/send-keys/capture/...` 류 오퍼레이션.

## 4. 렌더 파이프라인

| 단계 | SAFE 크레이트 |
|------|--------------|
| 윈도우/이벤트 | winit 0.30 |
| GPU 디바이스/서피스 | wgpu 29 (Windows=DX12, mac=Metal, Linux=Vulkan 자동) |
| 텍스트 셰이핑 | cosmic-text 0.19 (HarfRust) |
| 글리프 아틀라스/드로우 | glyphon 0.11 |
| 커서/보더/배경 사각형 | wgpu 자체 rect 셰이더 |
| CPU 폴백 | softbuffer 0.4 + tiny-skia 0.12 |

> 통합 *패턴*은 COSMIC Terminal이 입증(alacritty_terminal+glyphon+wgpu)했으나 cosmic-term은
> **GPL/STUDY** → 코드 미참조, 동일 SAFE 크레이트로 독립 구성. 버전 lockstep은 [02](02-windows-no-wsl-feasibility.md) [?].

## 5. cmux 기능 → zm-mux 컴포넌트 매핑 (clean-room 소싱)

| cmux 기능 (관찰, [01](01-cmux-analysis.md)) | macOS 결박 | zm-mux 컴포넌트 | SAFE 소싱(학습 대상) |
|------|------|------|------|
| 터미널 렌더(libghostty/Metal) | Metal | zm-render (wgpu+glyphon) | alacritty/cosmic 패턴 |
| PTY 호스팅 | POSIX PTY | zm-pty (portable-pty/ConPTY) | WezTerm pty, microsoft/terminal ConPTY 샘플 |
| 분할 레이아웃(Bonsplit) | — | zm-mux 레이아웃 | Zellij 레이아웃, 자체 설계 |
| 워크스페이스/탭 | — | zm-mux 세션 트리 | WezTerm/Zellij |
| 소켓 자동화 API(/tmp/cmux.sock) | UDS | zm-ipc (interprocess local socket) | tmux/cmux 계약 *형태* |
| tmux 호환(Claude Code) | — | zm-agent (tmux-shim) | psmux, cmux shim *접근* |
| 멀티 에이전트(Claude/Codex/Gemini) | — | zm-agent 감지/훅 | cmux 훅 *접근* |
| 데스크톱 알림 | macOS Notif | zm-agent 알림(OSC+toast) | 표준 OSC 9/99/777 |
| 임베디드 브라우저(WebKit) | WebKit | (후순위/선택) | — *(MVP 제외 권장)* |
| 원격 데몬/프레즌스 | — | (후순위) zm-ipc 원격 | — |

> 임베디드 브라우저는 cmux의 차별점이나 WebKit 결박 + 범위 과대 → zm-mux **MVP에서 제외 권장**,
> 후속에 WebView2(Windows)/WKWebView(mac) 추상화로 검토. [I]

## 6. SAFE 레퍼런스별 학습 매핑

| repo (SAFE) | 무엇을 배우나 |
|------|------|
| wezterm | term/gui/mux 크레이트 분리, portable-pty 사용법, ConPTY 처리, GPU 렌더 |
| zellij | server/client mux 프로토콜, 레이아웃/세션, Windows 네이티브 패턴 |
| alacritty + vte | VT 에뮬레이션·파서 사용법, 성능 |
| psmux | Windows ConPTY tmux 호환, **isTTY 우회**, Claude Code 팀 연동 |
| microsoft/terminal | ConPTY 공식 API/샘플 |

> STUDY repo(cmux/cosmic-term/wmux*/cmux-for-linux/cmux-linux)는 *동작·UX·접근*만 관찰. 코드 비복사.
> (wmux는 MIT이나 TS/Electron이라 코드 이식이 아니라 Windows UX/연동 *학습* 대상.)

## 7. 다음 단계로의 인계

- 구현 플랜에서: 본 크레이트 레이아웃으로 스캐폴드 → `cargo check`로 버전 lockstep 확정 →
  Phase 0 PoC(단일 pane: ConPTY+VT+GPU 렌더). 로드맵 [06](06-feasibility-and-roadmap.md).
