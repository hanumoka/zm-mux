# zm-mux

Cross-platform (Windows + macOS) AI agent terminal multiplexer built with Rust.

## What is zm-mux?

zm-mux is an open-source terminal multiplexer designed for AI coding agents. Run Claude Code, Codex, Gemini CLI, and other AI agents side-by-side with GPU-accelerated rendering, split-pane management, and a Socket API for programmatic control.

## Installation

Download the latest release from [GitHub Releases](https://github.com/hanumoka/zm-mux/releases).

**Windows**: Extract `zm-mux-windows-x64.zip`, add the directory to PATH.

**macOS (Apple Silicon)**: Extract `zm-mux-macos-arm64.tar.gz`, move binaries to `/usr/local/bin/`.

**From source**:
```bash
# Requires Rust 1.85+ (edition 2024)
cargo install --path crates/zm-app
cargo install --path crates/zm-mux-mcp
```

## Features

- GPU-accelerated rendering (wgpu: Metal/DX12/Vulkan) with CPU fallback (softbuffer)
- Split-pane management with keyboard/mouse/drag resize
- Text selection + clipboard (Ctrl+Shift+C/V/A)
- SGR mouse tracking (vim, htop, etc.)
- IME composition (Korean, Japanese, Chinese)
- Scrollback search (Ctrl+Shift+F, regex)
- Shift+Enter multi-line input (Kitty CSI u)
- Desktop notifications (OSC 9/777)
- Socket API with 9 JSON-RPC commands
- MCP server for AI tool integration (4 tools via rmcp)
- Multi-agent launcher with git worktree isolation
- Agent status detection with dynamic pane border colors
- Session save/restore (layout persistence)
- TOML configuration (fonts, colors, keybindings, shell)

## CLI Reference

| Command | Description |
|---------|-------------|
| `zm-mux` | Launch GUI (server mode) |
| `zm-mux list` | List all panes |
| `zm-mux status` | Show workspace status |
| `zm-mux send <pane_id> <text>` | Send text to pane |
| `zm-mux focus <pane_id>` | Focus a pane |
| `zm-mux split <pane_id> <h\|v>` | Split a pane |
| `zm-mux close-pane <pane_id>` | Close a pane |
| `zm-mux new-tab` | Create a new tab |
| `zm-mux close-tab <tab_id>` | Close a tab |
| `zm-mux agent-info <pane_id> [type] [status]` | Set agent info |
| `zm-mux launch <agent1> [agent2] ...` | Launch agents with worktrees |
| `zm-mux worktree list\|cleanup` | Manage git worktrees |
| `zm-mux save [name]` | Save session layout |
| `zm-mux sessions` | List saved sessions |

## Configuration

Config file location:
- **Windows**: `%APPDATA%\zm-mux\config.toml`
- **macOS**: `~/Library/Application Support/zm-mux/config.toml`
- **Linux**: `~/.config/zm-mux/config.toml`

```toml
[font]
family = "JetBrains Mono"
size = 16.0

[keybindings]
split_horizontal = "Ctrl+Shift+D"
split_vertical = "Ctrl+Shift+E"
new_tab = "Ctrl+T"
close_tab = "Ctrl+Shift+W"
search = "Ctrl+Shift+F"
copy = "Ctrl+Shift+C"
paste = "Ctrl+Shift+V"

[colors]
background = "#1a1a2e"
foreground = "#e0e0e0"

[shell]
program = ""  # empty = platform default
args = []

[scrollback]
max_lines = 10000
```

## MCP Server

zm-mux includes an MCP server (`zm-mux-mcp`) for AI tool integration.

Add to `.mcp.json`:
```json
{
  "mcpServers": {
    "zm-mux": {
      "type": "stdio",
      "command": "zm-mux-mcp"
    }
  }
}
```

**Tools**: `get_status`, `list_agents`, `send_message`, `peer_discover`

## Multi-Agent Mode

Launch multiple AI agents with isolated git worktrees:

```bash
zm-mux launch claude codex
```

Each agent gets its own branch and working directory under `.zm-worktrees/`.

## Tech Stack

| Component | Crate | Purpose |
|-----------|-------|---------|
| VT Emulation | `alacritty_terminal` | Terminal parsing and state |
| PTY | `portable-pty` | Cross-platform PTY (ConPTY + POSIX) |
| Text Shaping | `cosmic-text` | HarfBuzz-based text shaping |
| GPU Rendering | `glyphon` + `wgpu` | GPU-accelerated text rendering |
| CPU Fallback | `softbuffer` + `tiny-skia` | Software rendering fallback |
| MCP Server | `rmcp` | Official Rust MCP SDK |

## Project Structure

```
crates/
├── zm-core/       # Shared types, errors, config
├── zm-pty/        # PTY abstraction (portable-pty)
├── zm-term/       # VT emulation (alacritty_terminal)
├── zm-render/     # GPU/CPU rendering (wgpu + softbuffer)
├── zm-mux/        # Multiplexer (session/tab/pane tree)
├── zm-agent/      # Agent detection, worktree management
├── zm-socket/     # Socket API + CustomPaneBackend protocol
├── zm-app/        # Application entry point (GUI + CLI)
└── zm-mux-mcp/    # MCP server bridge (rmcp)
```

## License

MIT
