---
status: Accepted
version: v1
audience: architects, implementors, security engineers
date: 2026-04-27
---

# ADR-007 — Drafts → Sources is the only outbound promotion edge from privileged

## Status

Accepted.

## Context

Each Claw owns several privileged collections (Sources, Drafts, Powers, etc.)
and is bound to globally shared concerns (Modes, Inputs, Mind, Settings). When
the agent produces work, that work needs a home. Two questions arise:

1. Where does newly produced work live initially?
2. How does it become "real" — committed to a Source the user trusts as
   reference material going forward?

The natural pattern from chat UX (Claude Desktop's Artifacts, Claude Code's
generated files) is:

- Output appears in a **mutable scratch space** (Drafts / Artifacts).
- The user reviews, edits, and chooses whether to **promote** it to durable
  storage (Sources / files / repo).

H02's existing data model already has `Draft` as a separate type from `Source`
(`H02/src/types/index.ts`), with `DraftStatus = 'generating' | 'ready' |
'exported'`. The "exported" status hints at a promotion concept but doesn't
formalize where the export goes or what happens to the Draft.

In conversation, the user clarified:

> "Drafts are called artifacts by Claude. They are artifacts that haven't been
> saved to sources yet."

This naturally suggests modeling promotion as the single direction of outbound
data flow from the privileged box.

## Alternatives Considered

### Alternative A — Drafts and Sources are independent; outputs land in either

Powers can write directly to Sources, or to Drafts, depending on the Power's
configuration.

**Pros:** Powers can be efficient — no extra step for routine outputs (e.g., a
heartbeat that just updates a memory file).

**Cons:** Bypasses the user-review step. Lowers the safety bar (a misbehaving
Power can corrupt durable storage without approval). Splits the "outputs"
concept into two paths with different security semantics.

### Alternative B — All outputs go to Drafts; Sources are read-only at the agent layer

Powers can never write to Sources directly. Every output is a Draft. Promotion
to Source is always user-mediated (or autonomy-mediated under high posture).

**Pros:** One audit point for all writes. User always has the chance to review.
Maps cleanly onto the Claude Artifacts UX. Simplifies the autonomy matrix's
"Drafts → Sources" row to be the single write-path gate.

**Cons:** Requires a promotion step even for trivial cases (e.g., a Routine
that updates a memory file daily). Mitigated by autonomy posture: at high
autonomy, promotion can auto-happen.

### Alternative C — Outputs go directly to Sources; Drafts are a side effect

Inverse of Alternative B. Sources are the primary write target; Drafts are a
review buffer that the user must opt into.

**Pros:** Familiar to users from "save and forget" workflows.

**Cons:** Same as Alternative A — no enforced review point, lowered safety
bar, more complex security model.

## Decision

**Drafts → Sources is the only outbound promotion edge from privileged.**

Concretely:

- All Power outputs (and all Sub-agent outputs) write to Drafts. Sources are
  not directly writable by the agent layer.
- A Draft can be **promoted** to one or more Sources. Promotion is an
  explicit action with a destination.
- Promotion targets depend on the Source's type:
  - `filesystem` → write file at chosen path
  - `memory` → append to memory store
  - `connector-backed` → invoke connector op (e.g., "send email" via Gmail
    connector promotes a Draft email to the connector's outbox)
  - `mcp-resource` → MCP server's write op if available
- Promotion is gated by the autonomy matrix row "Drafts → Sources promotion":
  - **Low**: every promotion requires user approval (diff shown in approvals
    tray).
  - **Medium**: auto-promote to internal Sources (filesystem, memory);
    external (connector / MCP) requires approval.
  - **High**: auto-promote anywhere the connector / MCP allows.
- The promotion event creates a **Receipt** linking the Draft, the destination
  Source, the autonomy posture in effect, and the user (per ADR-004) or
  routine (per ADR-008) that triggered it.
- A Draft is NOT deleted when promoted; it transitions to status `'exported'`
  and gains a back-reference to the resulting Source entries. This preserves
  the audit trail and lets users re-promote if needed.

The autonomy invariants:

- Sources cannot be written without going through Drafts.
- Drafts cannot be promoted without going through the posture matrix.
- Routines that produce outputs follow the Routines-when-absent posture row,
  which can be stricter than the interactive posture (ADR-008).

## Consequences

### Positive

- Single write-audit point. Every change to Sources has a Draft predecessor
  and a Receipt.
- The autonomy matrix has a coherent "Drafts → Sources" row that means
  exactly one thing.
- The Mind UI can render a clear "what changed" view by listing recent
  promotions.
- Open Bias's POST_CALL evaluators can inspect Drafts for policy violations
  *before* promotion, rather than chasing already-written Source content.
- Users get a uniform review experience regardless of which Power produced the
  output.

### Negative

- Routines that update memory frequently cost a Draft → Promote step each
  time. Mitigated by allowing high-autonomy claws to auto-promote internal
  Sources (memory, filesystem) without explicit approval.
- The Draft type must carry enough metadata (target Source candidates,
  promotion history, autonomy snapshot at creation) to support audit. This
  expands the schema slightly versus the existing H02 `Draft`.
- A small UI affordance is needed everywhere a Draft appears: the "Promote
  to…" action with target picker.

### Security

- This invariant is the foundation of "the agent layer cannot quietly modify
  durable state." Removing it weakens every other security guarantee.
- Promotion to external destinations (Connector ops) is gated separately and
  more strictly because external writes are typically irreversible.
- The Receipt of a promotion records the autonomy posture *at the time of
  promotion* so audit can answer "would this have been gated under the
  current posture?"

## References

- ADR-003 — Two sidecars (Open Bias evaluates Drafts pre-promotion)
- ADR-008 — Three autonomy postures (Drafts → Sources promotion row)
- [01-claw-anatomy.md](../01-claw-anatomy.md) — privileged box layout
- [05-autonomy-postures.md](../05-autonomy-postures.md) — promotion row
- [08-mind.md](../08-mind.md) — Receipts and Drafts tabs
- [09-data-model.md](../09-data-model.md) — Draft and Receipt schemas
- [10-h02-migration.md](../10-h02-migration.md) — H02 `DraftStatus.exported`
  becomes `'promoted'`
