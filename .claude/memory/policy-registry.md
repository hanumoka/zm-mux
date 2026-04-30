# zm-mux Policy Registry (SSOT)

> 확정된 설계 정책. 새 요구사항은 이 테이블과 대조하여 충돌 여부를 확인한다.
> 최종 업데이트: 2026-04-30

## ARCH: Architecture Policies

| ID | Policy | Date | Rationale |
|:--:|--------|------|-----------|
| ARCH-01 | 크로스 플랫폼 (Windows + macOS) 터미널 멀티플렉서, WSL 불필요 | 2026-04-30 | docs/06 리서치. cmux=Mac전용, Warp=AGPL — 오픈소스 크로스 플랫폼 AI 터미널 부재 |
| ARCH-02 | 크로스 플랫폼 PTY: Windows=ConPTY, macOS=POSIX PTY, `portable-pty` 크레이트 | 2026-04-30 | WezTerm/COSMIC Terminal 검증 패턴 |
| ARCH-03 | GPU 렌더링: `glyphon` + `cosmic-text` + `wgpu`, 폴백: `softbuffer` + `tiny-skia` | 2026-04-30 | COSMIC Terminal 프로덕션 검증. docs/10 리스크 재평가에서 MEDIUM으로 하향 |
| ARCH-04 | cmux Socket API 프로토콜 호환 | 2026-04-30 | 에이전트 생태계 호환성 |
| ARCH-05 | Agent workflow-first 설계: split-pane, 알림, Socket API, 멀티 에이전트 | 2026-04-30 | 핵심 차별점 |
| ARCH-06 | 구현 언어: Rust | 2026-04-30 | 터미널 개발의 사실상 표준 (WezTerm, Alacritty, Warp, Rio, psmux) |
| ARCH-07 | VT 에뮬레이션: `alacritty_terminal` 크레이트 재사용 (직접 구현 금지) | 2026-04-30 | COSMIC Terminal 패턴. VT 파싱은 검증된 크레이트 사용이 안전하고 효율적 |

## TECH: Technology Policies

| ID | Policy | Date | Rationale |
|:--:|--------|------|-----------|
| TECH-01 | 에이전트 감지 환경변수: ZM_MUX_WORKSPACE_ID, ZM_MUX_SURFACE_ID | 2026-04-30 | cmux의 CMUX_WORKSPACE_ID/CMUX_SURFACE_ID 대응 |
| TECH-02 | 알림: OSC 9/99/777 이스케이프 시퀀스 지원 | 2026-04-30 | 표준 터미널 알림 프로토콜 |
| TECH-03 | Claude Code 통합 투트랙: (1) tmux 프로토콜 호환 즉시 적용 (psmux 패턴, TMUX 환경변수) (2) CustomPaneBackend JSON-RPC 프로토콜 후속 구현 (#26572 제안, 7개 오퍼레이션) | 2026-04-30 | docs/10 조사. tmux 호환으로 즉시 동작 보장, CustomPaneBackend로 공식 통합 추진 |
| TECH-04 | MCP 서버: `rmcp` 공식 Rust SDK (470만+ 다운로드) 사용 | 2026-04-30 | docs/10 조사. 공식 SDK 성숙도 확인 (MEDIUM-HIGH → LOW-MEDIUM으로 재평가) |

## PROD: Product Policies

| ID | Policy | Date | Rationale |
|:--:|--------|------|-----------|
| PROD-01 | 멀티 에이전트 지원: Claude Code + Codex + Gemini CLI 동시 실행 | 2026-04-30 | docs/08 조사. Side-by-Side + 에이전트 간 통신이 핵심 요구사항 |
