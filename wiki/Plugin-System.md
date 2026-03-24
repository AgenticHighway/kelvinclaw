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

- tool plugins: `tool_name`, `tool_input_schema`, `capability_scopes.env_allow`, `operational_controls.fuel_budget`
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

## Related Pages

- [Plugin Registry and Trust](Plugin-Registry-and-Trust)
- [Testing and Validation](Testing-and-Validation)

## Reference

- [Kelvin Core SDK](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/KELVIN_CORE_SDK.md)
- [Plugin author kit](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/plugins/plugin-author-kit.md)
- [Model plugin ABI](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/plugins/model-plugin-abi.md)
- [Tool plugin ABI](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/plugins/tool-plugin-abi.md)
- [Channel plugin ABI](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/gateway/channel-plugin-abi.md)
