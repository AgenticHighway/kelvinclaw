# Plugin Install Flow (No Local Compilation)

This flow is for end users installing prebuilt SDK plugins.

Goal:

- users install a plugin package
- users do not compile Rust locally
- plugin artifacts are isolated from Kelvin root code

## Package Format

A plugin package is a `.tar.gz` with:

- `plugin.json`
- `payload/<files...>`

`plugin.json` required fields:

- `id`
- `name`
- `version`
- `api_version`
- `entrypoint` (relative path under `payload/`)
- `capabilities` (non-empty list)

Optional fields:

- `entrypoint_sha256` (recommended integrity check)
- `publisher` (required when signature verification is enforced)
- `runtime` (`wasm_tool_v1` or `wasm_model_v1`)
- `tool_name` (tool runtime)
- `provider_name` + `model_name` (model runtime)
- `provider_profile` (recommended for generic model runtime host routing)
- `capability_scopes` / `operational_controls`
- `quality_tier` (`unsigned_local`, `signed_community`, `signed_trusted`)

Optional package file:

- `plugin.sig` (Ed25519 signature over `plugin.json`)

## Install Command

Install a plugin from the index by ID:

```bash
kelvin plugin install kelvin.cli
kelvin plugin install kelvin.anthropic
kelvin plugin install kelvin.openai
kelvin plugin install kelvin.openrouter
kelvin plugin install kelvin.browser
```

Install from a local tarball:

```bash
kelvin plugin install --package ./dist/acme.echo-1.0.0.tar.gz
```

`unsigned_local` and `signed_community` packages are still installable. Kelvin
prints a warning so community authors can develop locally without access to the
first-party signing platform.

Install from a remote plugin index with version pinning:

```bash
kelvin plugin install kelvin.cli
kelvin plugin install kelvin.cli --version 0.3.0
kelvin plugin update --dry-run
```

Discover index entries:

```bash
kelvin plugin search
kelvin plugin search anthropic
kelvin plugin info kelvin.cli
```

Run the hosted registry service instead of a raw `index.json`:

```bash
cargo run -p kelvin-registry -- --index ./index.json --bind 127.0.0.1:34619
KELVIN_PLUGIN_INDEX_URL=http://127.0.0.1:34619/v1/index.json kelvin plugin search
KELVIN_PLUGIN_INDEX_URL=http://127.0.0.1:34619/v1/index.json kelvin plugin install kelvin.cli
KELVIN_PLUGIN_INDEX_URL=http://127.0.0.1:34619/v1/index.json kelvin plugin update --dry-run
```

Default index URL:

- `https://raw.githubusercontent.com/AgenticHighway/kelvinclaw-plugins/main/index.json`
- Override with `KELVIN_PLUGIN_INDEX_URL`

Default install location:

- `~/.kelvinclaw/plugins/<plugin_id>/<version>/`
- symlink: `~/.kelvinclaw/plugins/<plugin_id>/current -> <version>`

Override install root:

```bash
KELVIN_PLUGIN_HOME=./.kelvin/plugins kelvin plugin install --package ./dist/acme.echo-1.0.0.tar.gz
```

Environment overrides:

- `KELVIN_PLUGIN_HOME`
- `KELVIN_TRUST_POLICY_PATH`
- `KELVIN_PLUGIN_INDEX_URL`

## List Installed Plugins

```bash
kelvin plugin list
kelvin plugin status
```

## Uninstall Plugin

```bash
kelvin plugin uninstall acme.echo
kelvin plugin uninstall acme.echo --yes   # skip confirmation prompt
```

## Validation Performed by Installer

- package structure exists (`plugin.json`, `payload/`)
- required manifest fields parse
- safe relative entrypoint path
- entrypoint file exists
- optional SHA-256 match (if provided)
- duplicate install protection (unless `--force`)

## Why This Is Privacy-Conscious

- no personal paths or host data in plugin artifacts
- no compilation step on user machine
- install root is local and user-scoped by default

## Runtime Security Notes

Install-time checks validate package integrity and structure. Runtime checks in `kelvin-brain` additionally enforce:

- trusted publisher signature verification (when enabled)
- capability scope allowlists
- execution timeout/retry/rate/circuit controls

## Publisher Signing Workflow

Generate `plugin.sig` from `plugin.json` and emit trust policy snippet:

```bash
scripts/plugin-sign.sh \
  --manifest ./plugin.json \
  --private-key /path/to/ed25519-private.pem \
  --publisher-id acme \
  --trust-policy-out ./trusted_publishers.acme.json
```

KMS-backed signing is also supported (see internal runbook):

```bash
scripts/plugin-sign.sh \
  --manifest ./plugin.json \
  --private-key ./acme-ed25519-private.pem \
  --publisher-id acme \
  --trust-policy-out ./trusted_publishers.acme.json
```

Reference template:

- `trusted_publishers.example.json`

## Verification

Run plugin lifecycle tests:

```bash
scripts/test-plugin-install.sh
```

Authoring/packaging flow:

```bash
scripts/kelvin-plugin-dev.sh new --id acme.echo --name "Acme Echo" --runtime wasm_tool_v1
scripts/kelvin-plugin-dev.sh test --manifest ./plugin-acme.echo/plugin.json
scripts/kelvin-plugin-dev.sh pack --manifest ./plugin-acme.echo/plugin.json
scripts/kelvin-plugin-dev.sh verify --package ./plugin-acme.echo/dist/acme.echo-0.1.0.tar.gz
```

Trust policy operations (direct JSON editing — no CLI wrapper):

- The trust policy lives at `~/.kelvinclaw/trusted_publishers.json` (or `KELVIN_TRUST_POLICY_PATH`).
- Edit the file directly to add, remove, or rotate publisher entries.
- Reference format: `trusted_publishers.example.json`
