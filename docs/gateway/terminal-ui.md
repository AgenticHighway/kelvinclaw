# Terminal UI (`kelvin-tui`)

`kelvin-tui` is the interactive terminal interface for KelvinClaw. It connects to a running
gateway over WebSocket, streams agent events in real time, and provides a full-featured text
editing and selection experience in any modern terminal — including inside Docker containers
where no clipboard daemon is available.

---

## Quick Start

Start the gateway (if not already running):

```bash
scripts/kelvin-local-profile.sh start
```

Then launch the TUI:

```bash
cargo run -p kelvin-tui
```

Or, if you have built a release binary:

```bash
./kelvin-tui
```

By default the TUI connects to `ws://127.0.0.1:34617` on session `main`. Use the flags
below to change either.

---

## CLI Flags

| Flag | Default | Description |
|---|---|---|
| `--gateway-url <url>` | `ws://127.0.0.1:34617` | WebSocket gateway address |
| `--auth-token <token>` | _(none)_ | Auth token sent on connect |
| `--session <id>` | `main` | Session / lane identifier |
| `--help`, `-h` | | Print usage and exit |

Examples:

```bash
# Remote gateway with auth
kelvin-tui --gateway-url ws://my-server:34617 --auth-token $KELVIN_GATEWAY_TOKEN

# Named session
kelvin-tui --session project-alpha
```

---

## Layout

```
┌─────────────────────────────────────────────────────┐
│  Chat (drag to select · ^C copy · PgUp/PgDn scroll) │
│                                                     │
│  > user message                                     │
│  ─────────────────────────────────────────────────  │
│                                                     │
│  ● assistant response                               │
│    continuation line                                │
│                                                     │
├─────────────────────────────────────────────────────┤
│  Tools                                              │
│  Tool          Phase    Summary              Duration│
│  bash          done     exit 0               142ms  │
│  Read          done     src/main.rs          8ms    │
├─────────────────────────────────────────────────────┤
│  Input (Enter=submit, ^C^C=quit)                    │
│  > _                                                │
├─────────────────────────────────────────────────────┤
│  ws://127.0.0.1:34617  connected  ·  ^T hide tools  │
└─────────────────────────────────────────────────────┘
```

**Chat panel** — scrollable conversation history. User messages are prefixed with cyan `> `.
Assistant messages are prefixed with white `● `. System messages appear in gray italic.
Messages are separated by a horizontal rule.

**Tools panel** — live tool execution log. Shows each tool call with its name, phase
(`running` / `done` / `error`), a one-line summary, and elapsed time. Toggle with `Ctrl+T`.

**Input box** — multi-line text entry. The cursor is always visible. Long pastes are
automatically collapsed to a compact label.

**Status bar** — shows the gateway URL, connection state, active run phase, and contextual
hints.

---

## Keybindings

### Input Editing

| Key | Action |
|---|---|
| `Enter` | Submit prompt |
| `Backspace` | Delete character before cursor |
| `Delete` | Delete character after cursor |
| `Left` / `Right` | Move cursor one character |
| `Ctrl+Left` / `Ctrl+Right` | Jump to previous / next word boundary |
| `Home` | Move to start of visual line |
| `End` | Move to end of visual line |
| `Up` / `Down` | Browse input history |

### Scrolling

| Key | Action |
|---|---|
| `Shift+Up` / `Shift+Down` | Scroll chat 3 lines |
| `Page Up` / `Page Down` | Scroll chat 10 lines |
| `Mouse Wheel Up/Down` | Scroll chat 3 lines (or tools panel 1 line when hovering it) |

The chat auto-scrolls to the bottom as new content arrives. Manual scrolling unpins it;
scrolling back to the bottom re-pins.

### Selection & Clipboard

| Key / Action | Effect |
|---|---|
| `Left-click + drag` in chat | Select text (character-level) |
| `Left-click` on tools row | Select that tool row |
| `Ctrl+C` with active selection | Copy selected text to clipboard |
| `Ctrl+C` with no selection | First press: shows quit hint; second press within 500 ms: quit |
| `Escape` | Clear current selection |

Copied text is sent via **OSC 52** — an escape sequence that passes the clipboard payload
through the TTY to the host terminal emulator. This works without any clipboard daemon,
including inside Docker. Supported by Kitty, Alacritty, WezTerm, iTerm2, Windows Terminal,
and most other modern terminals.

Sidebar decorations (`> `, `● `, `  `) and separator lines are stripped from the copied text.
Each separator between messages produces a blank line in the output.

### UI Controls

| Key | Action |
|---|---|
| `Ctrl+T` | Toggle tools panel on / off |
| `Ctrl+C` `Ctrl+C` | Quit (two presses within 500 ms) |

---

## Mouse Support

The TUI requests mouse capture on startup. In most terminals this enables:

- **Click** — place the selection anchor (chat) or highlight a tool row (tools panel).
- **Click + drag** — extend the selection across lines.
- **Scroll wheel** — scroll the chat or tools panel.

Mouse capture requires a terminal that supports `ENABLE_MOUSE_CAPTURE` (virtually all modern
terminals do). If mouse events are not received, verify that your SSH client or multiplexer
is not consuming them.

---

## Input History

The TUI keeps the last 500 submitted prompts in an in-memory history.

- `Up` / `Down` arrows navigate backward and forward through history.
- The current (unsaved) input is preserved and restored when you reach the end of history.
- History is per-session and not persisted between runs.

---

## Paste Handling

Short pastes (fewer than 3 lines and under 200 bytes) are inserted verbatim.

Large pastes are automatically collapsed to a compact label shown in magenta:

```
> [pasted 42 lines · 3.1 KB]
```

The full text is still sent when you press `Enter` — the label is display-only. Backspace
removes the entire pasted block at once.

---

## Tools Panel

The tools panel shows every tool call made during the current session.

| Column | Description |
|---|---|
| Tool | Tool name as reported by the agent |
| Phase | `running` (yellow), `done` (green), `error` (red) |
| Summary | One-line description of the call (if provided) |
| Duration | Elapsed time from call start to completion, or `…` while running |

Click a row to select it, then `Ctrl+C` to copy the row as tab-separated text. Toggle the
panel with `Ctrl+T` to reclaim vertical space.

---

## Connection & Reconnection

On startup the TUI attempts to connect to the gateway. The status bar shows the current
state:

| State | Meaning |
|---|---|
| `connecting` | Initial connection in progress |
| `connected` | WebSocket handshake succeeded |
| `disconnected` | Connection closed cleanly |
| `error: …` | Connection failed or lost unexpectedly |

If the gateway becomes unreachable the TUI reconnects automatically using exponential
backoff (250 ms → 500 ms → 1 s → 2 s, then every 2 s). A system message appears in the
chat on each reconnect attempt and on successful reconnection.

---

## Building from Source

```bash
# Debug build
cargo build -p kelvin-tui

# Release build
cargo build -p kelvin-tui --release

# Run directly
cargo run -p kelvin-tui -- --gateway-url ws://127.0.0.1:34617
```

The binary has no runtime dependencies beyond the terminal itself.

---

## Limitations

- **OSC 52 clipboard** — requires a terminal emulator that honours OSC 52. Older terminals
  (e.g. the Linux console, some SSH clients) will silently ignore the copy sequence.
- **Mouse capture** — may interfere with terminal-level text selection. Disable mouse
  capture by not forwarding `ENABLE_MOUSE_CAPTURE` in your terminal settings if preferred.
- **Wide characters** — CJK and emoji are handled via `unicode-width`. Selection column
  boundaries are accurate for standard Unicode but may be off by one for unusual combining
  sequences.
- **History** — input history is in-memory only and does not persist across restarts.
