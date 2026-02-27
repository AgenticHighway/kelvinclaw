# KelvinClaw High-Level Gap Analysis

This document tracks high-level parity gaps and closure work against the reference "Claw" product shape, while preserving KelvinClaw's security-first SDK and control/data plane separation.

## Completed Gap Closures

### 1) Secure Gateway Control Plane

Status: `DONE`

Implemented:

- new app: `apps/kelvin-gateway`
- typed WebSocket request/response/event envelopes
- strict connect-first handshake
- optional auth token enforcement on connect (`KELVIN_GATEWAY_TOKEN` / `--token`)
- idempotent `agent` submission via required `request_id`
- async run surfaces:
  - `agent` / `run.submit`
  - `agent.wait` / `run.wait`
  - `agent.state` / `run.state`
  - `agent.outcome` / `run.outcome`
- streamed runtime events from SDK runtime to connected clients

Security properties:

- fail-closed handshake validation
- explicit auth check before runtime operations
- method allowlist and typed parameter validation
- no direct plugin loading in gateway code (SDK-only composition path)

### 2) Model Failover + Retry Semantics

Status: `DONE`

Implemented in `kelvin-sdk`:

- `KelvinSdkModelSelection::InstalledPluginFailover`
- ordered provider chain selection
- bounded retries per provider (`max_retries_per_provider`)
- bounded backoff (`retry_backoff_ms`)
- fail-closed behavior:
  - retry/fallback only on transient classes (`backend`, `timeout`, `io`)
  - no fallback on non-recoverable classes (`invalid_input`, `not_found`)

Security and reliability properties:

- no silent fallback to unintended providers
- explicit provider ordering and retry bounds
- deterministic error surfaces when chain is exhausted

### 3) Reusable SDK Runtime for Host/Gateway Surfaces

Status: `DONE`

Implemented:

- `KelvinSdkRuntimeConfig`
- `KelvinSdkRuntime::initialize(...)`
- `KelvinSdkRuntime::submit/state/wait/wait_for_outcome`
- `KelvinSdkRunRequest` + `KelvinSdkAcceptedRun`

Architecture impact:

- external surfaces can now use the SDK runtime directly instead of composing root crates.
- host and gateway stay on the same policy-governed composition path.

## Remaining High-Level Gaps

These are still open and are prioritized by security, stability, and maintainability impact.

### 1) Channel Integrations

Status: `OPEN`

Needed:

- production channel adapters (chat/voice surfaces)
- per-channel auth/routing/allowlist policy
- deterministic delivery/retry + rate controls per channel

### 2) Daemon Lifecycle + Operator UX

Status: `OPEN`

Needed:

- first-class daemon install/start/stop/status
- startup health checks and fail-fast diagnostics
- remote-safe defaults for exposure/auth

### 3) Control UI and Operator Observability

Status: `OPEN`

Needed:

- minimal web/operator UI over gateway APIs
- run/session/event inspection
- policy and plugin state visibility

### 4) Rich Context Management (Compaction/Pruning)

Status: `OPEN`

Needed:

- deterministic compaction policy
- pruning thresholds + summaries
- run-level bounds on context growth

### 5) Multi-provider Auth Profiles and Routing Policy

Status: `OPEN`

Needed:

- credential profile abstraction
- policy-based model/provider routing
- typed fallback trees tied to workspace/session policy

## Near-Term TODO (Execution Order)

1. Add daemon/service management for `kelvin-gateway` (systemd/launchd docs + scripts).
2. Add gateway protocol schema docs and compatibility tests.
3. Add gateway security tests for malformed frames, replay pressure, and auth brute-force throttling.
4. Add compaction/pruning policy trait in SDK path with deterministic tests.
5. Add a minimal control UI shell consuming gateway methods.

