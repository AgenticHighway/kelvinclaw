# KelvinClaw

KelvinClaw is a secure, stable, and modular harness for agentic AI workflows.

## Quick Start

```bash
tar -xzf kelvinclaw-<version>-linux-<arch>.tar.gz
cd kelvinclaw-<version>-linux-<arch>

./kelvin init
./kelvin
```

On first run, `kelvin init` writes `~/.kelvinclaw/.env`, generates a gateway token, and helps you choose a provider. `./kelvin` then bootstraps required plugins into `~/.kelvinclaw`.

### Prerequisites

- `curl`
- `tar`
- `awk`
- `ca-certificates` (on minimal Linux images)

---

## Configuration

All launchers auto-read `.env` files in this order:

1. `~/.kelvinclaw/.env.local`
2. `~/.kelvinclaw/.env`
3. `./.env.local`
4. `./.env`

`~/.kelvinclaw/.env` is the canonical config path for release and Homebrew installs.

The recommended path is to let `kelvin init` create `~/.kelvinclaw/.env` for you:

```bash
./kelvin init
```

You can still copy `.env.example` manually if you want a project-local config.

Key variables:

| Variable | Description |
|---|---|
| `KELVIN_MODEL_PROVIDER` | Plugin ID of the active model provider |
| `ANTHROPIC_API_KEY` | Required when using `kelvin.anthropic` |
| `OPENAI_API_KEY` | Required when using `kelvin.openai` |
| `OPENROUTER_API_KEY` | Required when using `kelvin.openrouter` |
| `KELVIN_GATEWAY_TOKEN` | Auth token for gateway and TUI (generate with `openssl rand -hex 32`) |
| `BRAVE_API_KEY` | Required when using `kelvin.websearch` |
| `KELVIN_PLUGIN_INDEX_URL` | Plugin index URL (required for `kpm install`, `search`, `update`) |

---

## kelvin-gateway

Start and manage the KelvinClaw gateway daemon.

```bash
./kelvin-gateway start              # start as background daemon
./kelvin-gateway start --foreground # run attached to terminal
./kelvin-gateway stop
./kelvin-gateway restart
./kelvin-gateway status             # show pid, provider, uptime, log path
./kelvin-gateway start -- --bind 0.0.0.0:34617  # pass args to the gateway binary
```

---

## kelvin-tui

Launch the terminal user interface. The gateway must be running first.

```bash
./kelvin-tui
```

---

## kpm — Plugin Manager

Install, manage, and explore KelvinClaw plugins.

```bash
./kpm install kelvin.anthropic     # install a plugin
./kpm install kelvin.websearch
./kpm list                         # list installed plugins
./kpm status                       # show active provider and installed plugins
./kpm search <query>               # search the plugin index
./kpm info <plugin-id>             # show plugin details
./kpm update                       # update all installed plugins
./kpm uninstall <plugin-id>        # remove a plugin
```

---

## Available Plugins

### Model Providers

Install one of the following to use a real LLM. The default `kelvin.echo` requires no API key and echoes responses back — useful for testing.

| Plugin ID | Provider | Required env var |
|---|---|---|
| `kelvin.echo` | Built-in echo (no key needed) | — |
| `kelvin.anthropic` | Anthropic Claude | `ANTHROPIC_API_KEY` |
| `kelvin.openai` | OpenAI | `OPENAI_API_KEY` |
| `kelvin.openrouter` | OpenRouter | `OPENROUTER_API_KEY` |

Install a model provider and set it in `.env`:

```bash
./kpm install kelvin.anthropic
# then in .env:
# KELVIN_MODEL_PROVIDER=kelvin.anthropic
# ANTHROPIC_API_KEY=sk-ant-...
```

### Tool Plugins

| Plugin ID | Description | Required env var |
|---|---|---|
| `kelvin.websearch` | Web search via Brave Search API | `BRAVE_API_KEY` |

```bash
./kpm install kelvin.websearch
# then in .env:
# BRAVE_API_KEY=...
```

### CLI Plugin

`kelvin.cli` is the built-in CLI interaction plugin. It is auto-bootstrapped on first run and does not need to be installed manually.

---

## Bundle Layout

```
kelvinclaw-<version>-<platform>/
  bin/                   # compiled binaries (kelvin-host, kelvin-gateway, kelvin-tui, ...)
  share/                 # support scripts and plugin env
  kelvin                 # launcher
  kelvin-gateway         # gateway service manager launcher
  kpm                    # plugin manager launcher
  kelvin-tui             # TUI launcher
  .env.example           # configuration template
  LICENSE
  README.md
```

---

## More Information

- Full documentation and source: https://github.com/AgenticHighway/kelvinclaw
- Latest releases: https://github.com/AgenticHighway/kelvinclaw/releases/latest
