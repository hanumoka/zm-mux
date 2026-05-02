# Windows isTTY 우회 — psmux 패턴 분석 + zm-mux 적용 결정

> **목적**: P0 #2 결정 박제. Phase 2.1 (tmux 호환 + Claude Code 에이전트 팀) 진입 전 우회법 사양을 정확히 확정.
>
> **결과**: doc 10 의 가설 (TMUX 환경변수 우선)을 정정. 진짜 우회 메커니즘은 **PowerShell `claude` 래퍼 함수가 `--teammate-mode tmux` CLI flag 를 주입**. 환경변수는 보조 수단.

---

## 🔴 STATUS: BLOCKED (2026-05-02 spike 결과)

**이 문서의 결정 1~5 는 모두 BLOCKED.** zm-mux 의 Phase 2.1 진입 전 사전 spike 가 psmux 패턴이 작동하지 않음을 실측 증명. **후속 트랙은 [`docs/13-custompanebackend-track.md`](./13-custompanebackend-track.md) 참조**.

### Spike 결과 요약 (실측)

- ✅ STEP 0~3 통과: PS 7.6.1 환경에서 psmux 3.3.4 가 7 env var 정확히 주입 (`PSMUX_TARGET_SESSION` 1개 추가 발견), `claude` PowerShell wrapper 가 `Function` 으로 활성, Definition 안에 if/else 분기 + env var 동적 읽기 패턴 (우리 가설보다 정교)
- 🔴 STEP 4~5 실패: psmux 패인 안에서 `claude` 실행 (= `claude.exe --teammate-mode tmux` 로 wrapper invocation) → agent team 생성 prompt → **단일 패인에서 in-process Task 도구로 fallback**. 화면 분할 0건. status bar `[spike] 0:claude*` 일관 (multi-window 지표 없음).
- 🔴 `/team` 슬래시 명령 부재: autocomplete 결과 `/team-onboarding` (인간 동료 가이드, 무관) + `/terminal-setup` + `/tui` + `/remote-control` 4개. **AI teammate 를 spawn 하는 슬래시 명령 0개**.
- 🔴 **Claude Code 자체의 self-report**: spike 도중 Claude 가 *"leave it for you to swap to WSL+tmux?"* 제안 — Claude 가 native Windows 경로에서 agent team 동작 안 함을 implicit 인정.

### 결론

이슈 #26244 OP 의 0.3.3 시점 결과 (psmux 가 동작 안 함) 가 3.3.4 + 우리 환경에서도 그대로 재현. **PowerShell wrapper + 6 env var 모두 정확히 셋팅돼도 isTTY 게이트는 다른 코드 경로에서 차단** — wrapper 가 `--teammate-mode tmux` 를 통과시켜도 Claude 측이 *기능 노출 자체를 안 함* (`/team` 슬래시 명령 부재).

→ tmux 호환 트랙(이 문서의 결정 1~5) 은 **Claude Code 측에서 agent team feature 가 노출되지 않으므로 무효**. CustomPaneBackend (#26572) JSON-RPC 트랙으로 갈아타기.

---

## 1. 문제 재정의

Claude Code (Bun SFE on Windows) 는 다음 코드 경로에서 `isInteractive` 판정:

```javascript
// from issue #26244 OP — Claude Code 내부 코드
let D = $ || A || L || !process.stdout.isTTY;
setIsInteractive(!D);
```

Windows ConPTY 환경에서 `process.stdout.isTTY` 가 `undefined` → `isInteractive = false` → `isInProcessEnabled() === true` 무조건 → `~/.claude/settings.json` 의 `"teammateMode": "tmux"` **무시**.

**Anthropic 입장**: issue #26244 + #34150 모두 **closed as "not planned"**. 자체 fix 의향 없음 → **클라이언트(우리) 가 우회 책임**.

---

## 2. psmux 의 실제 우회 메커니즘 (소스 분석)

### 2.1 `set_tmux_env()` — 패인 spawn 시 6 env var 주입

`psmux/src/pane.rs` (~line 520-570), 4 spawn 경로(`create_window`/`spawn_warm_pane`/`create_window_raw`/`split_active_with_command`) 모두 호출:

```rust
pub fn set_tmux_env(builder: &mut CommandBuilder, pane_id: usize, control_port: Option<u16>,
                    socket_name: Option<&str>, session_name: &str, fix_tty: bool,
                    _force_interactive: bool) {
    let server_pid = std::process::id();
    let port = control_port.unwrap_or(0);
    let sn = socket_name.unwrap_or("default");
    builder.env("TMUX", format!("/tmp/psmux-{}/{},{},0", server_pid, sn, port));
    builder.env("TMUX_PANE", format!("%{}", pane_id));
    builder.env("PSMUX_SESSION", session_name);
    builder.env("MSYS2_ENV_CONV_EXCL", "TMUX");
    builder.env("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS", "1");
    if fix_tty {
        builder.env("PSMUX_CLAUDE_TEAMMATE_MODE", "tmux");
    }
}
```

각 변수 역할:

| 변수 | 값 | 역할 |
|---|---|---|
| `TMUX` | `/tmp/psmux-{pid}/{sock},{port},0` | tmux native 포맷. 외부 도구(`tmux -V` 등)가 "tmux 안" 으로 판정. **단독으로는 isTTY 게이트 무력화 못함** (issue #26244 OP 검증) |
| `TMUX_PANE` | `%{pane_id}` | tmux native 포맷. capture-pane / send-keys target 식별용 |
| `PSMUX_SESSION` | session 이름 | psmux 자체 마커 |
| `MSYS2_ENV_CONV_EXCL` | `"TMUX"` | Git Bash/MSYS2 의 자동 경로 변환에서 TMUX 값 제외 (그렇지 않으면 `/tmp/...` → `C:/...` 로 망가짐) |
| `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` | `"1"` | Claude Code 의 agent teams feature gate 활성화 |
| `PSMUX_CLAUDE_TEAMMATE_MODE` | `"tmux"` | 후술하는 PowerShell 래퍼가 읽는 마커 (env var 자체로는 Claude Code 가 보지 않음) |

### 2.2 PowerShell `claude` 래퍼 — 진짜 우회 메커니즘

psmux/docs/claude-code.md 인용:

> "The standalone binary ignores `teammateMode: "tmux"` from `~/.claude/settings.json`.
> psmux injects `--teammate-mode tmux` via a PowerShell wrapper function that's loaded in every pane."

검증법: `Get-Command claude | Format-List` → `CommandType: Function` (Application 아님) 이면 활성. config 키 `claude-code-fix-tty on` (default) 가 활성/비활성 토글.

**이 래퍼가 핵심**. 환경변수만으로는 isTTY 게이트를 통과 못 함 — issue #26244 OP 가 psmux 0.3.3 으로 직접 확인. **CLI flag `--teammate-mode tmux` 가 settings.json 의 같은 옵션과 다른 코드 경로**로 들어가서 isTTY 체크 이전에 처리되는 것으로 추정.

### 2.3 알려진 제약

- **PowerShell 7+ 강제 의존**. Windows PowerShell 5.1 미지원 (psmux 측 명시).
- **`claude -p` (pipe 모드) 우회 불가** — 의도적 in-process. 인터랙티브 invocation 만 동작.
- **worktree + tmux Windows 조합 hardcoded 비활성** — Opus 같은 상위 모델은 worktree 격리를 선호하는데 Windows 에서는 tmux 패인으로 나오지 않음 (env var 강제 옵션 없음).
- **`claude.exe` 직접 호출** (확장자 명시) 시 PowerShell 함수 우회 → 래퍼 무력. user 가 alias 가 아닌 path 직접 호출 시 fail-silently.

### 2.4 ⚠ 실증 갭

- **github issue 측 검증 0건**. #26244 OP 가 psmux 0.3.3 테스트 시 teammates 가 in-process 로 fallback 됨 (`tmux-shim` 로그 0 split-window/send-keys). #34150 OP 는 "동작한다" 만 주장하고 evidence 미첨부.
- **현재 버전(3.3.4) 에서 추가된 것**이 PowerShell 래퍼인지 다른 메커니즘인지는 외부에서 확인 불가.
- **psmux 자체 docs 가 유일한 "동작" 출처** → zm-mux 가 같은 패턴 따라가도 동작 보장 없음. **반드시 실험적 검증이 선행**.

---

## 3. zm-mux 적용 결정

> 🔴 **결정 1~5 모두 BLOCKED** (2026-05-02 spike). 향후 Anthropic 측이 isTTY 게이트를 fix 하거나 `--teammate-mode tmux` flag 를 진짜 코드 경로로 연결하면 unblock. 현재로선 reference 로만 보존.

### 결정 1: psmux 패턴 따르기 (소스 포팅 X, 사양만 차용)

`zm-agent` 크레이트에 `set_tmux_env(builder: &mut CommandBuilder, pane_id, session_id, ...)` 신규 — psmux 의 6 env var 와 동일한 구조 + 동일한 포맷 (`/tmp/zm-mux-{pid}/{sock},{port},0`, `%{pane_id}`).

**근거**:
- psmux 가 (검증 갭에도 불구하고) 가장 구체적인 사양을 제시. 동일 환경에서 다른 작동 패턴은 외부에 없음.
- env var 만으론 부족하지만 tmux CLI 명령(`tmux -V`, `split-window` 등)을 zm-mux shim 이 가로채는 데 필요. 미설정 시 Claude Code 가 tmux server 부재로 판정.
- env var 자체는 무해 — Claude Code 외 도구도 tmux 컨텍스트 식별 가능 (`echo $TMUX` 같은 사용자 디버깅).

### 결정 2: PowerShell `claude` 래퍼는 **zm-mux 가 직접 install/inject**

세 옵션 비교:

| 방식 | 장점 | 단점 |
|---|---|---|
| A. install 시 `$PROFILE` 에 함수 정의 추가 | 1회 설정, 이후 모든 pane 자동 적용 | 사용자 profile 수정 (invasive), 다른 셸 (cmd, Git Bash) 미커버 |
| **B. 패인 spawn 시 PS 명령 inline injection** | 사용자 profile 무수정, zm-mux off 시 자동 정리, per-pane 제어 | PS 7+ 강제 의존, cmd/Git Bash 사용자 별도 처리 필요 |
| C. zm-mux 가 `claude` 대신 `claude --teammate-mode tmux` 를 직접 spawn | 셸 무관, 가장 단순 | 사용자가 패인 안에서 `claude` 재실행하면 flag 누락. PowerShell 함수보다 약함 |

**채택: B (inline injection) + C (직접 spawn 시 flag 자동 추가) 하이브리드**

- 새 패인 시작 시 zm-mux 가 PowerShell command 으로 `function claude { & claude.exe --teammate-mode tmux @args }` 정의를 주입 (psmux 와 동일).
- 사용자가 메뉴 / 단축키로 "spawn Claude agent" 를 명시할 때는 zm-mux 가 직접 `claude.exe --teammate-mode tmux` 를 패인 명령으로 spawn (C).
- cmd / Git Bash 패인은 별도 wrapper 검토 (Phase 2.1 사전 spike 에서 결정).

### 결정 3: 7-day Phase 2.1.4 시작 전 **1-day 사전 spike** 추가

**근거**: github issue 측 실증 0건. psmux docs 만 신뢰하고 7일 투자했다가 `--teammate-mode tmux` 가 작동 안 하면 **단일 차단점이 단일 폭망점이 됨**.

**spike 작업 (1 day)**:
1. psmux 3.3.4 dev 머신 설치
2. `psmux new-session` → `claude` → "Create a team named X with 2 members"
3. teammates 가 별도 psmux 패인으로 spawn 하는지 직접 시각 확인
4. tmux-shim 로그(`psmux server log`) 에 split-window / send-keys 호출 발생하는지 확인
5. **Yes → Phase 2.1 진입 그린라이트** / **No → CustomPaneBackend (#26572) 트랙으로 우선순위 전환** + Anthropic 측 압력(이슈 추가, 댓글, X)

이 spike 가 Phase 2 의 게이트. 비용 1일, 가치 7일 투자 보호.

### 결정 4: PS 7+ 의존을 zm-mux Windows 요구사항에 명시

- `docs/01-windows-claude-code-issues.md` 의 셸 요구사항 섹션에 추가
- README.md 에 "Windows: PowerShell 7+ required for Claude Code agent team support" 기재
- Win 11 기본 PS 5.1 사용자는 별도 설치 안내 (winget install Microsoft.PowerShell)
- cmd / Git Bash 사용자는 agent team 지원 제외 또는 별도 wrapper (TBD)

### 결정 5: Phase 2.1.4 의 7일 분해 (사전 spike 통과 가정)

| 작업 | 일 | 대상 |
|---|---|---|
| 0. 사전 spike (psmux 실측) | **1** | dev 머신 (zm-mux 코드 0줄) |
| 1. `set_tmux_env()` Rust 포팅 | 1 | `zm-agent::env::set_tmux_env` |
| 2. `TMUX` socket path 포맷 + Windows 임시 디렉토리 매핑 (`/tmp/...` → `%TEMP%`) | 1 | `zm-agent::env` + tests |
| 3. PowerShell `claude` 래퍼 inline injection (셸 감지 + PS 7 / cmd / Git Bash 분기) | 2 | `zm-agent::shell_init` |
| 4. 경로 포맷 변환 (Unix↔Windows, #42848) — Claude Code 가 PowerShell 셸이면 Windows 경로 기대 | 1 | `zm-agent::path_convert` |
| 5. ConPTY 플래그 (`PSEUDOCONSOLE_RESIZE_QUIRK`) — psmux 가 사용. Win10 Build 17763 미만에서만 영향이라 우선순위 LOW | 0.5 | `zm-pty` |
| 6. E2E 검증 — Claude Code agent team `tmux` 모드 + zm-mux 의 tmux shim → split-window 라우팅 → 새 zm-mux 패인 spawn 확인 | 0.5 | 통합 테스트 |

**합계 7일** (+ spike 1일 → 실효 8일).

---

## 4. CustomPaneBackend (#26572) 와의 관계

- 이번 결정은 **즉시 동작 트랙(tmux 호환)** 한정. CustomPaneBackend JSON-RPC 7-op 트랙은 별개 (`docs/11` Phase 3.3.2).
- `--teammate-mode tmux` 가 동작 안 하면 (사전 spike 실패) → Phase 2 의 tmux 호환 트랙을 후순위로 강등하고 CustomPaneBackend 트랙을 가속 (Phase 3 → Phase 2 로 격상).
- 둘 다 동시 추진은 인력 분산이라 비추천. 사전 spike 결과로 단일 트랙 선택.

---

## 5. 미해결 (별도 트래킹 필요)

- **#42848 (경로 포맷)**: 결정 5의 작업 4에서 처리. 깊이는 spike 결과로 조정.
- **cmd.exe / Git Bash 사용자 지원**: PowerShell 래퍼만으로는 커버 못 함. PowerShell 7 강제 권고로 시작하고 사용자 요청 시 wrapper alternative 추가.
- **`claude -p` pipe 모드**: 우회 불가 — 인터랙티브 invocation 만 동작. UX 문서에 명시.
- **Opus + worktree 격리**: psmux 측도 hardcoded 비활성. zm-mux 도 동일하게 시작, 향후 Anthropic 측 fix 대기.
- **github issue 측 실증 갭**: 사전 spike 후 zm-mux 가 직접 동작 영상 + 로그를 #26244 / #34150 에 댓글로 남기면 커뮤니티 + Anthropic 측 압력 증대 (low-effort, high-leverage).

---

## 6. 참고

- [psmux Claude Code docs](https://github.com/psmux/psmux/blob/master/docs/claude-code.md) — 사용자 가이드
- [psmux/src/pane.rs `set_tmux_env`](https://github.com/psmux/psmux/blob/master/src/pane.rs) — 정확한 env 주입 사양
- [Claude Code issue #26244](https://github.com/anthropics/claude-code/issues/26244) — isTTY 본 이슈 (closed not planned)
- [Claude Code issue #34150](https://github.com/anthropics/claude-code/issues/34150) — psmux 호환 요청 (closed not planned)
- [Claude Code issue #42848](https://github.com/anthropics/claude-code/issues/42848) — 경로 포맷 (Unix↔Windows)
- [`docs/10-critical-risks-update.md` 리스크 1](./10-critical-risks-update.md) — 가설 정정 대상
- [`docs/11-implementation-roadmap.md` Phase 2.1.4](./11-implementation-roadmap.md) — 7일 구현 계획 분해

---

*조사 완료: 2026-05-02 / 결정 박제: zm-mux Phase 2.1 진입 게이트*
