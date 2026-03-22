# Memory Control/Data Plane Split

## Goal

Kelvin separates memory into:

- **Control Plane (Kelvin Root):** policy authority, orchestration, delegation minting.
- **Data Plane (Memory Controller):** execution authority for memory RPC, policy re-enforcement, WASM module sandboxing.

Root never links concrete storage/index drivers directly in the run path.

## Components

1. `Kelvin Root`
- owns run/session lifecycle and plugin admission.
- mints short-lived signed delegation JWTs per request.
- calls memory through `RpcMemoryManager` (implements `MemorySearchManager`).

2. `Memory Controller`
- exposes gRPC `MemoryService` (`v1alpha1`).
- verifies JWT issuer/audience/time validity and strict context equality.
- enforces replay protection via `jti` cache and bounded replay window.
- executes the selected memory WASM module with runtime limits.
- routes host calls through enabled provider features.

3. `WASM Memory Module`
- provides `handle_upsert/query/read/delete/health` handlers.
- can only use host imports declared in ABI.
- has no direct host FS/network/shell access.

## Trust Boundaries

- Root and controller are expected in the same trust domain/VPC for MVP.
- delegation token crosses the boundary from root to controller.
- controller treats all module code as untrusted and sandboxed.
- controller defaults to loopback-only plaintext binding; non-loopback requires TLS or explicit insecure override.

## Request Flow (MVP)

1. Root receives a memory request from brain/core orchestration.
2. Root mints JWT claims with operation, capability scope, and limits.
3. Root sends gRPC request with `RequestContext`.
4. Controller verifies token and context.
5. Controller checks module manifest + token scope + provider availability.
6. Controller executes module handler with fuel/timeout/memory constraints.
7. Controller executes provider operation and returns canonical response.
8. Controller emits audit log with allow/deny reason and latency.

## Failure Behavior

- Unreachable controller: root returns typed `KelvinError::Backend` memory-unavailable error.
- Token/context mismatch: controller returns invalid-argument typed errors.
- Module timeout/fuel trap: controller returns timeout/unavailable typed errors.
- Missing provider feature: module registration is rejected before serving traffic.

## Transitional Path

- Root composition can enable `memory_rpc`.
- Temporary `memory_legacy_fallback` exists for migration safety only.
- In-proc `kelvin-memory` remains transitional/deprecated and is not for new root paths.
- Rollout verification helper: `scripts/memory-rollout-check.sh` (legacy path + RPC path checks).
