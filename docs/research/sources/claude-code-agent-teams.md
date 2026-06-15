# SNAPSHOT — Claude Code: Orchestrate teams of Claude Code sessions

> URL: https://code.claude.com/docs/en/agent-teams
> 취득일: 2026-06-15 · 등급: 1차/공식 · 방식: WebFetch (markdown 변환, 이미지/네비 노이즈 일부 정리)

---

> Agent teams are experimental and disabled by default. Enable them by adding
> `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` to your settings.json or environment.
> Agent teams require Claude Code **v2.1.32 or later**.

Agent teams let you coordinate multiple Claude Code instances. One session is the **team lead**;
**teammates** work independently, each in its own context window, communicating directly.

## Enable
```json
{ "env": { "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1" } }
```

## Display modes
- **In-process**: all teammates in your main terminal. Shift+Down to cycle. Works in any terminal.
- **Split panes**: each teammate gets its own pane. **Requires tmux, or iTerm2.**

> `tmux` has known limitations on certain operating systems and traditionally works best on macOS.
> Using `tmux -CC` in iTerm2 is the suggested entrypoint into tmux.

`teammateMode` (in `~/.claude/settings.json`):
- default `"auto"` — split panes if already inside a tmux session or terminal is iTerm2; in-process otherwise.
- `"tmux"` — enables split-pane, auto-detects tmux vs iTerm2.
- `"in-process"` — force in-process. (`claude --teammate-mode in-process` per session.)

Split-pane requires either tmux or iTerm2 with the `it2` CLI (github.com/mkusaka/it2; enable Python API in iTerm2).

## Architecture
| Component | Role |
| Team lead | Main session: creates team, spawns teammates, coordinates |
| Teammates | Separate Claude Code instances on assigned tasks |
| Task list | Shared work items teammates claim/complete |
| Mailbox | Messaging between agents |

Storage (local, exist only while team active):
- Team config: `~/.claude/teams/{team-name}/config.json` (runtime state: session IDs, tmux pane IDs — do not hand-edit)
- Task list: `~/.claude/tasks/{team-name}/`

Task claiming uses **file locking** to prevent races. Dependencies auto-unblock.

## Permissions / context
- Teammates start with the lead's permission settings.
- Each teammate has own context window; loads CLAUDE.md, MCP servers, skills (NOT lead's conversation history).
- Messages delivered automatically; idle teammates notify the lead.

## Quality-gate hooks
- `TeammateIdle` — runs when a teammate about to go idle (exit 2 → feedback, keep working).
- `TaskCreated` — exit 2 prevents creation + feedback.
- `TaskCompleted` — exit 2 prevents completion + feedback.

## Limitations (verbatim highlights)
- No session resumption with in-process teammates (`/resume`, `/rewind` don't restore them).
- Task status can lag; shutdown can be slow.
- One team at a time; no nested teams; lead is fixed; permissions set at spawn.
- **Split panes require tmux or iTerm2. The default in-process mode works in any terminal.
  Split-pane mode isn't supported in VS Code's integrated terminal, Windows Terminal, or Ghostty.**

> ※ 이 공식 문서는 isTTY를 언급하지 않는다. Windows에서 tmux/psmux가 있어도 split이 막히는
> 메커니즘은 issue #26244(closed/not_planned)에 있다 — [../04-ai-agent-integration.md](../04-ai-agent-integration.md) 참조.
