# 05 — Reference Inventory (cloned repos + sources)

> `reference/`는 **gitignored** — repo에 포함되지 않는다. 이 문서가 그 SSOT(URL·라이선스·등급·핀 SHA).
> 재현: `bash scripts/clone-references.sh` 또는 `pwsh scripts/clone-references.ps1`.
> 클론일: 2026-06-15. 라이선스는 **각 repo의 LICENSE 파일 직독**으로 확정 [V].

## 재사용 등급 (clean-room)

| 등급 | 의미 |
|------|------|
| **SAFE** | MIT/Apache/BSD 등 permissive — 학습 + 코드 재사용 가능 |
| **STUDY** | GPL/AGPL 등 copyleft — **읽고 이해만**, 코드/주석/문서 텍스트를 zm-mux로 복사·번역·파생 **금지** |

> ⚠️ 등급은 *법적 재사용 가능성*만 뜻한다. wmux/psmux는 SAFE(MIT)지만 **TypeScript/Electron**이라
> 네이티브 Rust zm-mux엔 코드 직접 이식이 아니라 *설계·동작 학습* 대상이다.

## 클론된 repo

| dir (`reference/`) | upstream | 라이선스 [V] | 언어 | 등급 | 핀 SHA | 학습 포인트 |
|------|----------|----------|------|------|--------|-------------|
| `cmux` | github.com/manaflow-ai/cmux | **GPL-3.0-or-later OR Commercial** (dual) | Swift | **STUDY** | `48c9160` | 재현 대상. 기능·소켓 API·tmux shim·env var·macOS 결박 구조 |
| `wezterm` | github.com/wezterm/wezterm | **MIT** | Rust | SAFE | `69d1fb3` | term/gui/mux 크레이트 분리, `portable-pty` 원작, ConPTY 처리 |
| `zellij` | github.com/zellij-org/zellij | **MIT** | Rust | SAFE | `b6a5ad0` | server/client mux 모델, 레이아웃, WASM 플러그인, Windows 네이티브(v0.44+) |
| `alacritty` | github.com/alacritty/alacritty | **Apache-2.0** (LICENSE-APACHE; 통상 Apache/MIT 듀얼) | Rust | SAFE | `aaf3bd7` | VT 에뮬레이션, ConPTY 백엔드, 성능 패턴 |
| `vte` | github.com/alacritty/vte | **Apache-2.0 / MIT** | Rust | SAFE | `abeae76` | ANSI/VT 파서 상태머신 |
| `cosmic-term` | github.com/pop-os/cosmic-term | **GPL-3.0** | Rust | **STUDY** | `18b4450` | alacritty_terminal+glyphon+wgpu 통합 *패턴*(코드 복사 금지) |
| `psmux` | github.com/psmux/psmux | **MIT** | Rust | SAFE | `e4db1bf` | Windows 네이티브 tmux(ConPTY), tmux 명령 호환, isTTY 우회, Claude Code 팀 |
| `wmux-amirlehmam` | github.com/amirlehmam/wmux | **MIT** | TypeScript | SAFE | `d04fb59` | Windows 포트(Electron/ConPTY/named pipe/토스트) — 동작 학습 |
| `wmux-openwong2kim` | github.com/openwong2kim/wmux | **MIT** | TypeScript | SAFE | `61938a5` | 동명 별개 Windows 포트 — 둘 차이/접근 비교 |
| `cmux-for-linux` | github.com/cai0baa/cmux-for-linux (ptrcode) | **GPL-3.0** | TypeScript | **STUDY** | `407000a` | Tauri+React 크로스플랫폼 포트 *접근* |
| `cmux-linux` | github.com/bradwilson331/cmux-linux | **AGPL-3.0** | Swift+Rust | **STUDY** | `17d5088` | cmux(Swift) Linux 포트, ghostty FFI 브리징 *접근* |
| `microsoft-terminal` | github.com/microsoft/terminal | **MIT** | C++ | SAFE | `9853bc9` | ConPTY 공식 샘플/스펙 (`samples/`, `doc/` sparse) |

> `zellij`은 클론 스크립트 로그상 `FAIL`로 찍혔으나(stderr 경고로 인한 종료코드) 실제로는 정상
> 클론됨(Cargo.toml/LICENSE.md/HEAD `b6a5ad0` 확인). [V]

## 웹 원문 아카이브 (`docs/research/sources/`)

> 단계 3에서 핵심 URL 스냅샷을 저장(각 파일 머리에 URL+취득일+검증등급). 1차/공식은 `sources/`,
> 블로그 등 2차는 `sources/secondary/`. 전체 목록·링크는 [references.md](references.md).

핵심 1차/공식 출처:
- cmux: repo(LICENSE/Sources/CLI/docs), `cmux.com`, mintlify docs(automation/environment-variables, agent-integrations/claude-code-teams)
- Claude Code: `code.claude.com/docs/en/agent-teams`, GitHub issues #26572 / #26244 / #34150 / #36926 / #27868
- Microsoft ConPTY: `learn.microsoft.com/.../creating-a-pseudoconsole-session`, `microsoft/terminal` samples
- 크레이트: 각 `docs.rs` + `crates.io`

## 갱신 절차

1. `scripts/clone-references.*` 재실행 → 최신 HEAD 클론.
2. 새 SHA를 위 표에 반영. 라이선스 변경 여부 LICENSE 직독으로 재확인.
3. repo 추가 시 등급(SAFE/STUDY)을 LICENSE로 판정 후 표/스크립트 동시 갱신.
