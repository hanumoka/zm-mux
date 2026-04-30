# 멀티 AI 에이전트 협업 터미널 생태계 조사

## 개요

2026년 AI 코딩 에이전트 시대에서, 단일 AI가 아닌 **여러 AI (Claude Code + Codex + Gemini CLI 등)를 동시에 협업**시키는 "Agentmaxxing" 패턴이 부상하고 있다. zm-mux가 이를 네이티브로 지원해야 하는 근거와 구현 방안을 조사한다.

---

## 1. 멀티 에이전트 협업 도구 전수 조사

### 1.1 터미널 기반 멀티 에이전트 매니저

| 도구 | 언어 | Stars | 지원 에이전트 | 플랫폼 | 핵심 기능 | 라이선스 |
|------|------|-------|-------------|--------|----------|---------|
| **[Warp](https://www.warp.dev/agents)** | Rust | 24k+ | Claude/Codex/Gemini/Oz/OpenCode | Win/Mac/Linux | Vertical tabs, 알림, 코드 리뷰 | AGPL-3 |
| **[claude-squad](https://github.com/smtg-ai/claude-squad)** | Go | 7.2k | Claude/Codex/Gemini/Aider | Mac/Linux (tmux) | TUI, git worktree 격리, 백그라운드 실행 | — |
| **[claude_code_bridge](https://github.com/bfly123/claude_code_bridge)** (CCB) | Python | 2.4k | Claude/Codex/Gemini/OpenCode/Droid | Mac/Linux/WSL | split-pane, 에이전트 간 메시징 (`/ask`) | — |
| **[parallel-code](https://github.com/johannesjo/parallel-code)** | TypeScript | 591 | Claude/Codex/Gemini/Copilot | Mac/Linux | Electron, git worktree, diff viewer, Docker 샌드박싱 | MIT |
| **[AgentsRoom](https://agentsroom.dev/)** | — | — | Claude/Codex/Gemini/OpenCode/Aider | Mac | 비주얼 캔버스, 에이전트 역할(Dev/QA/PM/Security), 내장 브라우저 | — |
| **[cmux](https://github.com/manaflow-ai/cmux)** | Swift | 7.7k | 모든 CLI 도구 | Mac only | Socket API, 알림, 자동 리밸런싱 | AGPL-3 |
| **[Calyx](https://github.com/yuuichieguchi/Calyx)** | Swift | — | 모든 CLI 도구 | Mac only | MCP 서버 내장, 에이전트 IPC (7 도구) | — |

### 1.2 MCP 기반 크로스 에이전트 통신

| 도구 | Stars | 역할 | 핵심 기능 |
|------|-------|------|----------|
| **[pal-mcp-server](https://github.com/BeehiveInnovations/pal-mcp-server)** | 11k+ | Provider Abstraction Layer | Claude Code에서 Gemini/OpenAI/Grok/Ollama 등 호출, 컨텍스트 스레딩, 컨텍스트 리바이벌 |
| **[codex-plugin-cc](https://github.com/openai/codex-plugin-cc)** | 16.8k | Codex 플러그인 | Claude Code 내에서 /codex:review, /codex:adversarial-review 실행 |
| **[gemini-mcp](https://github.com/rlabs-inc/gemini-mcp)** | — | Gemini MCP 서버 | Claude Code에서 Gemini 모델 호출 |

### 1.3 에이전트 오케스트레이션 프레임워크

| 도구 | 접근법 | 특징 |
|------|--------|------|
| **Claude Code Agent Teams** | 내장 (Claude 전용) | 리드 에이전트 + 팀원, 공유 태스크 리스트, 메일박스 |
| **[agent-orchestrator](https://github.com/ComposioHQ/agent-orchestrator)** | 자동 오케스트레이션 | 태스크 분해, 에이전트 spawn, CI 수정, 머지 충돌 해결 |
| **[Cline Kanban](https://cline.bot/blog/announcing-kanban)** | CLI-agnostic 오케스트레이션 | 칸반 보드 기반 멀티 에이전트 |

---

## 2. 협업 패턴 분류

### Pattern A: Side-by-Side (병렬 독립 실행)
```
┌──────────────┬──────────────┬──────────────┐
│ Claude Code  │  Codex CLI   │ Gemini CLI   │
│ (Feature A)  │ (Feature B)  │ (Feature C)  │
│              │              │              │
│ git worktree │ git worktree │ git worktree │
│   /branch-a  │   /branch-b  │   /branch-c  │
└──────────────┴──────────────┴──────────────┘
```
- **도구**: claude-squad, parallel-code, Warp, cmux
- **핵심**: 각 에이전트가 독립 worktree에서 독립 작업
- **통신**: 없음 (각자 독립)
- **적합**: 독립적 feature 개발, 병렬 작업 극대화

### Pattern B: Review Chain (순차적 검증)
```
Claude Code (설계/구현)
    ↓
Codex (adversarial review)
    ↓
Claude Code (수정)
    ↓
Gemini (second opinion)
    ↓
최종 코드
```
- **도구**: codex-plugin-cc, pal-mcp-server
- **핵심**: 한 에이전트의 출력을 다른 에이전트가 검증
- **통신**: MCP 프로토콜 또는 플러그인 CLI
- **적합**: 품질 중요 코드, 보안 민감 영역

### Pattern C: Orchestrated Teams (오케스트레이션)
```
    ┌─────────────────┐
    │   Lead Agent    │
    │  (Claude Code)  │
    └────────┬────────┘
       ┌─────┼─────┐
       ↓     ↓     ↓
   ┌───────┐ ┌───────┐ ┌───────┐
   │ Agent │ │ Agent │ │ Agent │
   │(Codex)│ │(Gemin)│ │(Claude)│
   │ Review│ │ Test  │ │ Impl  │
   └───────┘ └───────┘ └───────┘
```
- **도구**: Claude Code Agent Teams, CCB, AgentsRoom
- **핵심**: 리드 에이전트가 작업 분배 + 결과 통합
- **통신**: 공유 태스크 리스트, 메일박스, `/ask` 메시징
- **적합**: 복잡한 프로젝트, 역할 분담 (Dev/QA/Security)

### Pattern D: Cross-Model MCP Bridge (모델 간 브릿지)
```
Claude Code (메인 세션)
    ↕ MCP Protocol
pal-mcp-server
    ├── → Gemini (second opinion)
    ├── → OpenAI (code review)  
    ├── → Grok (edge case 탐색)
    └── → Ollama (로컬 검증)
```
- **도구**: pal-mcp-server, gemini-mcp
- **핵심**: 하나의 에이전트가 MCP를 통해 다른 모델 호출
- **통신**: MCP 프로토콜 (표준화)
- **적합**: 컨텍스트 유지하면서 다양한 관점 수집

---

## 3. 실용적 한계

### Agentmaxxing 천장
- **실용적 최대**: 5~7개 동시 에이전트 (랩탑 기준)
- **병목**: API rate limit, 머지 충돌, 리뷰 병목
- **권장**: 2~3개 에이전트가 최적 (리뷰 품질 유지)

### 에이전트 간 통신 표준
- **MCP**: 사실상 표준 (Anthropic 제정, 1000+ 커뮤니티 서버)
- **tmux 프로토콜**: 세션/패인 관리 (claude-squad, psmux)
- **독자 API**: cmux Socket API, Calyx MCP 서버, CCB `/ask`

---

## 4. zm-mux가 지원해야 할 멀티 에이전트 기능

### 필수 기능 (MVP)

| 기능 | 설명 | 참고 구현체 |
|------|------|-----------|
| **Side-by-Side Panes** | 여러 에이전트를 각각의 split-pane에서 동시 실행 | cmux, Warp, claude-squad |
| **Git Worktree 자동 격리** | 에이전트별 독립 worktree 생성/정리 | claude-squad, parallel-code |
| **에이전트 자동 감지** | CLI 에이전트 종류 자동 인식 (Claude/Codex/Gemini) | Warp Universal Agent Support |
| **에이전트별 알림** | 각 패인의 상태(대기/작업중/완료/에러) 시각적 표시 | cmux (링, 뱃지), Warp (알림 센터) |
| **tmux 프로토콜 호환** | Claude Code Agent Teams 즉시 동작 | psmux |

### 확장 기능 (Phase 2)

| 기능 | 설명 | 참고 구현체 |
|------|------|-----------|
| **에이전트 간 메시징** | 패인 간 텍스트/컨텍스트 전달 | CCB `/ask`, Calyx MCP IPC |
| **MCP 서버 내장** | 에이전트가 zm-mux를 MCP 도구로 사용 | Calyx (7개 MCP 도구) |
| **Review Chain 자동화** | 구현→리뷰→수정 파이프라인 자동 실행 | codex-plugin-cc review gate |
| **에이전트 역할 프리셋** | Dev/QA/Security 역할별 설정 | AgentsRoom |
| **크로스 모델 MCP 브릿지** | zm-mux 내에서 pal-mcp-server 통합 | pal-mcp-server |

### 에이전트 감지 환경변수 설계

```bash
# zm-mux 기본 환경변수
ZM_MUX_WORKSPACE_ID=<workspace-uuid>
ZM_MUX_SURFACE_ID=<pane-uuid>
ZM_MUX_SOCKET_PATH=/tmp/zm-mux-<workspace>.sock

# tmux 호환 (Claude Code Agent Teams용)
TMUX=<zm-mux-session-path>

# 에이전트 메타데이터
ZM_MUX_AGENT_TYPE=claude|codex|gemini|unknown
ZM_MUX_AGENT_PID=<process-id>
```

### Socket API 설계 (cmux 호환 + 확장)

```json
// 패인 생성
{"action": "split", "direction": "right", "command": "claude"}

// 에이전트 상태 조회
{"action": "list-agents"}
// → [{"pane": "1", "agent": "claude", "status": "waiting"}, ...]

// 에이전트 간 메시지 (확장)
{"action": "send-message", "from": "pane-1", "to": "pane-2", "content": "review this diff"}

// MCP 도구 호출 (확장)
{"action": "mcp-call", "tool": "peer-discover", "args": {}}
```

---

## 5. 경쟁 포지셔닝

| 특성 | cmux | Warp | claude-squad | CCB | **zm-mux (목표)** |
|------|------|------|-------------|-----|------------------|
| 플랫폼 | Mac | Win/Mac/Linux | Mac/Linux | Mac/Linux/WSL | **Win/Mac** |
| 멀티 에이전트 | O | O | O | O | **O** |
| 에이전트 간 통신 | Socket API | X (독립) | X (독립) | `/ask` 메시징 | **Socket + MCP** |
| Git worktree | X | X | O | O | **O** |
| GPU 렌더링 | O (Metal) | O (Metal/WebGPU) | X (TUI) | X (tmux) | **O (WebGPU)** |
| 오픈소스 | AGPL-3 | AGPL-3 | — | — | **MIT 목표** |
| Claude Code 팀 호환 | 개발 중 | X | O (tmux) | O (tmux) | **O (tmux 호환)** |

**zm-mux 차별점**: 크로스 플랫폼 + GPU 렌더링 + 에이전트 간 통신(Socket+MCP) + MIT 오픈소스 + tmux 호환

---

## 참고 링크

- [Warp Universal Agent Support](https://www.warp.dev/blog/universal-agent-support-level-up-coding-agent-warp)
- [Warp Multi-Agent Guide](https://docs.warp.dev/guides/agent-workflows/how-to-run-multiple-ai-coding-agents)
- [claude-squad GitHub](https://github.com/smtg-ai/claude-squad) (7.2k stars)
- [claude_code_bridge GitHub](https://github.com/bfly123/claude_code_bridge) (2.4k stars)
- [parallel-code GitHub](https://github.com/johannesjo/parallel-code) (591 stars)
- [AgentsRoom](https://agentsroom.dev/)
- [pal-mcp-server GitHub](https://github.com/BeehiveInnovations/pal-mcp-server) (11k+ stars)
- [codex-plugin-cc GitHub](https://github.com/openai/codex-plugin-cc) (16.8k stars)
- [Agentmaxxing Guide](https://vibecoding.app/blog/agentmaxxing)
- [Claude Code Agent Teams Docs](https://code.claude.com/docs/en/agent-teams)
- [Addy Osmani — Code Agent Orchestra](https://addyosmani.com/blog/code-agent-orchestra/)

---

*조사일: 2026-04-30*
