# Kelvin Plugin Author Kit (Template)

This template directory is a reference starting point for third-party plugin authors.

Primary command flow:

```bash
scripts/kelvin-plugin-dev.sh new --id acme.echo --name "Acme Echo" --runtime wasm_tool_v1
scripts/kelvin-plugin-dev.sh test --manifest ./plugin-acme.echo/plugin.json
scripts/kelvin-plugin-dev.sh pack --manifest ./plugin-acme.echo/plugin.json
scripts/kelvin-plugin-dev.sh install --package ./plugin-acme.echo/dist/acme.echo-0.1.0.tar.gz
scripts/kelvin-plugin-dev.sh verify --package ./plugin-acme.echo/dist/acme.echo-0.1.0.tar.gz
```

For model plugins, the supported local runtime loop is:

```bash
scripts/kelvin-plugin-dev.sh smoke --manifest ./plugin.json
```

For working model-plugin source, also see:

- `plugins/kelvin-anthropic-plugin`
- `plugins/kelvin-openrouter-plugin`
- `docs/plugins/build-a-model-plugin.md`

Template manifests:

- `wasm_tool/plugin.json.template`
- `wasm_model/plugin.json.template`

New model plugins should declare a structured `provider_profile` object. Kelvin
core routes and adapts requests by `protocol_family`, so most new providers only
need manifest changes, not host-runtime changes.

The author-kit templates default to `unsigned_local` so community contributors can
build and install plugins locally without access to AgenticHighway's signing
platform. Kelvin warns on install for unsigned local packages, but still allows
them to load from a local plugin home.

Signing and trust policy:

```bash
scripts/plugin-sign.sh --manifest ./plugin.json --private-key /path/to/ed25519-private.pem --publisher-id acme --trust-policy-out ./trusted_publishers.acme.json
```

KMS-backed signing is also supported via `--kms-key-id` (see internal runbook).
