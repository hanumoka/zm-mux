# #26572 댓글 초안 — zm-mux 1차 reference 구현 advocacy

> **목적**: github issue [#26572 (CustomPaneBackend protocol proposal)](https://github.com/anthropics/claude-code/issues/26572) 에 zm-mux 의 minimal reference 구현 + spike 결과를 제출하여 Anthropic 측 채택 우선순위 압력 + 사양 ambiguity query.
>
> **사용 방법**: 사용자가 `mock_client` 영상 (asciinema 또는 OBS) 녹화 후, 아래 본문 + 영상 + 우리 repo 링크를 댓글로 게시. 본문 markdown 그대로 paste 가능.

---

## 녹화 가이드 (사용자 직접 작업, ~10분)

### asciinema (가장 가벼움, 텍스트 기반)

```powershell
# 1. asciinema 설치 (Windows 는 WSL 또는 cygwin 의존, native binary 없음)
#    Windows 에서는 OBS 권장 (다음 섹션). asciinema 는 Mac/Linux 에서만 단순.

# Mac/Linux 의 경우:
brew install asciinema     # 또는 apt/dnf
asciinema rec demo.cast
cargo run -p zm-socket --example mock_client
exit                       # asciinema 자동 종료
asciinema upload demo.cast # asciinema.org 에 게시 후 URL 획득
```

### OBS (Windows 권장, 영상 파일)

1. [OBS Studio](https://obsproject.com/) 설치
2. Source → Window Capture → PowerShell 7 창 선택
3. Recording 시작
4. PowerShell 에서:
   ```powershell
   cargo run -p zm-socket --example mock_client
   ```
5. 출력이 다 흐른 뒤 (~3초) Recording 정지
6. 결과 mp4 또는 webm 을 youtube unlisted 또는 github attachment 로 업로드

### 녹화 시 강조 포인트

- 시작 부분: "[server] listening" + "[client] connected" 라인이 보일 것
- `>>> request` / `<<< response` 페어 6 회 (initialize/spawn_agent/write/capture/list/list-after-exit/kill/list-empty 중 6 가지)
- **"--- [demo] triggering MinimalHandler::simulate_exit out of band"** 라인 후 **"<<< notification (server→client push)"** 출현 — 이게 7-op 의 마지막 op (`context_exited`) 검증
- 마지막 "Demo complete. 8 RPCs + 1 notification round-tripped over a Windows named pipe / Unix domain local socket." 라인이 platform 감지 증명

---

## 댓글 본문 초안 (영문, ready-to-post)

```markdown
# zm-mux: Reference Implementation of CustomPaneBackend (Phase 2.1.A)

Hi @Wirasm and the Claude Code team — we're shipping a reference implementation of this proposal as part of [zm-mux](https://github.com/hanumoka/zm-mux), a cross-platform (Windows + macOS) AI-agent terminal multiplexer in Rust. Sharing what we learned and what we've built in case it's useful as you evaluate adopting the protocol.

## Why we ended up here (spike summary)

Our original Phase 2.1 plan was the existing `teammateMode: "tmux"` path on Windows native, using the [psmux](https://github.com/psmux/psmux) pattern (the only documented workaround for the Windows isTTY blocker, [#26244](https://github.com/anthropics/claude-code/issues/26244)). A 1-day pre-implementation spike against psmux 3.3.4 + PowerShell 7.6.1 + Claude Code 2.1.126 confirmed:

- ✅ All 7 environment variables psmux's `set_tmux_env` documents are set correctly when a pane spawns
- ✅ The PowerShell `claude` wrapper function activates as `Function` (verifiable via `Get-Command claude | Format-List`); its `Definition` injects `--teammate-mode tmux` per `$env:PSMUX_CLAUDE_TEAMMATE_MODE`
- ✅ `claude` accepts the `--teammate-mode tmux` flag without complaint
- 🔴 **Agent teammates still spawn in-process.** No `tmux split-window` calls, status bar stays single-pane. Reproduces the original #26244 OP's finding.
- 🔴 No `/team`, `/teammate`, or similar slash command exists in 2.1.126 to invoke the team feature explicitly. `/help` autocomplete on `team` returns only `team-onboarding` (a human-onboarding guide, not AI teammate spawning).
- 🔴 During the spike, Claude Code itself suggested *"swap to WSL+tmux"* — an implicit acknowledgement that native Windows path doesn't work today.

We also did a static check on `claude.exe` (254 MB Bun SFE) for the strings of this proposal:

| Search | Hits |
|---|---|
| `CLAUDE_PANE_BACKEND`, `PANE_BACKEND`, `pane_backend` | **0** |
| `spawn_agent`, `context_exited`, `CustomPane` | **0** |
| `teammate_mode` (existing flag) | 6 |
| `TmuxBackend` (existing macOS class) | 28 |

So the existing tmux integration code is alive (28 `TmuxBackend` references) but gated by isTTY on Windows; the protocol proposed here doesn't appear to be in 2.1.126 yet. Given that, we adopted a **hedge**: ship a minimal reference implementation of this protocol now (positions us as 1st implementer if you adopt) plus an independent self-coordination layer for our users (carries value if you don't).

## What we built

`zm-socket::rpc` and `zm-socket::transport_sync` cover:

- All **6 client→server methods** (`initialize`, `spawn_agent`, `write`, `capture`, `kill`, `list`)
- The **`context_exited` server→client notification** (push, no `id`)
- JSON-RPC 2.0 envelope (`Request` / `Response{Success,Error}` untagged / `Notification`) with the 5 standard error code constants
- NDJSON framing over a local socket (Unix domain on macOS/Linux, named pipe on Windows) via the `interprocess` crate — same surface, single API
- **35 tests green** (19 unit on the types and handler, 14 `insta` snapshot tests pinning the wire format against JSON-RPC 2.0 spec, 2 end-to-end transport integration tests with a real server thread + client)

The `mock_client` example does an 8-RPC roundtrip + 1 notification flow against a real local socket. Every frame is printed as pretty JSON so the output is the wire format. Recording: **[link to asciinema / video]**.

Source for everything above:
- Types: [`crates/zm-socket/src/rpc/types.rs`](https://github.com/hanumoka/zm-mux/blob/master/crates/zm-socket/src/rpc/types.rs)
- Handler: [`crates/zm-socket/src/rpc/handler_min.rs`](https://github.com/hanumoka/zm-mux/blob/master/crates/zm-socket/src/rpc/handler_min.rs)
- Transport: [`crates/zm-socket/src/transport_sync.rs`](https://github.com/hanumoka/zm-mux/blob/master/crates/zm-socket/src/transport_sync.rs)
- Demo: [`crates/zm-socket/examples/mock_client.rs`](https://github.com/hanumoka/zm-mux/blob/master/crates/zm-socket/examples/mock_client.rs)

## Spec ambiguity we'd love clarification on

These are the calls we had to make in absence of explicit wire-format guidance in the proposal. Listed in order of "most likely to bite implementations":

1. **Parse-error response with no recoverable `id`** — JSON-RPC 2.0 §5 says use `null`. Should `id` in the protocol's responses be allowed to be `null` (in addition to number/string), or do you want a different shape? Affects whether implementations need a 3-variant `id` enum vs. 2-variant.
2. **`capture.lines` semantics** — number of *lines* or *characters*? If lines, do we count raw newlines, or rendered lines (after wrapping)? What's the encoding of the returned `data` — UTF-8 strict, ANSI-preserved, or raw bytes (base64)?
3. **`spawn_agent.cwd` inheritance** — when omitted, does the child inherit from the calling pane's cwd, the backend's cwd, or always the user's home? psmux uses the calling pane's cwd; we followed.
4. **`spawn_agent.env` merge policy** — when the client provides `env`, is it the complete environment, or merged on top of an inherited base (and if so, base from where)? We currently treat it as additive on top of the spawning process's env.
5. **`context_exited` ordering guarantees** — multiple notifications can buffer between client requests. Are they delivered in exit-order? In our impl they ride out before the next response in arrival order. Worth specifying so clients can rely on it.
6. **`initialize.protocol_version` mismatch handling** — server-defined error code, or specific code reserved by the protocol?
7. **`spawn_agent.argv` quoting** — explicitly listed as `argv[]` (we love this — escapes the shell-string trap), but should we document that the backend MUST `execve`-style spawn (no `system(3)`) so users can't be surprised by shell expansion?

We've encoded conservative answers to all of these in our reference; if you'd prefer different semantics we're happy to adjust before others adopt.

## Offer

- **Maintain reference implementation parity with the spec** as it evolves — we're motivated since this unblocks our own users on Windows
- **Cross-pollinate** to [cmux](https://github.com/manaflow-ai/cmux) and other terminal multiplexer projects (we noticed [#24189](https://github.com/anthropics/claude-code/issues/24189) for Ghostty, [#24122](https://github.com/anthropics/claude-code/issues/24122) for Zellij, [#23574](https://github.com/anthropics/claude-code/issues/23574) for WezTerm have similar requests — a single shared protocol unblocks all of them)
- **Carry the implementation cost on our side** — we already have it built; you reviewing the wire format and shipping the reader-side in `claude.exe` is a small net effort

Would love to hear what shape of feedback is most useful — a PR against `claude-code` for the reader-side hooks, a separate reference repo we maintain, or just iterations against this comment thread.

Thanks!
```

---

## 사용 후 followup

댓글 게시 후:

1. context.md 의 Done 섹션에 "github #26572 댓글 게시 (`<URL>`)" entry 추가
2. context.md 의 Decisions 섹션에 "외부 advocacy 1차 액션 완료, Anthropic 측 응답 대기" 추가
3. Anthropic 측 maintainer 응답 시점에 따라:
   - 채택 의향 + 사양 명확화 → docs/13 Section 4 (full reference) 의 12일 분해 진입
   - 미응답 / 무관심 → Phase 2.2 (zm-mux self-coordination) 진입, 본 reference 는 dormant maintenance 모드
   - 부분 응답 (사양 ambiguity 1~3개 답변) → 답변에 맞춰 우리 minimal 미세조정 + Section 4 의존 항목 갱신

이 advocacy 의 ROI 가 가장 큰 시점은 **댓글 게시 후 2주** — Anthropic 측이 본 issue 를 reactivate 할지의 결정 윈도우.

---

## 참고

- [github issue #26572 (CustomPaneBackend proposal)](https://github.com/anthropics/claude-code/issues/26572) — 댓글 대상
- [`docs/13-custompanebackend-track.md`](./13-custompanebackend-track.md) — 우리 reference 의 사양/계획
- [`docs/12-istty-workaround.md`](./12-istty-workaround.md) — spike 결과 (psmux 동작 안 함)
- `crates/zm-socket/examples/mock_client.rs` — 녹화 대상
