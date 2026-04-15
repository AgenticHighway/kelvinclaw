# KelvinClaw Wiki

KelvinClaw is a secure, stable, modular runtime for agentic workflows. The project is built around a small trusted core, a policy-gated SDK extension lane, a memory data plane, a secure gateway, and signed plugin distribution.

Use this wiki as the operator and contributor map for the system. For line-by-line protocol and ABI references, use the linked source docs in the main repository.

## Start Here

- New users: [Getting Started](Getting-Started)
- Operators: [Gateway and Operator Guide](Gateway-and-Operator-Guide)
- Plugin authors: [Plugin System](Plugin-System)
- Security reviewers: [Security Model](Security-Model)
- Contributors: [Development and Contributing](Development-and-Contributing)

## System Overview

- Control plane: `kelvin-host`, `kelvin-gateway`, `kelvin-brain`, `kelvin-sdk`
- Data plane: `kelvin-memory-controller` plus `kelvin-memory-api` and `kelvin-memory-client`
- Execution sandbox: `kelvin-wasm`
- Plugin distribution: packaged plugins, trust policy, hosted registry, compatibility CI

KelvinClaw keeps trusted code in the host/runtime path and pushes installable extension behavior onto the SDK/plugin lane. That split is the core design choice for both security and maintainability.

## Common Entry Points

Quick start:

```bash
scripts/quickstart.sh --mode local
scripts/quickstart.sh --mode docker
```

Local profile lifecycle:

```bash
scripts/kelvin-dev-stack.sh start
scripts/kelvin-dev-stack.sh status
scripts/kelvin-dev-stack.sh doctor
scripts/kelvin-dev-stack.sh stop
```

Gateway:

```bash
KELVIN_GATEWAY_TOKEN=change-me cargo run -p kelvin-gateway -- --bind 127.0.0.1:34617 --workspace "$PWD"
```

Hosted plugin registry:

```bash
cargo run -p kelvin-registry -- --index ./index.json --bind 127.0.0.1:34619
```

## Wiki Map

- [Getting Started](Getting-Started)
- [Architecture](Architecture)
- [Gateway and Operator Guide](Gateway-and-Operator-Guide)
- [Plugin System](Plugin-System)
- [Plugin Registry and Trust](Plugin-Registry-and-Trust)
- [Memory System](Memory-System)
- [Security Model](Security-Model)
- [Operations and Runbooks](Operations-and-Runbooks)
- [Testing and Validation](Testing-and-Validation)
- [Development and Contributing](Development-and-Contributing)
- [Reference Map](Reference-Map)

## Source References

- [README](https://github.com/AgenticHighway/kelvinclaw/blob/main/README.md)
- [Architecture doc](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/architecture.md)
- [Gateway protocol](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/gateway-protocol.md)
- [Getting started](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/GETTING_STARTED.md)
