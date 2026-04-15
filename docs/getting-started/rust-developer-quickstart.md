# Rust Developer Quickstart

This is the fastest path to try KelvinClaw from a fresh clone.
For beginner and WASM-author paths, see [docs/GETTING_STARTED.md](GETTING_STARTED.md).

## 1) Run Kelvin in one command

```bash
scripts/try-kelvin.sh "hello kelvin"
```

What this does:

- uses local `cargo` if installed
- otherwise falls back to Docker (`rust:1.93.1-bookworm` by default)
- reuses repo-local Docker cargo/build caches under `./.cache/docker` to avoid cold-start redownloads on every run
- installs/updates the first-party `kelvin_cli` WASM plugin package from the plugin index into `./.kelvin/plugins`
- runs `apps/kelvin-host` with a prompt
- auto-bootstraps Rust PATH (`$HOME/.cargo/bin`, `/usr/local/cargo/bin`) before cargo/rustup checks

Expected output includes:

- cli plugin preflight (`kelvin_cli executed ...`)
- run accepted
- lifecycle events (`start` / `end`)
- assistant payload (echo provider for MVP)

## 2) Force local or Docker mode

```bash
KELVIN_TRY_MODE=local scripts/try-kelvin.sh "status check"
KELVIN_TRY_MODE=docker scripts/try-kelvin.sh "status check"
```

Optional timeout override:

```bash
KELVIN_TRY_TIMEOUT_MS=8000 scripts/try-kelvin.sh "longer timeout"
```

## 3) Validate security/stability suites

SDK suites:

```bash
cargo test -p kelvin-sdk
scripts/test-plugin-author-kit.sh
scripts/test-plugin-trust.sh
scripts/test-docker.sh
```

Before final pushes:

```bash
scripts/test-docker.sh --final
```

If you need to reclaim disk from the shared Docker caches, remove `./.cache/docker`.

Memory controller OWASP + NIST suites:

```bash
cargo test -p kelvin-memory-controller --test memory_controller_owasp_top10_ai_2025
cargo test -p kelvin-memory-controller --test memory_controller_nist_ai_rmf_1_0
```

## 4) Current MVP behavior

- The default demo path uses the built-in echo model provider.
- CLI flow is SDK-first and runs through a WASM plugin (`kelvin_cli`) before run execution.
- Kelvin Core ships first-party SDK tool-pack plugins (`fs_safe_read`, `fs_safe_write`, `web_fetch_safe`, `schedule_cron`, `session_tools`).
- Memory/data-plane split exists and is tested.
- Plugin install path is prebuilt-package based (no recompiling root required).

For architecture details, see:

- [../architecture/architecture.md](../architecture/architecture.md)
- [../memory/memory-control-data-plane.md](../memory/memory-control-data-plane.md)
- [../plugins/plugin-install-flow.md](../plugins/plugin-install-flow.md)
