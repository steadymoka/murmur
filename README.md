# murmur

**Terminal companion for AI coding sessions.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021_edition-orange.svg)](https://www.rust-lang.org/)

Murmur wraps your terminal in a thin PTY layer that detects AI coding tools (Claude Code, Codex) and pins your prompts as a persistent context bar. No mode switching, no screen takeover — just a quiet hint bar until an AI session starts.

<!-- screenshot -->

## Features

- **Prompt pinning** — Automatically captures commands entered inside AI tools as a navigable history bar
- **AI tool detection** — Recognizes Claude Code and Codex by window title; shows the pin bar only during AI sessions
- **PTY passthrough** — Zero-interference raw terminal I/O with full ANSI support
- **Update notifications** — Background check for new releases, shown in the hint bar

## Quick Start

### Install globally

```bash
npm i -g murmur-tui
murmur
```

### Run without install

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

Murmur launches a shell in the current directory. A hint bar at the bottom shows the prefix key. When you start an AI coding tool, a pin bar appears above it with your prompt history.

## Keybindings

All input is forwarded to the PTY. `Ctrl+\` is the prefix key.

| Key | Action |
| --- | --- |
| `Ctrl+\` `[` | Previous pin (older) |
| `Ctrl+\` `]` | Next pin (newer) |
| `Ctrl+\` `x` | Delete current pin |
| `Ctrl+\` `u` | Show update info |
| `Ctrl+\` `q` | Quit |

## How It Works

Murmur attaches your terminal to a PTY and reserves a scroll region at the bottom for context bars. A VT100 parser runs in parallel to track window titles and input.

When the window title contains a known AI tool name, murmur starts recording commands you enter. Each Enter keystroke pins the command to a history bar visible above the hint bar. You can navigate through pinned prompts with the prefix key combinations.

The prefix key (`Ctrl+\`) is the only input murmur intercepts — everything else passes through untouched.

## License

MIT
