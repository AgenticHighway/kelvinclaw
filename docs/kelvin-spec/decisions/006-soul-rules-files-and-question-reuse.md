---
status: Accepted
version: v1
audience: architects, implementors, security engineers
date: 2026-04-27
---

# ADR-006 — Per-claw `soul.md` + `RULES.md` as file-backed config; reuse `Question`/`QuestionCarousel` for approvals

## Status

Accepted.

## Context

Two things must be configured per claw:

1. **Identity / charter / style** — what this claw is, what it values, how it
   speaks. The user named this "Soul" and explicitly said: "the soul can be one
   `.md` for now."
2. **Enforceable policy** — what this claw must not do, what content classes
   are out of bounds, what tool calls are restricted. This must be enforceable
   by the model-boundary sidecar (Open Bias per ADR-003), which already reads
   plain `RULES.md`.

Storing both as plain markdown files (rather than as JSON in a database) buys:

- **Git-versioned** — `RULES.md` changes get reviewed like code; soul revisions
  are auditable.
- **Human-editable** — users can edit in any text editor; no GUI lock-in.
- **Portable** — exporting a claw is "tar this directory."
- **Open Bias compatible out of the box** — Open Bias already reads `RULES.md`
  per service.

Separately, the autonomy approvals primitive — the tray that pops up when a
gated action fires and asks "Allow once / session / always?" — needs to be
designed. H02 already has a sophisticated user-clarification primitive:
`Question` / `QuestionCarousel` (`H02/src/types/index.ts`), with options,
trace items, attachments, urgency, confidence scores, and a carousel UI. It
was built for "agent needs clarification from user," which is structurally
identical to "agent needs approval from user."

## Alternatives Considered

### Alternative A — Store Soul and Rules in the database (JSON)

Soul fields and policy rules as JSON in the same store as other entities.

**Pros:** Single storage layer; no filesystem I/O; queryable.

**Cons:** Loses version control. Open Bias compatibility requires a
serialization layer to materialize `RULES.md` from JSON on every change.
Editing requires the GUI; no "open in editor" path. Loses the Anthropic-style
"agent file" affordance (users familiar with Claude Code's `claude.md`
recognize `soul.md`).

### Alternative B — Three files: `soul.md`, `style.md`, `rules.md`

Split Soul into identity (immutable-ish) and style (frequently tuned).

**Pros:** Different lifecycles for different content.

**Cons:** Splits a small thing too early. The user explicitly said "one `.md`
for now." Easy to add `style.md` later if needed.

### Alternative C (chosen for files) — Two files per claw: `soul.md` + `RULES.md`

`soul.md` holds identity, charter, style — one human-readable markdown file.
`RULES.md` holds enforceable policy in Open Bias's format. Both live under
the claw's directory. Both are git-versioned.

### Alternative I — Build a new approvals primitive from scratch

Design a fresh React component for the approvals tray, separate from any
existing H02 primitive.

**Pros:** Pure design freedom.

**Cons:** Reinvents the `Question`/`QuestionCarousel` wheel. The existing
primitive has options, urgency, trace items, attachments, confidence — every
field needed for an approvals tray. Throwing it away and rebuilding is waste.

### Alternative II (chosen for approvals) — Reuse `Question` / `QuestionCarousel`

Extend H02's existing `Question` type to be the foundation of the approvals
primitive. Add a `Question.kind` discriminator: `'clarification' | 'approval'`.
Approval-kind questions carry the additional fields needed for posture
overrides (`scope: 'once' | 'session' | 'claw' | 'forever'`).

**Pros:** Maximum reuse of existing UI work, type system, store actions, and
test coverage. The carousel UX (multiple pending approvals stacked) maps
naturally onto autonomy approvals.

**Cons:** `Question` becomes a slightly more discriminated union. Need to
audit existing consumers to ensure they handle the new `kind`. Mitigated by
the migration map in [10-h02-migration.md](../10-h02-migration.md).

## Decision

**Per-claw files:**

Every claw owns a directory; two markdown files within it are conventional:

```
<claw-id>/
├── soul.md      # identity, charter, style — single .md per user request
├── RULES.md     # enforceable policy — Open Bias format
└── (other claw assets, e.g., source manifests, draft cache)
```

`soul.md` content shape (free-form markdown; no schema enforcement in v1):

- Name and one-line purpose
- Charter (what this claw is for, what it explicitly is not for)
- Style (tone, verbosity, formatting preferences)
- Optional: starting notes, vocabulary, examples

`RULES.md` content shape (Open Bias's plain-markdown rules format):

- One section per rule
- Each rule: a description + an evaluator hint + an enforcement action
- See [07-sidecars.md](../07-sidecars.md) for examples and Open Bias docs
  for the canonical format.

Both files are loaded by:

- The GUI (for display, editing).
- The `ModelProvider` shim (which injects per-claw `RULES.md` selection into
  Open Bias via header — see
  [interfaces/sidecar-integration.md](../interfaces/sidecar-integration.md)).

**Approvals primitive:**

Extend the existing `Question` type with a `kind: 'clarification' | 'approval'`
discriminator. Approval-kind questions carry:

- `actionDescriptor` — the gated action being requested (tool, scope, parent
  Power, parent claw)
- `defaultChoice` — recommended choice with rationale
- `scopeOptions: ('once' | 'session' | 'claw' | 'forever')[]` — remember-this
  granularity per ADR-008
- `riskLevel` — derived from posture matrix row
- `expiresAt` — soft deadline before the approval auto-denies

The `QuestionCarousel` UI pattern stacks pending approvals in the same way it
already stacks clarifications. A persistent termination control is mirrored
in both the main chat composer area and Mind's session tab.

## Consequences

### Positive

- `soul.md` and `RULES.md` are inspectable, editable, version-controlled,
  shareable.
- Open Bias works with `RULES.md` natively — no transformation layer.
- The approvals primitive is built on shipped, tested H02 code rather than
  greenfield.
- Mind can render an "approvals" tab that's a filtered view of the existing
  Questions feed.
- Exporting a claw becomes `tar` the directory (foundation for v2 portable
  templates).

### Negative

- Two files per claw means filesystem-level concerns (directory layout,
  permissions, locking) need to be specified. See
  [interfaces/sidecar-integration.md](../interfaces/sidecar-integration.md)
  for path conventions.
- The `Question` type discriminator change requires updating consumers:
  `QuestionCarousel`, `useGingerStore` actions for `addQuestion`/`answerQuestion`,
  any component reading `Question` fields directly.
- `RULES.md` syntax is Open Bias's; users must learn it. Mitigated by shipping
  template `RULES.md` files and documentation in
  [07-sidecars.md](../07-sidecars.md).

### Security

- File-system permissions on the claw directory become part of the security
  model in v1 (single user; user owns the dir). v2 multi-user must address
  per-user file ACLs.
- A compromised process that writes to `RULES.md` can lower the policy bar.
  Mitigated by `git`-tracked changes (review surface) and read-only mounting
  in production deployment patterns.
- The approvals primitive must be UX-honest: a malicious model output cannot
  manipulate the approval tray content (the tray is rendered from server-trusted
  state, not from model output). This is a v1 implementation requirement.

## References

- ADR-003 — Two security sidecars (Open Bias model boundary)
- ADR-005 — Recursive Claw primitive (each claw owns its own files)
- ADR-008 — Three autonomy postures (approval scope semantics)
- [01-claw-anatomy.md](../01-claw-anatomy.md) — Soul and Rules in anatomy
- [06-approvals-primitive.md](../06-approvals-primitive.md) — full approvals UI spec
- [07-sidecars.md](../07-sidecars.md) — RULES.md format and examples
- [interfaces/sidecar-integration.md](../interfaces/sidecar-integration.md)
- H02 `src/types/index.ts` `Question` type
