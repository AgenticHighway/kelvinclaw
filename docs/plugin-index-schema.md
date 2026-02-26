# Plugin Index Schema (v1)

Kelvin runtime can install plugins from a remote index using:

```bash
scripts/plugin-index-install.sh --index-url <url> --plugin <id>
```

## Schema

```json
{
  "schema_version": "v1",
  "plugins": [
    {
      "id": "kelvin.cli",
      "version": "0.1.0",
      "package_url": "https://plugins.example.com/kelvin.cli-0.1.0.tar.gz",
      "sha256": "7db6...<64 hex chars>...",
      "trust_policy_url": "https://plugins.example.com/trusted_publishers.json"
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

Selection behavior:

- `--plugin <id>` required
- `--version <version>` optional
- if version is omitted, installer chooses the highest version (string-sort descending)

## Trust Policy

If `trust_policy_url` is present, installer fetches and merges it into local trust policy:

- `require_signature` remains strict (`base && incoming`)
- `publishers` merged by `id` (last entry wins for duplicates)

This keeps runtime signature verification strict by default.
