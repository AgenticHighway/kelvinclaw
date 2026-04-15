# Getting Started

KelvinClaw supports three practical onboarding tracks: daily-driver operator use, Rust runtime development, and Rust plus WASM plugin authoring.

## Docker Compose Setup (Validated Onboarding)

The validated method for running KelvinClaw is Docker Compose.

Prerequisites:

- `git`
- `docker` (with Compose v2)

Steps:

```bash
git clone https://github.com/AgenticHighway/kelvinclaw.git
cd kelvinclaw
cp .env.example .env
```

Open `.env` and configure your settings. At minimum:

```bash
KELVIN_GATEWAY_TOKEN=<a-secret-token-you-choose>
KELVIN_MODEL_PROVIDER=kelvin.anthropic   # or kelvin.echo for no-API testing
ANTHROPIC_API_KEY=<your-key>             # required if using kelvin.anthropic
```

Start the host and gateway:

```bash
docker compose up -d
```

Launch the TUI:

```bash
docker compose run kelvin-tui
```

## Quick Start Paths

Docker (recommended):

```bash
cp .env.example .env && docker compose up -d
```

Local profile lifecycle:

```bash
scripts/kelvin-dev-stack.sh start
scripts/kelvin-dev-stack.sh status
scripts/kelvin-dev-stack.sh doctor
scripts/kelvin-dev-stack.sh stop
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
cp .env.example .env
docker compose up -d
docker compose run kelvin-host --prompt "hello"
```

## Track 2: Rust Developer

Use this when you want local compile and test speed.

Prerequisites:

- `rustup`
- `cargo`

Bootstrap:

```bash
git clone https://github.com/AgenticHighway/kelvinclaw.git
cd kelvinclaw
kelvin init
cargo test -p kelvin-sdk
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
scripts/kelvin-plugin-dev.sh new --id acme.echo --name "Acme Echo" --runtime wasm_tool_v1
scripts/kelvin-plugin-dev.sh test --manifest ./plugin-acme.echo/plugin.json
```

## Common Runtime Commands

Single prompt:

```bash
cargo run -p kelvin-host -- --prompt "hello" --memory fallback
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

Using `kelvin plugin` (or `kelvin kpm`):

```bash
kelvin plugin install kelvin.cli
kelvin plugin install kelvin.anthropic
kelvin plugin install kelvin.openai
kelvin plugin search   # list all available plugins
```

See [Plugin System](Plugin-System) for the full plugin CLI reference.

## Verification

Broader validation:

- [Testing and Validation](Testing-and-Validation)
- [Operations and Runbooks](Operations-and-Runbooks)

## Reference

- [Repository quick start](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/getting-started/GETTING_STARTED.md)
- [Rust developer quickstart](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/getting-started/rust-developer-quickstart.md)
- [Runtime container first run](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/getting-started/runtime-container-first-run.md)
