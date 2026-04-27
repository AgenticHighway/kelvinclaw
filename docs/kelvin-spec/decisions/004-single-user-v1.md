---
status: Accepted
version: v1
audience: architects, product
date: 2026-04-27
---

# ADR-004 — Single-user v1; multi-user / shared spaces deferred to v2

## Status

Accepted.

## Context

Throughout the architecture conversation, several features touched on multi-user
concerns:

- **Shared Spaces** — H02's existing `Space.privacy: 'private' | 'shared' |
  'collaborative'` and `allowedUserIds` fields suggest multi-tenant intent.
- **Identity & access** — who owns a claw, who can invoke it, how authentication
  resolves to posture caps.
- **Cross-claw porosity across users** — a "Family-Finance" sub-claw shared
  between two users requires a different security model than a single-user
  porosity row.
- **Composio / connector credential ownership** — whose OAuth tokens are these?
  Does the shared sub-claw use one user's Gmail or both?

Multi-user is an architecturally large concern. It affects:

- The autonomy matrix (per-user posture caps; whose cap wins on a shared claw?)
- The data model (every entity needs an owner / ACL field)
- The runtime (session identity propagation through delegation)
- The sidecars (`RULES.md` selection by user vs. by claw)
- The Mind UI (whose receipts can you see?)
- The GUI (login, account switching, invitations, presence)

The user has acknowledged this is a significant decision and explicitly
proposed versioning it: ship single-user v1, layer multi-user on in v2.

## Alternatives Considered

### Alternative A — Multi-user from day one

Build the v1 with full ACLs, per-user posture caps, shared-Space invitations,
and identity propagation through every layer.

**Pros:** No retrofit later; the data model is "right" from the start.

**Cons:** Substantial scope expansion. Every entity gets ownership fields.
Every UI gets account-switching. Every autonomy enforcement point gets a
user-cap lookup. Quadruples the v1 surface area for a feature most personal-AI
users don't need on day one.

### Alternative B — Defer multi-user indefinitely

Build v1 single-user; never add multi-user.

**Pros:** Smallest scope.

**Cons:** Closes the door on a real future need (couples sharing finance,
families sharing health logs, teams sharing project Spaces). The `Space.privacy`
fields in H02 already reflect intent.

### Alternative C (chosen) — Single-user v1; multi-user v2 with explicit migration plan

v1 ships with a single user account per install. The data model includes
ownership fields (`createdBy`, `ownerId`) where they cost nothing to add now,
but does not implement ACL enforcement or invitation flows. v2 adds:

- Per-user posture caps (caps stack: install cap > user cap > claw cap)
- Shared claw ACLs (read / invoke / configure permissions)
- Invitation flows
- Cross-user delegation rules
- Audit attribution (Receipts show which user triggered what)

**Pros:** v1 is shippable; v2 has a clear path with no breaking model changes
for ownership fields that already exist. Defers complex UX work (account
switching, presence) until there's signal it's needed.

**Cons:** v1 users won't have shared spaces. Mitigated by exporting claws as
templates (v2) so workflows can be shared even if state isn't.

## Decision

**v1 ships single-user.** Specifically:

- One user account per install.
- No login UI in v1; the running user of the H02 GUI process is the implicit
  user.
- Data model includes `ownerId`, `createdBy`, `createdAt` fields on every
  persistent entity (Claw, Power, Connector, MCPServer, SubAgentTemplate,
  Source, Draft, Receipt). These are populated with the single user's id but
  enforcement is not wired.
- `Space.privacy` and `Space.allowedUserIds` from the legacy H02 schema are
  retained but not enforced; they act as forward-compatible placeholders.
- The autonomy matrix has only one cap level (User cap → Claw cap → per-action
  override). No per-user cap stacking.
- Authentication is implicit (filesystem ownership of config dir).

**v2 will add:**

- Multi-user account model with login.
- ACLs on Claws and Sources (`read | invoke | configure`).
- Per-user posture caps stacked above user caps.
- Invitation and presence flows.
- Cross-user delegation rules in the autonomy matrix (new row).
- Receipt attribution by user.
- Shared-claw conflict resolution (whose Soul wins? whose RULES.md wins?
  expected: claws have a single owner; collaborators get a "viewer" or
  "invoker" role but cannot edit Soul/Rules).

## Consequences

### Positive

- v1 scope is achievable. No login flow, no ACL UI, no invitation system.
- The data model carries forward-compatible ownership fields; v2 is additive,
  not a breaking schema change.
- Documentation can mark multi-user features clearly as v2 in the roadmap
  ([11-roadmap.md](../11-roadmap.md)).
- Single-user simplifies the autonomy invariants in ADR-008 (no need to think
  about whose cap wins).

### Negative

- v1 cannot be used by multiple humans on the same install (only one human at
  a time, with no isolation).
- Couples / families wanting a shared "household" claw must wait for v2 or
  manually share an install (with the security implications).
- Templates (v2) are the v1 workaround for sharing workflows; in v1, templates
  are local files only.

### Security

- Single-user means no cross-user authz checks to write or test in v1, but
  also no defense against multiple humans sharing one install (one human's
  posture is the install's posture).
- Filesystem ownership of the config dir is the de facto authentication
  boundary in v1; document this.
- All ownership fields are populated to the single user id so the v2 migration
  is purely additive (add ACL enforcement, no backfill).

## References

- ADR-005 — Recursive Claw primitive
- ADR-008 — Three autonomy postures (single cap level in v1)
- [11-roadmap.md](../11-roadmap.md) — v1/v2/v3 split
- [09-data-model.md](../09-data-model.md) — ownership fields
- [10-h02-migration.md](../10-h02-migration.md) — `Space.privacy` placeholder
  retention
