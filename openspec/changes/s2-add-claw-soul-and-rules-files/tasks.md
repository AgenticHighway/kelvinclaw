## 1. kelvinclaw — Filesystem helpers

- [ ] 1.1 Add a helper module in `crates/kelvin-core/src/` (e.g., `claw_files.rs`) exposing `claw_dir(claw_id)`, `read_soul(claw_id)`, `write_soul(claw_id, content)`, `read_rules(claw_id)`, `write_rules(claw_id, content)`, `seed_claw_files(claw_id, name)`, `remove_claw_dir(claw_id)`.
- [ ] 1.2 Define default `soul.md` and `RULES.md` templates as Rust string constants.
- [ ] 1.3 Implement atomic semantics: `claw.create` rolls back persisted record creation if directory seeding fails.

## 2. kelvinclaw — Claw CRUD integration

- [ ] 2.1 Update `claw.create` handler to call `seed_claw_files` after persistence; return error if seeding fails.
- [ ] 2.2 Update `claw.delete` handler to call `remove_claw_dir` after persistence removal.
- [ ] 2.3 On runtime start, walk persisted claws and ensure every one has a directory + the two files; create+seed any missing pieces (handles s1→s2 upgrade for existing claws).

## 3. kelvinclaw — Brain integration

- [ ] 3.1 Update `crates/kelvin-brain/src/system_prompt.rs` (or equivalent) to look up the active claw's `soul.md` content and use it as the system-prompt seed for the model call.
- [ ] 3.2 Implement fallback: if `soul.md` cannot be read, use the existing default system prompt and emit an `event` with kind `warning` and detail `soul-md-unreadable`.
- [ ] 3.3 Decide composition strategy (replace vs prepend vs append) — see design.md Open Question 1; document the choice in code.

## 4. kelvinclaw — New gateway methods

- [ ] 4.1 Register `claw.soul.read` method; reads from filesystem helper.
- [ ] 4.2 Register `claw.soul.write` method; writes via filesystem helper.
- [ ] 4.3 Register `claw.rules.read` method.
- [ ] 4.4 Register `claw.rules.write` method.
- [ ] 4.5 Update `gateway-protocol.md` with the four new methods.

## 5. H02 — Claw settings UI

- [ ] 5.1 Add a "Claw Settings" overlay (re-using `src/components/features/screens/` patterns) accessible from the claw switcher.
- [ ] 5.2 Inside the overlay, add a `soul.md` editor: textarea + markdown preview (re-using existing markdown renderer if present).
- [ ] 5.3 Inside the overlay, add a `RULES.md` editor with a "v0.6: not yet enforced" notice surfacing the v6 slice scope.
- [ ] 5.4 Wire editors to `claw.soul.read|write` and `claw.rules.read|write`.

## 6. Verification

- [ ] 6.1 Create a new claw in H02; verify the directory `<DATA_DIR>/claws/<id>/` exists with both files seeded.
- [ ] 6.2 Edit the claw's `soul.md` to "You speak only in haiku"; send a chat message; verify the response is haiku-shaped (model adherence permitting).
- [ ] 6.3 Delete the claw; verify the directory is removed.
- [ ] 6.4 Restart kelvinclaw; verify edited `soul.md` content survives.
- [ ] 6.5 Manually corrupt a claw's `soul.md` permissions (chmod 000); send a chat message; verify the warning event fires AND the chat still completes (fallback path).
- [ ] 6.6 Edit `RULES.md` to contain a rule; send a chat message that would violate it; verify the assistant ignores the rule (s2 does not enforce).

## 7. Archive

- [ ] 7.1 Once tasks above are green and a 5-minute "edit soul, claw tone changes" demo is recorded, run `openspec archive s2-add-claw-soul-and-rules-files`.
