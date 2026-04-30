# zm-mux 기능별 구현 가능성 분석

## 개요

8개 리서치 문서 기반으로 zm-mux의 모든 계획 기능을 4단계로 분류한다.
- **TIER 1**: 확실히 구현 가능 (검증된 레퍼런스 + 성숙한 크레이트)
- **TIER 2**: 노력하면 구현 가능 (레퍼런스 존재, 상당한 작업량 필요)
- **TIER 3**: 리스크/불확실 (외부 의존성, 미검증 접근법)
- **TIER 4**: MVP에서 구현 불가 (1~2인 팀 기준, Post-1.0 이후)

---

## TIER 1: 확실히 구현 가능 ✅

| # | 기능 | 예상 기간 | 레퍼런스 | Rust 크레이트 | 리스크 |
|---|------|----------|---------|-------------|--------|
| 1 | 크로스 플랫폼 터미널 (Win+Mac) | 4~8주 | WezTerm, Alacritty, Rio | `winit`, `portable-pty`, `vte`, `tokio` | LOW |
| 4 | Split Pane 관리 | 2~3주 | WezTerm mux 레이어 | 자체 구현 (바이너리 트리) | LOW |
| 5 | 탭 관리 | 1~2주 | WezTerm, Warp | `winit` | LOW |
| 9 | 스크롤백 버퍼 | 1~2주 | 모든 터미널 | `vte`, 링 버퍼 | LOW |
| 11 | ConPTY (Windows) | 1주 | WezTerm, psmux | `portable-pty` | LOW |
| 12 | POSIX PTY (macOS) | 3~5일 | 모든 Unix 터미널 | `portable-pty`, `nix` | LOW |
| 13 | 크로스 플랫폼 PTY 추상화 | 2~3일 | WezTerm `portable-pty` | `portable-pty` (crates.io 공개) | LOW |
| 14 | 프로세스 생명주기 관리 | 1~2주 | WezTerm | `portable-pty`, `tokio`, `signal-hook` | LOW |

**소계**: ~12~20주 (핵심 터미널 인프라)

---

## TIER 2: 노력하면 구현 가능 ⚠️

| # | 기능 | 예상 기간 | 레퍼런스 | 핵심 난점 | 리스크 |
|---|------|----------|---------|----------|--------|
| 2 | **GPU 렌더링 (WGPU)** | **6~10주** | Rio (유일한 선례) | GPU 텍스트 렌더링 파이프라인 (글리프 아틀라스, 셰이더) | **MEDIUM** |
| 3 | VT100/xterm 터미널 에뮬레이션 | 3~6주 | WezTerm `termwiz`, `vte` | 수백 개 이스케이프 시퀀스 완성도 | MEDIUM |
| 6 | Shift+Enter 지원 | 1~2주 | WezTerm (Kitty 프로토콜) | Windows ConPTY 입력 모드 | LOW-MEDIUM |
| 7 | Unicode/True Color | 2~3주 | 모든 최신 터미널 | CJK 넓은 문자, 이모지 시퀀스 | LOW-MEDIUM |
| 8 | **폰트 렌더링 (리거처, 이모지)** | **7~11주** | WezTerm `wezterm-font` | HarfBuzz 통합, 컬러 이모지, 플랫폼별 폰트 검색 | **MEDIUM** |
| 10 | 데스크톱 알림 (OSC 9/99/777) | 1~2주 | Ghostty, cmux | `notify-rust` | LOW-MEDIUM |
| 15 | **tmux 프로토콜 호환** | **4~8주** | psmux | 필요 명령어 하위 집합 식별, isTTY 우회 | **MEDIUM** |
| 16 | Claude Code 에이전트 팀 지원 | 2~3주 | psmux, claude-squad | Windows isTTY 이슈 (#26244) | MEDIUM |
| 20 | 에이전트 상태 알림 | 2~4주 | cmux (링, 뱃지), Warp | 에이전트 상태 감지 정확성 | LOW-MEDIUM |
| 21 | 에이전트 자동 리밸런싱 | 1~2주 | cmux | 레이아웃 재계산 로직 | LOW |
| 22 | **Socket API** | **2~4주** | cmux, fernandomenuk/wmux | AF_UNIX Windows 호환성 | LOW-MEDIUM |
| 23 | Named Pipe API (Windows) | 1~2주 | Windows Terminal | Socket API 폴백 | LOW |
| 31 | 세션 지속성 | 3~5주 | WezTerm, tmux | 프로세스 재생성 한계 (레이아웃+스크롤백만 복원 가능) | MEDIUM |

**소계**: ~35~60주 (AI 에이전트 통합 + 품질)

---

## TIER 3: 리스크/불확실 ❓

| # | 기능 | 예상 기간 | 핵심 리스크 | 왜 불확실한가 |
|---|------|----------|-----------|-------------|
| 17 | 멀티 에이전트 동시 실행 | 2~3주 | 외부 의존성 | Codex CLI, Gemini CLI의 Windows 호환성은 zm-mux가 통제 불가 |
| 18 | Git Worktree 자동 격리 | 2~4주 | Windows 엣지케이스 | Windows 심링크 제한, 크래시 시 정리, 서브모듈 |
| 19 | 에이전트 자동 감지 | 2~3주 | 유지보수 부담 | 에이전트별 프로세스명/인자 패턴 — 에이전트 업데이트 시 깨짐 |
| 24 | 에이전트 간 메시징 | 3~5주 | 에이전트 측 지원 필요 | Claude/Codex/Gemini가 zm-mux 메시징 프로토콜을 이해해야 함 |
| 25 | **MCP 서버 내장** | **3~5주** | Rust MCP SDK 성숙도 | MCP SDK가 TypeScript/Python 중심, Rust 생태계 미성숙 |
| 26 | 크로스 모델 MCP 브릿지 | 1~2주 | 3자 의존성 | pal-mcp-server(Node.js) API 변경 위험 |
| 27 | **Review Chain 자동화** | **4~8주** | 높은 복잡도 | 에이전트 상태 감지 + 출력 파싱 + 입력 주입 — 실패 모드 다수 |
| 29 | Vertical Tabs + Git/PR 정보 | 3~5주 | GitHub API 인증 | PR 상태 조회 rate limit, 인증 관리 |
| 30 | SSH 원격 세션 | 4~8주 | 프로토콜 복잡도 | SSH 에지케이스 (키 교환, 호스트 검증, 재연결) |
| 32 | Lua 스크립팅 설정 | 4~8주 (전체) | API 설계 | Lua 임베딩은 쉬우나 좋은 API 설계는 수개월 진화 필요 |

---

## TIER 4: MVP에서 구현 불가 🚫

| # | 기능 | 이유 | 대안 |
|---|------|------|------|
| 28 | **내장 브라우저** | WGPU 렌더러에 WebView 합성이 미해결 과제. cmux/Calyx는 네이티브 Cocoa라 가능, WGPU 기반은 전례 없음. 8~16주+ | 외부 브라우저 실행 (`open`/`start` 명령) |
| 33 | **플러그인/확장 시스템** | 코어 아키텍처 안정 전 플러그인 API 설계는 시기상조. ABI 호환 유지 부담. 8~16주+ | TOML 설정 → Lua 스크립팅 → 플러그인 순차 진화 |
| — | cmux 완전 호환 | cmux는 Swift + macOS 네이티브, Socket API는 호환 가능하나 GUI/렌더링 레벨 호환은 불가 | Socket API 프로토콜 레벨 호환만 추구 |
| — | Warp 수준 AI 통합 | Warp는 자체 GPUI 프레임워크 + 수백명 개발팀. 1~2인 팀으로 동등 수준 불가 | 핵심 기능(멀티 에이전트, 알림, 탭)만 선택적 구현 |

---

## 크리티컬 리스크 3가지

### 🔴 1. Windows isTTY 이슈 (#26244)
- Claude Code의 `process.stdout.isTTY`가 Windows ConPTY에서 항상 falsy
- `teammateMode: "tmux"` 무시됨 → 에이전트 팀 split-pane 불가
- **psmux가 우회법 보유** — 반드시 분석 필요
- **영향**: 이 이슈 미해결 시 zm-mux의 핵심 가치(Windows 에이전트 팀)가 무력화

### 🔴 2. WGPU 텍스트 렌더링 난이도
- GPU 기반 터미널 텍스트 렌더링의 Rust 선례는 **Rio 하나뿐** (4.5k stars)
- WezTerm(19k stars)은 OpenGL 사용 → WGPU보다 성숙
- 글리프 아틀라스, 서브픽셀 렌더링, ClearType(Windows) 처리 필요
- **대안**: OpenGL로 시작 → WGPU로 마이그레이션 (Apple deprecated이지만 아직 작동)

### 🔴 3. tmux 프로토콜 안정성
- Claude Code의 tmux 통합은 내부 구현 세부사항 (공식 API 아님)
- Anthropic이 사용하는 tmux 명령어 변경 시 호환성 깨짐
- **완화책**: Anthropic에 `teammateMode: "zm-mux"` PR 제출 (cmux 선례 있음 — 이슈 #36926)

---

## 현실적 로드맵

### Phase 1: MVP 터미널 (12~16주, 1~2명)

```
Week 1-3:   PTY 레이어 (portable-pty, 프로세스 관리)
Week 4-6:   VT 파싱 (vte 크레이트, 스크롤백)
Week 7-12:  WGPU 렌더러 (텍스트 파이프라인, true color, Unicode)
            └── 막히면 OpenGL 폴백 (WezTerm 패턴)
Week 13-16: Split Pane + Tab 관리
```
**산출물**: Win+Mac에서 Claude Code 실행 가능한 기본 터미널

### Phase 2: AI 에이전트 통합 (8~12주)

```
Week 17-20: tmux 프로토콜 호환 + Claude Code 에이전트 팀
Week 21-23: Socket API (Unix domain socket + named pipe)
Week 24-26: Shift+Enter + 데스크톱 알림
Week 27-28: 에이전트 감지, 상태 알림, 리밸런싱
```
**산출물**: Claude Code가 tmux로 인식, 에이전트 팀 split-pane 동작

### Phase 3: 멀티 에이전트 & 품질 (8~12주)

```
Week 29-32: 폰트 렌더링 (HarfBuzz, 이모지)
Week 33-35: 멀티 에이전트 실행 + Git worktree 격리
Week 36-38: MCP 서버 + Vertical Tabs
Week 39-40: 세션 지속성
```
**산출물**: Claude+Codex+Gemini 동시 실행, 에이전트 간 기본 통신

### Phase 4: Post-1.0 (무기한)

```
에이전트 간 메시징, Review Chain 자동화
크로스 모델 MCP 브릿지
내장 브라우저, SSH 원격, Lua 스크립팅, 플러그인
```

---

## 정직한 결론

| 질문 | 답변 |
|------|------|
| **1~2인 팀으로 가능한가?** | 가능. 단, 6~9개월 필요 (Phase 1+2). Rust 터미널 경험 있으면 더 빠름 |
| **가장 큰 리스크는?** | WGPU 텍스트 렌더링 (OpenGL 폴백으로 완화 가능) |
| **가장 큰 외부 의존성은?** | Windows isTTY 이슈 + Claude Code tmux 프로토콜 안정성 |
| **cmux 수준 달성 가능 시점?** | 12~18개월 (1~2인 풀타임 기준) |
| **Warp 수준 달성 가능한가?** | 불가 (Warp = 수백명 팀 + 수년 개발 + $100M+ 투자) |
| **차별화 포인트는?** | "크로스 플랫폼 + 오픈소스(MIT) + AI 에이전트 전용" 조합은 시장에 부재 |
| **MVP만으로 가치가 있는가?** | **있음**. Win+Mac에서 Claude Code 에이전트 팀 split-pane만 되어도 현재 시장 공백 해결 |

---

## 참고 링크

- [WezTerm 소스 (MIT)](https://github.com/wezterm/wezterm) — 아키텍처 최고 참고
- [Rio 소스 (MIT)](https://github.com/nicholasareed/rio) — WGPU 터미널 유일 선례
- [psmux 소스](https://github.com/psmux/psmux) — Windows tmux 호환 + isTTY 우회
- [portable-pty](https://docs.rs/portable-pty) — 크로스 플랫폼 PTY
- [wgpu 크레이트](https://docs.rs/wgpu) — WebGPU Rust 구현
- [vte 크레이트](https://docs.rs/vte) — VT 파서
- [Claude Code isTTY 이슈 #26244](https://github.com/anthropics/claude-code/issues/26244)
- [cmux teammateMode 이슈 #36926](https://github.com/anthropics/claude-code/issues/36926)

---

*분석일: 2026-04-30*
