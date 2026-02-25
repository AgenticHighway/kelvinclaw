# Runbook: Module Publisher Trust Policy

## Purpose

Ensure only approved publishers can ship memory modules.

## Policy

- maintain allowlist of trusted publisher IDs and signing keys.
- require manifest signature verification before module admission.
- require explicit module capability review for privileged host features.
- pin acceptable API versions (`v1alpha1` for MVP).

## Admission Checklist

1. Verify publisher identity and key ownership.
2. Verify artifact digest and manifest signature.
3. Validate manifest schema and required host features.
4. Run module in staging with deny-by-default claims.
5. Approve and record module/version in trust inventory.

## Revocation

- remove publisher key from allowlist.
- unload/reject affected modules.
- rotate trust metadata and notify operators.
