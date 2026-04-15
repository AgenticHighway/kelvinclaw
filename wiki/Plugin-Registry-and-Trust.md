# Plugin Registry and Trust

KelvinClaw supports both raw plugin indexes and a hosted registry service. The registry and trust policy model are designed to keep plugin installation convenient without weakening runtime enforcement.

## Package Layout

A plugin package is a `.tar.gz` containing:

- `plugin.json`
- `payload/<files...>`
- optional `plugin.sig`

Install-time validation checks:

- package structure
- required manifest fields
- safe entrypoint path
- entrypoint file existence
- optional SHA-256 integrity match
- duplicate install protection

## Install and Discovery

Install from a package:

```bash
kelvin plugin install --package ./dist/acme.echo-1.0.0.tar.gz
```

Install from a community index:

```bash
KELVIN_PLUGIN_INDEX_URL=https://your-host/index.json \
  kelvin plugin install some.community.plugin
```

First-party plugins (`kelvin.cli`, `kelvin.anthropic`, `kelvin.openrouter`, `kelvin.echo`)
are baked into the Docker image and installed by the `kelvin-init` container on startup.

Discover available plugins:

```bash
kelvin plugin search
kelvin plugin search kelvin.cli
kelvin plugin info kelvin.cli
```

Check for updates:

```bash
kelvin plugin update --dry-run
kelvin plugin update
```

## Hosted Registry Service

`apps/kelvin-registry` exposes filtered discovery and install metadata.

Run it locally:

```bash
cargo run -p kelvin-registry -- --index ./index.json --bind 127.0.0.1:34619
```

Endpoints:

- `GET /healthz`
- `GET /v1/index.json`
- `GET /v1/plugins`
- `GET /v1/plugins/{plugin_id}`
- `GET /v1/trust-policy`

Use it with the plugin CLI:

```bash
KELVIN_PLUGIN_INDEX_URL=http://127.0.0.1:34619/v1/index.json kelvin plugin search
KELVIN_PLUGIN_INDEX_URL=http://127.0.0.1:34619/v1/index.json kelvin plugin install kelvin.cli
KELVIN_PLUGIN_INDEX_URL=http://127.0.0.1:34619/v1/index.json kelvin plugin update --dry-run
```

## Install Root and Selection Model

Default plugin home:

- `~/.kelvinclaw/plugins/<plugin_id>/<version>/`
- `~/.kelvinclaw/plugins/<plugin_id>/current -> <version>`

Version selection is semver-aware. If no version is specified, Kelvin selects the highest dotted semver release.

## Quality Tiers

- `unsigned_local`
  - local development only
- `signed_community`
  - signature expected
- `signed_trusted`
  - signature expected and trust-policy verified

## Trust Policy Operations

The trust policy file lives at `~/.kelvinclaw/trusted_publishers.json` (or `KELVIN_TRUST_POLICY_PATH`). Edit it directly to manage publisher trust.

Reference format and fields: `trusted_publishers.example.json`

The file schema:

```json
{
  "require_signature": false,
  "publishers": [
    {
      "id": "acme",
      "public_key": "<base64-ed25519-public-key>",
      "revoked": false
    }
  ]
}
```

`require_signature: true` enforces signature verification for all installed plugins. Set it to `false` to allow unsigned local development packages.

Publisher entries can be added, removed, or have their keys rotated by editing this file directly.

## Supply-Chain Validation

KelvinClaw includes:

- registry-backed install and discovery
- update-check flow
- plugin author verification flow
- external ABI compatibility CI against published plugins

The compatibility workflow lives in the repository's GitHub Actions configuration and is intended to catch plugin/runtime drift before release.

## Related Pages

- [Plugin System](Plugin-System)
- [Security Model](Security-Model)
- [Testing and Validation](Testing-and-Validation)

## Reference

- [Plugin install flow](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/PLUGIN_INSTALL_FLOW.md)
- [Plugin index schema](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/plugin-index-schema.md)
- [Plugin quality tiers](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/plugin-quality-tiers.md)
