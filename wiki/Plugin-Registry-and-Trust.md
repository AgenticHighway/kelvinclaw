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
scripts/plugin-install.sh --package ./dist/acme.echo-1.0.0.tar.gz
```

Install from a community index (requires `KELVIN_PLUGIN_INDEX_URL` to be set):

```bash
KELVIN_PLUGIN_INDEX_URL=https://your-host/index.json \
  scripts/plugin-index-install.sh --plugin some.community.plugin
```

First-party plugins (`kelvin.cli`, `kelvin.anthropic`, `kelvin.openrouter`, `kelvin.echo`)
are baked into the Docker image and do not use the index install path.

Discover available plugins:

```bash
scripts/plugin-discovery.sh
scripts/plugin-discovery.sh --plugin kelvin.cli
scripts/plugin-discovery.sh --json
```

Check for updates:

```bash
scripts/plugin-update-check.sh --json
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

Use it with the plugin scripts:

```bash
scripts/plugin-discovery.sh --registry-url http://127.0.0.1:34619
scripts/plugin-index-install.sh --plugin kelvin.cli --registry-url http://127.0.0.1:34619
scripts/plugin-update-check.sh --registry-url http://127.0.0.1:34619 --json
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

Inspect:

```bash
scripts/plugin-trust.sh show --trust-policy ./.kelvin/trusted_publishers.json
```

Rotate key:

```bash
scripts/plugin-trust.sh rotate-key --publisher acme --public-key <base64-ed25519-public-key> --trust-policy ./.kelvin/trusted_publishers.json
```

Revoke or unrevoke:

```bash
scripts/plugin-trust.sh revoke --publisher acme --trust-policy ./.kelvin/trusted_publishers.json
scripts/plugin-trust.sh unrevoke --publisher acme --trust-policy ./.kelvin/trusted_publishers.json
```

Pin or unpin publisher:

```bash
scripts/plugin-trust.sh pin --plugin acme.echo --publisher acme --trust-policy ./.kelvin/trusted_publishers.json
scripts/plugin-trust.sh unpin --plugin acme.echo --trust-policy ./.kelvin/trusted_publishers.json
```

## Supply-Chain Validation

KelvinClaw includes:

- registry-backed install and discovery
- update-check flow
- plugin author verification flow
- external ABI compatibility CI against published plugins

The compatibility workflow lives in the repository’s GitHub Actions configuration and is intended to catch plugin/runtime drift before release.

## Related Pages

- [Plugin System](Plugin-System)
- [Security Model](Security-Model)
- [Testing and Validation](Testing-and-Validation)

## Reference

- [Plugin install flow](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/PLUGIN_INSTALL_FLOW.md)
- [Plugin index schema](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/plugin-index-schema.md)
- [Plugin trust operations](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/plugin-trust-operations.md)
- [Plugin quality tiers](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/plugin-quality-tiers.md)
