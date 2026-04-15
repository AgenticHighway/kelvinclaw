# Getting Started

KelvinClaw supports three onboarding tracks based on user experience level.
Each track has a verification command so the setup can be validated immediately.

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

### Minimum `.env`

The default `.env.example` uses `kelvin.echo` which works immediately with no API keys.
Just copy and go:

```bash
docker compose up -d
```

To use a real LLM, open `.env` and uncomment one of the provider blocks:

For Anthropic:

```bash
KELVIN_MODEL_PROVIDER=kelvin.anthropic
ANTHROPIC_API_KEY=<your-key>
```

For OpenRouter:

```bash
KELVIN_MODEL_PROVIDER=kelvin.openrouter
OPENROUTER_API_KEY=<your-key>
```

For OpenAI:

```bash
KELVIN_MODEL_PROVIDER=kelvin.openai
OPENAI_API_KEY=<your-key>
```

For public-facing deployments, generate a gateway token:

```bash
KELVIN_GATEWAY_TOKEN=$(openssl rand -hex 32)
```

Start the host and gateway:

```bash
docker compose up -d
```

Launch the TUI:

```bash
docker compose --profile tui run --rm kelvin-tui
```

## Canonical Quick Start (Daily Driver MVP)

Local profile (gateway + memory controller + SDK runtime):

```bash
scripts/quickstart.sh --mode local
```

Docker profile:

```bash
scripts/quickstart.sh --mode docker
```

Local profile lifecycle:

```bash
scripts/kelvin-dev-stack.sh start
scripts/kelvin-dev-stack.sh status
scripts/kelvin-dev-stack.sh doctor
scripts/kelvin-dev-stack.sh stop
```

Run modes:

- single prompt: `kelvin-host --prompt "hello"`
- interactive chat: `kelvin-host --interactive`
- daemon mode: `scripts/kelvin-dev-stack.sh start` (gateway + memory controller background services)

## Terminal UI (`kelvin-tui`)

`kelvin-tui` is a full-featured terminal interface that connects to a running gateway and
provides a live chat and tool-monitoring experience. It is the recommended way to interact
with KelvinClaw when the local profile is running.

Start the local profile, then launch the TUI:

```bash
scripts/kelvin-dev-stack.sh start
cargo run -p kelvin-tui
```

Or with a release binary:

```bash
scripts/kelvin-dev-stack.sh start
./kelvin-tui
```

Key capabilities:

- Stream assistant responses and tool calls in real time
- Click and drag to select text in the chat; `Ctrl+C` copies to clipboard via OSC 52
  (works inside Docker — no clipboard daemon required)
- `Ctrl+T` toggles the tools panel
- Input history (`Up` / `Down`), word-jump (`Ctrl+Left` / `Ctrl+Right`), and large-paste
  collapsing are all built in
- Auto-reconnects to the gateway with exponential backoff on disconnect

Connect to a non-default gateway or session:

```bash
kelvin-tui --gateway-url ws://my-server:34617 --auth-token $TOKEN --session my-session
```

Full reference: [`docs/gateway/terminal-ui.md`](../gateway/terminal-ui.md)

## Track 1: Docker-Only (No Rust/WASM Experience Required)

Use this if you want to run KelvinClaw without installing Rust locally.

Prerequisites:

- `git`
- `docker`

Steps:

```bash
git clone <repo-url>
cd kelvinclaw
scripts/run-runtime-container.sh
```

Optional browser automation profile during container setup:

```bash
KELVIN_SETUP_INSTALL_BROWSER_AUTOMATION=1 scripts/run-runtime-container.sh
```

Verification:

```bash
scripts/verify-onboarding.sh --track beginner
```

Expected result:

- `kelvin.cli` and the selected model provider plugin are installed from artifacts baked
  into the image — no external index or network access required.
- Running `kelvin-host --prompt "hello" --timeout-ms 3000` works without local Rust setup.

## Track 2: Rust Developer (Runtime Contributor)

Use this if you are comfortable with Rust and want local compile/test speed.

Prerequisites:

- `git`
- `rustup` + `cargo`
- `jq`
- `curl`
- `tar`
- `openssl`

Steps:

```bash
git clone <repo-url>
cd kelvinclaw
scripts/quickstart.sh --mode local
scripts/test-sdk.sh
```

Verification:

```bash
scripts/verify-onboarding.sh --track rust
```

Expected result:

- SDK test suite passes.
- Local profile boots gateway + memory controller and completes a host run.

## Track 3: Rust + WASM Plugin Author

Use this if you are building or testing WASM plugin modules.

Prerequisites:

- `git`
- `rustup` + `cargo`
- `wasm32-unknown-unknown` target

Docker alternative:

- `docker`
- `scripts/plugin-author-docker.sh` uses a repo-owned Ubuntu 24.04 plugin
  author image so you do not need to start from a blank Ubuntu container

Setup:

```bash
rustup target add wasm32-unknown-unknown
```

Steps:

```bash
git clone <repo-url>
cd kelvinclaw
CARGO_TARGET_DIR=target/echo-wasm-skill cargo build --target wasm32-unknown-unknown --manifest-path plugins/examples/echo-wasm-skill/Cargo.toml
cargo run -p kelvin-wasm --bin kelvin-wasm-runner -- --wasm target/echo-wasm-skill/wasm32-unknown-unknown/debug/echo_wasm_skill.wasm --policy-preset locked_down
export PATH="$PWD/scripts:$PATH"
kelvin plugin new --id acme.echo --name "Acme Echo" --runtime wasm_tool_v1
kelvin plugin test --manifest ./plugin-acme.echo/plugin.json
```

For the supported model-plugin contributor path, use:

- `docs/plugins/build-a-model-plugin.md`
- `plugins/kelvin-anthropic-plugin`
- `plugins/kelvin-openrouter-plugin`

Docker-first authoring shortcut:

```bash
git clone <repo-url>
cd kelvinclaw
scripts/plugin-author-docker.sh -- scripts/test-plugin-author-kit.sh
```

Verification:

```bash
scripts/verify-onboarding.sh --track wasm
```

Expected result:

- Sample WASM skill builds successfully.
- WASM runner executes the module under sandbox policy.
- Plugin author commands scaffold and validate plugin package structure without touching root crates.
- `kelvin plugin install` and `kelvin plugin smoke` cover the local package-install and model-runtime smoke path without requiring host flag memorization.
- Model plugins can be scaffolded, built, packed, and locally installed through the same public SDK surface.

## Verify All Tracks

Run full onboarding verification:

```bash
scripts/verify-onboarding.sh --track all
scripts/verify-onboarding.sh --track daily
```

`all` runs `beginner`, `rust`, and `wasm`. `daily` validates the default daily-driver local profile with a time-to-first-success threshold.

## Security and Stability Notes

- First-party plugins (`kelvin.cli`, `kelvin.anthropic`, `kelvin.openrouter`, `kelvin.echo`)
  are built from source in `plugins/` and baked into the Docker runtime image at build time.
  No external index or signing infrastructure is required for the Docker flow.
- Community and third-party plugins can be installed by setting `KELVIN_PLUGIN_INDEX_URL`
  to point at a community-hosted `index.json`. Signature enforcement is off by default;
  trust policy files can be added to re-enable it per deployment.
- First-party CLI plugin installation uses the same installed-plugin flow as other plugins.
- Onboarding verification intentionally checks runtime behavior and SDK tests, not only tool presence.
