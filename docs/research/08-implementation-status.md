# 08 — 구현 현황 (Phase 0–3 자율 완성)

> 오버나잇 자율 작업 결과 요약. 로드맵([06](06-feasibility-and-roadmap.md)) Phase 0~3 + 폴리시를 구현·검증했다.
> 각 항목은 빌드 + 테스트 + (가능 시) 런타임 종단 검증을 거쳤다. 범례 [00](00-overview.md).

## 0. 한눈에

- **빌드**: `cargo build --workspace` 클린(경고 0). **테스트 22개 통과**: zm-core 2 / zm-mux 7 / zm-term 5 / zm-pty 1 / zm-agent 4 / zm-ipc 3.
- **런타임**: Windows(WSL 없이)에서 GPU 단일/멀티 pane, 탭, 자동화 소켓, tmux-shim, 알림 모두 실측 동작.
- **실행**: `cargo run -p zm-app` (또는 `target\debug\zm-app.exe`).
- **자동화 CLI**: `target\debug\zm.exe`, **tmux shim**: `target\debug\tmux.exe`.

## 1. 크레이트 현황

| 크레이트 | 상태 | 내용 |
|------|------|------|
| zm-core | ✅ | 공유 타입(GridSize/Rgba/CellSnapshot/Cursor/Notification) + **Config(TOML)** + 색 hex 파싱 |
| zm-pty | ✅ | portable-pty 래퍼(ConPTY+POSIX). spawn/spawn_cmd/resize/kill/killer |
| zm-term | ✅ | alacritty_terminal 0.26 래퍼: feed/snapshot/resize/**scroll**/**capture_text**/DSR 응답/**OSC 9·777 알림**/Kitty 모드 + 256색 팔레트 |
| zm-render | ✅ | wgpu29+glyphon0.11: **다중 pane**, 셀 배경(rect 파이프라인), 블록/빔/언더라인 커서, **탭바**, 포커스 프레임, sRGB 정합 |
| zm-mux | ✅ | 분할 트리(좌우/상하)·탭·포커스·기하·close 자동 collapse (단위 테스트 5) |
| zm-ipc | ✅ | 로컬 소켓 server/client(interprocess) + JSON 프로토콜 + **`zm` CLI** |
| zm-agent | ✅ | **`tmux` shim**(tmux→zm-ipc 변환) + 에이전트 env 헬퍼(트랙 A) |
| zm-app | ✅ | winit 배선: pane 맵·라우팅·**설정 단축키**·마우스·휠·리사이즈·자동화 서버·알림·env 주입 |
| zm-probe | ✅ | R1 isTTY 하네스(Phase 0, [07](07-poc-conpty-istty-results.md)) |

## 2. 기능 + 검증 방법

### 2.1 멀티플렉서 (분할/탭)
- 설정 단축키(기본): 새탭 `Ctrl+T`, 닫기 `Ctrl+Shift+W`(탭)/`Ctrl+Shift+P`(pane),
  분할 `Ctrl+Shift+D`(상하)/`Ctrl+Shift+E`(좌우), 탭전환 `Ctrl+Tab`/`Ctrl+Shift+Tab`,
  **방향 포커스 `Alt+방향키`**, **줌 `Ctrl+Shift+Z`**.
- 마우스: **클릭 포커스**, **divider 드래그로 분할 비율 조정**, **휠 스크롤백**.
- 상단 **탭바**(활성 강조), 활성 pane **파란 프레임**, 분할 divider, **pane 줌(전체화면 토글)**.
- 검증: zm-mux 단위테스트 7(split/close/collapse/tabs/focus/resize/zoom) + 실행 후 조작. (설정은 `docs/configuration.md`.)

### 2.2 자동화 소켓 (Phase 2)
- `ZM_MUX_SOCKET` env로 노출(자식 pane 상속). `zm <cmd>` 로 제어.
- 명령: `list-panes`/`list-tabs`/`split [-v|-h]`/`new-tab`/`focus DIR`/`select-pane ID`/`send-keys DATA`/`capture-pane`/`kill-pane ID`.
- **실측**: 소켓으로 split→pane 스폰, list-panes→정확한 geometry, send-keys→입력, capture-pane→내용 회수 확인.
- 테스트: 앱 실행 후 다른 셸에서 `$env:ZM_MUX_SOCKET="zm-mux-<앱PID>.sock"; zm list-panes`.

### 2.3 에이전트 연동 — 트랙 A (Phase 3) ★
- pane에 **tmux-shim env 주입**: `TMUX`, `TMUX_PANE=%<id>`, `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`, PATH 앞에 shim dir.
- **`tmux` shim**이 `split-window`/`send-keys`/`select-pane`/`list-panes`/`new-window`/`kill-pane`/`display-message`/`-V`를 zm-ipc로 변환.
- **실측**: pane 안에서 `echo %TMUX%`→값 있음, `where tmux`→우리 shim 최우선, `tmux split-window -h -P`→`%2` + 실제 분할.
- **전제 충족**: isTTY=true([07]) + TMUX 존재 + 실동작 shim + AGENT_TEAMS=1 → Claude Code agent-teams 트랙 A 준비 완료.
- **남은 1건(사용자 실측)**: pane에서 `claude` 로그인 후 teammate 모드가 실제로 tmux 백엔드(우리 shim)를 구동하는지 관찰
  (`claude --debug-file`로 `isInProcessEnabled:false`/tmux 백엔드 시도 확인). 로그인·비용·상호작용 필요로 보류.

### 2.4 알림 + Shift+Enter (Phase 3)
- **OSC 9 / OSC 777** 파싱(청크 경계 캐리 처리) → 데스크톱 토스트(notify-rust, best-effort) + 작업표시줄 주의 환기 + 로그.
- **실측**: pane에서 node로 OSC 9 방출 → 로그 `알림: zm-mux — ...` 확인.
- **Shift+Enter**: Kitty 키보드 모드(alacritty `DISAMBIGUATE_ESC_CODES`) 활성 시 수식 Enter를 CSI-u(`ESC[13;mu`)로 인코딩.
  (앱이 Kitty 모드를 켜면 동작. 쿼리 응답은 alacritty가 처리.)

### 2.5 설정 (Phase 1)
- `%APPDATA%\zm-mux\config.toml`(또는 `$ZM_MUX_CONFIG`). 섹션: `[font] [colors] [scrollback] [shell] [keybindings] [agent]`.
- **사용자 기존 config 호환**(JetBrains Mono 16, #1a1a2e/#e0e0e0 등 그대로 로드). 미지 필드 무시, 누락 기본값.
- 폰트 미설치 시 자동 폴백. 스키마 전체: `docs/configuration.md`.

## 3. 검증 결과 요약 [V]

| 항목 | 결과 |
|------|------|
| R1 isTTY (Phase 0) | ✅ 12/12 true ([07]) |
| R2 버전 lockstep | ✅ 전체 cargo check ([07] 부록 A) |
| 단일/멀티 pane GPU 렌더 | ✅ 런타임 |
| 분할/탭/포커스/마우스/스크롤백 | ✅ 단위테스트 + 런타임 |
| 자동화 소켓(zm CLI) | ✅ 종단 실측 |
| tmux-shim + 에이전트 env | ✅ 종단 실측 |
| OSC 알림 | ✅ 종단 실측 + 단위테스트 |

## 4. 의식적 보류 (미검증 코드 미투입 원칙)

- **CPU 폴백(softbuffer+tiny-skia)**: GPU가 동작하는 타깃에선 트리거 불가 → 대형 신규 렌더 경로를 *검증 없이* 넣지 않음.
  대신 GPU 초기화 실패 시 **패닉 대신 명확한 메시지 + 정상 종료**(graceful)로 처리. (추후 GPU 강제 실패 환경에서 구현·검증.)
- **macOS 런타임**: 코드는 크로스플랫폼(분기 적용)이나 Windows에서만 실측. Mac 빌드/실행은 미검증.
- **claude teammate 라이브 실측**: §2.3 — 로그인/비용/상호작용 필요로 사용자 몫.

## 5. 다음 단계 후보

- claude teammate 라이브 실측 → 결과를 [07]/본 문서에 기록.
- CustomPaneBackend(#26572) 트랙 B 시제품(JSON-RPC) — tmux 비의존 정공법.
- CPU 폴백 실구현(GPU 강제 실패 테스트 포함), macOS 실측, 세션 지속/워크스페이스 자동네이밍(Phase 4).
- 드래그로 분할 비율 조정, pane 제목, 검색(스크롤백) 등 폴리시.

> 빌드/실행/테스트 방법은 §0, 설정은 `docs/configuration.md`, isTTY/lockstep 세부는 [07].
