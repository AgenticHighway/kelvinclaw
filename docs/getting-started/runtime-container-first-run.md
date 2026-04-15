# Runtime Container First Run

This flow is for end users who should not need Rust or `cargo`.

## Goal

- run Kelvin from a prebuilt Docker runtime image
- complete first-time setup via Docker Compose
- plugins are installed automatically from the index and from image-baked artifacts

## Docker Compose Setup

```bash
git clone https://github.com/AgenticHighway/kelvinclaw.git
cd kelvinclaw
cp .env.example .env
docker compose up -d
```

The `kelvin-init` container runs automatically before the gateway and:

- creates `trusted_publishers.json` if it does not exist
- installs the configured model provider plugin from the index (default: `kelvin.echo`)
- installs `kelvin.cli` from the index
- installs all locally-built plugins baked into the image with `--force`

Container defaults:

- `KELVIN_HOME=/kelvin`
- `KELVIN_PLUGIN_HOME=/kelvin/plugins`
- `KELVIN_TRUST_POLICY_PATH=/kelvin/trusted_publishers.json`

## Running Kelvin in the Container

After startup:

```bash
docker compose run kelvin-host --prompt "What is KelvinClaw?" --timeout-ms 3000
```

Launch the TUI:

```bash
docker compose --profile tui run --rm kelvin-tui
```

## Interactive One-Off Container

```bash
docker compose run kelvin-host
```

## Configuring Providers

Set `KELVIN_MODEL_PROVIDER` in `.env`:

```bash
KELVIN_MODEL_PROVIDER=kelvin.anthropic
ANTHROPIC_API_KEY=<your-key>
```

The init container installs the selected provider plugin automatically on startup.

## Security Notes

- `kelvin plugin install` requires `sha256` in index entries and fails closed on mismatch.
- Install-time manifest and payload checks run on every package.
- Runtime admission still enforces trusted publisher signatures via trust policy at load time.
