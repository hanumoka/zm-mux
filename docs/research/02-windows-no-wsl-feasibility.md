# 02 — Windows(WSL 없이) + 크로스플랫폼 실현성

> 판정: **요구사항 ②③ 실현 가능 [V]**. WezTerm(MIT)·Zellij(MIT)가 Win/Mac/Linux 네이티브로 이미 입증.
> 크레이트 버전은 crates.io API 2026-06-15 기준 [V]. 범례는 [00](00-overview.md).

## 1. ConPTY — Windows 네이티브 PTY (WSL 불필요의 핵심)

| 항목 | 값 | 출처 |
|------|-----|------|
| 최소 버전 | Windows 10 **1809 (build 17763)** / Server 2019 | learn.microsoft.com [V] |
| API | `CreatePseudoConsole` / `ResizePseudoConsole` / `ClosePseudoConsole` | MS docs + microsoft/terminal samples [V] |
| 인코딩 | UTF-8 | MS docs [V] |
| VT 처리 | ANSI/VT100 지원 (`ENABLE_VIRTUAL_TERMINAL_PROCESSING`) | MS docs [V] |

**알려진 함정** (구현 시 주의):
- 커서 위치 질의(`ESC[6n`) 응답 대기로 hang 가능 (microsoft/terminal #1965) [V]
- 과거 SGR 일부(italics 등) 누락 사례 (alacritty #2554) [V]
- 리사이즈는 `WINDOW_BUFFER_SIZE_EVENT` 적절 전달 필요 [V]
- 자식 CLI의 `isatty()` 판정이 핸들 타입에 의존 → 에이전트 연동의 핵심 이슈([04](04-ai-agent-integration.md)) [I]

→ **WSL 불필요**: ConPTY로 Windows 네이티브에서 인터랙티브 CLI 호스팅 가능. WSL/Cygwin/MSYS2 불요.

## 2. 권장 Rust 크레이트 스택 (Windows 지원 + 최신 버전)

| 역할 | 크레이트 | 최신 stable | Windows? | 출처 |
|------|----------|------------|----------|------|
| PTY 추상화 | **portable-pty** | 0.9.0 | ✅ (ConPTY+POSIX 단일 trait) | crates.io + WezTerm [V] |
| VT 파서 | **vte** | 0.15.0 | ✅ (순수 Rust) | crates.io [V] |
| 터미널 상태/그리드 | **alacritty_terminal** | 0.26.0 | ✅ (msvc 타깃) | crates.io [V] |
| 윈도잉/이벤트 | **winit** | 0.30.13 | ✅ (Tier 1) | crates.io [V] |
| GPU 렌더 | **wgpu** | 29.0.3 | ✅ (Windows 기본 DX12) | crates.io [V] |
| 글리프/텍스트 렌더 | **glyphon** | 0.11.0 | ✅ (wgpu 기반) | crates.io [V] |
| 텍스트 셰이핑 | **cosmic-text** | 0.19.0 | ✅ (HarfRust) | crates.io [V] |
| CPU 폴백(픽셀) | **softbuffer** | 0.4.8 | ✅ | crates.io [V] |
| CPU 폴백(래스터) | **tiny-skia** | 0.12.0 | ✅ | crates.io [V] |
| 로컬 IPC | **interprocess** | 2.4.2 | ✅ (named pipe↔UDS 추상화) | crates.io [V] |

> ⚠️ **버전 lockstep 미확정 [?]**: `glyphon 0.11` ↔ `wgpu 29` ↔ `cosmic-text 0.19` 정합은
> 구현 플랜에서 실제 `cargo check`로 확정해야 한다(glyphon이 특정 wgpu/cosmic-text 버전에 핀될 수 있음).
> portable-pty 0.9.0은 2025-02 갱신으로 비교적 정체 → ConPTY 동작은 WezTerm 본체(`pty` 모듈)도 참고.

## 3. 검증 — 네이티브 크로스플랫폼 멀티플렉서 선례

| 프로젝트 | repo | 라이선스 | Windows 네이티브(WSL X)? | 출처 |
|----------|------|----------|:---:|------|
| **WezTerm** | wezterm/wezterm | MIT [V] | ✅ | LICENSE 직독 + Windows 설치본 [V] |
| **Zellij** | zellij-org/zellij | MIT [V] | ✅ (v0.44+) | LICENSE + 1차 조사 [V] |
| **Alacritty** | alacritty/alacritty | Apache-2.0 [V] | ✅ (멀티플렉서 아님) | repo [V] |
| **psmux** | psmux/psmux | MIT [V] | ✅ (Rust+ConPTY tmux) | clone Cargo.toml [V] |
| COSMIC Terminal | pop-os/cosmic-term | GPL-3.0 [V] | ❌ Linux 전용 | repo [V] |

→ **결론**: MIT 라이선스 + Windows 네이티브 멀티플렉서가 **둘 이상 실재**(WezTerm, Zellij).
zm-mux의 ②③ 요구는 *이미 입증된 영역*이며, 핵심 학습 대상은 WezTerm(아키텍처)·psmux(tmux 호환).

## 4. Windows 함정 & IPC 선택

### isatty / isTTY (★ 에이전트 연동 최대 변수)
- ConPTY 자식의 `isatty()`는 핸들 타입에 의존. AI 에이전트(Claude Code 등)가 `process.stdout.isTTY`로
  인터랙티브 여부를 판단 → Windows에서 `undefined`가 되며 **분할패널 모드 차단**의 직접 원인.
  상세·우회는 [04](04-ai-agent-integration.md) (#26244). [V issue]

### 로컬 IPC: named pipe vs Unix domain socket
| 방식 | Windows | Unix | 크레이트 |
|------|:---:|:---:|---------|
| named pipe | ✅ | — | interprocess (os::windows::named_pipe) |
| Unix domain socket | (Win10 1803+ AF_UNIX 일부) | ✅ | interprocess (local_socket) |
| **크로스플랫폼 local socket** | ✅(named pipe) | ✅(UDS) | **interprocess::local_socket** [V] |

→ cmux의 `/tmp/cmux.sock`(UDS 고정)을 zm-mux는 `interprocess` local-socket 추상화로 양 OS 모두 커버.

### ANSI/VT
- ConPTY `SetConsoleMode` 플래그에 따라 VT 자동처리/raw 통과가 갈림 — 불일치 시 인코딩 깨짐 [V].

## 5. 종합

| 요구 | 판정 | 한 줄 근거 |
|------|------|-----------|
| ③ WSL 없이 Windows | **가능 [V]** | ConPTY + portable-pty |
| ② 크로스플랫폼 | **가능 [V]** | WezTerm/Zellij가 동일 스택으로 Win/Mac/Linux 입증 |
| (성능) | **GPU 가속 가능 [V]** | wgpu DX12/Metal/Vulkan + glyphon, CPU 폴백 softbuffer/tiny-skia |

남은 불확실성: 크레이트 버전 lockstep [?] (구현 시 cargo로 확정), 에이전트 isTTY 게이트(정책 이슈, [04]).
