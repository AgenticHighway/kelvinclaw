## Why

After s1, claws are persisted records but they don't yet have any
*identity* of their own — every claw uses the runtime's default
system prompt, regardless of name. The kelvin GUI design centres on
each claw having a per-claw `soul.md` (identity / charter / style)
and a `RULES.md` (enforceable policy, intended for future
Open-Bias-style enforcement in s6).

s2 introduces those files. The brain seeds the system prompt from
the claw's `soul.md` when running an `agent` turn for that claw.
`RULES.md` is created as a stub but is NOT yet enforced — it's the
file that s6 will hand off to a model-boundary sidecar when one is
introduced. Creating it now means the door is explicitly open and
forward-compatible without locking us into any specific sidecar.

## What Changes

- **NEW** capability `claw-soul-rules-files` — per-claw filesystem
  layout `<KELVIN_DATA_DIR>/claws/<claw_id>/{soul.md,RULES.md}`. Both
  files are created with sensible defaults on `claw.create`. New
  gateway methods `claw.soul.read`, `claw.soul.write`,
  `claw.rules.read`, `claw.rules.write` for editing. Brain reads
  `soul.md` and prepends/seeds it as the claw's system prompt for
  every `agent` turn associated with that claw.
- **MODIFIED** capability `claw-workspace-metadata` — `claw.create`
  now also creates the directory and seeds the two files; `claw.delete`
  cleans up the directory.

## Capabilities

### New Capabilities

- `claw-soul-rules-files`: Per-claw `soul.md` and `RULES.md` files with read/write gateway methods. Brain seeds claw-specific system prompt from `soul.md`. `RULES.md` exists but is not yet enforced (door opened for s6).

### Modified Capabilities

- `claw-workspace-metadata`: `claw.create` and `claw.delete` extend to manage the per-claw directory; new claws receive a default `soul.md` and stub `RULES.md`.

## Impact

- **Code (kelvinclaw)**: Filesystem helpers in `crates/kelvin-core/src/`
  for the `<DATA_DIR>/claws/<id>/` layout. Brain change in
  `crates/kelvin-brain/src/` to look up the active claw's `soul.md`
  and seed it into `system_prompt` for the `agent` turn. Four new
  gateway methods.
- **Code (H02)**: New "Settings" UI surface per claw — soul.md and
  RULES.md editors. Existing `src/components/features/screens/`
  pattern accommodates a new claw-settings overlay.
- **Code (kelvinclaw-plugins)**: Zero.
- **APIs**: Four new gateway methods (`claw.soul.read|write`,
  `claw.rules.read|write`) added to the existing protocol.
- **Documentation**: README addendum + gateway-protocol.md update.
- **Forward-compat note**: `RULES.md` exists but is unused in s2.
  s6 will introduce a ModelProvider profile (Open Bias passthrough)
  that consumes the file via header injection. No code change to
  `RULES.md` semantics is needed at s2.
