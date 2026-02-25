# Runbook: JWT Signing Key Rotation

## Purpose

Rotate Root signing keys used for memory delegation JWTs without downtime.

## Procedure

1. Generate a new Ed25519 keypair offline.
2. Publish new public key to controller config (`KELVIN_MEMORY_PUBLIC_KEY_PEM`) as staged value.
3. Deploy controller with acceptance window for both old/new issuers if needed.
4. Update Root to mint tokens with new private key.
5. Observe memory RPC success/error rates and token verification failures.
6. Remove old public key acceptance after steady-state window.
7. Revoke and securely destroy old private key material.

## Validation

- controller accepts new tokens.
- controller rejects old tokens after cutover window.
- no replay cache explosion or authz bypass during transition.
