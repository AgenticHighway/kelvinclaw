This directory holds the compiled WASM entrypoint for the plugin.

Run `./build.sh` (or `make build`) to compile `src/lib.rs` and place the
resulting `.wasm` file here as `plugin.wasm`.

The filename must match the `entrypoint` field in `plugin.json`. The Kelvin
runtime verifies the file's SHA-256 against `entrypoint_sha256` in the manifest;
`build.sh` keeps that field up to date automatically.

`.wasm` files are excluded from version control (see `.gitignore`).
