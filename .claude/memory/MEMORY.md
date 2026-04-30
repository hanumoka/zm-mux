# zm-mux Project Memory

## Project Metrics (auto-synced)
- Research docs: 11 (docs/01~11)
- Known mistakes: 4 (M-001~M-004)
- Agents: 4 (doc-researcher, code-reviewer, build-checker, doc-updater)
- Skills: 5 (zm-memory-save, zm-work-intake, zm-docs, zm-changelog, zm-troubleshoot)
- Hooks: 8 (session-start, mistake-guard, post-review, post-failure, pre/post-compact, notify-done, prompt-context)
- Policies: 7 ARCH + 4 TECH + 1 PROD (policy-registry.md)
- Phase: Research 완료 → 구현 준비

## Key Learnings
- **COSMIC Terminal 패턴**: alacritty_terminal + cosmic-text + glyphon + wgpu 조합이 프로덕션 검증됨 → zm-mux 기술 스택 확정
- **CustomPaneBackend (#26572)**: Claude Code의 tmux 의존을 대체하는 JSON-RPC 2.0 프로토콜 제안. 7개 오퍼레이션으로 Ghostty/WezTerm/Zellij도 통합 예정
- **rmcp SDK**: 공식 Rust MCP SDK, 470만+ 다운로드. MCP 서버 내장 기술적 문제 없음
- **psmux isTTY 우회**: TMUX 환경변수 설정으로 Windows isTTY 문제 우회 검증됨
- **GPU 폴백**: softbuffer + tiny-skia로 GPU 불가 환경 안전망 (COSMIC Terminal 패턴)
- **3대 리스크 완화**: isTTY(HIGH-MEDIUM), WGPU 렌더링(MEDIUM), tmux 프로토콜(MEDIUM) — 모두 CRITICAL에서 하향

## Rules Reference
- `.claude/rules/known-mistakes.md` — M-NNN 실수 패턴 레지스트리
- `.claude/rules/security.md` — 보안 규칙
- `.claude/memory/policy-registry.md` — ARCH/TECH/PROD 정책 SSOT
