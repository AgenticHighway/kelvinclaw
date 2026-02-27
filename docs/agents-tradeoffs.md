# AGENTS.md Tradeoffs and Rationale

This document records intentional tradeoffs where implementation detail differs from idealized policy.

## 1) Default Tool Pack Registration Policy

Tradeoff:

- First-party SDK tool-pack plugins are registered with an internal registration policy that allows
  `fs_read`, `fs_write`, and `network_egress`.

Why:

- `PluginSecurityPolicy::default()` denies privileged capabilities.
- The tool pack is part of Kelvin Core's first-party SDK lane and must remain available out-of-the-box.
- Runtime operation is still gated by explicit per-call approvals and strict scope checks.

Risk control:

- Sensitive operations are deny-by-default without explicit approval payloads.
- Path and host allowlists are enforced in tool code.
- Tool receipts are logged for auditability.

## 2) Scheduler Scope in MVP

Tradeoff:

- `schedule_cron` currently manages deterministic scheduler state files, not OS-level cron execution.

Why:

- Keeps root surface area minimal and avoids platform-specific daemon complexity.
- Preserves stability and reproducibility while still enabling SDK automation workflows.

Risk control:

- Mutation actions require explicit approval.
- State is workspace-scoped and deterministic.

## 3) Plugin Verify Cryptographic Scope

Tradeoff:

- `kelvin plugin verify` validates manifest/layout/tier checks and installability, but full signature trust
  enforcement remains authoritative in runtime loader (`kelvin-brain`).

Why:

- Runtime already enforces trusted publisher policy during load.
- Avoiding duplicate cryptographic stacks in shell scripts reduces maintenance risk.

Risk control:

- Tier checks enforce required `plugin.sig` and publisher metadata.
- Trusted tier verification can require trust policy membership.

## 4) Web Fetch Default Host Set

Tradeoff:

- SDK tool-pack web fetch defaults to a curated allowlist (`docs.rs`, `crates.io`, `raw.githubusercontent.com`, `api.openai.com`)
  rather than empty.

Why:

- Supports immediate developer workflows for Rust users while keeping egress bounded.

Risk control:

- Explicit operation approval still required.
- Host allowlist is overrideable with `KELVIN_TOOLPACK_WEB_ALLOW_HOSTS`.
