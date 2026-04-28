## 1. kelvinclaw — Claw store

- [ ] 1.1 Define `Claw` struct in `crates/kelvin-core/src/` with fields `id`, `name`, `parent_claw_id`, `icon_ref`, `description`, `created_at`, `updated_at`.
- [ ] 1.2 Implement persistent storage for Claw records (use the existing data-dir conventions; sled / sqlite / json — consistent with existing workspace persistence).
- [ ] 1.3 Implement validation: exactly one record with `parent_claw_id == None`; reject second create / delete-of-only-macro.
- [ ] 1.4 Implement validation: `parent_claw_id` references an existing claw.
- [ ] 1.5 Implement validation: cannot delete a claw with children; return `claw-has-children` with child ids.
- [ ] 1.6 On runtime first-start with empty store, create default macro claw `name: "Kelvin"`, `parent_claw_id: None`.

## 2. kelvinclaw — Gateway methods

- [ ] 2.1 Register `claw.list` method in `apps/kelvin-gateway/`; returns all persisted claws.
- [ ] 2.2 Register `claw.create` method; params `{ name, parent_claw_id, icon_ref?, description? }`; returns created record.
- [ ] 2.3 Register `claw.update` method; params `{ claw_id, patch }`; returns updated record.
- [ ] 2.4 Register `claw.delete` method; params `{ claw_id }`; honours the cannot-delete invariants.
- [ ] 2.5 Extend `agent` method's params to accept optional `claw_id`; route the resulting session to the named claw or to the macro claw if absent.
- [ ] 2.6 Update `gateway-protocol.md` with the four new methods + `agent` param addition.

## 3. H02 — Wire live claws

- [ ] 3.1 Add a `claws` slice to the Zustand store: `claws: Claw[]`, `activeClawId: string | null`, actions `loadClaws()`, `createClaw()`, `updateClaw()`, `deleteClaw()`, `setActiveClaw()`.
- [ ] 3.2 On gateway-connect, call `claw.list` and populate the store.
- [ ] 3.3 Update the frame navigation in `src/components/features/frame/` to render claws from the store, not from the existing mock `spaces` data.
- [ ] 3.4 Plumb `activeClawId` into the chat composer's submit so each `agent` request includes `claw_id`.
- [ ] 3.5 Add a "create claw" affordance in the frame nav (modal or inline) that calls `createClaw()`.
- [ ] 3.6 Add a "delete claw" affordance with confirmation; surfaces the gateway's `claw-has-children` error if applicable.

## 4. Verification

- [ ] 4.1 Start kelvinclaw with a fresh data dir; verify the macro claw is auto-created and `claw.list` returns exactly one record named "Kelvin".
- [ ] 4.2 Create a sub-claw "Personal" via the H02 UI; verify it appears in the frame nav and `claw.list`.
- [ ] 4.3 Switch to "Personal", send a chat message; verify the gateway sees `claw_id` matching Personal in the `agent` request.
- [ ] 4.4 Try to delete the macro claw via H02 UI; verify the request is rejected with `macro-claw-invariant`.
- [ ] 4.5 Try to delete "Personal" while it has a child "Personal/Health"; verify rejection with `claw-has-children`.
- [ ] 4.6 Restart kelvinclaw; verify all created claws survive (persistence works).

## 5. Archive

- [ ] 5.1 Once tasks above are green and a 5-minute "switch claws and chat" demo is recorded, run `openspec archive s1-add-claw-workspace-metadata`. Capabilities lift into `openspec/specs/`; s2 builds on this baseline.
