# Memory Module SDK (WASM)

## Scope

Memory extensions are WASM-only modules. Native third-party memory drivers are not loaded into root.

## ABI (MVP)

Required module exports:

- `handle_upsert`
- `handle_query`
- `handle_read`
- `handle_delete`
- `handle_health`

Host imports (`memory_host`):

- `kv_get`, `kv_put`
- `blob_get`, `blob_put`
- `emit_metric`, `log`
- `clock_now_ms`

Explicitly not available in MVP:

- host network fetch
- shell/command execution

WIT contract: `crates/kelvin-memory-module-sdk/wit/memory-module.wit`

## Manifest

Modules declare `MemoryModuleManifest`:

- `module_id`, `version`, `api_version`
- `capabilities`
- `required_host_features`
- `entrypoint`, `publisher`, `signature`

Controller enforces intersection:

- module manifest capabilities
- JWT delegated capabilities/operations
- platform enabled provider features

## Reference Module

Reference module artifacts:

- `crates/kelvin-memory-module-sdk/examples/memory_echo/memory_echo.wat`
- `crates/kelvin-memory-module-sdk/examples/memory_echo/manifest.json`

## Authoring Guidance

1. Keep module handlers side-effect minimal and deterministic.
2. Assume host limits may terminate execution at any time.
3. Avoid unbounded loops/allocations.
4. Fail with explicit non-zero result codes only for module-local errors.
