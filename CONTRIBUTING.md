# Contributing to KelvinClaw

Thank you for your interest in contributing to KelvinClaw. This document
explains how to get started.

## Getting Started

1. Fork the repository and clone your fork.
2. Install the Rust toolchain (stable channel).
3. Run `cargo build --workspace` to verify everything compiles.
4. Run `cargo test --workspace` to confirm all tests pass.

For Docker-based development without a local Rust toolchain:

```bash
scripts/plugin-author-docker.sh -- bash
```

## Development Workflow

### Before Submitting a PR

Run the full check suite locally:

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo audit
```

All four commands must pass cleanly. CI enforces the same checks on every PR.

### Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/) style:

- `fix:` for bug fixes
- `feat:` for new features
- `docs:` for documentation changes
- `style:` for formatting-only changes
- `refactor:` for code restructuring without behavior changes
- `test:` for adding or updating tests
- `ci:` for CI/CD changes
- `security:` for security-related changes

Keep commits scoped and atomic. Only stage files relevant to the change.

### Pull Requests

- Keep PRs focused on a single concern
- Separate refactors from feature additions from bug fixes
- Include a clear description of what the PR does and why
- Reference any related issues

### Code Style

- Follow `rustfmt` defaults (enforced by CI)
- Follow `clippy` recommendations (enforced by CI)
- Keep files under 400 lines
- Keep functions under 50 lines
- Pass dependencies explicitly rather than using hidden globals

### Testing

- Add tests for new logic, data transformations, and workflow paths
- Run `scripts/test-sdk.sh` for the SDK certification lane
- Run `scripts/test-docker.sh` for Docker-based verification

## Architecture

- **Crates are self-contained** and do not directly reference each other,
  except for the SDK which can reference all crates.
- **All WASM plugins** must be loaded through the SDK, not directly.
- **All network access** must go through the SDK with explicit allowlists.
- **Fail closed** on missing or invalid configuration.

See [OVERVIEW.md](OVERVIEW.md) and [docs/architecture/](docs/architecture/)
for detailed architecture documentation.

## Plugin Development

To build a new plugin, see:

- [Plugin Author Kit](docs/plugins/plugin-author-kit.md)
- [Build a Tool Plugin](docs/plugins/build-a-tool-plugin.md)
- [Build a Model Plugin](docs/plugins/build-a-model-plugin.md)

## Security

Report security vulnerabilities privately. See [SECURITY.md](SECURITY.md)
for instructions. Do not open public issues for security concerns.

## License

By contributing, you agree that your contributions will be licensed under the
[MIT License](LICENSE).
