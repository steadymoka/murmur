# murmur

**Terminal command center for AI coding sessions.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021_edition-orange.svg)](https://www.rust-lang.org/)

Murmur is a lightweight terminal multiplexer built for developers who work with AI coding agents. It provides a raw PTY passthrough with prefix-key session management — no mode switching, no screen takeover.

<!-- screenshot -->

## Features

- **Prefix-key session management** — Create, switch, and delete sessions without leaving your terminal (`Ctrl+\`)
- **Prompt pinning** — Automatically captures your last command as a persistent context bar
- **Claude Code detection** — Recognizes AI coding sessions and shows a pin bar with session context
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

Murmur launches with a shell session in the current directory. A hint bar at the bottom shows available prefix keys. During AI coding sessions, a pin bar appears above it with session context.

## Keybindings

All input is forwarded to the PTY. Use `Ctrl+\` as a prefix key to access commands.

| Key | Action |
| --- | --- |
| `Ctrl+\` `n` | New session |
| `Ctrl+\` `d` | Delete current session |
| `Ctrl+\` `1`–`9` | Switch to session N |
| `Ctrl+\` `q` | Quit |

## How It Works

Murmur attaches your terminal to a PTY session. Output is written directly to stdout through a scroll region that reserves space for persistent context bars. A VT100 parser runs in parallel to track screen state, window titles, and input for prompt pinning.

The prefix key (`Ctrl+\`) is the only input murmur intercepts — everything else passes through untouched. Session management (create, delete, switch) is handled entirely through prefix key combinations.

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
| `name` | Project name displayed in hint bar |
| `agent.command` | Shell or command to run (default: `$SHELL`) |
| `agent.args` | Arguments passed to the command |

## License

MIT
