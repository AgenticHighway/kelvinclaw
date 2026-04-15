# Plugin Index Schema (v1)

Kelvin runtime can install plugins from a remote index using:

```bash
kelvin plugin install <id>
```

Default index URL:

- `https://raw.githubusercontent.com/agentichighway/kelvinclaw-plugins/main/index.json`
- Override with `KELVIN_PLUGIN_INDEX_URL`

## Schema

```json
{
  "schema_version": "v1",
  "plugins": [
    {
      "id": "kelvin.cli",
      "version": "0.1.0",
      "package_url": "https://raw.githubusercontent.com/agentichighway/kelvinclaw-plugins/main/packages/kelvin.cli/0.1.0/kelvin.cli-0.1.0.tar.gz",
      "sha256": "7db6...<64 hex chars>...",
      "trust_policy_url": "https://raw.githubusercontent.com/agentichighway/kelvinclaw-plugins/main/trusted_publishers.kelvin.json",
      "quality_tier": "signed_trusted",
      "tags": ["first_party", "cli"]
    }
  ]
}
```

Field requirements:

- `schema_version`: required, must be `v1`
- `plugins`: required array
- per plugin entry:
  - `id`: required
  - `version`: required
  - `package_url`: required
  - `sha256`: required (fail-closed if missing/mismatch)
  - `trust_policy_url`: optional
  - `quality_tier`: optional (`unsigned_local`, `signed_community`, `signed_trusted`)
  - `tags`: optional string array for discovery/category

Selection behavior:

- `kelvin plugin install <id>` required
- `kelvin plugin install <id> --version <version>` optional
- if version is omitted, installer chooses the highest dotted semver release

## Hosted Registry API

`apps/kelvin-registry` serves the same `v1` index plus filtered discovery endpoints:

- `GET /healthz`
- `GET /v1/index.json`
- `GET /v1/plugins`
- `GET /v1/plugins/{plugin_id}`
- `GET /v1/trust-policy`

Example:

```bash
cargo run -p kelvin-registry -- --index ./index.json --bind 127.0.0.1:34619
KELVIN_PLUGIN_INDEX_URL=http://127.0.0.1:34619/v1/index.json kelvin plugin search
KELVIN_PLUGIN_INDEX_URL=http://127.0.0.1:34619/v1/index.json kelvin plugin install kelvin.cli
KELVIN_PLUGIN_INDEX_URL=http://127.0.0.1:34619/v1/index.json kelvin plugin update --dry-run
```

## Trust Policy

If `trust_policy_url` is present, installer fetches and merges it into local trust policy:

- `require_signature` remains strict (`base && incoming`)
- `publishers` merged by `id` (last entry wins for duplicates)

This keeps runtime signature verification strict by default.

## Discovery

```bash
kelvin plugin search
kelvin plugin search kelvin.cli
kelvin plugin info kelvin.cli
kelvin plugin update --dry-run
```
