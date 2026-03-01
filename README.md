# murmur

**Terminal command center for AI coding sessions.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021_edition-orange.svg)](https://www.rust-lang.org/)

Murmur is a lightweight terminal multiplexer built for developers who work with AI coding agents. It keeps your shell sessions organized in a dual-mode interface — a raw PTY passthrough for hands-on work, and a TUI overview for switching between sessions at a glance.

<!-- screenshot -->

## Features

- **Dual-mode interface** — Focus mode for full terminal passthrough, Overview mode for session management
- **Prompt pinning** — Automatically captures your last command as a persistent context bar
- **Claude Code detection** — Recognizes Claude sessions and marks them with a visual indicator
- **Session grid** — Responsive tile layout with live terminal previews
- **PTY passthrough** — Zero-interference raw terminal I/O with full ANSI support
- **Window title tracking** — Captures and displays terminal window titles per session
- **Alternate screen aware** — Correctly handles full-screen TUI apps like vim and htop

## Quick Start

### npx (no install needed)

```bash
npx murmur-tui
```

### Build from source

```bash
git clone https://github.com/steadymoka/murmur.git
cd murmur
cargo build --release
./target/release/murmur
```

Murmur launches in Focus mode with a shell session in the current directory. Two persistent bars appear at the bottom — a pin bar showing your last command, and a hint bar with keybinding reminders.

## Keybindings

### Focus Mode

All input is forwarded to the PTY. Use `Ctrl+\` as a prefix key to access commands.

| Key | Action |
| --- | --- |
| `Ctrl+\` `o` | Switch to Overview mode |
| `Ctrl+\` `q` | Quit |

### Overview Mode

| Key | Action |
| --- | --- |
| `j` / `k` / `h` / `l` | Navigate session grid (vim-style) |
| Arrow keys | Navigate session grid |
| `1`–`9` | Jump to session N |
| `Enter` | Focus selected session |
| `n` | New session (enter directory path) |
| `d` | Delete selected session |
| `q` | Quit |

## How It Works

Murmur operates as a state machine with two modes:

**Focus** attaches your terminal to a PTY session. Output is written directly to stdout through a scroll region that reserves space for persistent context bars. A VT100 parser runs in parallel to track screen state, window titles, and input for prompt pinning.

**Overview** enters an alternate screen and renders a responsive grid of session tiles using ratatui. Each tile shows the session status, window title, pinned prompt, and a live terminal preview. PTY output from all sessions continues to be processed in the background.

The prefix key (`Ctrl+\`) is the only input murmur intercepts in Focus mode — everything else passes through untouched.

## Configuration

Create a `murmur.toml` in your project root:

```toml
name = "my-project"

[agent]
command = "claude"
args = ["--project", "."]
```

| Field | Description |
| --- | --- |
| `name` | Project name displayed in Overview |
| `agent.command` | Shell or command to run (default: `$SHELL`) |
| `agent.args` | Arguments passed to the command |

## License

MIT
