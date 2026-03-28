# Architecture

KelvinClaw is interface-first and intentionally split into a trusted control plane, a separate memory data plane, and a policy-gated extension lane.

## Core Design Principles

- Contracts first: core behavior is defined by traits, not concrete implementations.
- Composition over inheritance: runtime behavior is assembled by injecting implementations.
- Deterministic orchestration: per-session lanes serialize execution.
- Failure containment: typed errors and bounded retries keep failures local.
- Minimal trusted core: installable extensions live on the SDK lane.

## Major Components

- `apps/kelvin-host`
  - thin trusted host executable over Kelvin SDK runtime composition
- `apps/kelvin-gateway`
  - secure WebSocket and HTTP ingress gateway with operator surface
- `apps/kelvin-registry`
  - hosted plugin registry and discovery service
- `crates/kelvin-core`
  - shared contracts, plugin manifest model, runtime types
- `crates/kelvin-brain`
  - orchestration and installed-plugin loading
- `crates/kelvin-sdk`
  - runtime composition, first-party tool pack, scheduler integration
- `crates/kelvin-wasm`
  - trusted native executive for untrusted WASM skills and model plugins
- `crates/kelvin-memory-*`
  - memory gRPC API, client, controller, and module SDK

## Stable Interfaces

The main seam types are:

- `Brain`
- `MemorySearchManager`
- `ModelProvider`
- `Tool` and `ToolRegistry`
- `PluginFactory` and `PluginRegistry`
- `SessionStore`
- `EventSink`
- `CoreRuntime` and `RunRegistry`

These interfaces are the compatibility anchor. Concrete implementations can change without breaking the runtime shape.

## Control Plane and Data Plane

Control plane:

- run acceptance
- session and lifecycle orchestration
- plugin admission and policy
- gateway connectivity
- delegation token minting for memory requests

Data plane:

- memory RPC handling
- JWT validation and replay protection
- WASM memory module execution
- provider feature routing inside the memory controller

The root runtime does not directly embed storage or indexing engines in the normal request path.

## Execution Flow

1. `kelvin-host` or `kelvin-gateway` accepts a run.
2. `kelvin-sdk` loads installed plugins and validates them against policy.
3. `CoreRuntime` accepts the run and records lifecycle state.
4. `kelvin-brain` assembles context, calls the configured model provider, and executes tool calls.
5. Events stream through `EventSink`.
6. Session history and run outcomes are persisted.

For channel-driven flows, the gateway also performs auth, routing, dedupe, rate limiting, and channel-policy enforcement before the run enters the SDK lane.

## Extensibility Points

- Models
  - installed SDK model plugins, including generic provider-profile routing
- Tools
  - first-party tool pack plus third-party SDK plugins
- Memory
  - RPC controller and WASM memory modules
- Sessions and events
  - storage and transport surfaces can be swapped behind interfaces
- Channels
  - gateway routes and optional per-channel WASM ingress policies

## Current Scope

Implemented today:

- secure gateway baseline with direct Telegram, Slack, and Discord ingress
- provider-profile model routing and legacy `wasm_model_v1` compatibility
- durable scheduler and audit history
- operator console shell for gateway, schedules, runs, sessions, plugins, registry, and trust state
- hosted plugin registry, update check flow, ABI compatibility CI

## Reference

- [Architecture source doc](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/architecture/architecture.md)
- [Kelvin Core SDK](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/architecture/kelvin-core-sdk.md)
- [Memory control/data plane split](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/memory/memory-control-data-plane.md)
