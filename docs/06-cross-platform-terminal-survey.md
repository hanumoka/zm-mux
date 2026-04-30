# 크로스 플랫폼 터미널 전수 조사 (Windows + Mac)

## 개요

zm-mux의 설계 방향 전환에 따른 전수 조사. Windows 전용에서 **Windows + Mac 크로스 플랫폼**으로 확장.
기존 docs/05는 Windows 중심이었으나, 이 문서는 양 플랫폼을 포괄한다.

---

## 1. 크로스 플랫폼 터미널 비교표 (Windows + Mac 지원)

AI 에이전트 터미널 개발에 중요한 기능 위주로 정리.

| 터미널 | 플랫폼 | 언어 | GPU 가속 | Split Pane | 멀티플렉서 | 알림 | 오픈소스 | 라이선스 | GitHub Stars |
|--------|--------|------|----------|------------|-----------|------|---------|---------|-------------|
| **WezTerm** | Win/Mac/Linux/BSD | Rust | O (OpenGL) | O | O (내장) | O | O | MIT | 19k+ |
| **Warp** | Win/Mac/Linux | Rust | O (Metal/WebGPU) | O | O | O | O | AGPL-3 | 24k+ |
| **Alacritty** | Win/Mac/Linux/BSD | Rust | O (OpenGL) | X | X | X | O | Apache 2.0/MIT | 58k+ |
| **Rio** | Win/Mac/Linux/BSD | Rust | O (WebGPU/WGPU) | O | X | O | O | MIT | 4.5k+ |
| **Tabby** | Win/Mac/Linux | TypeScript | O (Electron) | O | X | O | O | MIT | 61k+ |
| **Ghostty** | Mac/Linux (Win 로드맵) | Zig | O (Metal/OpenGL) | O | X | O | O | MIT | 28k+ |
| **Kitty** | Mac/Linux/BSD | C/Python | O (OpenGL) | O | O (내장) | O | O | GPL-3 | 26k+ |
| **Wave** | Win/Mac/Linux | Go | O | O | O (내장) | X | O | Apache 2.0 | 10k+ |

### Windows 전용

| 터미널 | 언어 | GPU 가속 | Split Pane | 멀티플렉서 | 알림 | 오픈소스 |
|--------|------|----------|------------|-----------|------|---------|
| **Windows Terminal** | C++ | O | O | X | O | O (MIT) |
| **ConEmu** | C++ | X | O | X | X | O (BSD) |
| **Mintty** | C | X | X | X | O | O (GPL-3) |

### Mac 전용

| 터미널 | 언어 | GPU 가속 | Split Pane | 멀티플렉서 | 알림 | 오픈소스 |
|--------|------|----------|------------|-----------|------|---------|
| **iTerm2** | Objective-C | O | O | X (tmux 통합) | O | O (GPL-2) |
| **Terminal.app** | Objective-C | O | O | X | X | X (내장) |
| **cmux** | Swift (libghostty) | O (Metal) | O | O (Socket API) | O | O (AGPL-3) |
| **Calyx** | Swift (libghostty) | O (Metal) | O | X | O | O |

---

## 2. AI 에이전트 터미널 (2026년 신생 카테고리)

### cmux (macOS)
- **핵심**: AI 에이전트 전용 터미널의 원조. libghostty 기반
- **Socket API**: Unix domain socket으로 CLI↔GUI 통신. JSON 포맷
- **에이전트 팀**: Claude Code의 teammateMode 백엔드로 공식 지원 (이슈 #36926)
- **환경변수**: `CMUX_WORKSPACE_ID`, `CMUX_SURFACE_ID`, `CMUX_SOCKET_PATH`
- **알림**: 패널 링, 뱃지, 데스크톱 알림 (OSC 9/99/777)
- **Stars**: 7.7k+

### Calyx (macOS 26+)
- **핵심**: cmux의 경쟁자. libghostty v1.3.1 기반, Liquid Glass UI
- **MCP 서버 내장**: AI 에이전트 IPC를 MCP 프로토콜로 구현 (7개 도구)
- **에이전트 간 통신**: 에이전트 인스턴스 간 peer discovery, 메시지 송수신, 브로드캐스트
- **추가 기능**: Git 사이드바, 커맨드 팔레트, WKWebView 브라우저 (25개 CLI 자동화 명령)

### Warp (Win/Mac/Linux)
- **핵심**: 유일한 크로스 플랫폼 AI 에이전트 터미널
- **Claude Code 통합**: 공식 플러그인 (`warpdotdev/claude-code-warp`)
- **기능**: Vertical tabs, 알림 센터, 코드 리뷰, 멀티 에이전트 관리
- **Block 모델**: 입력/출력을 블록 단위로 관리 (전통적 스크롤백과 다름)
- **렌더링**: Rust + Metal(Mac)/WebGPU, 144+ FPS
- **단점**: AGPL 라이선스, 클로즈드 코어(UI 프레임워크 미공개)

### psmux (Windows)
- **핵심**: Windows 네이티브 tmux. Rust 기반
- **Claude Code 호환**: TMUX 환경변수 설정 → Claude Code가 tmux 백엔드로 인식
- **제한**: 터미널 에뮬레이터가 아닌 멀티플렉서 (기존 터미널 위에서 동작)

---

## 3. Claude Code 터미널 호환성 이슈 (GitHub Issues 기반)

### Shift+Enter 문제
- tmux 내부에서 Shift+Enter가 줄바꿈이 아닌 제출로 작동 ([#26629](https://github.com/anthropics/claude-code/issues/26629))
- Ghostty + tmux에서 `/terminal-setup`이 Shift+Enter 수정 실패 ([#17168](https://github.com/anthropics/claude-code/issues/17168))
- **네이티브 지원 터미널**: WezTerm, Ghostty, Warp
- **설정 필요**: Alacritty (`/terminal-setup`), Kitty
- **미지원**: Windows Terminal (Ctrl+J 대체), VS Code Terminal

### Windows 에이전트 팀 제한
- `teammateMode: "tmux"` 설정이 Windows에서 무시됨 — `process.stdout.isTTY`가 항상 falsy ([#26244](https://github.com/anthropics/claude-code/issues/26244))
- Windows Terminal을 split-pane 백엔드로 추가 요청 ([#24384](https://github.com/anthropics/claude-code/issues/24384))
- psmux를 통한 Windows tmux 지원 요청 ([#34150](https://github.com/anthropics/claude-code/issues/34150))

### tmux 에이전트 팀 버그
- 팀원 spawn 시 `send-keys`가 셸 초기화 전에 전송 → 명령 미실행 ([#40168](https://github.com/anthropics/claude-code/issues/40168))
- `pane-base-index`가 1일 때 팀원 명령 전달 실패 ([#23527](https://github.com/anthropics/claude-code/issues/23527))
- 에이전트 팀이 현재 pane을 split하지 말고 새 tmux window에 spawn 요청 ([#23615](https://github.com/anthropics/claude-code/issues/23615))

### WezTerm teammateMode 요청
- WezTerm을 split-pane 백엔드로 추가 요청 ([#23574](https://github.com/anthropics/claude-code/issues/23574))

---

## 4. 터미널 렌더링 기술 비교

### GPU 가속 접근법

| 접근법 | 사용 터미널 | 장점 | 단점 |
|--------|-----------|------|------|
| **Metal** | Ghostty(Mac), Warp(Mac), cmux, Calyx | macOS 최적 성능, 2ms 레이턴시 | macOS 전용 |
| **OpenGL** | Alacritty, WezTerm, Kitty | 크로스 플랫폼, 검증됨 | Apple이 deprecated 선언 |
| **WebGPU/WGPU** | Rio, Warp(크로스 플랫폼) | 미래 표준, 크로스 플랫폼 | 아직 생태계 초기 |
| **DirectX** | Windows Terminal | Windows 최적 | Windows 전용 |
| **Vulkan** | Ghostty(Linux) | Linux/Windows 고성능 | 복잡한 API |
| **xterm.js (Canvas/WebGL)** | Tabby, Hyper, wmux들 | 쉬운 구현, 웹 기술 | 성능 오버헤드, Electron 의존 |

### 렌더링 성능 비교 (2026 추정치, 복수 벤치마크 기반)

| 터미널 | Key-to-Screen 레이턴시 | 메모리 사용 |
|--------|----------------------|-----------|
| Ghostty | ~2ms | 60-100MB |
| Alacritty | ~3ms | 30-50MB (최소) |
| Kitty | ~3ms | 60-100MB |
| WezTerm | ~4ms | 80-120MB |
| Warp | ~5ms | 150-250MB |
| Tabby (Electron) | ~10-15ms | 300-500MB |

---

## 5. 크로스 플랫폼 PTY 관리

### Windows: ConPTY
- Windows 10 1809+부터 지원
- `CreatePseudoConsole()` API
- 주요 플래그: `PSEUDOCONSOLE_RESIZE_QUIRK`, `PSEUDOCONSOLE_WIN32_INPUT_MODE`, `PSEUDOCONSOLE_PASSTHROUGH_MODE`

### macOS/Linux: POSIX PTY
- `posix_openpt()` / `forkpty()`
- 성숙하고 안정적

### 크로스 플랫폼 Rust 라이브러리

| 라이브러리 | 설명 | 사용처 |
|-----------|------|--------|
| **portable-pty** | WezTerm의 크로스 플랫폼 PTY 라이브러리 | WezTerm |
| **portable-pty-psmux** | portable-pty + ConPTY 플래그 지원 패치 | psmux |
| **pseudoterminal** | 비동기 지원 크로스 플랫폼 PTY | 독립 프로젝트 |
| **winpty-rs** | Windows PTY (WinPTY + ConPTY) | 독립 프로젝트 |

---

## 6. 아키텍처 패턴 분석 (zm-mux 설계 참고)

### WezTerm 아키텍처 (가장 참고 가치 높음)
```
Cargo workspace (19+ crates)
├── wezterm-term     # VTE 호환 터미널 에뮬레이션
├── wezterm-gui      # GUI 프론트엔드
├── wezterm-font     # 폰트 관리
├── pty/             # 크로스 플랫폼 PTY (portable-pty)
│   ├── unix.rs      # POSIX PTY
│   └── win/conpty.rs # ConPTY
├── mux/             # 멀티플렉서 (세션/윈도우/탭/패인 관리)
├── codec/           # RPC 프로토콜
└── mux-server       # 헤드리스 서버 (클라이언트-서버 분리)
```
- **핵심**: 터미널 에뮬레이션 ↔ GUI ↔ 멀티플렉서 분리
- **Mux**: 싱글턴 패턴, MuxWindow → Tab → Pane 트리 구조
- **Domain 추상화**: 로컬/SSH/원격 등 다양한 연결 유형 지원

### cmux Socket API 아키텍처
```
AI Agent (Claude Code)
  ↓ shell command
cmux CLI
  ↓ JSON message
Unix Domain Socket
  ↓ IPC
cmux Main App (GUI)
  → UI 업데이트 (패인 생성/분할/알림)
```
- **원칙**: "Primitive, Not Solution" — 저수준 빌딩 블록 제공
- **지연시간**: Unix domain socket으로 거의 즉시 통신

### Calyx MCP 서버 아키텍처
```
AI Agent (Claude Code)
  ↓ MCP protocol
Calyx MCP Server (내장)
  ↓ IPC
Calyx App
  → 에이전트 간 peer discovery, 메시지, 브로드캐스트
```
- **7개 MCP 도구**: 에이전트 간 통신을 MCP 프로토콜로 표준화
- **장점**: Claude Code MCP 생태계와 네이티브 호환

### Warp 렌더링 아키텍처
```
Custom UI Framework (GPUI-like)
  ├── Rust 코어 렌더링 엔진
  ├── Metal (macOS) / WebGPU (크로스 플랫폼)
  └── Block 모델 (입력/출력 블록 단위 관리)
```
- 자체 UI 프레임워크를 Rust로 구축 (브라우저 아키텍처와 유사)
- 144+ FPS 렌더링

---

## 7. zm-mux 설계 시사점

### 필수 기능 (모든 AI 에이전트 터미널 공통)
1. **Split Pane 관리** — 에이전트별 독립 패인, 자동 리밸런싱
2. **Socket/IPC API** — CLI↔GUI 통신 (cmux: Unix socket, Calyx: MCP)
3. **알림 시스템** — 에이전트 상태 변화 시 시각적 알림 + 데스크톱 알림
4. **환경변수 감지** — 에이전트가 zm-mux 내부 실행을 자동 감지

### 크로스 플랫폼 전략 옵션

| 전략 | 예시 | 장점 | 단점 |
|------|------|------|------|
| **A. Rust + 플랫폼 네이티브 렌더링** | WezTerm | 최고 성능, 네이티브 UX | 플랫폼별 렌더러 필요 |
| **B. Rust + WebGPU(WGPU)** | Rio | 단일 렌더러로 크로스 플랫폼 | WebGPU 아직 초기 |
| **C. Rust + OpenGL** | Alacritty | 검증됨, 안정적 | Apple deprecated |
| **D. libghostty 기반** | cmux, Calyx | 검증된 터미널 엔진 | macOS 전용 (Win 미지원) |
| **E. Electron + xterm.js** | Tabby, wmux들 | 빠른 개발 | 성능 오버헤드, M-001 해당 |

### Claude Code teammateMode 호환 전략

Claude Code는 현재 다음 백엔드를 지원:
1. **tmux** — 공식 지원, TMUX 환경변수로 감지
2. **iTerm2** — macOS 전용
3. **cmux** — 개발 중 (이슈 #36926)

zm-mux의 선택지:
- **Option A**: tmux 프로토콜 호환 (psmux 방식) → 즉시 동작
- **Option B**: 독자 백엔드 → Claude Code PR 필요
- **Option C**: cmux Socket API 호환 → cmux 생태계 활용

### 언어 선택

**Rust가 사실상 표준**: WezTerm, Alacritty, Warp, Rio, psmux 모두 Rust.
- 크로스 플랫폼 PTY: `portable-pty` 크레이트 (WezTerm 작성)
- GPU 렌더링: `wgpu` (WebGPU), `glutin` (OpenGL)
- 비동기: `tokio` 런타임
- GUI: `winit` (윈도우 관리), 또는 자체 프레임워크

---

## 참고 링크

- [Terminal Emulators Comparison Table 2026](https://terminaltrove.com/compare/terminals/)
- [Best Terminal Emulators for Developers 2026](https://scopir.com/posts/best-terminal-emulators-developers-2026/)
- [Warp Claude Code Integration](https://www.warp.dev/agents/claude-code)
- [WezTerm GitHub](https://github.com/wezterm/wezterm)
- [Ghostty GitHub](https://github.com/ghostty-org/ghostty)
- [cmux GitHub](https://github.com/manaflow-ai/cmux)
- [cmux Socket API Docs](https://www.cmux.dev/docs/api)
- [Calyx GitHub](https://github.com/yuuichieguchi/Calyx)
- [psmux GitHub](https://github.com/psmux/psmux)
- [Rio Terminal](https://rioterm.com/)
- [portable-pty Docs](https://docs.rs/portable-pty)
- [Warp Architecture Blog](https://www.warp.dev/blog/how-warp-works)
- [libghostty Announcement](https://mitchellh.com/writing/libghostty-is-coming)
- [Ghostty vs iTerm2 2026](https://tech-insider.org/ghostty-vs-iterm2-2026/)
- [Claude Code Terminal Config](https://code.claude.com/docs/en/terminal-config)
- [Claude Code Agent Teams #26244](https://github.com/anthropics/claude-code/issues/26244)
- [Claude Code WezTerm Request #23574](https://github.com/anthropics/claude-code/issues/23574)
- [Claude Code psmux Request #34150](https://github.com/anthropics/claude-code/issues/34150)

---

*조사일: 2026-04-30*
