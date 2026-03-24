# Tool Plugin ABI (`wasm_tool_v1`)

This document defines the v1 ABI and runtime contract for installed WASM tool plugins.

## Runtime Kind

- Manifest `runtime` must be `wasm_tool_v1`.
- Manifest `capabilities` must include `tool_provider`.
- Manifest must include:
  - `entrypoint`
  - `tool_name` (auto-derived from `id` if omitted, replacing `.` and `-` with `_`)

Optional manifest fields:

- `tool_input_schema`: JSON Schema object defining accepted arguments (defaults to `{"type":"object"}`)
- `capability_scopes.network_allow_hosts`: hostname allowlist required to use `http_call`
- `capability_scopes.env_allow`: list of env var names the guest may read via `get_env`
- `operational_controls.fuel_budget`: overrides the default WASM fuel limit

Example manifest:

```json
{
  "id": "kelvin.websearch",
  "name": "Kelvin Web Search",
  "version": "0.1.0",
  "api_version": "1.0.0",
  "runtime": "wasm_tool_v1",
  "entrypoint": "plugin.wasm",
  "entrypoint_sha256": "e14038fb8e1a02418a617b518fac7ac1b374fdd530df8d2747ee47adbdc74be5",
  "capabilities": ["tool_provider", "network_egress"],
  "capability_scopes": {
    "network_allow_hosts": ["api.search.brave.com"],
    "env_allow": ["BRAVE_API_KEY"]
  },
  "operational_controls": {
    "timeout_ms": 15000,
    "max_retries": 1,
    "fuel_budget": 20000000
  },
  "tool_name": "kelvin_websearch",
  "tool_input_schema": {
    "type": "object",
    "properties": {
      "query": { "type": "string", "description": "The search query." },
      "count": { "type": "integer", "description": "Number of results (1-20, default 5)." }
    },
    "required": ["query"]
  },
  "quality_tier": "unsigned_local"
}
```

## Guest Exports

The WASM guest module must export:

- `alloc(len: i32) -> i32`
- `dealloc(ptr: i32, len: i32)`
- `handle_tool_call(input_ptr: i32, input_len: i32) -> i64`
- linear memory export `memory`

`handle_tool_call` receives the UTF-8 tool arguments JSON and returns a packed `i64`:
`(output_ptr << 32) | output_len`, pointing at a UTF-8 `ToolCallResult` JSON in guest memory.

A return value of `0` (or any value where the upper 32 bits are zero and lower 32 bits are zero)
indicates a hard failure with no output. The host treats this as an error result.

Optional for backward compatibility:

- `run() -> i32`: v1 fallback entry point. Modules that export only `run` and not
  `handle_tool_call` use the legacy execution path with no JSON output.

## Host Imports

All imports must come from module `"claw"`. Any import from a different module name is
rejected at load time.

### `log(level: i32, msg_ptr: i32, msg_len: i32) -> i32`

Always available. Writes a UTF-8 message to the host log.

Level values: `0` = trace, `1` = debug, `2` = info, `3` = warn, `4` = error.

Returns `0` on success.

### `http_call(req_ptr: i32, req_len: i32, resp_ptr: i32, resp_max: i32) -> i32`

Available when `capabilities` includes `network_egress` **and**
`capability_scopes.network_allow_hosts` is non-empty. Rejected at load time otherwise.

The guest provides a pre-allocated response buffer at `resp_ptr` of size `resp_max`.
The host writes the response JSON into that buffer and returns the number of bytes written,
or `0` on error.

**Request JSON** (written by guest at `req_ptr`):
```json
{
  "url": "https://api.example.com/search?q=hello",
  "method": "GET",
  "headers": { "Accept": "application/json" },
  "body": ""
}
```

Supported methods: `GET`, `POST`, `PUT`, `DELETE`, `PATCH`.

**Response JSON** (written by host into `resp_ptr`):
```json
{ "status": 200, "body": "<response body as string>" }
```

**Hostname enforcement:** Requests to hosts not in `network_allow_hosts` return
`{"status":403,"body":"host not allowed"}` without any network call being made.
The allowlist supports exact hostnames (`"api.example.com"`) and subdomain wildcards
(`"*.example.com"`, which also matches the apex `example.com`). The value `"*"` allows
any host (for development only).

**Truncation:** If the serialized response JSON exceeds `resp_max` bytes, the host
truncates the body before writing. If even a truncated response does not fit, the body
is replaced with `"[response too large]"`.

### `get_env(key_ptr: i32, key_len: i32, val_ptr: i32, val_max: i32) -> i32`

Available when `capability_scopes.env_allow` is non-empty. Rejected at load time otherwise.

Reads the environment variable named by the UTF-8 key at `key_ptr`. The key must match
an entry in `env_allow` exactly (case-sensitive). If the key is allowed and the variable
is set, writes the value as UTF-8 into `val_ptr` and returns the number of bytes written.
Returns `0` if the key is not allowed, not set, or on any error.

## JSON Payloads

### Input to `handle_tool_call`

**The input is the raw tool arguments object — not a `ToolCallInput` wrapper struct.**

The bytes at `input_ptr` contain the JSON object matching the plugin's `tool_input_schema`,
exactly as the model sent it. Example:

```json
{ "query": "project nanda", "count": 5 }
```

There is no outer wrapper with `run_id`, `session_id`, or `workspace_dir`. Those fields
are available to the Rust host (`InstalledWasmTool::execute_once` in `kelvin-brain`) but
are not forwarded to the WASM guest. Do not look for an `"arguments"` key in the input bytes.

### Output from `handle_tool_call`

The guest must write a `ToolCallResult` JSON and return a packed pointer/length:

```json
{
  "summary":      "string — short description shown in logs",
  "output":       "string or null — primary output returned to the model",
  "visible_text": "string or null — human-readable UI output if different from output",
  "is_error":     false
}
```

Set `is_error: true` when the tool call failed. The `output` field should contain an
error description when `is_error` is true, so the model can see what went wrong.

## Runtime Controls

The host enforces the following limits per call:

| Control | Default | Override |
|---------|---------|----------|
| Max module size | 512 KiB | — |
| Max request JSON | 256 KiB | — |
| Max response JSON | 256 KiB | — |
| Fuel budget | 1,000,000 | `operational_controls.fuel_budget` in manifest |
| Timeout | per operational_controls | `operational_controls.timeout_ms` |
| Network hostname allowlist | empty (no HTTP) | `capability_scopes.network_allow_hosts` |
| Env var allowlist | empty (no env reads) | `capability_scopes.env_allow` |

Additional operational controls (all optional in manifest):

- `max_retries`: number of times the host retries a failed call
- `max_calls_per_minute`: per-plugin rate limit
- `circuit_breaker_failures`: number of consecutive failures before opening the circuit
- `circuit_breaker_cooldown_ms`: time before retrying after circuit opens

Each plugin instance runs in a fresh WASM module instance per call. Guest memory does not
persist between calls; the arena allocator pattern (static buffer, bump pointer, no-op
dealloc) is idiomatic for this reason.

## Compatibility

- The runtime is versioned by the `runtime` manifest field (`wasm_tool_v1`).
- The host import module is `"claw"` (unversioned in the module name).
- Modules exporting `handle_tool_call` use the v2 shared-memory path (JSON input/output).
- Modules exporting only `run()` use the v1 legacy path (no JSON exchange).
- Breaking ABI changes require a new runtime version (`wasm_tool_v2`) with side-by-side
  support during migration.
