## Why

After s0, H02 talks to kelvinclaw but every chat goes to the same
flat session. The user's design centres on a recursive Claw
primitive — a macro claw (Kelvin) plus domain sub-claws (Personal,
Work, Health, …) — each with its own identity. s1 introduces the
Claw concept *as workspace metadata* on top of the existing
`SessionDescriptor` (`session_id`, `session_key`, `workspace_dir` in
`crates/kelvin-core/src/sessions.rs`).

This slice does NOT yet introduce per-claw `soul.md` / `RULES.md`
files (s2), posture (s3), delegation (s5), or any sidecar (s6). It
just establishes the data shape and the user-visible "switch claws"
affordance.

## What Changes

- **NEW** capability `claw-workspace-metadata` — Claw is a persisted
  record extending the workspace concept with metadata: `id`, `name`,
  `parentClawId` (nullable; null only for the macro claw), `iconRef`,
  `description`, `createdAt`, `updatedAt`. Exactly one Claw per install
  has `parentClawId === null`. Sessions carry `claw_id` so messages
  route to a specific claw.
- **MODIFIED** capability `h02-gateway-connection` — the chat
  composer's `agent` submit gains a `claw_id` parameter, and the H02
  Zustand store loads its claw list from the gateway via new
  `claw.list` / `claw.create` / `claw.update` / `claw.delete`
  methods.

## Capabilities

### New Capabilities

- `claw-workspace-metadata`: Persisted Claw records as workspace metadata. Recursive `parentClawId` structure with exactly one macro claw. Gateway methods `claw.list`, `claw.create`, `claw.update`, `claw.delete`.

### Modified Capabilities

- `h02-gateway-connection`: extends s0's `agent` submit to include `claw_id`; adds claw-list rendering driven by gateway, not mock data.

## Impact

- **Code (kelvinclaw)**: New `Claw` struct in `crates/kelvin-core/src/`. Persistence in the existing workspace store. Four new gateway methods registered in `apps/kelvin-gateway/`.
- **Code (H02)**: New `claws` slice in the Zustand store (replaces or augments the existing `spaces` mock data). Claw switcher in the frame nav (existing `src/components/features/frame/`). Wire up `claw.list` etc.
- **Code (kelvinclaw-plugins)**: Zero.
- **APIs**: Adds 4 new methods to the gateway protocol. `agent` gains optional `claw_id`.
- **Documentation**: README addendum for the new methods; `gateway-protocol.md` section additions.
