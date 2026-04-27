---
status: Accepted
version: v1
audience: architects, implementors
date: 2026-04-27
---

# ADR-005 — Recursive Claw primitive: macro and sub-claws are structurally identical

## Status

Accepted.

## Context

The Kelvin GUI architecture distinguishes a **macro claw** (Kelvin — the root
dispatcher) from **sub-claws** (Health, Work, Personal, Finance, Learning,
Creative, …). Each sub-claw is itself a dispatcher for its space and may have
sub-sub-claws.

The user's mental model has been:

> "the Macro Claw is more of an Orchestrator/Chief of Agents (serves in a similar
> way to Claude's 'dispatch'), micro claws are other connected claw instances
> and chiefs of their ('X') space also serve as 'dispatches' to their own spaces."

That is, every claw is a dispatcher. The macro/sub distinction is positional, not
typed.

H02's existing data model partially reflects this:

- `Space` (`H02/src/types/index.ts`) has `parentSpaceId` (recursive parenting)
  and `chief: SpaceChief` (the dispatcher persona).
- `SpaceChief.reportingTo` field references the parent chief.

But `Space` also has fields that don't generalize cleanly (`isHome`,
`SpaceType` enum with hard-coded variants like `'user' | 'work' | …`),
suggesting a half-finished recursion.

A clean recursive primitive matters because:

1. **Code reuse**: one Claw component renders both macro and sub-claws.
2. **Consistency**: every claw has the same anatomy, the same Soul/Rules
   files, the same posture knobs, the same Mind tabs.
3. **Privilege flow**: parent caps child applies uniformly at every level.
4. **Onboarding**: spinning up a new sub-claw uses the same wizard as
   spinning up the macro claw.

## Alternatives Considered

### Alternative A — Macro and Sub-claw are different types

A `MacroClaw` type with global-only concerns (channels, modes, mind) and a
`SubClaw` type with claw-only concerns. Different schemas, different code paths.

**Pros:** Each schema captures only what's relevant to its scope.

**Cons:** Doubles the maintenance surface. UI components must branch on which
type they're rendering. Onboarding flows diverge. The "every claw is a
dispatcher" mental model is lost in the type system.

### Alternative B — Single Claw type with positional flags

One `Claw` schema. Fields that only apply to the macro level (e.g., global
channel bindings) are marked optional and ignored on sub-claws.

**Pros:** Single schema; simpler than Alternative A.

**Cons:** "Optional but only meaningful at root" fields are a smell. Validation
becomes positional. Future deep nesting (grand-sub-claws) requires more flag
gymnastics.

### Alternative C (chosen) — Single Claw type, fully recursive, with explicit globals separated

A single `Claw` type carries all the privileged anatomy boxes. Globally shared
concerns (Modes, Inputs, Mind, Settings) are NOT on the Claw — they live in
the Settings/UI layer and every claw sees them via context. Recursion is via
`parentClawId`. The macro claw is the unique claw with `parentClawId === null`.

**Pros:** Clean type. Recursion is uniform. UI components render the same Claw
view regardless of position. Parent-caps-child invariant maps to a tree walk.
Future deep nesting works without schema change.

**Cons:** The "globally shared" concept (Modes, Inputs, Mind, Settings) must be
explicitly modeled as not-on-Claw — easy to forget when adding new fields.
Mitigated by [01-claw-anatomy.md](../01-claw-anatomy.md) and ADR-005's
explicit globally-shared list.

## Decision

A single `Claw` TypeScript schema (see [09-data-model.md](../09-data-model.md))
captures the full anatomy:

- `id`, `name`, `parentClawId` (nullable; null only on the macro claw)
- `soul` (path to `soul.md`)
- `rules` (path to `RULES.md`)
- `sources` (collection)
- `drafts` (collection)
- `powers` (collection)
- `triggers` (hooks / heartbeats / watches)
- `channels` (bound subset)
- `boundConnectorIds` (subset of installed Connectors)
- `boundMcpServerIds` (subset of installed MCP servers)
- `subAgentTemplates` (optional presets per ADR-001)
- `autonomyPosture` (capped by parent per ADR-008)
- `ownerId`, `createdBy`, `createdAt` (per ADR-004)

**Globally shared (NOT on Claw):**

- **Modes** (Auto/Plan/Ask/Learn/Play/Make) — UI state, applies to current
  composer.
- **Inputs** (composer / voice / channels) — UI surface.
- **Mind** (observability, tabs, call-tree) — global view across all claws,
  filterable.
- **Settings** (identity, channel installs, Connector installs, MCP installs,
  sidecar config, user-level autonomy cap) — global config.

The recursion guarantee:

> Any claw can spawn child claws. A child claw is a `Claw` record with
> `parentClawId` set. The same component, the same wizard, the same posture
> picker, the same Soul/Rules editing UI applies at every depth.

Privilege invariants (formalized in ADR-008):

- `child.autonomyPosture <= parent.autonomyPosture` along every axis.
- `child.boundConnectorIds ⊆ parent.boundConnectorIds`.
- `child.boundMcpServerIds ⊆ parent.boundMcpServerIds`.
- A child claw may NOT bind a Connector or MCP server the parent has not bound.

## Consequences

### Positive

- One Claw component, one wizard, one anatomy diagram for every depth.
- The "spinning up a new sub-claw" UX flow doesn't differ from "spinning up
  the macro claw" beyond default values.
- Privilege invariants are tree-walk checks, easy to test.
- Mind's call-tree renders uniformly: every node is a claw or an action
  inside a claw.
- New depth levels (grand-sub-claws, etc.) require no schema or UI change.

### Negative

- The macro claw is identifiable only by `parentClawId === null`; code that
  needs "the root" must search rather than read a flag. Mitigated by a derived
  selector in the Zustand store.
- Globally shared concerns (Modes, Inputs, Mind, Settings) must be carefully
  kept off the Claw schema. New contributors may try to add them; lint or
  schema review must catch this.
- The H02 `Space.isHome` boolean must be removed in migration; consumers of
  this field migrate to a derived `isMacroClaw` selector.

### Security

- Privilege invariants are uniform across depth, simplifying the security
  model. No special "macro" privilege check.
- Subset enforcement (Connector/MCP bindings) propagates by validation at
  bind time and at every claw mutation.

## References

- ADR-001 — Sub-agents are runtime-only (per claw, no specialist roster)
- ADR-002 — Powers / Connectors / MCP / Sub-agents distinction
- ADR-008 — Three autonomy postures with parent-caps-child invariant
- [01-claw-anatomy.md](../01-claw-anatomy.md) — full anatomy diagram
- [09-data-model.md](../09-data-model.md) — `Claw` schema
- [10-h02-migration.md](../10-h02-migration.md) — `Space → Claw` migration
