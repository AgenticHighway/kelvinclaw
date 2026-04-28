## Context

s1 created `Claw` records with metadata (`name`, `parentClawId`, …)
but didn't change how the model call is made. Every `agent` turn
still uses the runtime's default system prompt. To make claws
*feel* like they have identity, the system prompt must vary per
claw.

The kelvin GUI design uses two per-claw markdown files: `soul.md`
(identity / charter / style) and `RULES.md` (enforceable policy).
This slice introduces both. `soul.md` is consumed immediately by
the brain. `RULES.md` is reserved for s6 (the slice that introduces
the optional Open Bias model-boundary profile).

## Goals / Non-Goals

**Goals:**
- Land per-claw filesystem layout that's the same shape Open Bias
  would consume later.
- Make the assistant's tone visibly change when the user edits a
  claw's `soul.md`.
- Keep the door open for s6 to introduce model-boundary policy
  without restructuring the file layout.

**Non-Goals:**
- No actual `RULES.md` enforcement (s6).
- No multi-file claw layouts (sources, drafts, etc.) — those land
  later when those concepts arrive.
- No file-watching for live edits (write → read round-trip is
  enough; the brain reads on each turn).
- No `RULES.md` syntax spec — Open Bias will dictate that when it's
  introduced.

## Decisions

### D1. Two markdown files: soul.md + RULES.md

Two files match the user's design ("the soul can be one .md for
now" and "RULES.md as the enforcement file"). They live alongside
each other in the per-claw directory.

**Alternative:** one combined `claw.md` with sections.
**Rejected because:** Open Bias's enforcement model expects a
separate rules file (per the user's earlier `RULES.md` references).
Keeping them separate from day one means s6 doesn't need a
file-split migration.

### D2. Brain reads soul.md on every turn (no cache)

Reading a small markdown file per turn is cheap. Avoids cache
invalidation complexity when the user edits the soul.

**Alternative:** in-memory cache invalidated on `claw.soul.write`.
**Rejected because:** premature optimization for v0. If the read
becomes a hotspot in real usage, add a cache later.

### D3. RULES.md exists but is not enforced in this slice

Creating the file now means s6 doesn't need a "migrate existing
claws to add RULES.md" step. The file is forward-compat scaffolding.

**Alternative:** wait until s6 to create `RULES.md`.
**Rejected because:** then s6 has to migrate every existing claw,
which is more work than seeding empty stubs from day one.

### D4. Defaults are minimal but non-empty

Empty `soul.md` would mean every claw acts identically until the
user edits it. Minimal-but-named defaults give immediate
differentiation.

## Risks / Trade-offs

[Risk: a malformed `soul.md` could produce confusing model
behaviour] → mitigation: the brain's read path SHALL fall back to
the runtime default if the file is unreadable. There's no syntactic
validation of `soul.md` content (it's just markdown the model sees);
malformed content is the user's problem to debug.

[Risk: filesystem permissions / quotas could fail directory
creation] → mitigation: surface the error from `claw.create`; do
not persist the Claw record without successful directory creation
(atomic semantics).

[Risk: `RULES.md` written before s6 may use a syntax that the
eventual sidecar doesn't understand] → mitigation: spec explicitly
says "RULES.md is not enforced in this slice," so users editing it
in s2 should understand it's a future-feature placeholder. Document
in H02's RULES editor with a note.

[Risk: `soul.md` content leaks into model context that may include
sensitive identity information] → mitigation: the file lives on
the user's filesystem; same trust boundary as everything else in
v1. v2 multi-user adds per-user file ACLs.

## Migration Plan

1. s1 archives → claw CRUD methods exist in
   `openspec/specs/claw-workspace-metadata/spec.md`.
2. s2 implements: filesystem helpers, brain seeding, four new
   methods, file editor in H02.
3. Existing claws (created under s1) get a one-shot migration:
   their directories are created and seeded the next time the
   runtime starts. Spec'd in tasks.md.
4. s2 archives → `openspec/specs/claw-soul-rules-files/spec.md`
   created; `claw-workspace-metadata` updated with extension.

## Open Questions

1. **`soul.md` system-prompt composition.**
   Does brain replace the default system prompt entirely with
   `soul.md`, or prepend `soul.md` and append the existing default?
   The spec says "use as the system prompt seed" which is
   intentionally loose. Decide at impl time; the answer affects how
   loud claw identity feels vs. how much the runtime's general
   instructions still hold.

2. **Is the runtime brain's existing `system_prompt` extensible
   per-claw, or does this require a new code path?**
   `crates/kelvin-brain/src/system_prompt.rs` exists. Likely a
   small extension reads claw's soul.md and merges. Verify at impl.

3. **Should H02's editor support markdown rendering?**
   The user types markdown and expects to see it rendered. Probably
   yes — H02 already has markdown rendering in chat. Re-use that
   component.

4. **What's the right H02 affordance for "edit RULES.md when it
   isn't enforced yet"?**
   Could be: hide the editor entirely until s6 lands; or show with
   a "v0.6" notice. I'd lean show-with-notice so users can start
   drafting their rules in advance.

5. **Plugin / sidecar repo (`agentichighway/kelvinclaw-plugins`)
   touch.**
   None in this slice. s6 will introduce a new ModelProvider profile
   that consumes `RULES.md`; that may ship as a new plugin in the
   distribution repo. No s2 dependency on that work.
