# zm-mux 구현 작업 계획서

## Context

10건의 리서치(docs/01~10)와 4회의 비판적 검토를 통해 기술 스택, 아키텍처, 리스크가 확정되었다. 이 계획서는 **리서치 완료 → Rust 구현 시작**까지의 전체 작업 로드맵을 Phase별로 정의한다.

기술 스택: Rust + alacritty_terminal + cosmic-text + glyphon + wgpu + portable-pty (COSMIC Terminal 검증 패턴)

---

## Phase 0: 프로젝트 기초 (1~2주)

### 0.1 Cargo workspace 초기화

```
zm-mux/
├── Cargo.toml              # workspace
├── crates/
│   ├── zm-core/            # 공유 타입, 에러, 트레이트, 설정 구조체
│   ├── zm-pty/             # PTY 추상화 (portable-pty 래핑)
│   ├── zm-term/            # VT 에뮬레이션 (alacritty_terminal 래핑)
│   ├── zm-render/          # GPU/CPU 렌더링 (glyphon + wgpu + softbuffer)
│   ├── zm-mux/             # 멀티플렉서 (세션/윈도우/탭/패인 트리)
│   ├── zm-agent/           # 에이전트 감지/관리/tmux 호환
│   ├── zm-socket/          # Socket API + MCP + CustomPaneBackend
│   └── zm-app/             # 애플리케이션 진입점 (winit + 이벤트 루프)
├── docs/                   # 리서치 문서 (기존)
├── .claude/                # Claude Code 자동화 (기존)
└── config/                 # 기본 설정 파일 (TOML)
```

**zm-core 역할**: 공유 에러 타입(`ZmError`), 설정 구조체(`ZmConfig`), 크레이트 간 공유 트레이트, 이벤트 타입. 모든 다른 크레이트가 의존.

### 0.2 의존성 검증 + 빌드 확인

| 작업 | 설명 | 예상 |
|------|------|------|
| 0.2.1 | `alacritty_terminal` crates.io 가용성 확인 (`cargo search`) | 0.5일 |
| 0.2.2 | `glyphon` ↔ `wgpu` ↔ `cosmic-text` 버전 호환성 확인 | 0.5일 |
| 0.2.3 | `portable-pty` macOS ARM (Apple Silicon) 빌드 확인 | 0.5일 |
| 0.2.4 | 전체 의존성 트리 확인 (`cargo tree`), 버전 핀 | 0.5일 |
| 0.2.5 | Windows x64 + macOS ARM 크로스 빌드 (`cargo check`) | 1일 |

**대체 경로**: `alacritty_terminal`이 crates.io에 없으면 → git 의존성 또는 `vte` + 자체 상태 관리로 전환

### 0.3 CI/CD 기초

- GitHub Actions: Windows (x64) + macOS (ARM) 매트릭스 빌드
- `cargo clippy`, `cargo test`, `cargo fmt --check`
- 릴리즈 빌드: `cargo build --release` (바이너리 크기 확인)

### 0.4 프로젝트 파일

- `README.md`: 프로젝트 소개, 빌드 방법, 라이선스(MIT)
- `LICENSE`: MIT 라이선스 파일
- `config/default.toml`: 기본 설정 스키마 (폰트, 색상, 키바인딩, 셸 경로)

**산출물**: `cargo build` 성공 (빈 크레이트), CI 그린, 의존성 호환 확인

---

## Phase 1: 기본 터미널 (14~18주)

### Milestone 1.1: PTY 레이어 (Week 1~3)

| 작업 | 크레이트 | 설명 | 예상 |
|------|---------|------|------|
| 1.1.1 | `zm-pty` | `portable-pty` 래핑: PTY 생성, 읽기/쓰기, 리사이즈 | 3일 |
| 1.1.2 | `zm-pty` | 프로세스 spawn: 셸 시작, 환경변수, CWD, 기본 셸 자동 감지 | 3일 |
| 1.1.3 | `zm-pty` | 프로세스 생명주기: 종료 감지, 시그널(macOS)/TerminateProcess(Win) | 4일 |
| 1.1.4 | `zm-app` | 최소 CLI: PTY → stdin/stdout 파이프 → 셸 실행 (raw mode) | 2일 |
| 1.1.5 | `zm-pty` | 단위 테스트: spawn/kill/resize, 플랫폼별 동작 검증 | 2일 |

**검증**: `cargo run`으로 bash/zsh/PowerShell 실행, 명령어 입출력 동작 (Win + Mac)

### Milestone 1.2: VT 에뮬레이션 (Week 4~6)

| 작업 | 크레이트 | 설명 | 예상 |
|------|---------|------|------|
| 1.2.1 | `zm-term` | `alacritty_terminal` 통합: Term 인스턴스 생성, PTY 출력 피딩 | 4일 |
| 1.2.2 | `zm-term` | 터미널 그리드: 셀 속성(색상, 볼드, 이탤릭), 커서, **alternate screen buffer** | 4일 |
| 1.2.3 | `zm-term` | 스크롤백 버퍼: 링 버퍼, **키보드/마우스 스크롤 네비게이션** | 3일 |
| 1.2.4 | `zm-term` | 입력 처리: 키보드 → VT 시퀀스 변환, **bracketed paste mode** | 3일 |
| 1.2.5 | `zm-term` | **마우스 이벤트**: 클릭, 드래그 선택, 스크롤 휠, 마우스 트래킹 모드 | 3일 |
| 1.2.6 | `zm-term` | 단위 테스트: ANSI 시퀀스 파싱, 색상, alternate screen 전환 | 2일 |

**검증**: `vim`, `htop`, `less` 정상 동작 (alternate screen), Claude Code UI 정상 표시

### Milestone 1.3: CPU 렌더링 우선 → GPU 전환 (Week 7~14)

> **전략 변경**: CPU 렌더링(softbuffer)으로 먼저 동작 확인 → GPU(wgpu) 최적화. 위험 감소.

| 작업 | 크레이트 | 설명 | 예상 |
|------|---------|------|------|
| 1.3.1 | `zm-render` | `winit` 윈도우 생성, **DPI/HiDPI 스케일링** (Retina Mac, 4K Win) | 3일 |
| 1.3.2 | `zm-render` | `cosmic-text` 폰트 시스템: 모노스페이스 로드, 셰이핑, **폰트 폴백** | 5일 |
| 1.3.3 | `zm-render` | **`softbuffer` + `tiny-skia` CPU 렌더러**: 텍스트 → 비트맵 → 화면 | 7일 |
| 1.3.4 | `zm-render` | 배경/전경색: 24-bit true color, 16색, 256색, **CJK/이모지 기본** | 5일 |
| 1.3.5 | `zm-render` | 커서 렌더링: 블록, 빔, 언더라인 + **깜빡임 애니메이션** | 2일 |
| 1.3.6 | `zm-render` | **선택 + 클립보드**: 마우스 드래그 선택, 복사(Ctrl+C), **붙여넣기(Ctrl+V)**, 플랫폼별 클립보드 API | 5일 |
| 1.3.7 | `zm-render` | **윈도우 크롬**: 타이틀 바 (세션명/브랜치), **벨(BEL) 알림** | 3일 |
| 1.3.8 | `zm-render` | 리사이즈: 윈도우 크기 변경 → PTY 리사이즈 → 그리드 재계산 | 3일 |
| 1.3.9 | `zm-render` | **`glyphon` + `wgpu` GPU 렌더러**: 글리프 아틀라스, 셰이더, GPU 가속 | 10일 |
| 1.3.10 | `zm-render` | GPU/CPU 자동 전환: wgpu 실패 시 softbuffer 폴백 | 2일 |
| 1.3.11 | — | **검증**: GPU+CPU 양쪽에서 Claude Code/vim/htop 정상, **FPS 벤치마크** | 3일 |

**검증 기준**:
- GPU: 60fps 이상 (4-pane split, `wgpu::PresentMode` 기준)
- CPU: 30fps 이상 (폴백 최소 기준)
- Key-to-screen 레이턴시: <10ms
- 메모리: 단일 pane <100MB

### Milestone 1.4: Split Pane + Tab (Week 15~18)

| 작업 | 크레이트 | 설명 | 예상 |
|------|---------|------|------|
| 1.4.1 | `zm-mux` | 패인 트리: 바이너리 트리 수평/수직 분할, 비율 관리, **최소 크기 제한** | 5일 |
| 1.4.2 | `zm-mux` | 탭 관리: 생성/삭제/전환, 탭별 패인 트리 | 3일 |
| 1.4.3 | `zm-mux` | 패인 포커스: 키보드(Alt+Arrow)/마우스 클릭 전환, 포커스 하이라이트 | 3일 |
| 1.4.4 | `zm-mux` | 패인 리사이즈: 테두리 드래그, PTY 리사이즈 연동 | 3일 |
| 1.4.5 | `zm-render` | 패인 테두리 + 탭 바 렌더링, **포커스 인디케이터** | 3일 |
| 1.4.6 | `zm-app` | 키바인딩: 분할(Ctrl+Shift+D/E), 탭(Ctrl+T), 닫기(Ctrl+W), 전환 | 3일 |
| 1.4.7 | `zm-mux` | 통합 테스트: 4개 패인 분할, 리사이즈, 포커스 전환 | 2일 |

**Phase 1 산출물**: Win+Mac에서 Claude Code 실행 가능한 GPU 가속 터미널 (split pane + tab + alternate screen + clipboard)

---

## Phase 2: AI 에이전트 통합 (10~14주)

### Milestone 2.1: tmux 프로토콜 호환 (Week 19~24)

| 작업 | 크레이트 | 설명 | 예상 |
|------|---------|------|------|
| 2.1.1 | `zm-agent` | psmux 소스 분석: Claude Code가 사용하는 tmux 명령어 서브셋 식별 (~20개 명령) | 4일 |
| 2.1.2 | `zm-agent` | TMUX 환경변수: `TMUX=zm-mux-session` 자동 주입 | 2일 |
| 2.1.3 | `zm-agent` | tmux CLI shim 구현: `split-window`, `send-keys`, `select-pane`, `list-panes`, `kill-pane`, `capture-pane`, `display-message`, `has-session`, `new-session`, `resize-pane` | 14일 |
| 2.1.4 | `zm-agent` | **Windows isTTY 우회**: psmux 패턴 (TMUX env + CLI 래퍼), **경로 포맷 변환** (Unix↔Windows, #42848), **ConPTY 플래그** (PSEUDOCONSOLE_RESIZE_QUIRK) | 7일 |
| 2.1.5 | `zm-agent` | tmux shim 통합 테스트: psmux 테스트 케이스 포팅 | 3일 |
| 2.1.6 | — | **검증**: Claude Code `teammateMode: "tmux"` → 에이전트 팀 split-pane 동작 (Win+Mac) | 5일 |

**검증 시나리오**:
1. `claude` 실행 → 에이전트 팀 생성 지시 → split-pane 자동 생성 확인
2. 팀원 에이전트에 작업 할당 → 명령 전달 확인 (`send-keys`)
3. 팀원 종료 → pane 정리 확인 (`kill-pane`)

### Milestone 2.2: Socket API (Week 25~27)

| 작업 | 크레이트 | 설명 | 예상 |
|------|---------|------|------|
| 2.2.1 | `zm-socket` | **macOS**: Unix domain socket 서버 (`tokio::net::UnixListener`) | 3일 |
| 2.2.2 | `zm-socket` | **Windows**: AF_UNIX 소켓 (Win10+) + named pipe 폴백 (`tokio::net::windows`) | 4일 |
| 2.2.3 | `zm-socket` | JSON 프로토콜 스키마: `split`, `list-agents`, `send-keys`, `get-status`, `kill`, **에러 응답**, **프로토콜 버전** | 5일 |
| 2.2.4 | `zm-socket` | CLI 클라이언트: `zm-mux split`, `zm-mux send`, `zm-mux list` | 3일|
| 2.2.5 | `zm-socket` | 통합 테스트: 소켓 연결, 명령 전송, 응답 검증 | 2일 |

### Milestone 2.3: 터미널 기능 완성 (Week 28~30)

| 작업 | 크레이트 | 설명 | 예상 |
|------|---------|------|------|
| 2.3.1 | `zm-term` | **Shift+Enter**: Kitty 키보드 프로토콜 / CSI u, **ConPTY WIN32_INPUT_MODE 처리** | 5일 |
| 2.3.2 | `zm-term` | **OSC 알림**: OSC 9/99/777 파싱 → `notify-rust` 데스크톱 알림 + **OSC 8 하이퍼링크** | 4일 |
| 2.3.3 | `zm-term` | **IME 지원**: 한국어/일본어/중국어 입력 (`winit` IME 이벤트 처리) | 4일 |
| 2.3.4 | `zm-app` | **TOML 설정**: 폰트/색상/키바인딩/셸 경로, `config/default.toml` 파싱 | 3일 |
| 2.3.5 | `zm-term` | **스크롤백 검색**: Ctrl+Shift+F, regex 지원, 하이라이트 | 4일 |

### Milestone 2.4: 에이전트 관리 (Week 31~32)

| 작업 | 크레이트 | 설명 | 예상 |
|------|---------|------|------|
| 2.4.1 | `zm-agent` | 에이전트 감지: **ZM_MUX_AGENT_TYPE env var 우선** → 프로세스명 폴백 → 설정 기반 | 3일 |
| 2.4.2 | `zm-agent` | 상태 알림: 패인 테두리 색상 (대기=파랑, 작업=초록, 완료=회색, 에러=빨강) | 3일 |
| 2.4.3 | `zm-mux` | 자동 리밸런싱: 패인 생성/삭제 시 레이아웃 재계산 | 2일|
| 2.4.4 | `zm-agent` | 환경변수 주입: ZM_MUX_WORKSPACE_ID, ZM_MUX_SURFACE_ID, ZM_MUX_SOCKET_PATH | 1일|

**Phase 2 산출물**: Claude Code 에이전트 팀 + Socket API + Shift+Enter + 알림 + IME + 검색

---

## Phase 3: 멀티 에이전트 & 품질 (10~14주)

### Milestone 3.1: 폰트 렌더링 고급 (Week 33~36)

| 작업 | 크레이트 | 설명 | 예상 |
|------|---------|------|------|
| 3.1.1 | `zm-render` | 리거처: cosmic-text HarfBuzz (Fira Code, JetBrains Mono 등) | 5일 |
| 3.1.2 | `zm-render` | 컬러 이모지: SBIX(macOS) / COLR(Windows) | 5일 |
| 3.1.3 | `zm-render` | 볼드/이탤릭/언더라인/스트라이크스루 조합 정확도 | 3일 |
| 3.1.4 | `zm-render` | 폰트 폴백 체인: 1차 → 시스템 폰트 탐색 (font-kit) | 3일 |
| 3.1.5 | `zm-render` | **서브픽셀 렌더링**: ClearType(Win), LCD(Mac) | 4일 |

### Milestone 3.2: 멀티 에이전트 실행 (Week 37~40)

| 작업 | 크레이트 | 설명 | 예상 |
|------|---------|------|------|
| 3.2.1 | `zm-agent` | 멀티 에이전트 런처: Claude+Codex+Gemini 동시 spawn, 에이전트별 환경변수 | 3일 |
| 3.2.2 | `zm-agent` | Git worktree 격리: `git2` 생성/정리, **크래시 시 정리 로직**, **Windows 심링크 폴백** | 7일 |
| 3.2.3 | `zm-mux` | Vertical tabs: 사이드바, 에이전트 정보(이름, 상태, 브랜치), **git2 브랜치 감지** | 8일 |
| 3.2.4 | — | 검증: 3개 에이전트 동시 실행, 각 독립 worktree, 알림 동작 | 3일|

### Milestone 3.3: MCP + CustomPaneBackend + 세션 (Week 41~46)

| 작업 | 크레이트 | 설명 | 예상 |
|------|---------|------|------|
| 3.3.1 | `zm-socket` | `rmcp` MCP 서버: peer-discover, send-message, get-status, list-agents (4개 도구) | 5일 |
| 3.3.2 | `zm-socket` | **CustomPaneBackend JSON-RPC**: initialize, spawn_agent, write, capture, kill, list, context_exited — **파라미터 검증, 에러 처리, 이벤트 루프 통합** | 12일 |
| 3.3.3 | `zm-mux` | 세션 지속성: 레이아웃+스크롤백 직렬화/복원 (serde) | 5일 |
| 3.3.4 | `zm-app` | 세션 복원 CLI: `zm-mux restore`, `zm-mux sessions` | 2일|
| 3.3.5 | — | 검증: CustomPaneBackend로 Claude Code 에이전트 팀 동작 (tmux shim 없이) | 5일|

### Milestone 3.4: 패키징 & 문서 (Week 47~48)

| 작업 | 설명 | 예상 |
|------|------|------|
| 3.4.1 | macOS: `.app` 번들 + Homebrew formula (`brew install zm-mux`) | 3일 |
| 3.4.2 | Windows: `.exe` 인스톨러 + winget manifest | 3일 |
| 3.4.3 | 사용자 문서: 설치, 설정, 키바인딩, 에이전트 팀 가이드 | 3일 |
| 3.4.4 | 보안 검토: 프로세스 격리, 시크릿 노출, 입력 검증 점검 | 2일 |

**Phase 3 산출물**: 멀티 에이전트 + MCP + CustomPaneBackend + 세션 + 배포 패키지

---

## Phase 4: Post-1.0 (우선순위순)

| 우선순위 | 기능 | 예상 | 의존성 |
|---------|------|------|--------|
| P1 | 에이전트 간 메시징 (`/ask` 패턴) | 3~5주 | Socket API (Phase 2.2) |
| P1 | Review Chain 자동화 (구현→리뷰→수정) | 4~8주 | 에이전트 감지 + 메시징 |
| P2 | 크로스 모델 MCP 브릿지 (pal-mcp-server) | 1~2주 | MCP 서버 (Phase 3.3) |
| P2 | SSH 원격 세션 (`russh`) | 4~8주 | PTY (Phase 1.1) |
| P3 | Lua 스크립팅 (`mlua`) | 4~8주 | 코어 API 안정 후 |
| P3 | 내장 브라우저 (`wry`) | 8~16주 | 렌더러 안정 후 |
| P4 | 플러그인 시스템 (WASM/Lua) | 8~16주 | Lua 스크립팅 후 |

---

## 타임라인 요약

| Phase | 기간 | 산출물 | 가치 |
|-------|------|--------|------|
| **0** | 1~2주 | Cargo workspace + CI + 의존성 검증 | 개발 인프라 |
| **1** | 14~18주 | GPU/CPU 터미널 (split pane + tab + clipboard + alt screen) | 기본 터미널 |
| **2** | 10~14주 | **MVP** — Claude Code 에이전트 팀 + Socket API + IME + 검색 | **시장 공백 해결** |
| **3** | 10~14주 | 멀티 에이전트 + MCP + CustomPaneBackend + 패키징 | 경쟁력 + 배포 |
| **4** | 무기한 | 메시징, SSH, Lua, 브라우저 | 생태계 확장 |

**MVP (Phase 0~2)**: 약 7~9개월 (1~2인 풀타임)
**1.0 (Phase 0~3)**: 약 10~13개월

---

## 리스크 관리

| 리스크 | 등급 | 완화 전략 | 트리거 |
|--------|------|----------|--------|
| WGPU 텍스트 렌더링 막힘 | MEDIUM | **CPU 렌더러(softbuffer) 먼저 구현** → GPU는 점진 전환 (1.3 전략) | Phase 1.3.9에서 2주 이상 지연 시 |
| `alacritty_terminal` crates.io 미공개 | LOW-MEDIUM | git 의존성 사용 또는 `vte` + 자체 상태관리로 전환 | Phase 0.2에서 확인 |
| Windows isTTY 우회 실패 | HIGH-MEDIUM | psmux 소스 포크 + Anthropic 이슈 리포트 | Phase 2.1.4 테스트 실패 |
| tmux 프로토콜 변경 | MEDIUM | CustomPaneBackend 구현 가속 (Phase 3.3) | Claude Code 업데이트 후 호환 깨짐 |
| `portable-pty` ARM 비호환 | LOW | psmux 포크(`portable-pty-psmux`) 사용 | Phase 0.2 빌드 실패 |

---

## 테스트 전략

### 단위 테스트 (각 크레이트)
- `zm-pty`: spawn, kill, resize, 환경변수 주입
- `zm-term`: VT 시퀀스 파싱, 색상, alternate screen, bracketed paste
- `zm-render`: 색상 변환, 셀→픽셀 좌표 계산, DPI 스케일링
- `zm-mux`: 패인 트리 분할/머지, 탭 관리, 리밸런싱
- `zm-agent`: tmux 명령 파싱, 환경변수 생성, 에이전트 감지
- `zm-socket`: JSON 프로토콜 파싱, 소켓 연결/응답

### 통합 테스트
- PTY + VT: 셸 실행 → 명령 입력 → 출력 파싱 → 색상 검증
- 렌더링: 스크린샷 비교 (reference image vs actual)
- tmux 호환: Claude Code 에이전트 팀 E2E 시나리오

### CI 매트릭스
- Windows 11 x64, macOS ARM (M-series)
- Rust stable + nightly
- GPU 테스트: CI에서 softbuffer(CPU) 모드로 실행

### 성능 벤치마크 (Phase 1.3.11)
- FPS: `cargo bench` + 프레임 타이밍 측정 (target: GPU 60fps, CPU 30fps)
- 레이턴시: 키 입력 → 화면 표시 시간 (target: <10ms)
- 메모리: 단일 pane <100MB, 4-pane <300MB
- 스크롤백: 10만줄 히스토리에서 검색 <100ms

---

## 검증 체크리스트 (Phase별)

### Phase 0
- [ ] `cargo build` 성공 (Win x64 + Mac ARM)
- [ ] `alacritty_terminal` 크레이트 가용성 확인됨
- [ ] `glyphon` ↔ `wgpu` ↔ `cosmic-text` 버전 호환 확인됨
- [ ] CI 그린 (clippy + test + fmt)
- [ ] LICENSE (MIT) 파일 존재

### Phase 1
- [ ] `claude`, `vim`, `htop`, `less` 정상 동작 (Win+Mac)
- [ ] alternate screen buffer 전환 정상 (vim → shell → vim)
- [ ] 4개 패인 split + 독립 셸 + 리사이즈 정상
- [ ] 클립보드 복사/붙여넣기 동작 (Win+Mac)
- [ ] GPU: 60fps, CPU: 30fps (4-pane 기준)
- [ ] DPI 스케일링 정상 (Retina Mac, 4K Windows)

### Phase 2
- [ ] Claude Code `teammateMode: "tmux"` → 에이전트 팀 split-pane 자동 생성 (Win+Mac)
- [ ] Shift+Enter 멀티라인 입력 동작
- [ ] OSC 알림 → 데스크톱 알림 표시
- [ ] IME 한국어 입력 동작
- [ ] Socket API: 외부 스크립트에서 패인 생성/명령 전송
- [ ] 스크롤백 검색 동작 (Ctrl+Shift+F)

### Phase 3
- [ ] Claude + Codex + Gemini 3개 동시 실행, 각 독립 worktree
- [ ] CustomPaneBackend로 Claude Code 에이전트 팀 동작 (tmux shim 없이)
- [ ] MCP 서버로 에이전트 peer discovery
- [ ] 앱 재시작 후 레이아웃+스크롤백 복원
- [ ] Homebrew/winget 설치 동작

---

*작성일: 2026-04-30*
*검토: 4차 (22건 이슈 반영)*
