# 크리티컬 리스크 최신 업데이트

## 개요

docs/09 구현 가능성 분석에서 식별된 3대 크리티컬 리스크에 대해 최신 자료를 수집하여 재평가한다. **결론: 3개 리스크 모두 완화 경로가 확인되었으며, 일부는 리스크 등급이 하향 조정된다.**

---

## 리스크 1: Windows isTTY 이슈 (#26244)

### 원래 평가
- **등급**: CRITICAL (해결 불가 시 프로젝트 핵심 가치 무력화)
- **내용**: Claude Code의 `process.stdout.isTTY`가 Windows Bun SFE에서 항상 `undefined` → `teammateMode: "tmux"` 무시

### 최신 조사 결과

**근본 원인 확인:**
```
Bun SFE on Windows → process.stdout.isTTY = undefined
→ isInteractive = false
→ isInProcessEnabled() = true (무조건)
→ teammateMode: "tmux" 설정 무시
```

**psmux의 우회법 확인:**
- psmux는 ConPTY로 패인 생성 시 `TMUX` 환경변수를 설정 (+ TMUX_PANE, CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS, PSMUX_CLAUDE_TEAMMATE_MODE 등 6개)
- psmux는 `--teammate-mode tmux`를 자동 주입하는 PowerShell `claude` 래퍼 함수 제공
- **정정 (2026-05-02, [docs/12](./12-istty-workaround.md) 참조)**: 이전 표기 "isTTY 보다 TMUX env 가 우선" 은 **틀림**. issue #26244 OP 가 직접 검증 — psmux 환경에서도 teammates 가 in-process 로 fallback 됨. **진짜 우회 메커니즘은 PowerShell 래퍼의 CLI flag (`--teammate-mode tmux`) 주입**. 환경변수는 tmux CLI 명령(`tmux -V`/`split-window`) 라우팅용 보조 수단.

**이슈 #26244 제안 수정:**
```javascript
// 현재: isTTY 게이트가 모든 것을 차단
if (!isInteractive) return true; // 강제 in-process

// 제안: 명시적 설정이 isTTY보다 우선
if (teammateMode === "tmux") return false; // tmux 강제
```

**실제 작동 상태:**
- psmux + Claude Code: **작동 확인** (이슈 #34150에서 보고)
- 단, 경로 포맷 이슈 발생: Unix 스타일 경로가 Windows PowerShell 스타일이 아님 ([#42848](https://github.com/anthropics/claude-code/issues/42848))

### 🟡 재평가: CRITICAL → HIGH-MEDIUM
- psmux가 우회법을 검증함 → zm-mux도 동일 접근법 적용 가능
- TMUX 환경변수 설정 + `--teammate-mode tmux` 자동 주입으로 해결
- 경로 포맷 이슈는 추가 핸들링 필요하지만 기술적 해결 가능

---

## 리스크 2: WGPU 텍스트 렌더링 난이도

### 원래 평가
- **등급**: CRITICAL (Rio만이 유일한 선례, 4.5k stars)
- **내용**: GPU 기반 터미널 텍스트 렌더링을 Rust+WGPU로 구현한 성숙한 사례 부족

### 최신 조사 결과

**COSMIC Terminal 발견 (System76):**
- **핵심**: COSMIC Desktop의 터미널 — **WGPU + glyphon + cosmic-text** 조합으로 구현
- **GitHub**: [pop-os/cosmic-term](https://github.com/pop-os/cosmic-term)
- **아키텍처**:
  - 터미널 백엔드: `alacritty_terminal` 크레이트 재사용
  - 텍스트 셰이핑: `cosmic-text` (HarfBuzz 기반, 리거처 지원)
  - GPU 렌더링: `glyphon` (wgpu용 2D 텍스트 렌더러)
  - 글리프 아틀라스: `etagere` (아틀라스 패킹)
  - **GPU 불가 시 폴백**: `softbuffer` + `tiny-skia` (CPU 렌더링)
- **프로덕션 사용**: System76 Linux 데스크톱에서 실사용 중

**glyphon 크레이트 성숙도:**
- `glyphon` = cosmic-text + etagere + wgpu 통합 텍스트 렌더러
- 글리프 셰이핑 → 아틀라스 패킹 → GPU 렌더링 파이프라인을 **하나의 크레이트**로 제공
- [crates.io](https://crates.io/crates/glyphon)에 공개, 활발히 유지 관리

**Rio 최근 개선 (2026):**
- wgpu v0.28 + Rust v1.92 업데이트
- **GPU 메모리 사용량 83% 감소**
- Sugarloaf 렌더링 엔진 전면 재작성 (동일 렌더 패스 사용, 레이아웃 변경 시에만 재계산)

**WGPU 터미널 렌더링 실현 경로 확립:**
```
alacritty_terminal (VT 파싱 + 터미널 상태)
    ↓
cosmic-text (텍스트 셰이핑, HarfBuzz, 리거처)
    ↓
glyphon (글리프 → 아틀라스 → wgpu 렌더링)
    ↓
wgpu (Metal/DX12/Vulkan 자동 선택)
    ↓
softbuffer + tiny-skia (GPU 불가 시 폴백)
```

### 🟢 재평가: CRITICAL → MEDIUM
- Rio 외에 **COSMIC Terminal**이 WGPU 터미널의 두 번째 프로덕션 선례
- `glyphon` + `cosmic-text` 조합으로 텍스트 렌더링 파이프라인이 크레이트 수준에서 해결됨
- GPU 불가 시 `softbuffer` 폴백으로 안전망 확보
- `alacritty_terminal` 크레이트로 VT 파싱을 처음부터 구현할 필요 없음

---

## 리스크 3: tmux 프로토콜 안정성

### 원래 평가
- **등급**: CRITICAL (Claude Code 내부 구현 변경 시 호환 깨짐)
- **내용**: Claude Code의 tmux 통합은 내부 구현 세부사항, 공식 API 아님

### 최신 조사 결과

**CustomPaneBackend 프로토콜 제안 ([#26572](https://github.com/anthropics/claude-code/issues/26572)):**

이것이 **가장 중요한 발견**이다. KILD 개발자(@Wirasm)가 tmux CLI 의존을 대체하는 공식 프로토콜을 제안했다.

**프로토콜 사양:**
- **전송**: JSON-RPC 2.0 over NDJSON
- **연결**: 환경변수로 지정
  - `CLAUDE_PANE_BACKEND=/path/to/binary` (on-demand spawn)
  - `CLAUDE_PANE_BACKEND_SOCKET=/path/to/server.sock` (pre-running 서버)

**7개 필수 오퍼레이션:**

| 메서드 | 대체하는 tmux 명령 | 설명 |
|--------|-------------------|------|
| `initialize` | `display-message -p "#{pane_id}"` | 핸드셰이크, `self_context_id` 반환 |
| `spawn_agent` | `split-window` | 에이전트 프로세스 시작 (`argv[]` 직접 전달, 셸 문자열 아님) |
| `write` | `send-keys` | stdin에 데이터 전송 (base64) |
| `capture` | `capture-pane` | 스크롤백 읽기 (선택적) |
| `kill` | `kill-pane` | 컨텍스트 종료 |
| `list` | `list-panes` | 활성 컨텍스트 열거 |
| `context_exited` | (없음) | 푸시 이벤트: 컨텍스트 종료 알림 |

**해결하는 기존 이슈들:**
- #23615: 레이스 컨디션 (split-window + send-keys 비동기) → 영속 백엔드가 직렬화
- #23572: 사일런트 폴백 → 명시적 `CLAUDE_PANE_BACKEND` 환경변수
- #24189 (Ghostty), #24122 (Zellij), #23574 (WezTerm): 터미널별 차단 → 클린 프로토콜

**현재 상태:**
- 이슈 상태: **OPEN** (제안 단계)
- Anthropic 공식 응답: 미확인
- KILD에서 참조 구현 준비됨 (이미 tmux shim으로 ~20개 명령 인터셉트 중)

**zm-mux에 대한 시사점:**
- 이 프로토콜이 채택되면 zm-mux는 **tmux 호환 없이 직접 Claude Code와 통합** 가능
- JSON-RPC 2.0은 Rust에서 쉽게 구현 가능 (`jsonrpc-core` 또는 `tower-lsp` 크레이트)
- **투트랙 전략 가능**: tmux 호환(Phase 2)으로 즉시 동작 + CustomPaneBackend 구현(Phase 3)으로 공식 통합

### 🟡 재평가: CRITICAL → MEDIUM
- CustomPaneBackend 제안이 채택되면 tmux 프로토콜 의존성 자체가 사라짐
- 미채택이어도 tmux 호환 방식(psmux 검증)이 현재 동작 중
- **투트랙 전략**으로 어느 경우든 대응 가능

---

## 추가 발견: Rust MCP SDK 성숙도

### 원래 평가
- **등급**: MEDIUM-HIGH (Rust MCP SDK 미성숙)

### 최신 조사 결과

**공식 SDK 확인: `rmcp`**
- [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk) — **공식 Rust SDK**
- crates.io 다운로드: **470만+** (2026년 초 기준)
- 버전: 0.16.0
- 매크로 기반 API로 MCP 서버 구축 지원
- 서버 + 클라이언트 + 다양한 전송(stdio, SSE) 지원

**대안 SDK:**
- `rust-mcp-sdk`: 비동기 SDK, 최신 MCP 프로토콜(2025-11-25) 완전 구현
- `mcp-sdk-rs`: 또 다른 구현체

### 🟢 재평가: MEDIUM-HIGH → LOW-MEDIUM
- 공식 Rust MCP SDK가 470만+ 다운로드로 충분히 성숙
- zm-mux MCP 서버 내장은 기술적으로 문제없음

---

## 추가 발견: alacritty_terminal 크레이트 재사용

COSMIC Terminal이 `alacritty_terminal` 크레이트를 VT 파싱/터미널 상태 관리에 재사용하고 있다. 이는 zm-mux도 **VT 에뮬레이션을 처음부터 구현할 필요 없음**을 의미한다.

```toml
[dependencies]
alacritty_terminal = "0.24"  # VT 파싱 + 터미널 상태
cosmic-text = "0.12"         # 텍스트 셰이핑 (HarfBuzz)
glyphon = "0.6"              # wgpu 텍스트 렌더링
wgpu = "0.28"                # GPU 추상화
portable-pty = "0.8"         # 크로스 플랫폼 PTY
```

이 5개 크레이트만으로 터미널의 핵심 기능(PTY → VT 파싱 → 텍스트 셰이핑 → GPU 렌더링)이 커버된다.

---

## 리스크 재평가 요약

| 리스크 | 원래 등급 | 새 등급 | 변경 사유 |
|--------|---------|--------|----------|
| Windows isTTY (#26244) | 🔴 CRITICAL | 🟡 HIGH-MEDIUM | psmux 우회법 확인, TMUX 환경변수 방식 |
| WGPU 텍스트 렌더링 | 🔴 CRITICAL | 🟢 MEDIUM | COSMIC Terminal 선례 + glyphon/cosmic-text 크레이트 |
| tmux 프로토콜 안정성 | 🔴 CRITICAL | 🟡 MEDIUM | CustomPaneBackend 제안(#26572) + 투트랙 전략 |
| Rust MCP SDK 성숙도 | 🟡 MEDIUM-HIGH | 🟢 LOW-MEDIUM | 공식 SDK rmcp 470만+ 다운로드 |

**결론**: 3대 CRITICAL 리스크가 모두 MEDIUM 이하로 완화. 특히 COSMIC Terminal의 아키텍처 패턴(`alacritty_terminal` + `glyphon` + `wgpu`)이 zm-mux의 기술 스택을 사실상 확정시킨다.

---

## 업데이트된 핵심 기술 스택

```
zm-mux 기술 스택 (확정)
├── Rust                        # ARCH-06
├── portable-pty                # 크로스 플랫폼 PTY (ARCH-02)
├── alacritty_terminal          # VT 파싱 + 터미널 상태 [NEW]
├── cosmic-text                 # 텍스트 셰이핑 (HarfBuzz) [NEW]
├── glyphon                     # wgpu 텍스트 렌더링 [NEW]
├── wgpu                        # GPU 렌더링 (ARCH-03)
├── softbuffer + tiny-skia      # GPU 폴백 [NEW]
├── winit                       # 윈도우 관리
├── tokio                       # 비동기 런타임
├── rmcp                        # MCP 서버 SDK [NEW]
└── serde_json + jsonrpc-core   # Socket API + CustomPaneBackend
```

---

## 참고 링크

- [Claude Code isTTY 이슈 #26244](https://github.com/anthropics/claude-code/issues/26244)
- [psmux Claude Code 문서](https://github.com/psmux/psmux/blob/master/docs/claude-code.md)
- [psmux 경로 이슈 #42848](https://github.com/anthropics/claude-code/issues/42848)
- [**CustomPaneBackend 제안 #26572**](https://github.com/anthropics/claude-code/issues/26572) ← 가장 중요
- [COSMIC Terminal](https://github.com/pop-os/cosmic-term)
- [glyphon 크레이트](https://crates.io/crates/glyphon)
- [cosmic-text 크레이트](https://crates.io/crates/cosmic-text)
- [Rio Terminal GitHub](https://github.com/raphamorim/rio)
- [rmcp 공식 SDK](https://github.com/modelcontextprotocol/rust-sdk)
- [Ghostty teammateMode 요청 #24189](https://github.com/anthropics/claude-code/issues/24189)
- [WezTerm teammateMode 요청 #23574](https://github.com/anthropics/claude-code/issues/23574)

---

*조사일: 2026-04-30*
