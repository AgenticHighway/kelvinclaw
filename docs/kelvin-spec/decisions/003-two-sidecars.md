---
status: Accepted
version: v1
audience: architects, security engineers, runtime authors
date: 2026-04-27
---

# ADR-003 — Two security sidecars (model boundary + tool boundary), not one

## Status

Accepted.

## Context

The Kelvin GUI architecture must defend against several failure classes:

1. **Prompt injection** — instructions embedded in fetched URLs, pasted text, or
   external content that try to override the agent's charter.
2. **Off-policy model output** — the model produces content that violates the
   claw's `RULES.md` (PII leak, off-topic, jailbreak compliance, secret exposure).
3. **Unauthorized tool execution** — a model output (even policy-compliant)
   requests a tool call that the user's autonomy posture forbids.
4. **Network egress from skills** — a WASM skill makes a network call that should
   be denied under the current sandbox preset.
5. **Cross-claw exfiltration** — a sub-claw reads or writes data outside its
   privileged scope.

A single chokepoint cannot catch all of these because they happen at different
boundaries of the system. Recent industry incidents (March 2026) — the Claude
Code Security Review action, Gemini CLI Action, and GitHub Copilot Agent all
leaking secrets through PR-title prompt injection — demonstrate that
model-output evaluation alone does not stop tool-execution risks, and tool gates
alone do not stop policy violations in model output.

The user explicitly named **Open Bias** (https://github.com/open-bias/open-bias)
as the sidecar of choice. Open Bias is a Python proxy that sits at
`http://localhost:4000/v1` between the application and an LLM provider, intercepts
PRE_CALL / LLM_CALL / POST_CALL, and enforces rules from a plain `RULES.md`.
It is OpenAI/Anthropic-compatible as a drop-in URL replacement.

The kelvinclaw runtime already provides `ToolRegistry` and `Tool` traits in
`crates/kelvin-core` (see `OVERVIEW.md`), and a WASM sandbox with sandbox-policy
presets `locked_down`, `dev_local`, `hardware_control` in `crates/kelvin-wasm`
(see `docs/architecture/trusted-executive-wasm.md`). These are tool-boundary
gates — they decide what tools and skills can do, not what the model can say.

## Alternatives Considered

### Alternative A — One sidecar at the model boundary (Open Bias only)

Rely on Open Bias for policy enforcement; do not add a separate tool-boundary
gate beyond what kelvinclaw already provides natively.

**Pros:** One sidecar to deploy, monitor, version. Single `RULES.md` per claw.

**Cons:** Open Bias evaluates model I/O. It cannot stop a model output that is
*policy-compliant in text* but requests a tool call that exceeds the user's
posture. Misses the autonomy-matrix enforcement layer entirely. Also misses
WASM egress, which lives below the model boundary.

### Alternative B — One sidecar at the tool boundary (kelvinclaw native only)

Use kelvinclaw's existing `ToolRegistry` and WASM sandbox for all enforcement;
skip a model-boundary policy proxy.

**Pros:** No new sidecar process; everything in Rust.

**Cons:** Cannot stop model from producing prompt-injection-influenced output
that leaks PII or secrets directly in the assistant turn (no tool involved).
Cannot enforce per-claw `RULES.md` content policies. Loses the OpenTelemetry
audit trail Open Bias provides for free.

### Alternative C — One unified sidecar built in-house

Build a single Rust sidecar that intercepts both model calls and tool calls.

**Pros:** One process, one config language.

**Cons:** Reinvents what Open Bias already does well at the model boundary.
Mixes two concerns (model policy vs. tool autonomy) in one config surface.
Substantial build cost. No upstream community / Apache-2.0 license benefit.

### Alternative D (chosen) — Two sidecars, one per boundary

- **Open Bias on the model boundary**: Python proxy at `:4000`, enforces per-claw
  `RULES.md`, runs PRE_CALL / POST_CALL evaluators, intervenes on next call.
- **kelvinclaw `ToolRegistry` on the tool boundary**: native Rust gate on every
  tool call, enforces autonomy posture (see ADR-008), wraps WASM sandbox
  presets and Connector op gating.

Both must be healthy for normal operation. If either is unreachable, autonomy
floors to **Low** (see ADR-008) — fail-closed.

**Pros:** Each sidecar is in its native language for its problem; both have
clear responsibility boundaries; reuses Open Bias upstream and kelvinclaw native.
Each gate produces an audit signal (Open Bias OpenTelemetry traces; ToolRegistry
emit events) that flows into Mind's Receipts tab.

**Cons:** Two processes to deploy, two configs, two health checks. Mitigated by
shipping both in the same `docker-compose.yml` and surfacing a single
"sidecars healthy" indicator in the GUI.

## Decision

The Kelvin GUI architecture uses **two security sidecars**:

1. **Model boundary**: Open Bias (`https://github.com/open-bias/open-bias`)
   running at `http://localhost:4000/v1`. kelvinclaw's `ModelProvider` points
   at this URL; Open Bias forwards to the upstream provider (Anthropic / OpenAI)
   and enforces the invoking claw's `RULES.md` per request. Per-claw `RULES.md`
   is selected via a request header — see
   [interfaces/sidecar-integration.md](../interfaces/sidecar-integration.md).
2. **Tool boundary**: kelvinclaw's existing `ToolRegistry` enforces autonomy
   posture per tool invocation. WASM skills are sandbox-bounded by the
   `locked_down` / `dev_local` / `hardware_control` presets. Connector ops are
   gated by per-claw posture row "Connector writes."

Sidecar health is monitored:

- If Open Bias `:4000` is unreachable, kelvinclaw `ModelProvider` MUST
  fail-closed (refuse model calls) rather than fall through to upstream.
  This is consistent with kelvinclaw's `AGENTS.md` "fail-closed on missing
  config" principle.
- If `ToolRegistry` is misconfigured (missing posture, missing manifest), every
  tool call is denied.
- The GUI surfaces a single "Sidecars: healthy / degraded / down" indicator;
  on degraded/down, autonomy floors to Low globally and a banner explains why.

## Consequences

### Positive

- Each enforcement layer is in the right language for its problem (Python for
  model-output evaluation; Rust for tool-call gating).
- `RULES.md` is human-readable and per-claw, naturally pairs with `soul.md`
  (ADR-006).
- OpenTelemetry trace data from Open Bias flows into Mind's Receipts tab for
  free.
- Adding a new claw is "create directory + soul.md + RULES.md + posture
  config"; no code change.
- The autonomy matrix axes have unambiguous enforcement points.

### Negative

- Two processes to deploy and monitor. `docker-compose.yml` grows.
- Per-claw `RULES.md` injection requires a header-passthrough shim in
  kelvinclaw's `ModelProvider`. See
  [interfaces/sidecar-integration.md](../interfaces/sidecar-integration.md).
- Latency adds one in-process hop per model call (Open Bias is local, so this
  is small but not zero).

### Security

- Fail-closed on either sidecar down is mandatory. This is the only configuration
  that makes the autonomy invariants in ADR-008 hold.
- Open Bias version pinning matters; an Open Bias upgrade that changes
  evaluator semantics changes effective policy. Pin in `docker-compose.yml`.
- The header used to select per-claw `RULES.md` MUST be signed or validated,
  so a compromised plugin cannot inject a different RULES file. See
  [interfaces/sidecar-integration.md](../interfaces/sidecar-integration.md).

## References

- ADR-006 — `soul.md` + `RULES.md` as file-backed config
- ADR-008 — Three autonomy postures; sidecar-down floors to Low
- [07-sidecars.md](../07-sidecars.md) — full sidecar topology and diagrams
- [interfaces/sidecar-integration.md](../interfaces/sidecar-integration.md)
- [interfaces/tool-gate-postures.md](../interfaces/tool-gate-postures.md)
- Open Bias: https://github.com/open-bias/open-bias
- `kelvinclaw/OVERVIEW.md` — `Tool`/`ToolRegistry` seam
- `kelvinclaw/docs/architecture/trusted-executive-wasm.md` — sandbox presets
- `kelvinclaw/AGENTS.md` — fail-closed principle
