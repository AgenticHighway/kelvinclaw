# KelvinClaw

KelvinClaw is a secure, stable, and modular harness for agentic AI workflows.

## Quick Start

```bash
tar -xzf kelvinclaw-<version>-linux-<arch>.tar.gz
cd kelvinclaw-<version>-linux-<arch>

./bin/kelvin init    # interactive first-run setup (provider, API key, token)
./bin/kelvin         # choose CLI chat or TUI on first run
```

`kelvin init` writes `~/.kelvinclaw/.env`, generates auth keys, and creates a permissive trust policy.
After that, the first interactive bare `kelvin` launch asks whether you want CLI chat or the TUI app,
remembers that choice in `~/.kelvinclaw/preferences.env`, and routes future `kelvin` launches there by default.
Run `kelvin /help` from the shell to show the interactive quickstart.

---

## Configuration

`kelvin init` handles first-run configuration interactively. To edit settings afterward, update `~/.kelvinclaw/.env`.

`.env` files are loaded in this order (first match per key wins):

1. `~/.kelvinclaw/.env.local`
2. `~/.kelvinclaw/.env`
3. `./.env.local`
4. `./.env`

`~/.kelvinclaw/.env` is the canonical config path for release and Homebrew installs.

Kelvin stores remembered launcher preferences separately in:

1. `~/.kelvinclaw/preferences.env`

Variables already in the environment are never overwritten.

Key variables:

| Variable | Description |
|---|---|
| `KELVIN_MODEL_PROVIDER` | Plugin ID of the active model provider |
| `KELVIN_GATEWAY_TOKEN` | Auth token for gateway and TUI |
| `ANTHROPIC_API_KEY` | Required when using `kelvin.anthropic` |
| `OPENAI_API_KEY` | Required when using `kelvin.openai` |
| `OPENROUTER_API_KEY` | Required when using `kelvin.openrouter` |
| `BRAVE_API_KEY` | Required when using `kelvin.websearch` |
| `KELVIN_PLUGIN_INDEX_URL` | Plugin index URL |

---

## kelvin — Unified CLI

All lifecycle and plugin operations go through the single `kelvin` binary.

Add `bin/` to your `PATH` so you can use `kelvin` without a path prefix:

```bash
export PATH="/path/to/kelvinclaw-<version>-<platform>/bin:$PATH"
```

The examples below assume `kelvin` is on your `PATH`.

### Stack management

```bash
kelvin                          # start full stack (gateway + memory) and open TUI
kelvin start                    # start daemons in background, print status
kelvin start --no-memory        # start gateway only
kelvin stop                     # stop all background daemons
kelvin tui                      # open TUI (gateway must already be running)
```

### Gateway

```bash
kelvin gateway start            # start gateway daemon
kelvin gateway start --foreground
kelvin gateway start -- --bind 0.0.0.0:34617   # pass args to gateway binary
kelvin gateway stop
kelvin gateway restart
kelvin gateway status
kelvin gateway approve-pairing <code>
```

### Memory controller

```bash
kelvin memory start
kelvin memory stop
kelvin memory restart
kelvin memory status
```

### Plugin manager (`kpm`)

`kelvin plugin` and `kelvin kpm` are interchangeable.

```bash
kelvin plugin install kelvin.anthropic
kelvin plugin install kelvin.websearch
kelvin plugin install --package ./my-plugin-0.1.0.tar.gz
kelvin plugin list
kelvin plugin status
kelvin plugin search <query>
kelvin plugin info <plugin-id>
kelvin plugin update
kelvin plugin update <plugin-id> --dry-run
kelvin plugin uninstall <plugin-id>
kelvin plugin uninstall <plugin-id> --yes
```

### Diagnostics

```bash
kelvin medkit           # offline diagnostics (env, plugins, daemons)
kelvin medkit --fix     # attempt to fix problems automatically
kelvin medkit --json    # machine-readable output
kelvin doctor           # live WebSocket probe of running gateway
```

### System service

```bash
kelvin service install-systemd      # install systemd user unit
kelvin service render-systemd       # print unit to stdout
kelvin service install-launchd      # install launchd plist (macOS)
kelvin service render-launchd       # print plist to stdout
```

### Shell completions

```bash
kelvin completions bash             # print completion script
kelvin completions zsh
kelvin completions fish
kelvin completions --write bash     # write to default location
```

---

## Available Plugins

### Model Providers

Install one of the following to use a real LLM. The default `kelvin.echo` requires no API key — useful for testing.

| Plugin ID | Provider | Required env var |
|---|---|---|
| `kelvin.echo` | Built-in echo (no key needed) | — |
| `kelvin.anthropic` | Anthropic Claude | `ANTHROPIC_API_KEY` |
| `kelvin.openai` | OpenAI | `OPENAI_API_KEY` |
| `kelvin.openrouter` | OpenRouter | `OPENROUTER_API_KEY` |
| `kelvin.ollama` | Ollama (local) | — |

```bash
kelvin plugin install kelvin.anthropic
# then in ~/.kelvinclaw/.env:
# KELVIN_MODEL_PROVIDER=kelvin.anthropic
# ANTHROPIC_API_KEY=sk-ant-...
```

### Tool Plugins

| Plugin ID | Description | Required env var |
|---|---|---|
| `kelvin.websearch` | Web search via Brave Search API | `BRAVE_API_KEY` |

### CLI Plugin

`kelvin.cli` is auto-installed on first run and does not need to be installed manually.

---

## Bundle Layout

```
kelvinclaw-<version>-<platform>/
  bin/
    kelvin                  # unified CLI — the primary entrypoint
    kelvin-gateway          # gateway daemon binary
    kelvin-tui              # terminal UI binary
    kelvin-host             # host binary
    kelvin-memory-controller
    kelvin-registry
  share/
    official-first-party-plugins.env
  .env.example              # configuration template
  LICENSE
  README.md
```

Add `bin/` to your `PATH` to use `kelvin` from any directory:

```bash
export PATH="/path/to/kelvinclaw-<version>-<platform>/bin:$PATH"
```

---

## More Information

- Full documentation and source: https://github.com/AgenticHighway/kelvinclaw
- Latest releases: https://github.com/AgenticHighway/kelvinclaw/releases/latest
