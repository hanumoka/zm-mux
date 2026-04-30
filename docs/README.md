# zm-mux 조사 자료

## 프로젝트 배경

Windows 환경에서 Claude Code 사용 시 macOS 대비 에이전트 팀 기능 등에 제약이 있다. macOS에서는 cmux라는 AI 에이전트 전용 터미널이 등장하여 에이전트 팀을 네이티브 split-pane으로 관리할 수 있으나, Windows에는 동등한 도구가 부재하다. 이 조사는 Windows용 AI 에이전트 터미널 멀티플렉서 개발을 위한 기초 자료이다.

---

## 문서 구조

| 파일 | 내용 | 핵심 키워드 |
|------|------|------------|
| [01-windows-claude-code-issues.md](01-windows-claude-code-issues.md) | Windows에서 Claude Code 사용 시 알려진 문제점 | 에이전트 팀 제한, 버그, 설치 문제 |
| [02-cmux-overview.md](02-cmux-overview.md) | cmux (macOS AI 에이전트 터미널) 상세 분석 | libghostty, 알림, 내장 브라우저, Socket API |
| [03-tmux-vs-cmux.md](03-tmux-vs-cmux.md) | tmux와 cmux 비교 분석 | 설계 철학, 기능 비교, 사용 시나리오 |
| [04-wmux-existing-projects.md](04-wmux-existing-projects.md) | 기존 wmux 프로젝트들 분석 | 6개 구현체 비교, 기능 격차 분석 |
| [05-windows-terminal-comparison.md](05-windows-terminal-comparison.md) | Windows 터미널 프로그램 비교 | Shift+Enter, 알림, GPU, 추천 조합 |
| [06-cross-platform-terminal-survey.md](06-cross-platform-terminal-survey.md) | **크로스 플랫폼 터미널 전수 조사** | Win+Mac 전체, AI 에이전트 터미널, 아키텍처 분석 |
| [07-claude-codex-dynamic-duo.md](07-claude-codex-dynamic-duo.md) | **Claude Code + Codex 결합 워크플로우** | adversarial review, 계획 검증, 경제적 활용 |
| [08-multi-agent-collaboration.md](08-multi-agent-collaboration.md) | **멀티 AI 에이전트 협업 생태계** | Side-by-Side, Review Chain, 오케스트레이션, MCP 브릿지 |
| [09-feasibility-analysis.md](09-feasibility-analysis.md) | **기능별 구현 가능성 분석** | TIER 1~4 분류, 로드맵, 크리티컬 리스크, 정직한 결론 |
| [10-critical-risks-update.md](10-critical-risks-update.md) | **크리티컬 리스크 최신 업데이트** | 3대 리스크 재평가, CustomPaneBackend, COSMIC Terminal, rmcp |
| [11-implementation-roadmap.md](11-implementation-roadmap.md) | **구현 작업 계획서** | Phase 0~4, Cargo 크레이트 구조, 마일스톤별 작업/검증 |

---

## 핵심 발견사항 요약

1. **cmux**는 2026년 2월 출시된 macOS 전용 AI 에이전트 터미널 (Ghostty/libghostty 기반, 7.7k+ stars)
2. **Calyx**는 cmux의 경쟁자. libghostty 기반 + MCP 서버 내장으로 에이전트 간 IPC 지원
3. **Warp**는 유일한 크로스 플랫폼(Win/Mac/Linux) AI 에이전트 터미널 (Rust, AGPL-3)
4. **psmux**는 Windows 네이티브 tmux (Rust) — Claude Code 에이전트 팀과 즉시 호환
5. **WezTerm**은 가장 완성도 높은 크로스 플랫폼 Rust 터미널 멀티플렉서 (MIT, 19k+ stars)
6. **Rust가 터미널 개발의 사실상 표준** — Warp, WezTerm, Alacritty, Rio, psmux 모두 Rust
7. **GPU 렌더링**: Metal(Mac), WebGPU(크로스 플랫폼), OpenGL(레거시) 3가지 선택지
8. **Claude Code 에이전트 팀**: Windows에서 isTTY 문제로 split-pane 미지원 (이슈 #26244)

---

## 조사 출처

- Claude Code 공식 문서: https://code.claude.com/docs/
- cmux 공식: https://cmux.com/
- cmux GitHub: https://github.com/manaflow-ai/cmux
- Claude Code GitHub Issues: https://github.com/anthropics/claude-code/issues
- wmux 관련 GitHub 저장소들 (04번 문서 참조)

---

*조사일: 2026-04-30*
