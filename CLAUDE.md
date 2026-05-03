# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

zm-mux is a cross-platform (Windows + macOS) AI agent terminal multiplexer built with Rust. Inspired by cmux (macOS-only) and addressing the gap that no open-source, cross-platform AI agent terminal exists.

### Core Problem

- Claude Code agent teams require tmux or cmux for split-pane mode — neither works natively on Windows
- cmux/Calyx are macOS-only; Warp is cross-platform but AGPL with closed UI framework
- Existing wmux projects are Electron + xterm.js based, with performance/memory overhead
- No open-source cross-platform AI agent terminal exists (zm-mux fills this gap)

### Design Goals

- Cross-platform terminal multiplexer (Windows + macOS) without WSL dependency
- Rust implementation following COSMIC Terminal pattern:
  - `alacritty_terminal` — VT 파싱 + 터미널 상태 (직접 구현 금지)
  - `portable-pty` — 크로스 플랫폼 PTY (ConPTY + POSIX PTY)
  - `cosmic-text` — 텍스트 셰이핑 (HarfBuzz, 리거처)
  - `glyphon` — wgpu 텍스트 렌더링 (글리프 아틀라스)
  - `wgpu` — GPU 가속 렌더링 (Metal/DX12/Vulkan 자동 선택)
  - `softbuffer` + `tiny-skia` — GPU 불가 시 CPU 폴백
- AI agent workflow-first: split-pane 관리, 알림, Socket API, 멀티 에이전트
- Claude Code 통합 투트랙:
  1. **즉시**: tmux 프로토콜 호환 (TMUX 환경변수, psmux 검증 패턴)
  2. **후속**: CustomPaneBackend JSON-RPC 프로토콜 (#26572 제안, 7개 오퍼레이션)
- 멀티 AI 에이전트 지원: Claude Code + Codex + Gemini CLI 동시 실행
- Key terminal features: Shift+Enter, desktop notifications (OSC 9/99/777), Unicode/color

### Key References

**COSMIC Terminal** (System76): 가장 직접적 아키텍처 참조. `alacritty_terminal` + `glyphon` + `wgpu` 조합을 프로덕션 검증. GPU 폴백 포함.

**WezTerm**: 19+ Rust crate 모듈 설계 (term ↔ gui ↔ mux 분리), MIT. `portable-pty` 크레이트 원작자.

**cmux**: "Primitive, Not Solution" — Socket API via Unix domain socket. 에이전트 감지: `CMUX_WORKSPACE_ID` / `CMUX_SURFACE_ID`.

**psmux**: Windows-native tmux in Rust. Claude Code 에이전트 팀 호환 검증. isTTY 우회법 보유.

**CustomPaneBackend (#26572)**: Claude Code의 tmux 의존을 대체하는 JSON-RPC 2.0 프로토콜 제안. `spawn_agent`, `write`, `capture`, `kill`, `list`, `initialize`, `context_exited` 7개 오퍼레이션.

---

## Work Methodology

- Claude Code 자율 개발 + 사용자 리뷰
- 커밋은 사용자 요청 시에만 생성
- Git convention: `<type>(<scope>): <subject>` + Co-Authored-By footer

---

## Session Management

### Auto-Start (SessionStart hook)
세션 시작 시 자동으로 context.md의 Focus/TODOs/Blockers 표시

### Session Save
`/zm-memory-save` 로 세션 컨텍스트 저장 (context.md 업데이트)

### Context Files
- `.project-memory/context.md` — 현재 세션 상태 (Focus, TODOs, Blockers, Decisions, Metrics)
- `.project-memory/pre-compact-recovery.md` — 컴팩션 전 자동 백업 (hook 관리)

---

## Skills Quick Reference

| Skill | 설명 |
|-------|------|
| `/zm-memory-save` | 세션 저장: context.md + 작업 완료 기록 |
| `/zm-work-intake` | 새 작업 수용: 요구사항 검증 + 영향 분석 |
| `/zm-docs` | docs/ 리서치 문서 관리 |
| `/zm-changelog` | 변경 이력 조회 |
| `/zm-troubleshoot` | 트러블슈팅 패턴 관리 |

---

## Agents

| Agent | Model | 역할 |
|-------|-------|------|
| `doc-researcher` | sonnet | 기술 조사/리서치 (읽기 전용) |
| `code-reviewer` | sonnet | 코드 리뷰 + 보안 검사 (읽기 전용) |
| `build-checker` | haiku | 빌드/타입 체크 (빠른 검증) |
| `doc-updater` | haiku | 문서 자동 업데이트 |

---

## Mandatory Protocols

### Auto Memory Protocol
아키텍처 결정, TODO 완료, 블로커 발생, 포커스 변경 시 context.md 자동 업데이트

### Work Completion Protocol
작업 완료 시 bugfix/feature/research 유형별 관련 문서 업데이트

### Mistake Recording Protocol
- Tier 1: `.claude/rules/known-mistakes.md` (M-NNN, [BLOCK]/[WARN])
- 반복 가능한 실수는 즉시 등록

### Compact Recovery Protocol
PreCompact hook이 context + git 상태 저장 → PostCompact hook이 복원

---

## Implementation Quality Standard (MANDATORY)

1. **유연성**: 하드코딩 금지, 설정 기반
2. **안정성**: 에러 핸들링, 프로세스 격리
3. **표준화**: 일관된 코딩 스타일, 네이밍 컨벤션

---

## Research Documents

All research is in `docs/`. These are in Korean.

| File | Topic |
|------|-------|
| `01-windows-claude-code-issues.md` | Windows Claude Code limitations (agent teams, bugs, sandboxing) |
| `02-cmux-overview.md` | cmux feature analysis (Socket API, notifications, browser, GPU rendering) |
| `03-tmux-vs-cmux.md` | tmux vs cmux comparison, Claude Code team integration details |
| `04-wmux-existing-projects.md` | 6 existing wmux implementations compared |
| `05-windows-terminal-comparison.md` | Terminal comparison from Claude Code compatibility perspective |
| `06-cross-platform-terminal-survey.md` | Cross-platform terminal survey (Win+Mac), architecture patterns |
| `07-claude-codex-dynamic-duo.md` | Claude Code + Codex plugin workflow |
| `08-multi-agent-collaboration.md` | Multi-agent collaboration ecosystem (4 patterns) |
| `09-feasibility-analysis.md` | Feature feasibility analysis (TIER 1-4), roadmap |
| `10-critical-risks-update.md` | Critical risk reassessment, COSMIC Terminal, CustomPaneBackend |
| `11-implementation-roadmap.md` | Phase 0-4 work plan, Cargo crate layout, milestone-by-milestone tasks |
| `12-istty-workaround.md` | 🔴 Windows isTTY workaround (BLOCKED track, kept for reference) — psmux env vars, `/team` slash absent |
| `13-custompanebackend-track.md` | CustomPaneBackend JSON-RPC track (Phase 2.1+), #26572 7-op breakdown, sync→tokio staging |
| `14-26572-advocacy-draft.md` | Ready-to-post #26572 comment + recording guide for the minimal reference demo |

## Language

Project documentation is in Korean. Code comments and commit messages should be in English.
