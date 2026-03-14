# Getting Started

KelvinClaw supports three practical onboarding tracks: daily-driver operator use, Rust runtime development, and Rust plus WASM plugin authoring.

## Quick Start Paths

Daily-driver local profile:

```bash
scripts/quickstart.sh --mode local
```

Docker-only profile:

```bash
scripts/quickstart.sh --mode docker
```

Local profile lifecycle:

```bash
scripts/kelvin-local-profile.sh start
scripts/kelvin-local-profile.sh status
scripts/kelvin-local-profile.sh doctor
scripts/kelvin-local-profile.sh stop
```

## Track 1: Docker-Only

Use this when you want KelvinClaw running without a local Rust toolchain.

Prerequisites:

- `git`
- `docker`

Bootstrap:

```bash
git clone https://github.com/AgenticHighway/kelvinclaw.git
cd kelvinclaw
scripts/run-runtime-container.sh
```

Verify:

```bash
scripts/verify-onboarding.sh --track beginner
```

## Track 2: Rust Developer

Use this when you want local compile and test speed.

Prerequisites:

- `rustup`
- `cargo`
- `jq`
- `curl`
- `tar`
- `openssl`

Bootstrap:

```bash
git clone https://github.com/AgenticHighway/kelvinclaw.git
cd kelvinclaw
scripts/quickstart.sh --mode local
scripts/test-sdk.sh
```

Verify:

```bash
scripts/verify-onboarding.sh --track rust
```

## Track 3: Rust + WASM Plugin Author

Use this when you are building SDK plugins or WASM modules.

Setup:

```bash
rustup target add wasm32-unknown-unknown
export PATH="$PWD/scripts:$PATH"
```

Bootstrap:

```bash
CARGO_TARGET_DIR=target/echo-wasm-skill cargo build --target wasm32-unknown-unknown --manifest-path examples/echo-wasm-skill/Cargo.toml
cargo run -p kelvin-wasm --bin kelvin-wasm-runner -- --wasm target/echo-wasm-skill/wasm32-unknown-unknown/debug/echo_wasm_skill.wasm --policy-preset locked_down
kelvin plugin new --id acme.echo --name "Acme Echo" --runtime wasm_tool_v1
kelvin plugin test --manifest ./plugin-acme.echo/plugin.json
```

Verify:

```bash
scripts/verify-onboarding.sh --track wasm
```

## Common Runtime Commands

Single prompt:

```bash
scripts/try-kelvin.sh "hello"
```

Interactive host:

```bash
cargo run -p kelvin-host -- --interactive --workspace "$PWD" --state-dir "$PWD/.kelvin/state"
```

Gateway:

```bash
KELVIN_GATEWAY_TOKEN=change-me cargo run -p kelvin-gateway -- --bind 127.0.0.1:34617 --workspace "$PWD"
```

## Install First-Party Plugins

CLI plugin:

```bash
scripts/install-kelvin-cli-plugin.sh
```

OpenAI model plugin:

```bash
scripts/install-kelvin-openai-plugin.sh
```

Anthropic model plugin:

```bash
scripts/install-kelvin-anthropic-plugin.sh
```

## Verification

Targeted onboarding:

```bash
scripts/verify-onboarding.sh --track daily
scripts/verify-onboarding.sh --track all
```

Broader validation:

- [Testing and Validation](Testing-and-Validation)
- [Operations and Runbooks](Operations-and-Runbooks)

## Reference

- [Repository quick start](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/GETTING_STARTED.md)
- [Rust developer quickstart](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/RUST_DEVELOPER_QUICKSTART.md)
- [Runtime container first run](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/runtime-container-first-run.md)
