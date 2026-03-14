# Memory System

KelvinClaw separates memory into a control plane and a data plane. The runtime mints scoped delegation tokens, and the memory controller re-validates those claims before executing the request.

## Control Plane vs Data Plane

Control plane responsibilities:

- run and session orchestration
- policy authority
- delegation JWT minting
- root-side `MemorySearchManager` abstraction

Data plane responsibilities:

- gRPC `MemoryService`
- JWT and context validation
- replay detection
- WASM memory module execution
- canonical response and audit behavior

## Main Components

- `kelvin-memory-client`
  - root-side RPC adapter
- `kelvin-memory-api`
  - protobuf and gRPC contract
- `kelvin-memory-controller`
  - memory data plane server and sandbox runtime
- `kelvin-memory-module-sdk`
  - ABI helpers and WIT surface for memory modules

## RPC Contract

Service:

- `Upsert`
- `Query`
- `Read`
- `Delete`
- `Health`

Every request carries `RequestContext`:

- `delegation_token`
- `request_id`
- `tenant_id`
- `workspace_id`
- `session_id`
- `module_id`

The controller requires strict equality between context and token claims.

## Transport Security

Defaults:

- local/dev: `http://127.0.0.1:50051`
- production: TLS expected
- optional mTLS available

Root-side JWT and TLS controls:

- `KELVIN_MEMORY_SIGNING_KEY_PEM`
- `KELVIN_MEMORY_SIGNING_KEY_PATH`
- `KELVIN_MEMORY_RPC_TLS_CA_PEM`
- `KELVIN_MEMORY_RPC_TLS_CA_PATH`
- `KELVIN_MEMORY_RPC_TLS_DOMAIN_NAME`
- `KELVIN_MEMORY_RPC_TLS_CLIENT_CERT_PEM`
- `KELVIN_MEMORY_RPC_TLS_CLIENT_KEY_PEM`

## Request Flow

1. Root receives a memory request.
2. Root mints a short-lived JWT with op, scope, and limits.
3. Root calls the memory controller over gRPC.
4. Controller verifies the token and request context.
5. Controller enforces replay and request limit controls.
6. Controller invokes the selected WASM memory module.
7. Controller returns canonical responses and audit metadata.

## Safety Controls

- loopback-only plaintext by default
- explicit TLS or insecure override for public binds
- JWT replay denial via `jti` cache and replay window
- module capability and provider-feature checks before serving traffic
- timeout, size, and result-count limits

## Transitional Path

The repository still includes in-process memory backends for migration and fallback scenarios, but new root paths are expected to go through the RPC controller path.

## Reference

- [Memory control/data plane split](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/memory-control-data-plane.md)
- [Memory RPC contract](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/memory-rpc-contract.md)
- [Memory controller deployment profiles](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/memory-controller-deployment-profiles.md)
- [Memory module SDK](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/memory-module-sdk.md)
