# Changelog

All notable changes to KelvinClaw will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- CI workflow running on every push and pull request (`cargo fmt`, `clippy`, `build`, `test`, `audit`)
- Web search tool plugin (`kelvin-websearch-plugin`)
- Documentation reorganized into categorized subdirectories (`architecture/`, `gateway/`, `getting-started/`, `memory/`, `plugins/`, `security/`)

### Fixed
- All 37 broken documentation links in README now point to correct subdirectory paths
- All clippy warnings resolved across the workspace
- Formatting normalized across 22 source files

### Security
- Bumped `aws-lc-sys` to 0.39.0 (fixes RUSTSEC-2026-0044, RUSTSEC-2026-0048)
- Bumped `rustls-webpki` to 0.103.10 (fixes RUSTSEC-2026-0049)

## [0.1.8] - 2025-03-14

### Added
- Release executables workflow for multi-platform builds (Linux, macOS, Windows)
- Debian package generation for `amd64` and `arm64`
- Plugin author Docker workflow
- Plugin signing and trust policy operations
- OpenRouter model plugin
- Anthropic model plugin
- Memory controller deployment profiles
- Memory module SDK and WIT contract
- Gateway channel plugin ABI (Telegram, Slack, Discord ingress)
- Operator console on gateway HTTP listener
- Plugin quality tiers and compatibility contracts
- NIST AI RMF 1.0 and OWASP Top 10 AI 2025 compliance documentation

### Changed
- Plugins are now built from source and baked into Docker runtime image
- CLI plugin executed through secure installed-plugin path

## [0.1.7] - 2025-03-10

- Gateway WebSocket protocol and TUI client
- Memory RPC contract and gRPC service
- Plugin index schema and discovery

## [0.1.6] - 2025-03-07

- Trusted executive + untrusted WASM skills split model
- Sandbox policy presets and capability gates
- Ed25519 plugin signing

## [0.1.5] - 2025-03-04

- SDK runtime integration path
- Plugin factory and registry
- In-memory plugin registry with policy-gated registration

## [0.1.4] - 2025-02-28

- Initial workspace structure
- Core contracts and shared types
- Brain agent loop orchestration
- WASM execution engine
- Memory backends (Markdown, InMemoryVector, fallback)

[Unreleased]: https://github.com/AgenticHighway/kelvinclaw/compare/v0.1.8...HEAD
[0.1.8]: https://github.com/AgenticHighway/kelvinclaw/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/AgenticHighway/kelvinclaw/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/AgenticHighway/kelvinclaw/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/AgenticHighway/kelvinclaw/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/AgenticHighway/kelvinclaw/releases/tag/v0.1.4
