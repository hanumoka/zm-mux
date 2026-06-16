# 07 — PoC 실측: ConPTY 자식 isTTY (R1 게이트)

> Phase 0 PoC의 **최우선 검증(R1)** 결과. zm-mux의 ConPTY 자식 안에서 `isatty()`/
> `process.stdout.isTTY`가 true가 되는지 **실측**했다. 범례 [00](00-overview.md): [V]=1차/실측, [I]=추론, [?]=미확인.
>
> 한 줄 결론: **R1 게이트 통과 — zm-mux ConPTY 직속 자식의 isTTY = true (12/12 셀, 셸 1홉 포함).**
> Claude Code 분할패널 차단 메커니즘(#26244의 `isTTY=undefined`)은 **zm-mux 환경에서는 재현되지 않음**.
> → docs [04](04-ai-agent-integration.md) **트랙 A(자체 PTY로 진짜 TTY 부여)** 진행 가능. [V]

## 1. 환경 [V]

| 항목 | 값 |
|------|-----|
| OS | Windows 11, build **10.0.26200** (WSL 미사용) |
| Rust | rustc/cargo **1.95.0** (x86_64-pc-windows-msvc) |
| PTY 크레이트 | **portable-pty 0.9.0** (crates.io; `index.crates.io-...`) |
| ConPTY 플래그(실제 컴파일) | `INHERIT_CURSOR \| RESIZE_QUIRK \| WIN32_INPUT_MODE` (PASSTHROUGH_MODE **미설정**) — registry src `portable-pty-0.9.0/src/win/psuedocon.rs:87-89` 직독 [V] |
| 프로브 도구 | node **v24.14.1**, python **3.13.12**, claude **2.1.177**, pwsh **7.6.2**, powershell 5.1 |
| 측정일 | 2026-06-15 |
| repo | HEAD `08b19fc` + Phase 0 작업트리(미커밋) |
| 재현 | `cargo build -p zm-probe && cargo run -p zm-probe --bin zm-probe-harness` |

## 2. 프로브 세트 & 하네스

크레이트 `crates/zm-probe`(headless, GPU 비의존, CI 가능):
- **`zm-probe`** — ConPTY 자식으로 실행되어 `std::io::IsTerminal`로 stdout/stdin/stderr TTY 여부를 한 줄 마커로 출력.
- **`zm-probe-harness`** — node/python/rust 프로브를 **(직접 / cmd / pwsh / powershell)** 런치 경로로 zm-pty(ConPTY) 안에서 spawn, 자식이 본 isTTY를 수집해 표로 출력. node-direct stdout=true면 exit 0(CI 게이트).
- 마커: `ZMPROBE lang=.. out/in/err=true|false END` (+ node `ZMPROBE_RAW rawout=` 로 `false` vs `undefined`(#26244 증상) 구분). cols=200으로 우측 줄바꿈 회피.
- node/python 프로브는 임시 파일(`%TEMP%\zm_probe.{js,py}`)로 기록해 `node <file>`로 실행(.js 직접 실행 회피).

## 3. 결과 매트릭스 [V]

stock portable-pty 0.9.0 플래그 기준. **12/12 PASS, 전부 `out=in=err=true`.**

| lang | launch | stdout | stdin | stderr | raw(node) | status |
|------|--------|:---:|:---:|:---:|:---:|:---:|
| node | direct | true | true | true | **true** | PASS |
| node | cmd `/c` | true | true | true | true | PASS |
| node | pwsh `-Command` | true | true | true | true | PASS |
| node | powershell `-Command` | true | true | true | true | PASS |
| py | direct | true | true | true | – | PASS |
| py | cmd / pwsh / powershell | true | true | true | – | PASS |
| rust | direct | true | true | true | – | PASS |
| rust | cmd / pwsh / powershell | true | true | true | – | PASS |

> **node-direct stdout.isTTY = `true` (raw=`true`, undefined 아님)** = #26244가 말하는 게이트의 직접 통과.
> 셸 1홉(cmd/pwsh/powershell)을 거쳐도 isTTY가 유지됨 → **셸 런치 제약 없음**.

## 4. ConPTY 함정 실측 (구현 시 필수 주의) [V]

PoC 과정에서 두 함정을 실측·교정했다. **zm-app(셸 호스팅)에도 동일하게 적용해야 한다.**

1. **conin(writer)을 자식 수명 동안 닫지 말 것.**
   spawn 직후 writer를 drop(stdin EOF)하면 ConPTY가 close 이벤트를 자식에 보내
   자식이 **`STATUS_CONTROL_C_EXIT` (exit code `3221225786` = `0xC000013A`)** 로 즉시 죽고
   **출력 전에 종료**된다. → writer는 자식 종료 후(또는 EOF 후)에 drop.
2. **`ESC[6n`(커서 위치 질의, DSR)에 응답할 것.**
   ConPTY는 핸드셰이크로 `\x1b[6n`을 보내고 conin 응답을 기다린다. 응답하지 않으면
   `ClosePseudoConsole`/리더가 **hang**한다(docs [02](02-windows-no-wsl-feasibility.md) §1의 그 함정 [V]).
   → 하네스는 출력 스트림에서 `\x1b[6n`을 감지하면 conin으로 CPR `\x1b[1;1R`을 회신.
   zm-app에서는 alacritty `Term`이 DSR을 `EventListener`의 `PtyWrite` 이벤트로 넘기므로,
   **리스너가 PtyWrite를 PTY writer로 포워딩**해야 한다(zm-app 구현 시 NoopListener로는 불가).

> 두 함정을 모두 처리하기 전에는 자식 출력이 ConPTY 시작 시퀀스(`[6n[?9001h…[2J…]0;<title>…[?25h`)만
> 87바이트 캡처되고 본문이 비는 증상이 난다(실측). 정석 패턴은 reference/wezterm/pty/examples/whoami.rs + 위 보강.

## 5. 결론 → R1 & 트랙 판정 [V]

- **R1(docs [06](06-feasibility-and-roadmap.md) §2) 해소**: "ConPTY 자식 isTTY=true 인가?" → **예(실측, 12/12)**.
  리스크 R1의 가능성/영향 재평가: isTTY 축에서는 **차단 없음**.
- **docs [04](04-ai-agent-integration.md) §5 트랙 선택**: **트랙 A 진행 가능**. zm-mux는 ConPTY로 진짜 TTY를
  부여하므로 wmux(Electron)·브라우저 PTY의 한계와 달리 isTTY를 충족한다([04] §5의 [?] 가설을 [V]로 확정).
- **단, 범위 한정**: 본 측정은 **isTTY 불리언**까지다. Claude Code가 실제로 분할/teammate 백엔드를
  선택하는지(`teammateMode`, 가짜 `TMUX`/`TMUX_PANE`, `--teammate-mode tmux`, `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`)는
  **행동 관찰**이 필요하며 다음 단계(수동, claude 로그인/비용 발생)로 미룬다. isTTY=true는 그 **필요조건**을 충족.

## 6. 후속 액션

1. (Phase 3) Claude Code teammate 행동 관찰: ConPTY 안에서 `claude --debug-file`로
   `isInProcessEnabled:false`/tmux 백엔드 시도 여부 확인(수동). 본 doc에 결과 추가.
2. zm-app 구현 시 §4의 두 함정 반영(conin 유지 + DSR PtyWrite 포워딩).
3. **macOS 재측정(보류)**: 동일 하네스를 Mac에서 실행해 POSIX PTY 자식 isatty=true 확인 후 표에 행 추가.
4. (선택) 플래그 매트릭스: 현재 stock 플래그로 true이므로 불필요. WIN32_INPUT_MODE/PASSTHROUGH 영향 분석은
   isTTY-중립으로 예상되어 미실시.

---

## 부록 A. R2 — 크레이트 버전 lockstep 실측 [V]

`cargo check --workspace` 통과(2026-06-15, exit 0, 경고 0). docs/02 §2의 [?]를 [V]로 확정.
핵심 수정: docs/02가 적은 cosmic-text **0.19**는 glyphon 0.11과 비정합 → glyphon이 동반하는 **0.18** 사용
(`glyphon::cosmic_text` 재노출). cosmic-text 직접 의존 안 함.

| 크레이트 | 확정 버전 | 비고 |
|------|------|------|
| portable-pty | 0.9.0 | ConPTY 플래그 INHERIT_CURSOR\|RESIZE_QUIRK\|WIN32_INPUT_MODE |
| alacritty_terminal | 0.26.0 | vte 0.15.0 동반(`alacritty_terminal::vte::ansi`) |
| vte | 0.15.0 | `Color::{Named,Spec,Indexed}`, `Processor` |
| winit | 0.30.13 | ApplicationHandler |
| wgpu | 29.0.3 | DX12 기본(Windows). naga 29.0.3 |
| glyphon | 0.11.0 | → wgpu ^29 + cosmic-text ^0.18 |
| cosmic-text | 0.18.2 | glyphon 경유(직접 의존 X) |
| raw-window-handle | **0.6.2 (단일)** | winit0.30 ↔ wgpu29 정합 — R3 분기 해소 |

## 부록 B. Phase 0 PoC 런타임 검증 [V]

- 워크스페이스: 8 크레이트(zm-core/pty/term/render/app/probe + zm-mux/ipc/agent 스텁) 스캐폴드.
- 단위 테스트: zm-core 2 / zm-pty 1 / zm-term 3 통과.
- **zm-app 실행(Windows, WSL 없이)**: wgpu 디바이스 초기화 → winit 창 → ConPTY로 `cmd.exe` 자식 스폰 →
  이벤트 루프 가동(패닉/렌더 오류 없음). 셸 호스팅·렌더 파이프라인 런타임 동작 확인.
- 렌더(현재): solid-rect 파이프라인(전체 배경 + **셀별 배경색** + 빔/언더라인 커서)
  + glyphon 텍스트(셀별 전경색) + **블록/할로 커서는 셀 색 반전 베이크**. sRGB 정합(rect는 CPU에서 linear 변환).
  **남은 Phase 1: CPU 폴백(softbuffer)·탭/분할/레이아웃·TOML 설정·스크롤백.**
- macOS: 코드는 크로스플랫폼 유지, 실행 검증은 보류(추후 Mac에서 `cargo run -p zm-app` / `zm-probe-harness`).
