# Plugin Trust Operations

The trust policy file lives at `~/.kelvinclaw/trusted_publishers.json` (or `KELVIN_TRUST_POLICY_PATH`). Edit it directly to manage publisher trust.

## File Format

```json
{
  "require_signature": false,
  "publishers": [
    {
      "id": "acme",
      "public_key": "<base64-ed25519-public-key>",
      "revoked": false,
      "pinned_plugins": []
    }
  ]
}
```

Reference template: `trusted_publishers.example.json`

## Operations

**Show trust policy:**

```bash
cat ~/.kelvinclaw/trusted_publishers.json
```

**Add or rotate a publisher key:** edit the `publishers` array directly, setting `id` and `public_key`.

**Revoke a publisher:** set `"revoked": true` on the publisher entry.

**Unrevoke a publisher:** set `"revoked": false`.

**Pin a plugin to a publisher:** add the plugin id to the publisher's `pinned_plugins` array.

```json
{
  "id": "acme",
  "public_key": "...",
  "revoked": false,
  "pinned_plugins": ["acme.echo"]
}
```

**Unpin a plugin:** remove it from `pinned_plugins`.

**Enable mandatory signature verification:** set `"require_signature": true`.

Note: when installing from the plugin index, `kelvin plugin install` automatically fetches
and merges the `trust_policy_url` from the index entry if present. The merge rule keeps
`require_signature` strict (`base && incoming`) so a plugin's index policy cannot loosen
your local setting.

## Strict Install Mode

For developers and security-conscious users, `kelvin plugin install` supports an opt-in `--strict` flag that enforces signature verification at install time:

```bash
# Require a valid signature from a trusted publisher
kelvin plugin install acme.echo --strict

# Strict mode works with all install paths
kelvin plugin install --package ./acme.echo-1.0.0.tar.gz --strict
kelvin plugin install --from-dir ./plugin-acme.echo --strict
```

In strict mode, the installer verifies:

1. `plugin.sig` exists in the plugin package
2. The plugin manifest has a `publisher` field
3. The publisher is listed in `trusted_publishers.json` with a valid Ed25519 public key
4. The signature is a valid Ed25519 signature over the manifest bytes

If any check fails, the install is rejected with a clear error message:

```
error: strict install rejected: plugin 'acme.echo' is missing plugin.sig
error: strict install rejected: publisher 'acme' is not in the trust policy
error: strict install rejected: signature verification failed for acme.echo@1.0.0 from publisher 'acme'
```

Without `--strict`, the installer preserves today's permissive behavior: warnings are printed for unsigned plugins, but installation proceeds. The Kelvin runtime still verifies signatures when present regardless of install mode.

## Enforced By Runtime

Installed plugin loader enforces:

- signature requirement when configured
- publisher trust list membership
- revoked publisher rejection
- plugin->publisher pin consistency

Source:

- `crates/kelvin-brain/src/installed_plugins.rs`
