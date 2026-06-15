# 04 — AI 에이전트 연동 (Claude Code 외)

> zm-mux의 가장 큰 *비기술* 리스크는 Claude Code 분할패널 연동의 Windows 차단(Anthropic 정책)이다.
> 이슈 상태는 GitHub API 2026-06-15 라이브 확인 [V]. 범례 [00](00-overview.md).

## 1. Claude Code agent teams — 공식 사양 (code.claude.com/docs/en/agent-teams)

| 항목 | 내용 | 출처 |
|------|------|------|
| 활성화 | 실험적, 기본 off. `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` | 공식 docs [V] |
| 최소 버전 | Claude Code **v2.1.32+** | 공식 docs [V] |
| 디스플레이 모드 | **in-process**(아무 터미널) vs **split-pane** | 공식 docs [V] |
| split-pane 요건 | **tmux 또는 iTerm2(+`it2` CLI)** | 공식 docs [V] |
| `teammateMode` | `auto`(tmux/iTerm2 안이면 split, 아니면 in-process) / `tmux` / `in-process` | 공식 docs [V] |
| **split 미지원** | **VS Code 통합 터미널, Windows Terminal, Ghostty** | 공식 docs Limitations [V] |
| 저장 | team config `~/.claude/teams/{name}/config.json`, **task list `~/.claude/tasks/{name}/`** | 공식 docs [V] |
| 메시징 | Mailbox(자동 전달), 공유 task list(file lock claim) | 공식 docs [V] |
| 훅 | `TeammateIdle`, `TaskCreated`, `TaskCompleted` (exit 2로 피드백) | 공식 docs [V] |

> 공식 docs는 **isTTY를 언급하지 않는다**. "split은 tmux/iTerm2 필요 + Windows Terminal 등 미지원"까지만.
> Windows에서 tmux/psmux가 있어도 막히는 *메커니즘*은 이슈 #26244에 있다(아래).

## 2. 핵심 차단 메커니즘 — isTTY 게이트 (#26244)

- 상태: **closed / not_planned** (GitHub API [V]).
- 내용: Windows에서 `process.stdout.isTTY`가 `undefined` → Claude Code가 in-process로 강제 폴백,
  `teammateMode:"tmux"`를 명시해도 무시. 즉 **Windows split-pane 사실상 봉쇄**. [V issue]
- 함의: zm-mux가 완벽한 tmux 호환을 제공해도, Claude Code가 *Windows에서 분할을 시도조차 안 할* 수 있음.

## 3. 관련 이슈 라이브 상태 (GitHub API 2026-06-15)

| # | 제목(요약) | 상태 | 의미 |
|---|------|------|------|
| #26572 | CustomPaneBackend 프로토콜 제안 | **open** | tmux 의존 분리 JSON-RPC. zm-mux의 *정공법* 트랙 |
| #26244 | isTTY 게이트가 Windows split 차단 | closed/not_planned | 근본 차단, 미수정 |
| #34150 | Windows에서 psmux로 tmux 팀 지원 | closed/not_planned | psmux 미채택 |
| #36926 | cmux를 teammateMode 백엔드로 | **open** | macOS 네이티브 연동 제안(진행 중) |
| #27868 | Kitty 키보드 감지(KITTY_WINDOW_ID 무시) | closed/duplicate | Shift+Enter 감지 버그(§6) |

## 4. cmux 선례 (소스 확정, [01](01-cmux-analysis.md) §6)

- cmux는 **tmux-shim**으로 우회: Claude Code가 "tmux 안"이라 믿게 만들고(가짜 `TMUX`/`TMUX_PANE`),
  tmux 명령을 소켓 API로 변환. macOS에선 isTTY 문제 없음 → 동작.
- env: `CMUX_WORKSPACE_ID/SURFACE_ID/SOCKET_PATH/SOCKET_MODE` + `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` 주입.
- 추가로 cmux를 *정식* teammateMode로 넣자는 #36926가 open(아직 미채택).

## 5. zm-mux 전략 (Windows 연동) — 3 트랙

| 트랙 | 방법 | 장점 | 리스크 |
|------|------|------|--------|
| **A. tmux-shim + env 우회** | psmux/cmux식. zm-mux가 tmux 호환 소켓 제공 + 가짜 `TMUX`/`TMUX_PANE` + **PTY로 자식에 진짜 TTY 부여**해 isTTY=true 만들기 | 즉시·Anthropic 비의존 | isTTY 게이트가 env까지 무시하면 한계([?] 실측 필요) |
| **B. CustomPaneBackend (#26572)** | `CLAUDE_PANE_BACKEND`(바이너리/소켓) + JSON-RPC: `initialize/spawn_agent/write/capture/kill/list` + 푸시 `context_exited/context_output` | 정공법·tmux 비의존·크로스플랫폼 | **제안 open, 미채택** → 사양 변동/미구현 리스크 |
| **C. in-process 수용** | split 포기, in-process 모드 + zm-mux는 일반 멀티플렉서로 가치 | 항상 동작 | cmux식 "각 에이전트=패널" UX 미달 |

> **권장**: A를 1차(자체 PTY로 isTTY 충족 가능성 실측 — zm-mux는 ConPTY로 진짜 PTY 부여하므로
> 브라우저/wmux의 한계와 다를 수 있음 [?]) + B를 병행 추적(#26572 채택 시 정식 백엔드). C는 폴백.
> **핵심 미해결 [?]**: "zm-mux ConPTY 자식에서 Claude Code의 isTTY가 true가 되는가"는 구현 PoC에서 실측 필요.

## 6. 터미널 기능 — 에이전트가 기대하는 것

- **Shift+Enter**: Kitty 키보드 프로토콜(CSI-u) 필요. Claude Code가 `TERM_PROGRAM`을 `KITTY_WINDOW_ID`보다
  먼저 검사 → 커스텀 터미널에서 미활성(#27868, closed/duplicate). zm-mux는 CSI-u 지원 + 감지에 잡히는
  `TERM`/`KITTY_WINDOW_ID` 설정 전략 필요. [V issue]
- **데스크톱 알림**: OSC **9**(iTerm2/ConEmu/Windows Terminal), OSC **99**(Kitty), OSC **777**(VTE). zm-mux는
  이들 파싱 + Windows 토스트/macOS Notification 송출. [V Phase1]
- **유니코드/컬러/리거처**: cosmic-text 셰이핑으로 커버([02](02-windows-no-wsl-feasibility.md)).

## 7. 기타 에이전트

| 에이전트 | 멀티패널 | 비고 (출처 1차 조사 [I], 별도 재검증 권장 [?]) |
|----------|----------|------|
| Claude Code | agent teams(§1) | 본 문서 핵심 |
| Codex CLI | 내장 팀 없음 | cmux가 훅 지원(`CMUX_CODEX_*`, CodexTeamsApprovalBridge.swift [V 소스]) |
| Gemini CLI | 서브에이전트 O, 팀 X | tmux/zellij로 수동 오케스트레이션 |

> Codex/Gemini 세부는 1차 조사 기반 [I] → 구현 착수 전 공식 docs 재검증 권장 [?].

## 8. 요약

- ②③(크로스플랫폼·WSL 없음)은 순수 기술로 해결됨([02](02-windows-no-wsl-feasibility.md)).
- ①의 "cmux 동등 *에이전트 연동*"은 **Anthropic isTTY 정책**이 진짜 관문. zm-mux는 A(shim+진짜 PTY)
  우선·B(#26572) 병행·C(in-process) 폴백으로 설계하고, **A의 isTTY 충족 여부를 PoC에서 최우선 실측**한다.
