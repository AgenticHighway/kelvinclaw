# Plugin Trust Operations

Use `scripts/plugin-trust.sh` for trust policy lifecycle operations.

## Commands

Show trust policy:

```bash
scripts/plugin-trust.sh show --trust-policy ./.kelvin/trusted_publishers.json
```

Rotate publisher key:

```bash
scripts/plugin-trust.sh rotate-key \
  --trust-policy ./.kelvin/trusted_publishers.json \
  --publisher acme \
  --public-key <base64-ed25519-public-key>
```

Revoke / unrevoke publisher:

```bash
scripts/plugin-trust.sh revoke --publisher acme --trust-policy ./.kelvin/trusted_publishers.json
scripts/plugin-trust.sh unrevoke --publisher acme --trust-policy ./.kelvin/trusted_publishers.json
```

Pin / unpin plugin publisher:

```bash
scripts/plugin-trust.sh pin --plugin acme.echo --publisher acme --trust-policy ./.kelvin/trusted_publishers.json
scripts/plugin-trust.sh unpin --plugin acme.echo --trust-policy ./.kelvin/trusted_publishers.json
```

## Enforced By Runtime

Installed plugin loader enforces:

- signature requirement when configured
- publisher trust list membership
- revoked publisher rejection
- plugin->publisher pin consistency

Source:

- `crates/kelvin-brain/src/installed_plugins.rs`
