# Plugin System

KelvinClaw’s public extension surface is the Kelvin Core SDK lane. The SDK lane is the supported path for installable tools and model providers, while the root lane is reserved for trusted maintainers.

## Root Lane vs SDK Lane

Root lane:

- direct internal integration
- trusted code only
- not a security boundary

SDK lane:

- installable plugin packages
- compatibility-gated admission
- explicit capability declarations
- recommended for ecosystem extensions

See also:

- [Security Model](Security-Model)

## Plugin Runtimes

Supported package runtimes include:

- `wasm_tool_v1`
- `wasm_model_v1`

Model plugins should prefer the generic host-routed `provider_profile` field over legacy provider-specific imports when possible.

Examples of provider profiles:

- `openai.responses`
- `anthropic.messages`

## Manifest Shape

Common manifest fields:

- `id`
- `name`
- `version`
- `api_version`
- `capabilities`
- `entrypoint`
- optional integrity and trust metadata such as `entrypoint_sha256`, `publisher`, `quality_tier`

Runtime-specific fields:

- tool plugins: `tool_name`, `tool_input_schema`, `capability_scopes.env_allow`, `capability_scopes.network_allow_hosts`, `operational_controls.fuel_budget`
- model plugins: `provider_name`, `model_name`, `provider_profile`

## Installed Runtime Behavior

Installed plugins are loaded from the plugin home and validated before they can participate in runtime composition.

The installed-plugin loader enforces:

- manifest integrity
- optional mandatory signatures
- trust policy membership
- publisher revocation and plugin pinning
- capability scopes for file and network access
- operational controls such as timeout, retries, rate limit, and circuit breaker

## First-Party Tool Pack

Kelvin ships first-party SDK tools through the same plugin path:

- `fs_safe_read`
- `fs_safe_write`
- `web_fetch_safe`
- `schedule_cron`
- `session_tools`

Sensitive operations require explicit per-call approvals.

## First-Party Tool Plugins

Community-extensible tool plugins built from source in `plugins/` and baked into the
Docker runtime image at build time:

| Plugin ID | Source directory | Capabilities | API key env var |
|---|---|---|---|
| `kelvin.websearch` | `plugins/kelvin-websearch-plugin` | `tool_provider`, `network_egress` | `BRAVE_API_KEY` |

`scripts/gateway-plugin-init.sh` automatically installs all builtin tool plugins at
gateway startup by scanning for manifests with the `tool_provider` capability. No manual
install step is needed for plugins that ship in the image.

## Author Workflow

Add scripts to `PATH`:

```bash
export PATH="$PWD/scripts:$PATH"
```

Create, test, package, and verify a plugin:

```bash
kelvin plugin new --id acme.echo --name "Acme Echo" --runtime wasm_tool_v1
kelvin plugin test --manifest ./plugin-acme.echo/plugin.json
kelvin plugin pack --manifest ./plugin-acme.echo/plugin.json
kelvin plugin verify --package ./plugin-acme.echo/dist/acme.echo-0.1.0.tar.gz
```

## First-Party Model Plugins

First-party plugins are built from source in `plugins/` and baked into the Docker runtime
image at build time. No external index is required.

Bundled providers:

| Plugin ID | Source directory | API key env var |
|---|---|---|
| `kelvin.echo` | `plugins/kelvin-echo-plugin` | — |
| `kelvin.anthropic` | `plugins/kelvin-anthropic-plugin` | `ANTHROPIC_API_KEY` |
| `kelvin.openrouter` | `plugins/kelvin-openrouter-plugin` | `OPENROUTER_API_KEY` |

Set the active provider via `KELVIN_MODEL_PROVIDER` in `.env` or the environment before
running `docker compose up`. The init container installs the selected plugin automatically.

To rebuild plugins after source changes:

```bash
docker compose build   # plugin-builder stage recompiles plugins/
```

`kelvin.cli` (the required tool plugin) is vendored as a prebuilt tarball at
`release/vendor/kelvin.cli-0.1.2.tar.gz` and installed by `kelvin-setup.sh` on first run.

## Plugin Index and kpm

Outside of Docker, plugins are installed from a plugin index served at `KELVIN_PLUGIN_INDEX_URL`.
The index is a JSON document listing available plugins with their metadata and download URLs.

**`kpm`** (Kelvin Plugin Manager) is the command-line tool for managing plugins in release
bundles and local environments. It is bundled in the release archive as `./kpm`.

### Subcommands

```
kpm install <plugin-id> [--version <ver>] [--force]
kpm uninstall <plugin-id> [--yes]
kpm update [<plugin-id>] [--dry-run]
kpm search [<query>]
kpm info <plugin-id>
kpm list
kpm status
```

### Examples

Search for available plugins:

```bash
export KELVIN_PLUGIN_INDEX_URL=https://example.com/plugins/index.json
./kpm search
./kpm search anthropic
```

Install a plugin:

```bash
./kpm install kelvin.anthropic
./kpm install kelvin.anthropic --version 0.3.0
```

Inspect a plugin:

```bash
./kpm info kelvin.anthropic
```

List installed plugins and current configuration:

```bash
./kpm list
./kpm status
```

Update all installed plugins:

```bash
./kpm update
./kpm update --dry-run   # show what would be updated without installing
```

Remove a plugin:

```bash
./kpm uninstall kelvin.anthropic
./kpm uninstall kelvin.anthropic --yes   # skip confirmation prompt
```

### Environment Variables

| Variable | Required for | Default |
|---|---|---|
| `KELVIN_PLUGIN_INDEX_URL` | install, search, info, update | — |
| `KELVIN_HOME` | all | `~/.kelvinclaw` |
| `KELVIN_PLUGIN_HOME` | all | `$KELVIN_HOME/plugins` |
| `KELVIN_TRUST_POLICY_PATH` | install | `$KELVIN_HOME/trusted_publishers.json` |
| `KELVIN_MODEL_PROVIDER` | status (informational) | `kelvin.echo` |

### kelvin-gateway (service manager)

The `kelvin-gateway` script in the release bundle is a lifecycle manager for the gateway
daemon. It auto-installs the configured model provider plugin if needed, then manages
start/stop/restart/status.

```bash
export KELVIN_MODEL_PROVIDER=kelvin.anthropic
export KELVIN_PLUGIN_INDEX_URL=https://example.com/plugins/index.json
export ANTHROPIC_API_KEY=<your-key>

./kelvin-gateway start                          # daemon mode
./kelvin-gateway start -- --bind 0.0.0.0:34617 # with gateway args
./kelvin-gateway start --foreground             # attached to terminal
./kelvin-gateway status                         # show pid, provider, uptime
./kelvin-gateway stop
./kelvin-gateway restart
```

## Related Pages

- [Plugin Registry and Trust](Plugin-Registry-and-Trust)
- [Testing and Validation](Testing-and-Validation)

## Reference

- [Kelvin Core SDK](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/architecture/kelvin-core-sdk.md)
- [Plugin author kit](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/plugins/plugin-author-kit.md)
- [Model plugin ABI](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/plugins/model-plugin-abi.md)
- [Tool plugin ABI](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/plugins/tool-plugin-abi.md)
- [Channel plugin ABI](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/gateway/channel-plugin-abi.md)
