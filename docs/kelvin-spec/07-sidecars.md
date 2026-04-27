---
status: Draft
version: v1
audience: architects, security engineers, ops
date: 2026-04-27
---

# Sidecars — Two Boundaries, Two Gates

The Kelvin GUI architecture defends with **two security sidecars**, one per
boundary:

1. **Open Bias** on the **model boundary** — enforces per-claw `RULES.md`
   policy at the LLM call site.
2. **kelvinclaw `ToolRegistry`** on the **tool boundary** — enforces autonomy
   posture per tool / Connector / MCP call.

This doc describes the topology, the per-claw `RULES.md` contract, the
fail-closed semantics, and the cross-boundary trace flow.

See [ADR-003](decisions/003-two-sidecars.md) for the rationale.

## Topology

```
┌──────────────────┐      ┌─────────────────────────────────────────────┐
│ H02 GUI          │◀────▶│ kelvinclaw runtime (Rust)                    │
│ - Composer       │  WS  │  ┌──────────────────────────────────────┐   │
│ - Mind           │      │  │ KelvinBrain orchestration            │   │
│ - Approvals      │      │  └─────────────┬────────────────────────┘   │
└──────────────────┘      │                │                            │
                          │   ┌────────────▼────────────┐               │
                          │   │ ModelProvider shim       │               │
                          │   │ injects per-claw          │               │
                          │   │ X-Kelvin-Claw-Rules-Ref   │               │
                          │   │ header                    │               │
                          │   └────────────┬────────────┘               │
                          │                │                            │
                          │   ┌────────────▼────────────┐               │
                          │   │ ToolRegistry (TOOL GATE) │  ← BOUNDARY 2 │
                          │   │ posture enforcement      │               │
                          │   │ + WASM sandbox presets   │               │
                          │   └──────────────────────────┘               │
                          └────────────────┬─────────────────────────────┘
                                           │ HTTP
                                           ▼
                          ┌─────────────────────────────────────────────┐
                          │ Open Bias (Python proxy)                     │
                          │ http://localhost:4000/v1                     │
                          │  ← BOUNDARY 1 (MODEL GATE)                   │
                          │                                              │
                          │  PRE_CALL  → check claw RULES.md             │
                          │  LLM_CALL  → forward to upstream             │
                          │  POST_CALL → check output against RULES.md   │
                          │                                              │
                          │  OpenTelemetry traces → exportable           │
                          └────────────────┬─────────────────────────────┘
                                           ▼
                          ┌─────────────────────────────────────────────┐
                          │ Anthropic / OpenAI / OpenRouter              │
                          └─────────────────────────────────────────────┘
```

Both boundaries see every model-using operation. The tool boundary sees
every tool call (including ones the model didn't initiate, e.g., direct
runtime calls). The model boundary sees every LLM call (including ones
that don't lead to tool calls).

## Boundary 1 — Open Bias (model boundary)

[Open Bias](https://github.com/open-bias/open-bias) is a Python proxy.
Drop-in OpenAI/Anthropic-compatible URL. Reads policy from `RULES.md`
files. Enforces at three lifecycle points per LLM call:

- **PRE_CALL** — inspect outgoing request (system prompt, user messages)
  against rules; can rewrite, reject, or pass through.
- **LLM_CALL** — proxy to upstream provider.
- **POST_CALL** — inspect upstream response against rules; can rewrite or
  reject.

Open Bias listens on `http://localhost:4000/v1` by default. The kelvinclaw
`ModelProvider` is configured to point its `base_url` at this address.

### Per-claw `RULES.md` selection

Each claw has its own `RULES.md` (per
[ADR-006](decisions/006-soul-rules-files-and-question-reuse.md)). Open Bias
must know *which* claw's RULES file to apply per request. The selection is
done via a request header:

```
POST http://localhost:4000/v1/messages
X-Kelvin-Claw-Rules-Ref: <claw_id>
X-Kelvin-Claw-Posture: <effective posture json>
Content-Type: application/json

{ ... Anthropic Messages API body ... }
```

The header is set by the kelvinclaw `ModelProvider` shim (see
[interfaces/sidecar-integration.md](interfaces/sidecar-integration.md)). Open
Bias's per-request `RULES.md` selector reads this header and loads the
matching file.

The header is **integrity-bound**:

- Open Bias is configured with the kelvinclaw data directory mount, so it
  reads RULES.md from the same paths the runtime writes them to.
- Open Bias is configured to require the header on every call (no header =
  reject).
- A malicious plugin that tries to set its own header value gets the actual
  RULES.md for whichever claw it claims to be — but it can only claim to
  be claws the runtime has bound it to, since the runtime computes the
  header value, not the plugin.

### Example `RULES.md`

Open Bias's format is human-readable markdown with structured rule sections.
A starting template:

```markdown
# Personal Claw — RULES

## No PII leakage
Detect: Outgoing assistant messages containing the user's home address,
SSN, banking info, or named family members not authorized for this
conversation context.
Action: Strip the matched content and replace with "[REDACTED — see Personal
claw RULES]".

## No off-topic
Detect: Responses primarily about politics or current-events advocacy.
Action: Respond with "I'll keep this claw focused on your personal
matters. Try the General claw for that."

## No instructions to install software
Detect: Assistant suggesting `pip install`, `npm install`, `apt-get`, etc.
Action: Replace with a note that software installs go through the
configured plugin install flow.

## Charter alignment
Detect: Responses that contradict the claw's `soul.md` charter.
Action: Append a note "[off-charter — please rephrase]" and rewrite if
possible.
```

Each rule section has a `Detect` clause (hint to evaluators; can be a
keyword, regex, or LLM-judge prompt) and an `Action` clause. Open Bias's
documentation is the canonical reference for the syntax.

## Boundary 2 — kelvinclaw ToolRegistry (tool boundary)

The kelvinclaw runtime's `ToolRegistry` (defined in `crates/kelvin-core`,
documented in `OVERVIEW.md`) wraps every tool call. The Kelvin GUI extends
it with **autonomy posture enforcement**:

- Before every tool call, the registry consults the calling claw's
  effective posture per the relevant axis
  ([05-autonomy-postures.md](05-autonomy-postures.md)).
- If the posture says "auto," the call proceeds.
- If the posture says "ask," an approval is created
  ([06-approvals-primitive.md](06-approvals-primitive.md)) and the call
  blocks until decision.
- If denied, the call is denied with a Receipt.

WASM-backed Skills are further sandboxed by preset:

- `locked_down` — no network, no fs writes
- `dev_local` — local fs writes within `kelvinDataDir`, no outbound network
- `hardware_control` — selected hardware ops (audio, camera) under explicit
  per-Skill permission

The autonomy axis "WASM egress" selects the preset in effect for each
Skill invocation. See `docs/architecture/trusted-executive-wasm.md` for
preset specifics.

See [interfaces/tool-gate-postures.md](interfaces/tool-gate-postures.md) for
the runtime mapping.

## Health, configuration, fail-closed

### Health probes

- The GUI ships a single "Sidecars: healthy / degraded / down" indicator.
  The state is computed from:
  - Open Bias `:4000/health` returning 200 within 1s
  - `ToolRegistry` config check (manifest present, posture loaded)

### Sidecar-down behavior

Per [ADR-008](decisions/008-three-postures-cap-invariant.md):

| Sidecar state | Effect |
|---|---|
| Both healthy | normal |
| Open Bias `:4000` unreachable | model calls **refused** by `ModelProvider` shim (fail-closed); banner |
| `ToolRegistry` misconfigured | tool calls **denied**; banner |
| Both down | safe mode — denied banner; only static UI works |

Critically, the `ModelProvider` shim does NOT fall through to the upstream
provider when Open Bias is down. That would bypass the model boundary
entirely. Fail-closed is mandatory; see
[interfaces/sidecar-integration.md](interfaces/sidecar-integration.md) for
the contract.

### Version pinning

Both sidecars are version-pinned in deployment:

- `docker-compose.yml` pins Open Bias to a specific image tag.
- kelvinclaw's `ToolRegistry` ships with the runtime; kelvinclaw is
  version-controlled together.
- An Open Bias upgrade that changes evaluator semantics is a **policy
  change** — must be reviewed alongside `RULES.md` updates.

## Cross-boundary trace flow

A single user-initiated turn produces traces across both boundaries:

1. User submits message via H02 → kelvinclaw via WebSocket
2. kelvinclaw determines target claw + Power
3. `ModelProvider` shim adds `X-Kelvin-Claw-Rules-Ref` header → Open Bias
4. Open Bias PRE_CALL evaluators check; emit OpenTelemetry span
5. Open Bias forwards to Anthropic / OpenAI; emits LLM_CALL span
6. Open Bias POST_CALL evaluators check upstream response; emit span
7. Response returns to `ModelProvider`; passed to KelvinBrain
8. Brain decides on tool calls
9. ToolRegistry gates each tool call against autonomy posture; emit Receipt
10. Tool runs (Connector op / MCP op / built-in / WASM Skill)
11. Result returned to Brain; loop or finish

OpenTelemetry trace IDs are correlated to Receipts via
`Receipt.otelTraceId`, so Mind's call-tree view can deep-link to the
underlying spans for forensic investigation.

## Operational concerns

### Local development

- Open Bias runs as a sidecar container; the H02 dev environment includes
  it in `docker-compose.dev.yml`.
- A "skip Open Bias" dev mode is **forbidden** (would bypass the boundary).
  Instead, dev environments use a permissive `RULES.md` template.
- WASM sandbox preset `dev_local` is the default in development.

### Production deployment

- Open Bias and kelvinclaw run on the same host (localhost-only Open Bias).
- Hosting Open Bias remotely is unsupported in v1 (latency + auth
  complexity). The runtime must verify it's talking to a same-host Open
  Bias.
- Logs and OpenTelemetry traces are exported to the user's chosen sink
  (file by default).

### Multi-claw RULES.md inheritance

A future enhancement (v2) is RULES.md inheritance: a child claw's RULES
can include directives from its parent. v1 does NOT support this; each
claw has its own self-contained `RULES.md`. Users can still choose to
duplicate-or-symlink rules across claws manually.

## Cross-references

- [ADR-003](decisions/003-two-sidecars.md) — rationale
- [ADR-006](decisions/006-soul-rules-files-and-question-reuse.md) — RULES.md
  is per-claw file
- [ADR-008](decisions/008-three-postures-cap-invariant.md) — sidecar-down
  floor invariants
- [05-autonomy-postures.md](05-autonomy-postures.md) — what the tool gate enforces
- [interfaces/sidecar-integration.md](interfaces/sidecar-integration.md) —
  ModelProvider shim contract
- [interfaces/tool-gate-postures.md](interfaces/tool-gate-postures.md) —
  ToolRegistry binding
- Open Bias: https://github.com/open-bias/open-bias
- `OVERVIEW.md` — `Tool` / `ToolRegistry` seam
- `docs/architecture/trusted-executive-wasm.md` — sandbox presets
- `AGENTS.md` — fail-closed principle
