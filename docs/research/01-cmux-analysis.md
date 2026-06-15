# 01 — cmux 정밀 분석 (소스 기반)

> 분석 대상: `reference/cmux` @ `48c9160` (github.com/manaflow-ai/cmux, 클론 2026-06-15).
> 범례 [V]/[I]/[?]는 [00](00-overview.md) 참조. 본 문서의 "소스" 인용은 *동작 관찰*이며, clean-room
> 규칙상 구현 방법은 zm-mux에서 독립 도출한다([05](05-reference-inventory.md)).

## 1. 정체 / 라이선스

| 항목 | 값 | 출처 |
|------|-----|------|
| repo | github.com/manaflow-ai/cmux | [V] |
| 저작권 | Manaflow, Inc. (2024–present) | cmux/LICENSE [V] |
| 라이선스 | **dual: GPL-3.0-or-later** (오픈소스) **OR Commercial** (founders@manaflow.com) | cmux/LICENSE [V] |
| 주언어 | Swift | GitHub API + 소스 [V] |
| 활성도 | 활발 (pushed 2026-06-15), README 20+개 언어 | GitHub API [V] |
| stars | ~21.7k 규모 | 1차 조사 [I] |

> **clean-room 함의**: GPL-3.0-or-later. 코드/구조 차용 시 zm-mux도 GPL 의무 발생 → zm-mux는
> **이해용 분석만** 하고 MIT/Apache 스택으로 독립 구현한다(사용자 승인). 상업 라이선스는 별도 계약 사안.

## 2. 기술 스택 (top-level 소스 직독)

| 레이어 | 실체 (cmux/) | 비고 |
|--------|--------------|------|
| 빌드/패키징 | `cmux.xcodeproj`, `cmux.xcworkspace`, `*.entitlements`(release/nightly/helper), `cmux-Bridging-Header.h` | **Xcode 전용** → macOS 빌드 환경 결박 [V] |
| 앱/UI | `Sources/`, `Native/`, `Packages/` (Swift) | AppKit 기반 macOS 앱 [V 소스 + I AppKit] |
| 터미널 렌더 | `ghostty` (서브모듈 → manaflow-ai/ghostty 포크), `ghostty.h` | **libghostty = Metal GPU 렌더** [V 서브모듈 + I Metal] |
| 레이아웃 | `vendor/bonsplit` (서브모듈 → manaflow-ai/bonsplit) | 분할 레이아웃 라이브러리 "Bonsplit" **실재 확정** [V] |
| 웹/브라우저 | `web/`, `webviews/`, `workers/`, `package.json`+`bun.lock`+`biome.json` | **WebKit 임베디드 브라우저** + bun 툴체인 [V 소스] |
| CLI | `CLI/` (Swift, `cmux.swift` + `CMUXCLI+*.swift` 다수) | 소켓 클라이언트 + 에이전트 훅 [V] |
| 데몬 | `daemon/remote` | 원격 데몬(remote-daemon-spec.md) [V] |
| 모바일 | `ios/` | iOS 변종 동시 존재 [V] |
| 기타 | `skills/`, `docs/`, `homebrew-cmux`(서브모듈), `THIRD_PARTY_LICENSES.md` | |

`.gitmodules` [V]:
```
ghostty        -> https://github.com/manaflow-ai/ghostty.git (branch main)
vendor/bonsplit-> https://github.com/manaflow-ai/bonsplit.git
homebrew-cmux  -> https://github.com/manaflow-ai/homebrew-cmux.git
```

## 3. macOS 전용 차단요인 (왜 그대로 포팅 불가인가)

| # | 차단요인 | 근거 | Windows 대체 (→[03](03-crossplatform-architecture.md)) |
|---|----------|------|----------|
| 1 | **AppKit UI** | Swift `Sources/` + 앱 구조 | winit + wgpu 자체 UI |
| 2 | **Metal 렌더 (libghostty)** | `ghostty` 서브모듈, `ghostty.h` | wgpu(DX12/Vulkan/Metal) + glyphon |
| 3 | **Xcode 빌드/entitlements/bridging** | `*.xcodeproj`/`*.entitlements`/bridging header | cargo + 표준 Rust 빌드 |
| 4 | **Unix domain socket** `/tmp/cmux*.sock` | CLI 소스 토큰 `cmux.sock`/`/tmp/cmux` | `interprocess`(Windows=named pipe) |
| 5 | **macOS 알림센터** | `docs/notifications.md` | Windows 토스트 / OSC 9·99·777 |
| 6 | **Swift 언어 + Apple 프레임워크** | 전 소스 | Rust |
| 7 | 자동 업데이트(Sparkle 추정) | 1차 조사 | electron-updater 무관, 별도 채택 |

> #7 Sparkle은 본 재검증에서 소스로 직접 확인하지 않음 **[I/?]**. 나머지 1–6은 소스 확정 **[V]**
> (단 AppKit/Metal은 "Swift+ghostty"라는 소스 사실로부터의 **[I]**).
> 결론: cmux는 **부분 포팅 불가** — 동등 *기능*을 다른 스택으로 재구현해야 한다.

## 4. 기능 인벤토리 (cmux/docs + CLI 소스 기반)

> `cmux/docs/`에 설계 문서가 다수 존재 → 기능을 *공식 소스*로 확인 [V].

- **워크스페이스/탭/분할**: Bonsplit 레이아웃, `workspace-groups.md`, `canvas-layout-design.md`, 사이드바(`custom-sidebars.md`, `data-driven-sidebar-plan.md`).
- **소켓 자동화 API**: §5. (`cli-contract.md`, `AutomationSocketUITests.swift`)
- **알림**: `notifications.md` (데스크톱 + OSC). 패널별 알림 링/배지(1차 조사 [I]).
- **임베디드 브라우저**: `agent-browser-port-spec.md` + `webviews/` (Playwright급 자동화, Vercel agent-browser 포팅 — 1차 조사 [I]).
- **에이전트 훅/감지**: `agent-hooks.md`, `CMUXCLI+AgentHookDefinitions.swift`. Claude/Codex/Amp/Antigravity/CodeBuddy/Hermes 등 다수 [V 파일명].
- **워크스페이스 자동 네이밍**: `CMUXCLI+AutoNaming*.swift` (요약 기반).
- **원격 데몬/프레즌스**: `remote-daemon-spec.md`, `presence-service.md`, `daemon/remote`.
- **이벤트/피드**: `events.md`, `feed.md`, `FeedEventClassifier.swift`.
- **테마/설정**: `configuration.md`, ghostty config 재사용(1차 조사 [I]), `CMUXCLI+Themes.swift`.
- **vault / dock / state engine**: `vault.md`, `dock.md`, `state-engine-design.md`.

## 5. 소켓 자동화 API (소스 토큰 확정)

- 전송: Unix domain socket (`/tmp/cmux*.sock`) [V].
- tmux 호환 명령 토큰 (CLI/Sources 소스 grep, **블로그 아닌 소스 확정** [V]):
  `new-session`, `new-window`, `split-window`, `send-keys`, `send-key`,
  `read-screen`, `capture-pane`, `list-panes`, `select-pane`, `kill-pane`, `notify`.
- 철학: "primitive, not solution" — 저수준 pane/workspace 오퍼레이션 제공, 상위 오케스트레이션은 에이전트가 구성(1차 조사 [I]).

## 6. Claude Code 연동 = tmux shim (소스 확정)

- 메커니즘: cmux가 **tmux 호환 셰임**을 만들어 Claude Code가 "tmux 안"이라 인식하게 함. 셰임이
  tmux 명령(split-window/send-keys/capture-pane/…)을 cmux 소켓 API로 변환.
  - 소스 근거 [V]: `CLI/CMUXCLI+TmuxCompatSupport.swift`, `CMUXCLI+TmuxCompatHUDSupport.swift`,
    env `CMUX_CLAUDE_WRAPPER_SHIM`, `CMUX_CLAUDE_TEAMS_CMUX_BIN`, `CMUX_CLAUDE_TEAMS_TERM`, `CMUX_CLAUDE_PID`.
- 주입 env (공식 docs + 소스):
  - 가짜 `TMUX`, `TMUX_PANE` (tmux 위장) [V 공식/1차]
  - `CMUX_WORKSPACE_ID`, `CMUX_SURFACE_ID`, `CMUX_SOCKET_PATH`, `CMUX_SOCKET_MODE` [V 공식 docs]
  - `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` (에이전트 팀 활성) [V 공식 Claude Code docs]
- 소스엔 **40+개 `CMUX_*` env**가 존재(grep [V]): 예) `CMUX_BIN`, `CMUX_AGENT_SESSION_ID`,
  `CMUX_AGENT_LAUNCH_{EXECUTABLE,CWD,ARGV_B,KIND}`, `CMUX_ALLOW_SOCKET_OVERRIDE`, `CMUX_CODEX_SESSION_ID` 등.

> 이 tmux-shim 패턴이 **zm-mux의 Windows 연동 핵심 차용 아이디어**(코드가 아닌 *접근*)다.
> Anthropic isTTY 게이트와의 관계는 [04](04-ai-agent-integration.md).

## 7. zm-mux 관점 요약

cmux의 *가치 명제*(에이전트 우선 멀티플렉서 + 소켓 자동화 + tmux 호환 + 알림 + 브라우저)는
플랫폼 중립적이다. macOS 결박은 전적으로 **구현 선택(Swift/AppKit/Metal/Xcode)** 때문이며,
네이티브 Rust로 동등 기능을 **재구현 가능**하다. 기능→컴포넌트 매핑은 [03](03-crossplatform-architecture.md).
