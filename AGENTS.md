# AGENTS.md

AI 코딩 도구 공통 규칙 (Claude Code, GitHub Copilot, Gemini Code Assist 등).

---

## 프로젝트 개요

zm-mux는 크로스 플랫폼(Windows + macOS) AI 에이전트 터미널 멀티플렉서이다. Rust로 구현하며, GPU 가속 렌더링(WebGPU/WGPU), split-pane 에이전트 관리, Socket API를 제공한다. macOS의 cmux에 대응하되 Windows까지 지원하는 것이 차별점이다.

---

## 디렉토리 구조

```
zm-mux/
├── CLAUDE.md              # Claude Code 전용 지침
├── AGENTS.md              # 도구 중립 규칙 (이 파일)
├── .claude/               # Claude Code 자동화
│   ├── settings.json      # Hooks 설정
│   ├── settings.local.json # 권한/환경변수
│   ├── hooks/             # 라이프사이클 훅 (8개)
│   ├── agents/            # 서브에이전트 정의 (4개)
│   ├── skills/            # 슬래시 커맨드 (5개)
│   ├── rules/             # 코딩 규칙
│   └── memory/            # 프로젝트 메모리
├── .project-memory/       # 세션 상태
│   └── context.md         # Focus/TODOs/Blockers/Decisions
├── .mcp.json              # MCP 서버 설정
└── docs/                  # 리서치 문서 (한국어)
```

---

## 커밋 컨벤션

```
<type>(<scope>): <subject>

Co-Authored-By: Claude <model> <noreply@anthropic.com>
```

**type**: feat, fix, docs, refactor, test, chore, perf
**scope**: core, renderer, pty, mux, vt, socket, agent, mcp, docs, config

---

## 파일 네이밍

- 리서치 문서: `NN-kebab-case.md` (예: `06-cross-platform-terminal-survey.md`)
- 소스 코드: Rust 컨벤션 (snake_case, 모듈은 mod.rs 또는 파일명)
- 설정 파일: kebab-case

---

## 보안 규칙 (MANDATORY)

- .env 내용 출력/로깅 절대 금지
- API 키, 비밀번호, 토큰을 코드/문서에 포함 금지
- 위반 발견 시 즉시 `[REDACTED]` 교체 + 사용자 알림
- ConPTY 사용 시 프로세스 격리 확인
- 사용자 입력은 항상 검증/이스케이프 (커맨드 인젝션 방지)

---

## 플랫폼 규칙

- **대상 OS**: Windows 11 + macOS (크로스 플랫폼)
- **구현 언어**: Rust
- **VT 에뮬레이션**: `alacritty_terminal` 크레이트 (직접 구현 금지, M-004)
- **PTY API**: ConPTY (Windows) + POSIX PTY (macOS), `portable-pty` 크레이트
- **렌더링**: `glyphon` + `cosmic-text` + `wgpu` (COSMIC Terminal 검증 패턴)
- **GPU 폴백**: `softbuffer` + `tiny-skia` (GPU 불가 환경)
- **MCP SDK**: `rmcp` (공식 Rust MCP SDK)
- **개발 환경 Shell**: bash (Windows: Git for Windows, macOS: zsh/bash)
- **Python**: `python` 사용 (`python3` 아님), `PYTHONUTF8=1` 접두사 권장

---

## 모호성 프로토콜

요구사항이 불명확할 때:
1. 작업 중단
2. 3개 이상의 구체적 질문 제시
3. 사용자 답변 후 진행

추측 기반 구현 금지.

---

## 설계 변경 프로토콜

1. `.claude/memory/policy-registry.md` 확인 (기존 정책과 충돌 여부)
2. 충돌 시 사용자에게 명시적 승인 요청
3. 승인 후 policy-registry.md 업데이트
4. CLAUDE.md 설계 목표와 정합성 확인

---

*최종 업데이트: 2026-04-30*
