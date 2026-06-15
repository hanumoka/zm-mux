# 검증 원자료 — 2026-06-15

> 1차 출처 직접 질의 결과 원본. 재현 명령 포함. 이 데이터가 분석 문서의 [V] 주장 근거.

## A. GitHub 저장소 (api.github.com/repos/<r>)

```
repo                          license(spdx)  lang         archived  pushed
manaflow-ai/cmux              NOASSERTION*   Swift        false     2026-06-15
amirlehmam/wmux               MIT            TypeScript   false     2026-06-11
openwong2kim/wmux             MIT            TypeScript   false     2026-06-15
psmux/psmux                   MIT            PowerShell** false     2026-06-13
wezterm/wezterm               NOASSERTION*   Rust         false     2026-06-14
zellij-org/zellij             MIT            Rust         false     2026-06-10
pop-os/cosmic-term            GPL-3.0        Rust         false     2026-06-12
alacritty/alacritty           Apache-2.0     Rust         false     2026-06-02
cai0baa/cmux-for-linux        GPL-3.0        TypeScript   false     2026-03-23
bradwilson331/cmux-linux      AGPL-3.0       Swift        false     2026-03-31
```
- `*` NOASSERTION = GitHub 자동탐지 실패 → **LICENSE 파일 직독으로 확정**(B 참조):
  cmux = dual GPL-3.0-or-later OR Commercial / wezterm = MIT.
- `**` GitHub "language=PowerShell" 오탐 → **클론 직독: psmux = Rust**(Cargo.toml+crates+rust-toolchain.toml).

## B. LICENSE 파일 직독 (클론본 head)

```
cmux/LICENSE              : "dual-licensed: 1) GPL-3.0-or-later  2) Commercial (founders@manaflow.com)"
wezterm/LICENSE.md        : "MIT License ... Wez Furlong"
psmux/LICENSE             : "MIT License ... 2025 Josh"
wmux-amirlehmam/LICENSE   : "MIT License ... 2025-2026 Amir Lehmam"
wmux-openwong2kim/LICENSE : "MIT License ... 2025 openwong2kim"
cmux-linux/LICENSE        : "GNU AFFERO GENERAL PUBLIC LICENSE v3"
cmux-for-linux/LICENSE    : "GNU GENERAL PUBLIC LICENSE v3"
cosmic-term/LICENSE       : "GNU GENERAL PUBLIC LICENSE v3"
alacritty/LICENSE-APACHE  : Apache-2.0
vte/LICENSE-APACHE        : Apache-2.0 (+MIT 통상 듀얼)
```

## C. Claude Code 이슈 상태 (api.github.com/repos/anthropics/claude-code/issues/<n>)

```
#26572  open    reason=None         Proposal: CustomPaneBackend protocol — decouple agent teams from tmux
#26244  closed  reason=not_planned  Split-pane agent teams blocked on Windows by isTTY gate ...
#34150  closed  reason=not_planned  Support tmux agent teams on Windows via psmux ...
#36926  open    reason=None         Support cmux as a teammateMode backend for agent teams
#27868  closed  reason=duplicate    Kitty keyboard protocol detection ignores KITTY_WINDOW_ID ...
```

## D. 크레이트 최신 버전 (crates.io/api/v1/crates/<c>)

```
portable-pty         stable=0.9.0      updated=2025-02-11
alacritty_terminal   stable=0.26.0     updated=2026-04-06
vte                  stable=0.15.0     updated=2025-02-02
wgpu                 stable=29.0.3     updated=2026-05-02
glyphon              stable=0.11.0     updated=2026-04-13
cosmic-text          stable=0.19.0     updated=2026-04-22
softbuffer           stable=0.4.8      updated=2025-12-13
tiny-skia            stable=0.12.0     updated=2026-02-02
winit                stable=0.30.13    (max=0.31.0-beta.2)
interprocess         stable=2.4.2      updated=2026-04-19
```

## E. cmux 소스 grep (reference/cmux @ 48c9160)

- `.gitmodules`: ghostty(manaflow-ai/ghostty), vendor/bonsplit(manaflow-ai/bonsplit), homebrew-cmux.
- 소켓/tmux 토큰(CLI/Sources): `/tmp/cmux`, `cmux.sock`, `new-session new-window split-window
  send-keys send-key read-screen capture-pane list-panes select-pane kill-pane notify`.
- tmux-shim 소스: `CLI/CMUXCLI+TmuxCompatSupport.swift`, `CMUXCLI+TmuxCompatHUDSupport.swift`.
- env(40+ `CMUX_*`): CMUX_BIN, CMUX_AGENT_SESSION_ID, CMUX_AGENT_LAUNCH_{EXECUTABLE,CWD,ARGV_B,KIND},
  CMUX_ALLOW_SOCKET_OVERRIDE, CMUX_CLAUDE_TEAMS_CMUX_BIN, CMUX_CLAUDE_WRAPPER_SHIM, CMUX_CLAUDE_PID,
  CMUX_CODEX_SESSION_ID, CMUX_API_BASE_URL, ... (+ 공식 docs: CMUX_WORKSPACE_ID/SURFACE_ID/SOCKET_PATH/SOCKET_MODE).
- top-level: cmux.xcodeproj/xcworkspace, *.entitlements, cmux-Bridging-Header.h, Sources/, CLI/,
  ghostty/(+ghostty.h), vendor/bonsplit, web/ webviews/ workers/ (+package.json/bun.lock/biome.json), ios/.

## 재현 명령

```bash
# 저장소 라이선스/언어
curl -s -H "User-Agent: x" https://api.github.com/repos/<owner>/<repo>
# 이슈 상태
curl -s -H "User-Agent: x" https://api.github.com/repos/anthropics/claude-code/issues/<n>
# 크레이트 버전
curl -s -H "User-Agent: x" https://crates.io/api/v1/crates/<crate>
# 소스 직독
bash scripts/clone-references.sh   # → reference/*
```
