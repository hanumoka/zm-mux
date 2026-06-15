# 06 — 종합 실현성 판정 · 리스크 · 로드맵

> 범례 [00](00-overview.md). 본 문서는 *다음(구현) 플랜의 입력*이다.

## 1. 종합 판정

| 요구사항 | 판정 | 신뢰도 |
|----------|------|--------|
| ① Windows에서 cmux 동등 경험 | **재구현으로 가능** | 기능=높음 / 에이전트연동=조건부([04](04-ai-agent-integration.md)) |
| ② 크로스플랫폼(Win+Mac) | **가능** | 높음 [V] |
| ③ WSL 없이 Windows | **가능** | 높음 [V] |

**한 줄 결론**: zm-mux는 **네이티브 Rust로 기술적으로 충분히 실현 가능**하다. cmux 자체는 macOS에
구조적으로 결박되어 포팅 불가이나, 동등 *기능*은 입증된 SAFE 크레이트(WezTerm/Zellij가 동일 스택으로
Win/Mac/Linux 입증)로 재구현된다. 유일한 *조건부* 항목은 Claude Code 분할패널 연동의 Windows
isTTY 게이트(Anthropic 정책)이며, 이는 PoC 실측 + 3트랙 전략으로 관리한다([04](04-ai-agent-integration.md)).

## 2. 리스크 레지스터

| ID | 리스크 | 영향 | 가능성 | 완화 |
|----|--------|------|--------|------|
| R1 | **Claude Code isTTY 게이트**(#26244 closed) — Windows split 봉쇄 | 높음(① 핵심 UX) | 중 | 트랙 A(자체 ConPTY로 진짜 TTY 부여) PoC 최우선 실측 + 트랙 B(#26572) 병행 + C 폴백 |
| R2 | 크레이트 버전 lockstep (glyphon↔wgpu↔cosmic-text) | 중(빌드) | 중 | 스캐폴드 직후 `cargo check`로 확정, 필요 시 버전 핀 |
| R3 | ConPTY 함정(커서질의 hang, SGR, 리사이즈) | 중 | 중 | WezTerm/alacritty/microsoft-terminal 샘플 학습, VT 처리 모드 신중 |
| R4 | clean-room 규율 위반(STUDY 코드 유입) | 높음(법적) | 낮음 | [05](05-reference-inventory.md) 규칙, 리뷰 시 출처 점검, cmux/cosmic 코드 비참조 |
| R5 | 범위 과대(임베디드 브라우저 등) | 중(일정) | 중 | MVP에서 브라우저/원격데몬 제외([03](03-crossplatform-architecture.md)) |
| R6 | portable-pty 정체(0.9.0, 2025-02) | 낮음 | 낮음 | WezTerm 본체 `pty` 모듈 대안 학습, 필요 시 직접 ConPTY 바인딩 |

## 3. 단계별 로드맵 (제안 — 다음 플랜에서 확정)

| Phase | 목표 | 산출/DoD | 크레이트 |
|-------|------|----------|----------|
| **0. PoC** | 단일 pane 동작 + **isTTY 실측** | Windows(WSL X)+mac에서 셸 1개 실행·렌더. **zm-mux ConPTY 자식의 Claude Code isTTY 결과 기록(R1 게이트)** | zm-core/pty/term/render/app |
| **1. Mux 코어** | 탭·분할·레이아웃·CPU 폴백·설정 | 탭/스플릿 조작, GPU 실패 시 폴백, TOML 설정 | zm-mux, zm-render |
| **2. 자동화 소켓** | 로컬 소켓 server/client + tmux 호환 명령 | `new-window/split/send-keys/capture/list/...` 동작, 외부 `zm` CLI | zm-ipc |
| **3. 에이전트 연동** | tmux-shim+env(트랙 A), 알림, Shift+Enter | Claude Code 팀 패널 시도(가능 시), OSC 9/99/777→toast, CSI-u | zm-agent |
| **3b. CustomPaneBackend** | #26572 트랙 시제품 | JSON-RPC 백엔드(initialize/spawn_agent/write/capture/kill/list) | zm-agent |
| **4. 패리티/폴리시** | 워크스페이스 자동네이밍·사이드바·세션 지속 | cmux UX 근접 | 전반 |
| (후순위) | 임베디드 브라우저·원격 데몬 | WebView2/WKWebView 추상화 검토 | — |

> **Phase 0의 R1 실측이 전체 방향의 게이트**: 트랙 A가 Windows에서 통하면 cmux 동등 UX에 가장 근접.
> 안 통하면 B(#26572 채택 대기) 또는 C(in-process)로 기대치 조정.

## 4. 미해결 질문 (다음 플랜/사용자 확인)

1. **MVP 우선순위**: 멀티플렉서 기본(탭/분할) 먼저 vs Claude Code 연동 먼저? (R1 결과 의존)
2. **브라우저/원격데몬**: 후순위 확정? (현 권장=MVP 제외)
3. **타깃 OS 범위**: Win+Mac 우선, Linux는 보너스? (Zellij/WezTerm 스택상 Linux는 거의 무상)
4. **배포 형태**: 단일 바이너리 + 스크립트 설치(scoop/winget/brew)?
5. **Codex/Gemini 연동** 깊이: 1차 조사 [I] → 필요 시 공식 docs 재검증 [?].

## 5. 다음 액션

- 본 1차 산출물 검토 → 승인 시 **구현 플랜(Phase 0 PoC)** 작성:
  스캐폴드(03 크레이트 레이아웃) → `cargo check` 버전 확정 → 단일 pane PoC + isTTY 실측.
