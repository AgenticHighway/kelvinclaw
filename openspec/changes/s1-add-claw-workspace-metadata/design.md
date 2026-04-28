## Context

s0 wired H02 to kelvinclaw via the existing `agent` method, with no
notion of multiple workspaces. kelvinclaw already has
`SessionDescriptor { session_id, session_key, workspace_dir }` in
`crates/kelvin-core/src/sessions.rs`. This slice adds a `Claw`
record that overlays metadata on top of the workspace concept —
`name`, `parentClawId`, `iconRef`, `description` — and exposes
CRUD methods on the gateway.

H02 already has a `spaces` mock-data taxonomy with similar shape
(macro/child structure, name, icon). Migrating from "spaces (mock)"
to "claws (live)" is mostly a frame-layer rewire.

## Goals / Non-Goals

**Goals:**
- Establish the recursive Claw primitive with the exactly-one-macro
  invariant. This is the data foundation for every later slice.
- Land the four CRUD methods on the gateway as new but uncomplicated
  additions to the existing protocol.
- Replace H02's mock spaces with live claws.

**Non-Goals:**
- No `soul.md` or `RULES.md` files yet (s2).
- No autonomy posture per claw (s3 onwards).
- No bound-Connectors or bound-MCP-servers subset invariants (later
  slices once those concepts land).
- No `ownerId`/`createdBy` ownership fields. Single-user is the v1
  trust boundary; we can add ownership fields when multi-user lands.

## Decisions

### D1. Claw is metadata over the existing workspace concept

Rather than introducing a parallel data model, Claw extends what
`SessionDescriptor` already provides. The runtime's
`workspace_dir` continues to anchor a session; `claw_id` is a
new metadata link that may map to a sub-directory under the
data dir.

**Alternative:** introduce a fully separate Claw store unrelated to
workspaces.
**Rejected because:** adds a parallel persistence layer when the
existing one already covers most needs. Better to extend.

### D2. Recursive primitive: single Claw type, `parentClawId` nullable

Macro and sub-claws are structurally identical. The macro is
identified by `parentClawId === null`. This was an early architecture
decision (preserved here as the central design constraint).

**Alternative:** separate `MacroClaw` / `SubClaw` types.
**Rejected because:** doubles the surface area for no semantic gain;
recursion in later slices (s5 delegation) becomes positional rather
than uniform.

### D3. Auto-create macro claw on first start

A fresh install needs SOMETHING for the gateway's existing `agent`
method to route to. Auto-creating a default macro claw named
"Kelvin" means s0's "submit without `claw_id`" behaviour stays
useful — those submits route to the macro.

**Alternative:** require explicit setup before first `agent` call.
**Rejected because:** that's UX friction with no safety benefit.

### D4. Claws cannot be deleted while they have children

The simplest invariant that prevents accidental orphan trees. Force
the user to delete (or re-parent) children first.

**Alternative:** cascade delete.
**Rejected because:** destroys data without explicit consent;
cascade can be added later as an explicit `claw.delete --cascade`
flag.

## Risks / Trade-offs

[Risk: H02's existing `spaces` mock structure may have fields that
don't map cleanly onto Claw — e.g., `chief`, `defaultSubAgents`,
`SpaceType` enum, `privacy`] → mitigation: those fields live in s5+
slices when sub-agents and posture appear. For s1, we map only the
fields Claw defines: `name`, `parentSpaceId → parentClawId`,
`iconRef`, `description`. Other H02 fields stay in mock state until
their slice lands.

[Risk: people may want to bulk-import their existing H02 mock
spaces as live claws] → mitigation: ship a one-shot migration
script in H02 dev tools that walks the mock spaces and issues
`claw.create` calls. Not part of the spec — implementation tooling.

[Risk: `claw_id` becomes a privileged routing parameter; spoofing
it could let one claw's session be misrouted] → mitigation:
single-user v1 trust boundary means filesystem ownership of the
data dir bounds the threat. v2 multi-user adds per-user `claw_id`
authorization.

## Migration Plan

This slice depends on s0 having archived. Order:

1. s0 archives → `openspec/specs/h02-gateway-connection/spec.md`
   exists.
2. s1 implements: kelvinclaw adds Claw store + 4 methods; H02 wires
   live claw list.
3. s1 archives → `openspec/specs/claw-workspace-metadata/spec.md`
   created; `openspec/specs/h02-gateway-connection/spec.md`
   modified.

## Open Questions

1. **Filesystem layout for per-claw data.**
   `<KELVIN_DATA_DIR>/claws/<claw_id>/` is the natural choice and
   forward-compatible with s2 (soul.md / RULES.md). But the
   existing workspace concept uses `workspace_dir` directly. Decide
   whether claws have their own subdirectory under data dir or
   point at arbitrary `workspace_dir`s.

2. **claw_id format.**
   ULID? UUID? Slug-from-name? ULIDs sort by creation time which is
   nice for browsing; slugs are human-readable. Probably ULID with
   a slugged display label. Confirm at impl time.

3. **Should `claw.list` be paginated?**
   For single-user installs with O(10) claws, no. v2 multi-user
   with shared claws may want pagination. Spec leaves it unpaginated
   in v1; `MODIFIED` later if needed.
