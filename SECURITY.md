# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in KelvinClaw, please report it
responsibly. **Do not open a public GitHub issue for security vulnerabilities.**

### How to Report

1. **Email**: Send a description of the vulnerability to the repository maintainers
   via the contact information on the [AgenticHighway organization profile](https://github.com/AgenticHighway).
2. **GitHub Security Advisories**: Use the
   [private vulnerability reporting](https://github.com/AgenticHighway/kelvinclaw/security/advisories/new)
   feature on this repository.

### What to Include

- A description of the vulnerability and its potential impact
- Steps to reproduce or a proof of concept
- The affected version(s)
- Any suggested fix or mitigation

### Response Timeline

- **Acknowledgement**: Within 48 hours of receiving the report
- **Assessment**: Within 7 days, we will provide an initial assessment
- **Fix**: Critical vulnerabilities will be prioritized for the next patch release

### Scope

The following are in scope for security reports:

- All Rust crates in the `crates/` and `apps/` directories
- Plugin signing and trust verification (`Ed25519`, `plugin.sig`)
- WASM sandbox policy enforcement (`SandboxPolicy`, `SandboxPreset`)
- Gateway authentication and WebSocket protocol
- Memory controller RPC and access controls
- Docker images and runtime container security
- Secret handling and credential management

### Out of Scope

- Third-party plugins not published by AgenticHighway
- Vulnerabilities in upstream dependencies (report those to the upstream project)
- Issues requiring physical access to the host machine

## Security Design

KelvinClaw is designed with security as a first-class concern:

- **Plugin sandboxing**: Untrusted WASM plugins run in a sandboxed environment
  with explicit capability gates
- **Signed packages**: Plugin manifests are signed with Ed25519 and verified
  against a trusted publisher policy
- **Fail-closed defaults**: Missing or invalid configuration causes startup
  failure rather than permissive fallback
- **Network mediation**: All network access from plugins is host-mediated with
  explicit allowlists
- **Memory isolation**: Memory operations go through the data-plane RPC with
  security checks

For detailed security documentation, see:

- [SDK OWASP Top 10 AI 2025](docs/security/sdk-owasp-top10-ai-2025.md)
- [SDK NIST AI RMF 1.0](docs/security/sdk-nist-ai-rmf-1-0.md)
- [SDK Test Matrix](docs/security/sdk-test-matrix.md)
- [Core Admission Policy](docs/architecture/core-admission-policy.md)
- [Trusted Executive WASM](docs/architecture/trusted-executive-wasm.md)
