# Getting Started

KelvinClaw supports three onboarding tracks based on user experience level.
Each track has a verification command so the setup can be validated immediately.

## Track 1: Docker-Only (No Rust/WASM Experience Required)

Use this if you want to run KelvinClaw without installing Rust locally.

Prerequisites:

- `git`
- `docker`

Steps:

```bash
git clone <repo-url>
cd kelvinclaw
scripts/run-runtime-container.sh --index-url https://example.com/kelvin/plugins/index.json
```

Verification:

```bash
scripts/verify-onboarding.sh --track beginner
```

Expected result:

- Interactive setup wizard runs on container start.
- Required `kelvin.cli` plugin is installed from plugin index.
- Running `kelvin-host --prompt "hello" --timeout-ms 3000` works without local Rust setup.

## Track 2: Rust Developer (Runtime Contributor)

Use this if you are comfortable with Rust and want local compile/test speed.

Prerequisites:

- `git`
- `rustup` + `cargo`

Steps:

```bash
git clone <repo-url>
cd kelvinclaw
scripts/try-kelvin.sh "hello"
scripts/test-sdk.sh
```

Verification:

```bash
scripts/verify-onboarding.sh --track rust
```

Expected result:

- SDK test suite passes.
- Local runtime run completes with echo payload output.

## Track 3: Rust + WASM Plugin Author

Use this if you are building or testing WASM plugin modules.

Prerequisites:

- `git`
- `rustup` + `cargo`
- `wasm32-unknown-unknown` target

Setup:

```bash
rustup target add wasm32-unknown-unknown
```

Steps:

```bash
git clone <repo-url>
cd kelvinclaw
CARGO_TARGET_DIR=target/echo-wasm-skill cargo build --target wasm32-unknown-unknown --manifest-path examples/echo-wasm-skill/Cargo.toml
cargo run -p kelvin-wasm --bin kelvin-wasm-runner -- --wasm target/echo-wasm-skill/wasm32-unknown-unknown/debug/echo_wasm_skill.wasm --policy-preset locked_down
```

Verification:

```bash
scripts/verify-onboarding.sh --track wasm
```

Expected result:

- Sample WASM skill builds successfully.
- WASM runner executes the module under sandbox policy.

## Verify All Tracks

Run full onboarding verification:

```bash
scripts/verify-onboarding.sh --track all
```

This command runs `beginner`, `rust`, and `wasm` checks in sequence.

## Security and Stability Notes

- Plugin execution is policy-gated and signature-verified by default.
- First-party CLI plugin installation uses the same installed-plugin flow as other plugins.
- Onboarding verification intentionally checks runtime behavior and SDK tests, not only tool presence.
