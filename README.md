# zm-mux

Cross-platform (Windows + macOS) AI agent terminal multiplexer built with Rust.

## What is zm-mux?

zm-mux is an open-source terminal multiplexer designed for AI coding agents. It enables running Claude Code, Codex, Gemini CLI, and other AI agents side-by-side with GPU-accelerated rendering, split-pane management, and native Claude Code Agent Teams support.

**Key differentiator**: No open-source, cross-platform AI agent terminal exists today — zm-mux fills this gap.

## Features (Planned)

- GPU-accelerated rendering (WebGPU/WGPU) with CPU fallback
- Split-pane management with auto-rebalancing for AI agents
- Claude Code Agent Teams support (tmux protocol compatible)
- Socket API for programmatic control (cmux compatible)
- Multi-agent execution (Claude + Codex + Gemini simultaneously)
- Desktop notifications (OSC 9/99/777)
- CustomPaneBackend protocol support (JSON-RPC 2.0)
- Built-in MCP server for agent IPC

## Tech Stack

| Component | Crate | Purpose |
|-----------|-------|---------|
| VT Emulation | `alacritty_terminal` | Terminal parsing and state |
| PTY | `portable-pty` | Cross-platform PTY (ConPTY + POSIX) |
| Text Shaping | `cosmic-text` | HarfBuzz-based text shaping |
| GPU Rendering | `glyphon` + `wgpu` | GPU-accelerated text rendering |
| CPU Fallback | `softbuffer` + `tiny-skia` | Software rendering fallback |
| MCP Server | `rmcp` | Official Rust MCP SDK |

## Build

```bash
# Prerequisites: Rust 1.80+
cargo build --workspace

# Run
cargo run -p zm-app

# Test
cargo test --workspace

# Lint
cargo clippy --workspace
```

## Project Structure

```
crates/
├── zm-core/       # Shared types, errors, traits
├── zm-pty/        # PTY abstraction (portable-pty)
├── zm-term/       # VT emulation (alacritty_terminal)
├── zm-render/     # GPU/CPU rendering
├── zm-mux/        # Multiplexer (session/tab/pane tree)
├── zm-agent/      # Agent detection, tmux compat
├── zm-socket/     # Socket API, MCP, CustomPaneBackend
└── zm-app/        # Application entry point
```

## License

MIT
