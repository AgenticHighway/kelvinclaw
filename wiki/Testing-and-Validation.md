# Testing and Validation

KelvinClaw's validation strategy is deliberately broad: formatting, linting, supply-chain checks, protocol tests, workspace tests, SDK certification, E2E flows, and clean Docker validation.

## Core Validation Commands

Formatting:

```bash
cargo fmt --all -- --check
```

Linting:

```bash
cargo clippy --all -- -D warnings
```

Security and dependency posture:

```bash
cargo audit
cargo outdated
```

Workspace tests:

```bash
cargo test --workspace --tests
```

SDK lane:

```bash
cargo test -p kelvin-sdk
```

End-to-end:

```bash
scripts/test-e2e.sh
```

Final clean Docker validation:

```bash
scripts/test-docker.sh --final
```

## Targeted Validation Lanes

Gateway:

```bash
cargo test -p kelvin-gateway
```

Registry:

```bash
cargo test -p kelvin-registry
```

Plugin lifecycle:

```bash
scripts/test-plugin-author-kit.sh
scripts/test-plugin-abi-compat.sh
```

Contracts:

```bash
scripts/test-contracts.sh
```

## What the Suites Cover

- SDK admission, projection, determinism, and concurrency behavior
- model-provider routing and failover
- tool sandbox approval and scope enforcement
- gateway handshake, auth, ingress, malformed-frame, replay, and restart persistence behavior
- plugin install, discovery, trust, and compatibility
- memory control/data plane contract and transport behavior

## Security and Governance Mapping

KelvinClaw maintains explicit test mapping for:

- OWASP Top 10 AI
- NIST AI RMF

These mappings live in the repository docs and are intended to be used during security review and release validation, not only during development.

## Recommended Release Gate

For a high-confidence release candidate:

1. `cargo fmt --all -- --check`
2. `cargo clippy --all -- -D warnings`
3. `cargo audit`
4. `cargo outdated`
5. `cargo test --workspace --tests`
6. `cargo test -p kelvin-sdk`
7. `scripts/test-e2e.sh`
8. `scripts/test-docker.sh --final`

## Reference

- [SDK test matrix](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/security/sdk-test-matrix.md)
- [SDK OWASP Top 10 AI coverage](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/security/sdk-owasp-top10-ai-2025.md)
- [SDK NIST AI RMF coverage](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/security/sdk-nist-ai-rmf-1-0.md)
