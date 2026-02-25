# memory_echo (reference module)

This is a minimal reference memory module for the Kelvin memory ABI (`v1alpha1`).

- Source: `memory_echo.wat`
- Manifest: `manifest.json`
- Required exports: `handle_upsert`, `handle_query`, `handle_read`, `handle_delete`, `handle_health`

Build wasm from WAT:

```bash
wat2wasm memory_echo.wat -o memory_echo.wasm
```

Or load this source in tests and compile with the `wat` crate.
