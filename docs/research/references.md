# References (서지)

> 취득일: **2026-06-15**. 등급 [V]=1차/공식, [I]=2차/추론 근거. 라이브 값(이슈/버전)은 이후 변동 가능.
> 클론된 repo의 핀 SHA·라이선스는 [05-reference-inventory.md](05-reference-inventory.md).

## 1차/공식 (Primary)

### cmux (STUDY)
- repo + 소스(LICENSE/Sources/CLI/docs): https://github.com/manaflow-ai/cmux  [V]
- 환경변수 docs: https://manaflow-ai-cmux.mintlify.app/automation/environment-variables  [V]
- Claude Code 통합 docs: https://cmux.com/docs/agent-integrations/claude-code-teams  [V]
- 사이트: https://cmux.com/  [V]
- 서브모듈: ghostty(github.com/manaflow-ai/ghostty), bonsplit(github.com/manaflow-ai/bonsplit)  [V .gitmodules]

### Claude Code
- agent teams 공식 docs: https://code.claude.com/docs/en/agent-teams  [V 직접 fetch]
- issue #26572 CustomPaneBackend: https://github.com/anthropics/claude-code/issues/26572 — **open** [V API]
- issue #26244 isTTY 게이트: https://github.com/anthropics/claude-code/issues/26244 — **closed/not_planned** [V API]
- issue #34150 psmux 지원: https://github.com/anthropics/claude-code/issues/34150 — **closed/not_planned** [V API]
- issue #36926 cmux teammateMode: https://github.com/anthropics/claude-code/issues/36926 — **open** [V API]
- issue #27868 Kitty 키보드: https://github.com/anthropics/claude-code/issues/27868 — **closed/duplicate** [V API]

### Windows / ConPTY
- ConPTY 세션 생성: https://learn.microsoft.com/en-us/windows/console/creating-a-pseudoconsole-session  [V]
- ConPTY 소개(블로그/공식): https://devblogs.microsoft.com/commandline/windows-command-line-introducing-the-windows-pseudo-console-conpty/  [V]
- microsoft/terminal ConPTY 샘플: https://github.com/microsoft/terminal/tree/main/samples/ConPTY  [V]

### 레퍼런스 멀티플렉서 (SAFE)
- WezTerm: https://github.com/wezterm/wezterm — MIT [V LICENSE]
- Zellij: https://github.com/zellij-org/zellij — MIT [V LICENSE]
- Alacritty: https://github.com/alacritty/alacritty — Apache-2.0 [V]
- vte: https://github.com/alacritty/vte — Apache-2.0/MIT [V]
- psmux: https://github.com/psmux/psmux — MIT, Rust [V]

### 크레이트 (crates.io / docs.rs, 최신 stable 2026-06-15) [V]
- portable-pty 0.9.0 · alacritty_terminal 0.26.0 · vte 0.15.0 · wgpu 29.0.3 · glyphon 0.11.0 ·
  cosmic-text 0.19.0 · softbuffer 0.4.8 · tiny-skia 0.12.0 · winit 0.30.13 · interprocess 2.4.2
- API: `https://crates.io/api/v1/crates/<name>` , 문서: `https://docs.rs/<name>`

### 포팅 사례 (STUDY/참고)
- cosmic-term (pop-os): https://github.com/pop-os/cosmic-term — GPL-3.0 [V]
- cmux-for-linux (ptrcode, Tauri): https://github.com/cai0baa/cmux-for-linux — GPL-3.0 [V]
- cmux-linux (bradwilson331): https://github.com/bradwilson331/cmux-linux — AGPL-3.0, Swift+Rust [V]
- wmux (amirlehmam): https://github.com/amirlehmam/wmux — MIT, TS/Electron [V]
- wmux (openwong2kim): https://github.com/openwong2kim/wmux — MIT, TS [V]

## 2차 (Secondary — 사실 근거 아닌 맥락용, [I])
- Better Stack cmux 가이드: https://betterstack.com/community/guides/ai/cmux-terminal/
- DEV(arshtechpro) cmux: https://dev.to/arshtechpro/cmux-the-native-macos-terminal-built-for-running-ai-coding-agents-in-parallel-52il
- ice-ice-bear cmux 분석: https://ice-ice-bear.github.io/posts/2026-03-16-cmux-terminal/

> 2차 출처의 주장(Bonsplit, 소켓 명령명, AGPL 등)은 본 검토에서 **1차 출처로 재검증**했고
> 일부는 교정됨([00](00-overview.md) §5).

## 아카이브
- 공식 docs 스냅샷 + 검증 원자료: [`sources/`](sources/_index.md)
