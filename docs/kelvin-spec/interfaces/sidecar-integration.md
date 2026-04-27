---
status: Draft
version: v1
audience: runtime authors, security engineers
date: 2026-04-27
---

# Sidecar Integration — `ModelProvider` Shim Contract

The kelvinclaw `ModelProvider` is the seam where outgoing LLM calls are
made. To enforce per-claw `RULES.md` at the model boundary
([07-sidecars.md](../07-sidecars.md)), every model call must route through
**Open Bias** at `http://localhost:4000/v1` with the calling claw's RULES
identifier in a request header.

This doc is the contract between kelvinclaw's `ModelProvider` shim and
Open Bias.

See [ADR-003](../decisions/003-two-sidecars.md) for rationale and
[`docs/security/`](../../security/) for related security docs.

## Overview

The `ModelProvider` trait (per [`OVERVIEW.md`](../../../OVERVIEW.md)) is
a kelvinclaw seam. The Kelvin GUI implementation:

1. Wraps the configured upstream provider (Anthropic, OpenAI, OpenRouter)
   so that `base_url` points at Open Bias `:4000` instead of the upstream.
2. On every call, injects:
   - `X-Kelvin-Claw-Rules-Ref: <claw_id>` — selects which `RULES.md` Open
     Bias evaluates against
   - `X-Kelvin-Claw-Posture: <posture-json>` — informational context for
     evaluators (e.g., a rule that says "stricter on PII at low posture")
3. Fails closed on unreachability (no fallback to upstream).

## Configuration

```toml
# kelvinclaw config (e.g., kelvin.toml)
[model_provider]
base_url = "http://localhost:4000/v1"        # Open Bias
upstream_provider = "anthropic"              # what Open Bias forwards to
upstream_api_key_ref = "${ANTHROPIC_API_KEY}"
fail_closed = true                           # MUST be true; AGENTS.md principle
require_open_bias_health = true              # probe before each session
```

`fail_closed = false` is **forbidden** in v1. The configuration loader
rejects this value and refuses to start
([ADR-008](../decisions/008-three-postures-cap-invariant.md) sidecar-down
floor relies on fail-closed).

## Request flow

```
KelvinBrain
    │
    │ ModelProvider::send_message({ messages, model_id, ... })
    ▼
ModelProvider shim
    │
    │ 1. Read calling claw_id from session context
    │ 2. Compute posture snapshot for this call
    │ 3. Build HTTP request:
    │    POST http://localhost:4000/v1/messages
    │    Headers:
    │      X-Kelvin-Claw-Rules-Ref: <claw_id>
    │      X-Kelvin-Claw-Posture: <json>
    │      Authorization: Bearer <upstream-api-key>
    │    Body: <Anthropic Messages API JSON>
    │
    │ 4. Send to Open Bias
    ▼
Open Bias :4000
    │
    │ PRE_CALL evaluator: check request against <claw_id>'s RULES.md
    │ (rewrite, reject, or pass)
    │
    │ Forward to https://api.anthropic.com/v1/messages
    │
    │ POST_CALL evaluator: check response against RULES.md
    │ (rewrite, reject, or pass)
    │
    │ Emit OpenTelemetry span; return response
    ▼
ModelProvider shim
    │
    │ 5. On 200: return response to KelvinBrain
    │ 6. On 4xx (RULES violation): map to typed error;
    │    return as denied-policy outcome to KelvinBrain
    │ 7. On 5xx / connect-error: fail-closed → return denied-policy
    │    with detail "open-bias-unreachable"; trigger sidecar-down banner
    ▼
KelvinBrain
```

## The `X-Kelvin-Claw-Rules-Ref` header

### Format

```
X-Kelvin-Claw-Rules-Ref: claw_<id>
```

`<id>` matches the `Claw.id` field. The header value MUST be set by the
shim (which knows the calling context); plugins / Powers MUST NOT be able
to override it.

### Open Bias-side handling

Open Bias is configured to:

- Reject calls without the header (return 400).
- Read `<claw_id>`, look up `<kelvin_data_dir>/claws/<id>/RULES.md`, and
  apply that file's rules.
- Emit an OTEL span with `kelvin.claw_id` attribute for trace correlation.

If Open Bias cannot read the file (missing, perms), it MUST reject the
call (fail-closed). This catches cases where a plugin claims to be a claw
that doesn't exist.

### Validity check

The shim itself validates the header value before sending:

- Must be a non-empty string starting with `claw_`.
- Must match a claw the runtime knows exists.
- Must match the claw whose context is making this call (cross-checked
  against session state).

Mismatch → the shim refuses to send and returns an internal error.

## The `X-Kelvin-Claw-Posture` header

### Format

```
X-Kelvin-Claw-Posture: {"toolExecution":"medium","subAgentSpawn":"low",...}
```

JSON-encoded `PosturePerAxis` snapshot for the calling claw (effective,
post-cap-chain).

### Open Bias-side handling

Posture-aware rules can read this header to apply graduated severity. For
example, a rule:

```markdown
## PII handling
Detect: outgoing assistant text containing patterns matching SSN or CCN.
Action:
  - if posture.toolExecution == "low": REJECT entire call
  - else: REDACT and replace with "[REDACTED]"
```

Posture is informational — Open Bias still owns the policy decision; the
posture just informs what severity to apply.

## Health probing

### Startup

On runtime startup, before accepting any user submits, the shim probes:

```
GET http://localhost:4000/health
Expected: 200 OK within 1 second
```

If the probe fails:

- `require_open_bias_health = true` → refuse to start; log `sidecar-down`
- `require_open_bias_health = false` → start with sidecar-down banner;
  all model calls fail-closed until probe succeeds (this mode exists for
  diagnostic startup; not recommended for normal operation)

### Steady-state

Per-call: a connect-error or 5xx response triggers a sidecar-down state.

A separate watcher probes every 5s; on recovery, sidecar-down state
clears.

State changes emit `sidecar-health` events on the gateway (see
[interfaces/h02-protocol.md](h02-protocol.md)) so the H02 banner updates.

## Error mapping

Open Bias responses map to runtime outcomes:

| Open Bias response | Runtime outcome | Receipt |
|---|---|---|
| 200 (clean) | success | `power-invocation` allowed |
| 200 (rewritten) | success | `power-invocation` allowed; `outcomeDetail` records rewrite |
| 4xx (rejected by rule) | denied-policy | `power-invocation` outcome=denied-policy |
| 5xx (Open Bias internal) | denied-policy + sidecar-degraded | trigger sidecar-down |
| connect-error | denied-policy + sidecar-down | as above |
| timeout (>30s default) | denied-policy + sidecar-degraded | retry policy: 2 retries with backoff before declaring sidecar-down |

## Cost accounting

The shim records cost per call:

- Token in/out from upstream response usage block
- Dollar cost from a pricing table per `(provider, model_id)`
- Wallclock from request start to response complete

These are written into the corresponding `Receipt` (see
[09-data-model.md](../09-data-model.md), `Receipt.tokensIn / tokensOut /
costDollars / wallclockMs`).

## OTEL trace correlation

The shim sets a parent OTEL trace ID on each request; Open Bias attaches
its span as a child. The trace ID is recorded in
`Receipt.otelTraceId` so Mind's call-tree can deep-link into Open Bias's
trace data.

## File-system path conventions

The shim and Open Bias agree on data directory structure:

```
<kelvin_data_dir>/
├── claws/
│   ├── claw_<id>/
│   │   ├── soul.md
│   │   ├── RULES.md
│   │   ├── sources/
│   │   ├── drafts/
│   │   └── ...
│   └── ...
├── connectors/
├── mcp/
├── plugins/
└── ...
```

`<kelvin_data_dir>` is configurable; both processes resolve the same
absolute path (Open Bias is configured with `KELVIN_DATA_DIR`
environment variable).

## v1 implementation outline

A new wrapper around the existing provider implementations in
`crates/kelvin-providers/`:

```rust
pub struct OpenBiasShimProvider {
    inner: Box<dyn ModelProvider>,
    open_bias_url: Url,
    fail_closed: bool,
    health_state: Arc<RwLock<SidecarHealth>>,
}

impl ModelProvider for OpenBiasShimProvider {
    async fn send_message(&self, ctx: &CallContext, req: ModelRequest) -> Result<ModelResponse> {
        // 1. Validate calling claw exists, posture snapshot is valid
        let claw_id = ctx.claw_id();
        let posture = ctx.posture_snapshot();

        // 2. Build HTTP request to Open Bias
        let resp = self.client
            .post(self.open_bias_url.join("messages")?)
            .header("X-Kelvin-Claw-Rules-Ref", claw_id)
            .header("X-Kelvin-Claw-Posture", serde_json::to_string(&posture)?)
            .json(&req)
            .send()
            .await;

        // 3. Map response
        match resp {
            Ok(r) if r.status().is_success() => Ok(parse_response(r).await?),
            Ok(r) if r.status() == 400 || r.status() == 403 => {
                Err(ModelError::DeniedPolicy(read_detail(r).await))
            }
            Ok(r) => self.fail_closed_or_propagate(r).await,
            Err(_) => {
                self.mark_sidecar_down().await;
                Err(ModelError::SidecarDown)
            }
        }
    }
}
```

The "inner" provider is essentially unused at runtime (Open Bias is the
upstream as far as the shim is concerned), but kept for the case where
Open Bias is configured to forward to a specific provider chosen at
runtime — the shim passes the choice via Open Bias's own routing
configuration.

## Cross-references

- [ADR-003](../decisions/003-two-sidecars.md) — two sidecars rationale
- [ADR-006](../decisions/006-soul-rules-files-and-question-reuse.md) — RULES.md as files
- [ADR-008](../decisions/008-three-postures-cap-invariant.md) —
  fail-closed invariant
- [07-sidecars.md](../07-sidecars.md) — sidecar topology
- [interfaces/tool-gate-postures.md](tool-gate-postures.md) — companion gate
- [interfaces/h02-protocol.md](h02-protocol.md) — sidecar-health events
- [`docs/security/`](../../security/) — related security docs
- [`AGENTS.md`](../../../AGENTS.md) — fail-closed principle
- [`OVERVIEW.md`](../../../OVERVIEW.md) — `ModelProvider` seam
- Open Bias: https://github.com/open-bias/open-bias
