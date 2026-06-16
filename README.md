# zm-mux

Cross-platform (Windows-without-WSL + macOS) **AI-agent terminal multiplexer** — a
clean-room, native-Rust answer to the macOS-only **cmux**. GPU 렌더(wgpu+glyphon),
분할/탭, 자동화 소켓, tmux 호환 shim(Claude Code 에이전트 연동 트랙 A)을 제공한다.

> **Status: Phase 0–3 구현 완료(검증됨).** 설계·실현성 검토는 [`docs/research/`](docs/research/00-overview.md)
> (00~06), Phase 0 실측은 [07](docs/research/07-poc-conpty-istty-results.md), **구현 현황은
> [08](docs/research/08-implementation-status.md)**, 설정은 [`docs/configuration.md`](docs/configuration.md).

## 요구사항 (목표)

1. **Windows**에서 cmux와 *거의 동등*한 프로그램 (cmux는 macOS 전용).
2. **크로스 플랫폼** (Windows + macOS, Linux는 보너스).
3. Windows에서 **WSL 없이** 동작 (네이티브 ConPTY).

## 빌드 / 실행

```bash
cargo build --workspace      # 전체 빌드
cargo run -p zm-app          # 앱 실행 (GPU 터미널 창)
cargo test --workspace       # 테스트(22개)
```

요구: Rust 1.95+, Windows 10 1809+(ConPTY) 또는 macOS/Linux, GPU(DX12/Metal/Vulkan).
산출 바이너리: `zm-app`(앱), `zm`(자동화 CLI), `tmux`(tmux 호환 shim).

## 기능

- **멀티플렉서**: 분할(좌우/상하)·탭·방향 포커스·**줌**·마우스(클릭 포커스, divider 드래그 리사이즈, 휠 스크롤백). 탭바 + 활성 pane 프레임.
- **렌더**: wgpu+glyphon, 셀별 전/배경색, 블록/빔/언더라인 커서, sRGB 정합, 시스템 모노폰트.
- **자동화 소켓**(`zm` CLI): `list-panes`/`split`/`new-tab`/`focus`/`send-keys`/`capture-pane`/`kill-pane`.
- **에이전트 연동(트랙 A)**: pane에 `TMUX`/`TMUX_PANE`/`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` 주입 + PATH 앞 `tmux` shim → `tmux split-window`/`send-keys`를 zm-mux로 변환. (ConPTY 자식 isTTY=true 실측, [07])
- **알림**: OSC 9/777 → 토스트(best-effort) + 작업표시줄 주의 환기. **Shift+Enter**: Kitty CSI-u.
- **설정**: `%APPDATA%\zm-mux\config.toml` — 폰트/색/스크롤백/셸/단축키/에이전트.

기본 단축키·자동화 명령·설정 스키마는 [`docs/configuration.md`](docs/configuration.md), [08](docs/research/08-implementation-status.md) 참조.

## 크레이트

```
zm-core   공유 타입 + 설정(TOML)        zm-mux    분할 트리/탭/포커스/줌(모델)
zm-pty    PTY(ConPTY/POSIX) 래퍼        zm-ipc    로컬 소켓 + 프로토콜 + `zm` CLI
zm-term   VT(alacritty)+팔레트+OSC      zm-agent  tmux shim + 에이전트 env
zm-render GPU(wgpu+glyphon) 렌더        zm-app    winit 앱 배선
zm-probe  isTTY 검증 하네스(Phase 0)
```

## 보류(문서화됨)

CPU 폴백(softbuffer) — GPU 실패 시 graceful 종료로 처리, 전체 CPU 렌더러는 보류.
macOS 런타임(코드는 크로스플랫폼이나 Windows에서만 실측). claude teammate 라이브 실측(사용자).
상세는 [08 §4](docs/research/08-implementation-status.md).

## 라이선스

**clean-room MIT/Apache-2.0.** cmux는 GPL-3.0-or-later OR Commercial 듀얼 라이선스로,
copyleft 참고 소스는 *이해용 분석만* 하고 코드/텍스트를 복사하지 않는다. 구현은 permissive
Rust 크레이트로 빌드. 상세는 [`docs/research/05-reference-inventory.md`](docs/research/05-reference-inventory.md).
