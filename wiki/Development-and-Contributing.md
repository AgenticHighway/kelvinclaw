# Development and Contributing

KelvinClaw is structured so contributors can work safely on isolated modules without collapsing boundaries between core contracts, tools, agents, and workflows.

## Repository Layout

- `apps/kelvin-host`
- `apps/kelvin-gateway`
- `apps/kelvin-registry`
- `crates/kelvin-core`
- `crates/kelvin-brain`
- `crates/kelvin-sdk`
- `crates/kelvin-wasm`
- `crates/kelvin-memory-*`
- `scripts/`
- `docs/`
- `examples/`

## Architectural Guardrails

- keep crates self-contained
- route plugin loading through the SDK, not direct root coupling
- keep network access host-mediated and allowlist-based
- validate configuration and fail closed on missing or invalid values
- keep files and functions small enough to reason about
- isolate side effects from pure logic where practical

## Common Contributor Flows

Runtime contributors:

```bash
scripts/quickstart.sh --mode local
scripts/test-sdk.sh
cargo test --workspace --tests
```

Gateway work:

```bash
cargo test -p kelvin-gateway
```

Plugin authoring:

```bash
scripts/kelvin-plugin.sh new --id acme.echo --name "Acme Echo" --runtime wasm_tool_v1
scripts/kelvin-plugin.sh test --manifest ./plugin-acme.echo/plugin.json
scripts/kelvin-plugin.sh pack --manifest ./plugin-acme.echo/plugin.json
```

Docs and operator surface work should keep the repository docs, runbooks, and wiki in sync.

## Code Review Expectations

Priorities are:

1. security
2. stability
3. reliability
4. simplicity
5. maintainability

Changes should be small, reviewable, and explicit about new trust assumptions or compatibility impact.

## Before Finalizing Changes

Run the relevant focused tests first, then the broader validation lanes needed for the scope of the change. For release-grade confidence, use the full suite from [Testing and Validation](Testing-and-Validation).

## Reference

- [AGENTS.md](https://github.com/AgenticHighway/kelvinclaw/blob/main/AGENTS.md)
- [Core admission policy](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/architecture/core-admission-policy.md)
- [SDK principles](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/architecture/sdk-principles.md)
- [Compatibility contracts](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/architecture/compatibility-contracts.md)
