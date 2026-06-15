# 00 — Overview & Verification Methodology

> zm-mux 정밀 검토 1차 산출물의 진입 문서. 요구사항·방법론·검증 범례·최종 실현성 판정.

## 1. 요구사항 (사용자 정의, 그대로)

1. **Windows 환경에서도 cmux와 거의 동일한 프로그램**을 사용하고 싶다. (cmux는 현재 macOS 전용)
2. 가능하면 **크로스 플랫폼**을 지원해야 한다.
3. Windows에서 **WSL 없이** 동작 가능해야 한다.

추가 지시: 누락·오판·추측 없이 정밀 검토. 웹 참고자료 + 참고 가능한 오픈소스를 로컬에 받아 관리.
cmux가 오픈소스면 로컬에 받아 분석.

## 2. 확정된 프로젝트 결정 (사용자 승인)

| 항목 | 결정 |
|------|------|
| 라이선스/재사용 | **Clean-room (MIT/Apache)** — copyleft 코드는 이해용 분석만, 복사 금지 |
| 1차 산출물 | **자료수집 + 정밀분석 문서** (구현/스캐폴드는 다음 플랜) |
| 목표 아키텍처 | **네이티브 Rust** (WezTerm/COSMIC 스택) |
| 참고 관리 | `reference/`(클론, gitignored) + `docs/research/`(분석·원문) |

## 3. 검증 방법론 ("오판/추측 없이")

3-축 병렬 웹 조사(에이전트 3) → **1차 출처 재검증 패스**로 교차확인:
- **GitHub REST API** (`api.github.com/repos/...`, `/issues/...`) — 라이선스·주언어·아카이브·이슈 상태.
- **crates.io API** (`crates.io/api/v1/crates/...`) — 크레이트 최신 stable/max 버전.
- **클론된 실소스 직독** (`reference/*`) — cmux LICENSE/Sources/CLI, 각 repo의 LICENSE/Cargo.toml.
- **공식 문서** (`code.claude.com/docs/...`) — Claude Code agent-teams.

> ⚠️ 재검증에서 **1차 조사의 다수 오판을 교정**했다(§5). 블로그 등 2차 출처는 1차 출처로 대체/강등.

### 검증 범례 (모든 문서 공통)

| 태그 | 의미 |
|------|------|
| **[V]** | 1차/공식 출처로 교차확인됨 (repo 소스 / GitHub·crates.io API / 공식 docs) |
| **[I]** | 합리적 추론 (근거는 있으나 직접 확인 안 됨) |
| **[?]** | 미확인 — 다음 단계(구현 플랜)에서 재검증 대상 |

## 4. 최종 실현성 판정 (요약)

| 요구사항 | 판정 | 근거 |
|----------|------|------|
| ① cmux 동등 경험 (Windows) | **실현 가능, 단 재구현** [V/I] | cmux는 Swift+AppKit+libghostty(Metal)+WebKit로 macOS에 구조적으로 결박 → 포팅 불가, 동등 *기능*을 네이티브 Rust로 재구현해야 함. 상세 [01](01-cmux-analysis.md) |
| ② 크로스 플랫폼 | **실현 가능** [V] | WezTerm(MIT)·Zellij(MIT)가 Win/Mac/Linux 네이티브로 이미 입증. 상세 [02](02-windows-no-wsl-feasibility.md) |
| ③ WSL 없이 Windows | **실현 가능** [V] | ConPTY(Win10 1809+) + `portable-pty`로 WSL 불필요. 상세 [02](02-windows-no-wsl-feasibility.md) |

**핵심 리스크(기술 아님)**: AI 에이전트(특히 Claude Code) **분할패널 연동**은 Anthropic 측
`isTTY` 게이트(issue #26244, *closed/not_planned*)로 Windows에서 막혀 있다. zm-mux는 이를
tmux-shim/`CMUX_*`식 env 우회 또는 CustomPaneBackend(#26572, *open*) 트랙으로 풀어야 한다.
상세 [04](04-ai-agent-integration.md).

## 5. 1차 조사 대비 교정 사항 (검증의 성과)

| 항목 | 1차 조사(오판) | 재검증 확정값 | 출처 |
|------|----------------|----------------|------|
| wmux 라이선스 | "AGPL-3.0" | **MIT** (amirlehmam, openwong2kim **둘 다 실재·MIT**) | repo LICENSE [V] |
| psmux 주언어 | (혼동) GitHub=PowerShell | **Rust** (Cargo.toml+crates+rust-toolchain) | clone 직독 [V] |
| cmux 라이선스 | "GPL-3.0" | **dual: GPL-3.0-or-later OR Commercial** | cmux/LICENSE [V] |
| bradwilson331/cmux-linux | "Rust+GTK4" | **Swift+Rust 혼합, AGPL-3.0** | repo [V] |
| Claude Code task 경로 | `~/.claude/teams/{name}/` | task list = **`~/.claude/tasks/{name}/`** | 공식 docs [V] |
| cmux Bonsplit/소켓명 | 블로그(2차) | **소스 확정**(vendor/bonsplit 서브모듈 + CLI 소스 토큰) | clone 직독 [V] |

## 6. 문서 맵

- [01-cmux-analysis.md](01-cmux-analysis.md) — cmux 소스 기반 정밀 분석
- [02-windows-no-wsl-feasibility.md](02-windows-no-wsl-feasibility.md) — ConPTY·Rust 스택 실현성
- [03-crossplatform-architecture.md](03-crossplatform-architecture.md) — 권장 네이티브 Rust 아키텍처 + 기능 매핑
- [04-ai-agent-integration.md](04-ai-agent-integration.md) — Claude Code/AI 에이전트 연동
- [05-reference-inventory.md](05-reference-inventory.md) — 참고 repo/원문 인벤토리 (라이선스·등급·SHA)
- [06-feasibility-and-roadmap.md](06-feasibility-and-roadmap.md) — 종합 판정·리스크·로드맵
- [references.md](references.md) — 전 출처 서지

> 작성 기준일: 2026-06-15. 재검증 시점의 라이브 값(이슈 상태/버전)은 이후 변동 가능.
