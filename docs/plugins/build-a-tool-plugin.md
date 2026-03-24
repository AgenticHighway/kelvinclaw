# Build a Tool Plugin

This is the supported KelvinClaw contributor path for new `wasm_tool_v1` plugins.
You do not need to modify Kelvin core internals to follow it.

## Prerequisites

- `cargo`
- `rustup`
- `jq`

If you do not want to install Rust locally, the supported Docker path uses the
repo-owned Ubuntu 24.04 plugin-author image:

```bash
scripts/plugin-author-docker.sh -- bash
```

That wrapper builds a cached local image, mounts the repository, and reuses
repo-local Cargo registry/git/target caches for fast iteration.

## Option 1: Scaffold a New Tool Plugin

```bash
scripts/kelvin-plugin.sh new \
  --id acme.websearch \
  --name "Acme Web Search" \
  --runtime wasm_tool_v1
```

That command creates a complete, ready-to-build project at `./plugin-acme.websearch/`:

- `plugin.json` — manifest with `tool_name`, `tool_input_schema`, capabilities
- `src/lib.rs` — Rust WASM guest with arena allocator and `handle_tool_call` entry point
- `Cargo.toml` — `no_std` cdylib crate targeting `wasm32-unknown-unknown`
- `build.sh` — compile, copy `.wasm` to `payload/`, patch SHA-256 in manifest
- `Makefile` — convenience targets for the full authoring loop
- `README.md`, `.gitignore`

Then iterate with:

```bash
cd ./plugin-acme.websearch
make build    # compile WASM and patch plugin.json SHA-256
make test     # validate manifest structure and capability declarations
make pack     # create dist/acme.websearch-0.1.0.tar.gz
make install  # install into local Kelvin plugin home
make smoke    # end-to-end: build, pack, install, run
```

Or equivalently with the script directly:

```bash
cd ./plugin-acme.websearch
./build.sh
../scripts/kelvin-plugin.sh test --manifest ./plugin.json
../scripts/kelvin-plugin.sh pack --manifest ./plugin.json
../scripts/kelvin-plugin.sh install --package ./dist/acme.websearch-0.1.0.tar.gz
../scripts/kelvin-plugin.sh smoke --manifest ./plugin.json
```

The same flow in Docker:

```bash
scripts/plugin-author-docker.sh -- bash -lc '
  scripts/kelvin-plugin.sh new \
    --id acme.websearch \
    --name "Acme Web Search" \
    --runtime wasm_tool_v1
  cd ./plugin-acme.websearch
  make build test pack install smoke
'
```

## Option 2: Copy the Maintained Example

The canonical first-party tool plugin is:

- `plugins/kelvin-websearch-plugin`

Copy it, rename the manifest fields, and adjust:

- `id`
- `name`
- `tool_name`
- `tool_input_schema`
- `capability_scopes.network_allow_hosts` (or remove if no HTTP needed)
- `capability_scopes.env_allow` (or remove if no env vars needed)
- `operational_controls.fuel_budget` (raise if your tool does heavy computation)

Then update `src/lib.rs` to implement your tool logic.

## Manifest Essentials

Tool-plugin-specific manifest fields:

**`tool_name`** — The name exposed to the model as a callable tool. Auto-derived from `id`
(`.` and `-` replaced with `_`) if omitted. Use descriptive names: `kelvin_websearch`,
`acme_calculator`.

**`tool_input_schema`** — JSON Schema object defining the tool's accepted arguments. The
model uses this schema to construct calls. Defaults to `{"type":"object"}` (accepts anything).

**`capabilities`** — Must include `"tool_provider"`. Add `"network_egress"` if the plugin
makes HTTP requests.

**`capability_scopes.network_allow_hosts`** — Required to use the `http_call` host import.
Lists exact hostnames or wildcard patterns (`"*.example.com"`, `"*"` for any host).

**`capability_scopes.env_allow`** — Required to use the `get_env` host import. Lists env var
names the guest may read (case-sensitive).

**`operational_controls.fuel_budget`** — Overrides the default WASM fuel limit (1,000,000).
Plugins that do significant computation or JSON parsing (especially `no_std` implementations)
may need a higher budget. The websearch plugin uses 20,000,000.

## Input/Output Contract

**Input:** `handle_tool_call` receives the **raw tool arguments object** — the same JSON the
model passed to the tool, matching your `tool_input_schema`. There is no outer wrapper.

```json
{ "query": "project nanda", "count": 5 }
```

Do not look for `"arguments"`, `"run_id"`, or `"session_id"` keys in the input bytes.
See [Tool Plugin ABI](tool-plugin-abi.md) for the full specification.

**Output:** Write a `ToolCallResult` JSON into guest memory and return `(ptr << 32) | len`:

```json
{
  "summary":      "searched for: project nanda",
  "output":       "1. Result title\nhttps://example.com\nDescription...",
  "visible_text": null,
  "is_error":     false
}
```

## Host Imports

Available imports from the `claw` module:

| Import | Signature | Requires |
|--------|-----------|---------|
| `log` | `(level: i32, msg_ptr: i32, msg_len: i32) -> i32` | Always available |
| `http_call` | `(req_ptr: i32, req_len: i32, resp_ptr: i32, resp_max: i32) -> i32` | `network_egress` cap + `network_allow_hosts` |
| `get_env` | `(key_ptr: i32, key_len: i32, val_ptr: i32, val_max: i32) -> i32` | `env_allow` in capability_scopes |

For `http_call`, the guest provides a pre-allocated response buffer and the host writes
`{"status":<int>,"body":"..."}` into it. Blocked hosts receive `{"status":403,...}` without
any network call. See [Tool Plugin ABI](tool-plugin-abi.md) for full details.

## Local Install and Run

Local development plugins can stay `unsigned_local`:

```bash
scripts/kelvin-plugin.sh install --package ./dist/acme.websearch-0.1.0.tar.gz
scripts/kelvin-plugin.sh smoke --manifest ./plugin.json
```

Kelvin prints a warning for `unsigned_local`, but still installs the package so you can
develop without access to the first-party signing platform.

## Publishing

Local/community development happens in source repos like `kelvinclaw` or your own plugin
repo. The `kelvinclaw-plugins` repository is only for published artifacts:

- package tarballs
- `index.json`
- trust metadata

Only AgenticHighway first-party releases currently use the official KMS signing platform.
Community authors can keep using unsigned local plugins or their own PEM signing flow:

```bash
scripts/plugin-sign.sh \
  --manifest ./plugin.json \
  --private-key /path/to/ed25519-private.pem \
  --publisher-id your.publisher.id \
  --trust-policy-out ./trusted_publishers.json
```
