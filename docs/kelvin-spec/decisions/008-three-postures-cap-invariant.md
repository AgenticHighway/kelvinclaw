---
status: Accepted
version: v1
audience: architects, security engineers, implementors
date: 2026-04-27
---

# ADR-008 — Three autonomy postures with parent-caps-child invariant; sidecar-down floors to Low

## Status

Accepted.

## Context

The user requested three security postures: **low**, **medium**, **high
autonomy**. These map to "how much can the agent do without asking?" — not
to "how much can the agent do at all" (the latter is governed by Sources,
Powers, Connectors, MCP bindings, and Sub-agent templates).

The conversation refined this into a multi-axis matrix rather than a single
slider, because different concerns have different appropriate trust profiles
even at the same overall posture level. Specifically:

- Tool execution risk
- Sub-claw delegation (passing to a peer claw with its own posture)
- Sub-agent spawning (transient sub-session creation)
- Source reads
- Connector writes
- Drafts → Sources promotion (per ADR-007)
- Plugin / Skill install
- Memory writes
- WASM egress (sandbox preset selection)
- Routines firing user-absent
- Cross-claw porosity (data flow between claws)
- Power model spend (Powers that bring their own model can rack up bills)

The matrix needs invariants:

1. **Parent caps child** — a sub-claw cannot exceed its parent claw's posture
   on any axis (per ADR-005's recursion).
2. **User caps macro** — the user-level posture caps the macro claw.
3. **Routines run at the claw's posture, not the user's session posture** —
   because the user is not present to override.
4. **Sidecar-down floors to Low** — if either Open Bias (model boundary) or
   `ToolRegistry` (tool boundary) is unavailable, the system cannot honor
   higher postures, so it floors to Low and surfaces a banner (per ADR-003).

Per-action overrides are allowed within the cap (with scope: once / session /
claw / forever) and surface as a delta on the posture screen.

## Alternatives Considered

### Alternative A — Single global slider: Low / Medium / High

One radio button, one cap, applies to everything.

**Pros:** Simplest UX. Easiest to reason about.

**Cons:** Real risk profiles vary by concern. A user might want "auto-write
memory but ask before sending email" — impossible with a single slider.
Forces all-or-nothing tradeoffs and pushes power users to bypass the system.

### Alternative B — Per-action approval only, no posture concept

Every gated action always asks. No postures, no defaults.

**Pros:** Maximum user control; never surprised.

**Cons:** Approval fatigue is real. After 50 "Allow web_search?" prompts, the
user clicks Allow without reading. Worse than postures because the *appearance*
of safety isn't safety.

### Alternative C — Many independent posture sliders, one per axis

12+ sliders the user must configure per claw.

**Pros:** Maximum precision.

**Cons:** Configuration nightmare. New users have no defaults. The user must
become a security expert to use the product.

### Alternative D (chosen) — Three named postures with a per-axis matrix as defaults

The user picks **Low / Medium / High**; that selection populates the matrix
with defaults per axis. Per-axis overrides are allowed within the cap and
shown as a delta. New axes added in v2 ship with sensible defaults per
posture so existing users aren't dropped into a config screen.

**Pros:** Beginners pick one of three; experts override per axis. New axes
have clear default semantics. Postures are a useful summary; matrix is the
truth.

**Cons:** The relationship between "posture name" and "matrix state" must be
explained clearly, especially when a user has overrides (the posture badge
shows "Medium*" with the asterisk meaning "with overrides"). UI work in
[06-approvals-primitive.md](../06-approvals-primitive.md) and
[05-autonomy-postures.md](../05-autonomy-postures.md).

## Decision

### Posture model

- Three named postures: **Low**, **Medium**, **High**.
- Each posture maps to defaults across the matrix axes (see
  [05-autonomy-postures.md](../05-autonomy-postures.md) for the full table).
- Per-axis overrides allowed; shown as deltas; revocable.
- Per-action overrides ("remember this") with scope `once | session | claw |
  forever`; surface as pills on the matrix.

### Matrix axes (v1)

| Axis | What it gates |
|---|---|
| Tool execution | Direct tool calls via `ToolRegistry` |
| Sub-claw delegation | Handoff to a peer claw |
| Sub-agent spawn | Creating a transient sub-session (per ADR-001) |
| Source reads | Per-source per-session approval requirement |
| Connector writes | Outbound calls via Connector ops |
| Drafts → Sources promotion | Per ADR-007 |
| Plugin / Skill install | New Powers, MCP servers, model-bound Powers |
| Memory writes | Append/update on memory Sources |
| WASM egress | Sandbox preset (`locked_down` / `dev_local` / per-skill) |
| Routines firing user-absent | Hooks/heartbeats/watches when user is away |
| Cross-claw porosity | Data flow between claws (read or invoke) |
| Power model spend | Powers that bring their own model |

### Invariants (must hold)

1. `child.autonomyPosture <= parent.autonomyPosture` per axis (parent caps
   child).
2. `macro.autonomyPosture <= user.autonomyPosture` per axis (user caps
   macro).
3. Routines fire at the **claw's** posture, NOT the user's session posture.
   A separate "Routines firing user-absent" axis can FURTHER restrict.
4. If Open Bias `:4000` is unreachable, system-wide effective posture
   floors to Low across all axes; banner explains.
5. If `ToolRegistry` is misconfigured (missing manifest, missing posture
   binding), every tool call is denied (Low equivalent).
6. Per-action "remember forever" requires the user's explicit confirmation
   in a separate UI step (not a one-tap default).
7. Per-claw posture changes are recorded as Receipts.
8. A spawned Sub-agent inherits the spawning claw's posture as a hard cap;
   re-checked on each Power invocation within the sub-session because the
   parent's posture can change mid-flight.

### Sidecar-down behavior (formalized)

| State | Effective posture | Tool calls | Model calls | Banner |
|---|---|---|---|---|
| Both sidecars healthy | as configured | gated per posture | proxied via Open Bias | none |
| Open Bias down | floor to Low | gated per posture | refused (fail-closed) | "Model sidecar unreachable; LLM calls disabled" |
| ToolRegistry misconfigured | floor to Low | denied | proxied normally | "Tool gate misconfigured; tools disabled" |
| Both down | denied | denied | denied | "Sidecars down; system in safe mode" |

## Consequences

### Positive

- Three named postures cover beginner UX; matrix overrides cover power users.
- Invariants are tree-walks (parent-cap) and per-call checks (routines) —
  testable.
- Sidecar-down floor-to-Low + banner gives users a clear failure mode rather
  than silent posture changes.
- New axes in v2 (per-user caps, trust-score gating) compose with existing
  invariants without breaking changes.
- Receipts on posture changes provide an audit trail for compliance use cases.

### Negative

- The matrix is information-dense. Mitigated by showing "delta from posture
  default" rather than full table by default.
- Invariant enforcement spans GUI, runtime, and sidecars. Mistakes here are
  silent security failures. Tests must cover the full grid (axis × posture ×
  cap-from-parent).
- Sidecar-down behavior must be visibly different from "no permission" so
  users understand WHY actions are denied.

### Security

- Floor-to-Low on sidecar-down is the keystone invariant. Any path that
  bypasses it (e.g., a fall-through to upstream LLM provider when Open Bias
  is down) breaks the security model. The shim in
  [interfaces/sidecar-integration.md](../interfaces/sidecar-integration.md)
  MUST fail-closed.
- Per-action "forever" overrides accumulate over time and become a stealth
  posture loosening. The matrix UI MUST list active "forever" overrides
  prominently with one-tap revoke.
- Routines posture is separate because the user is not present to interrupt;
  conservative defaults are essential.
- Posture matrix consistency across docs is a verification concern (see
  spec-wide verification list in the Plan); the same axes/wording must
  appear in [05-autonomy-postures.md](../05-autonomy-postures.md),
  [06-approvals-primitive.md](../06-approvals-primitive.md), and
  [07-sidecars.md](../07-sidecars.md).

## References

- ADR-001 — Sub-agents are runtime-only (Sub-agent spawn axis)
- ADR-003 — Two sidecars (sidecar-down floor)
- ADR-005 — Recursive Claw primitive (parent-caps-child)
- ADR-007 — Drafts → Sources promotion edge (promotion row)
- [05-autonomy-postures.md](../05-autonomy-postures.md) — full matrix
- [06-approvals-primitive.md](../06-approvals-primitive.md) — approvals UI
- [07-sidecars.md](../07-sidecars.md) — sidecar topology
- [interfaces/tool-gate-postures.md](../interfaces/tool-gate-postures.md)
- [interfaces/sidecar-integration.md](../interfaces/sidecar-integration.md)
- `kelvinclaw/docs/architecture/trusted-executive-wasm.md` — WASM egress
  presets
