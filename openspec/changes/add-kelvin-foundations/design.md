## Context

Kelvin's architecture is centred on a recursive "Claw" primitive.
Every dispatcher (the macro claw the user talks to first, plus every
sub-claw for a domain like Health/Personal/Work) has the same anatomy
and the same posture knobs. This design doc captures the foundational
decisions: the recursion model, the four-way distinction between
agent-facing concepts, the lifecycle split between persistent
configuration and runtime spawn, and the single-write-edge invariant
from Drafts to Sources.

The kelvinclaw runtime already provides the `Tool` / `ToolRegistry`
seam (see `OVERVIEW.md`) and `ModelProvider` shim point. H02 (the
front-end at https://github.com/kmondlane/h02) has a rich type system
that pre-figures much of this design but conflates several concepts
that this change separates.

## Goals / Non-Goals

**Goals:**
- Specify the recursive Claw primitive once and apply it identically
  at every depth.
- Disambiguate the four agent-facing concepts so each has a clear
  lifecycle, storage home, trust profile, and authoring path.
- Define every persistent schema and the validation invariants that
  hold across them.
- Describe the call-tree shape so observability (in
  `add-kelvin-ui-runtime`) and security (in `add-kelvin-security`)
  have a stable contract to attach to.

**Non-Goals:**
- No multi-user / shared-claw enforcement (deferred to v2; ADR-004).
- No autonomy posture *enforcement* — that lives in
  `add-kelvin-security`. This change defines the data shape only
  (`PosturePerAxis`).
- No GUI surfaces — those land in `add-kelvin-ui-runtime`.
- No model or tool boundary integration — `add-kelvin-security`.
- No code; this is a behavioural spec.

## Decisions

### D1. Sub-agents are runtime-only, not stored per-claw

**Context.** Earlier drafts modelled sub-agents as a privileged box
inside every Claw, alongside Sources, Drafts, and Powers, with a fixed
enum of 25 specialist roles. That coupled three concerns: capability
(what the agent can do — a Power), persona (system prompt, role
identity), and lifecycle (spawn vs. discard).

**Alternatives considered.**
- Keep stored Sub-agents per claw (status quo H02). Forces a
  redundant taxonomy, dead state when not running, rigid Power
  coupling.
- Collapse Sub-agents into Powers entirely. Loses the transient
  identity / sub-session / budget concept; conflates function with
  process invocation; weakens Mind's call-tree legibility.
- *Chosen:* Sub-agents are runtime instances spawned for one
  sub-session. Optional per-claw `SubAgentTemplate`s preset common
  spawns. The 25 H02 specialists become seed templates.

**Consequences.** `Power.agentType` is removed. `AGENT_CATEGORIES`
collapses into `POWER_CATEGORIES`. A `SubAgentInstance` exists
runtime-only and is referenced by Receipts via id, never persisted.
The spawning claw's posture caps the Sub-agent on every axis,
re-checked per Power invocation.

### D2. Powers, Connectors, MCP servers, Sub-agents are four distinct concepts

**Context.** These four words have been used interchangeably; H02's
type system reflects that muddiness (`Power.agentType` ties a Power
to a specialist; `UserIntegration` is a half-formed Connector; MCP
servers don't exist in the type system at all). Each concept has a
different lifecycle, trust profile, settings home, and upgrade story.

**Alternatives considered.**
- One taxonomy "tools" with type discriminators. Loses every
  operationally-relevant distinction.
- Two taxonomies (extensions vs runtime instances). Still flattens
  the install/auth/protocol distinctions among extensions.
- *Chosen:* Four distinct concepts with explicit nesting —
  Sub-agents → Powers → Connector ops / MCP tools / built-in tools.
  Connectors and MCP servers are global (Settings); Powers are
  per-claw (privileged); Sub-agents are runtime-only.

**Consequences.** Five distinct schemas. `Power.requires` declares
dependencies. A claw must bind a Connector/MCP server before its
Powers can use them. Plugin authoring paths differ by concept.

### D3. Recursive Claw primitive — macro and sub-claws are structurally identical

**Context.** H02's existing `Space` partially models recursion via
`parentSpaceId` and `chief`, but mixes in fields like `isHome` and a
fixed `SpaceType` enum that don't generalise.

**Alternatives considered.**
- Separate `MacroClaw` and `SubClaw` types. Doubles UI surface;
  loses "every claw is a dispatcher" mental model.
- Single Claw type with positional flags. Optional-but-only-meaningful-
  at-root fields are a smell.
- *Chosen:* Single Claw schema with `parentClawId` (null only for
  macro). Globally shared concerns (Modes, Inputs, Mind, Settings)
  live in the UI/context layer, NOT on the Claw schema.

**Consequences.** One Component renders both macro and sub-claws.
Privilege invariants are tree walks: `child.boundConnectorIds ⊆
parent.boundConnectorIds` (and similarly for MCP). Posture cap-chain
is uniform at every depth.

### D4. Drafts → Sources is the only outbound promotion edge from privileged

**Context.** When agents produce work, that work needs a home.
Without enforcement, Powers could write directly to Sources, bypassing
user review.

**Alternatives considered.**
- Drafts and Sources are independent; outputs land in either. No
  enforced review; lowered safety bar.
- All outputs go to Sources; Drafts as opt-in side effect. Same
  problem inverted.
- *Chosen:* All Power/Sub-agent outputs land as Drafts. Drafts can
  be promoted to Sources via an explicit action gated by autonomy
  axis `draftPromotion` (specced in `add-kelvin-security`).

**Consequences.** Single audit point for every change to Sources.
Receipts produced by promotion record which Draft and which posture
were in effect. Routine memory updates pay a Draft+Promote
round-trip; at high autonomy this can be auto-promoted with an
internal-target carve-out.

## Risks / Trade-offs

[Risk: forward-compat `ownerId` / `createdBy` fields locked in v1
shape may not match v2 multi-user model] → mitigation: the fields
are opaque-to-enforcement; v2 changes the enforcement layer, not the
schema shape. Worst case v2 also adds an `ownerType` discriminator
which is purely additive.

[Risk: Drafts→Sources audit overhead becomes painful for high-
frequency Routine memory updates (288/day per Routine)] →
mitigation: high-autonomy claws may auto-promote internal-target
Drafts (filesystem, memory) without explicit approval. Audit trail
remains via Receipts. Deferred to `add-kelvin-security`.

[Risk: same-uid sibling processes can mutate `RULES.md` or
credentials] → not solved by this change. The data-model capability
documents this assumption but does not enforce process isolation.
Trust boundary is filesystem ownership in v1; documented in release
notes.

[Risk: cap-chain validation cost at unbounded recursion depth] →
mitigation: a runtime PostureService cache invalidates per-claw on
mutation; lookups are O(1). Real-world depth >5 unlikely.

[Risk: 4-concepts taxonomy adds learning surface] → mitigation:
beginners interact at the Power level; Settings → Connectors / MCP
panels are progressive disclosure. The disambiguation table in the
`concepts-taxonomy` capability is the on-boarding reference.

## Migration Plan

This change is documentation-only at the kelvinclaw level — it
specifies behaviour, not code. Concurrent code work happens against
the requirements:

1. **Runtime (kelvinclaw)**: implement schemas + validation;
   `SubAgentInstance` runtime registry; append-only Receipt store.
2. **GUI (H02)**: execute the migration from `useGingerStore` to
   `useClawStore`. Slice plan: type-layer renames → store rename →
   constants update → component updates. Tracked in `tasks.md`.

Archive order: this change archives FIRST, before
`add-kelvin-security` (which references the data-model capability).

## Open Questions

1. **Should the v1 schema include `ownerId` / `createdBy` at all?**
   The fields exist for v2 multi-user but are populated with the
   single-user value and never enforced. A leaner v1 would omit
   them. Choosing to include them locks v1 storage to a v2 design
   that hasn't been validated. Validate against an early v2 spike.

2. **Sub-agent depth cap = 1 in v1.** Sub-agents cannot themselves
   spawn Sub-agents. This avoids fan-out explosion but limits
   patterns where a Researcher delegates to a Critic. Revisit if v1
   usage shows the limit hurting.

3. **Cycle prevention scope.** The current spec prevents ancestor
   delegation and same-session sibling cycles. Cross-session sibling
   cycles are not prevented (sibling A delegated to B in session X;
   in session Y, B delegates to A). Acceptable in v1; revisit for
   v2 patterns.

4. **`Power.kind = 'delegate-to-sub-claw'` ergonomics.** Modelling
   delegation as a Power keeps audit uniform but means the macro
   claw's library has one Power per active sub-claw. With many
   sub-claws this becomes noisy. Mitigation: hide
   `delegate-to-sub-claw` Powers in the default Powers UI; surface
   them in a "Sub-claws" panel.
