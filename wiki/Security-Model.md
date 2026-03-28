# Security Model

KelvinClaw is opinionated about fail-closed behavior. Security claims primarily apply to the SDK lane and the trusted-host plus untrusted-module split, not to arbitrary root-side code.

## Trust Lanes

Root lane:

- direct internal integrations
- intended for trusted maintainers
- not treated as a security boundary

SDK lane:

- plugin contracts and admission checks
- explicit capability declaration
- policy-gated registration
- installable by unknown users

If an extension is meant for general distribution, it belongs on the SDK lane.

## Trusted Executive, Untrusted Modules

`kelvin-wasm` is the trusted executive. It:

- links only allowed host imports
- rejects unsupported imports at instantiation
- enforces module size and fuel limits
- records host calls for observability

Default policy is locked down. Filesystem, network, hardware, or other privileged capabilities are opt-in.

## Gateway Security Defaults

- connect-first handshake
- optional token auth
- public plaintext binds disabled by default
- direct ingress disabled by default
- typed request validation
- idempotent run submission
- bounded connection, queue, frame, and body limits
- per-IP auth backoff

Channel ingress adds per-platform verification and replay controls on top.

## Plugin Trust Model

Install-time and runtime controls combine:

- package structure validation
- optional entrypoint hash validation
- optional Ed25519 signatures
- publisher trust policy
- publisher revocation
- plugin-to-publisher pinning
- capability scopes
- operational controls such as timeout and retry caps

## Memory Security Model

The memory controller re-validates:

- JWT issuer, audience, time validity
- context equality
- replay state
- module and provider capability alignment

This means the data plane does not trust the caller just because it is part of the same broader system.

## Security Validation

KelvinClaw tracks security and governance through dedicated suites and documents:

- OWASP Top 10 AI coverage
- NIST AI RMF mapping
- SDK certification tests
- tool sandbox suites
- gateway ingress hardening tests
- plugin ABI compatibility checks

## Reference

- [Root vs SDK trust model](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/architecture/root-vs-sdk.md)
- [Trusted executive and WASM skills](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/architecture/trusted-executive-wasm.md)
- [SDK OWASP Top 10 AI coverage](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/security/sdk-owasp-top10-ai-2025.md)
- [SDK NIST AI RMF coverage](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/security/sdk-nist-ai-rmf-1-0.md)
