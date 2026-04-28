## Why

After s0/s1/s2, claws exist with identity but the agent acts
freely — every tool the model wants to call goes through unchecked.
The user's design centres on autonomy postures (Low / Medium / High)
that gate tool execution per claw. s3 introduces the *first axis*
of the matrix — `toolExecution` — and the basic approval UI built on
H02's existing `Question` / `QuestionCarousel` primitive.

This is the smallest end-to-end "the security model works" demo.
Other axes (sub-claw delegation, connector writes, drafts promotion,
etc.) accrete in later slices. Open Bias is NOT involved (s6).
WASM-egress preset selection is NOT involved (later). The point of
s3 is to prove the gate-and-approve pattern works on one axis.

## What Changes

- **NEW** capability `autonomy-posture-tool-execution` — per-claw
  posture record `{ toolExecution: 'low' | 'medium' | 'high' }`.
  ToolRegistry consults the calling claw's `toolExecution` posture
  before dispatching any tool call. Low = ask before every call.
  Medium = ask only for tools marked `isWrite` or `isExternal`.
  High = auto-allow.
- **NEW** capability `approvals-primitive-basic` — H02's existing
  `Question` type extended with `kind: 'clarification' | 'approval'`
  discriminator. Approval-kind questions carry `actionDescriptor`
  (the tool call being requested) and a single decision option:
  Allow once / Deny. Scope expansion (session / claw / forever)
  comes in a later slice.
- **MODIFIED** capability `claw-workspace-metadata` — `Claw` record
  gains a `posture` field (only `toolExecution` populated for now;
  matrix grows in later slices). `claw.create` accepts an optional
  `posture` param; `claw.update` accepts posture patches.

## Capabilities

### New Capabilities

- `autonomy-posture-tool-execution`: First axis of the autonomy posture matrix. ToolRegistry-side gating: low/medium/high mapping to ask-always / ask-on-write / auto. Per-claw posture stored on Claw record.
- `approvals-primitive-basic`: H02 `Question` extended with `kind` discriminator and approval-kind action descriptor. Allow-once / Deny only in this slice; remember-this scopes deferred.

### Modified Capabilities

- `claw-workspace-metadata`: Claw record gains `posture` field; `claw.create` and `claw.update` accept posture mutations.

## Impact

- **Code (kelvinclaw)**: Posture struct in `crates/kelvin-core/`.
  ToolRegistry extension in `crates/kelvin-core/src/tools.rs` (or
  wrapping wrapper) to call into a posture-evaluation function
  before dispatch. New gateway events for approval requests
  (`approval.requested`) and approval resolutions
  (`approval.resolved`) flowing through the existing event channel.
- **Code (H02)**: New `ApprovalCard.tsx` rendered inside the
  existing `QuestionCarousel`. New gateway method
  `approval.respond` for sending the user's decision back. Wire
  the carousel to render kind=approval cards alongside existing
  kind=clarification cards.
- **Code (kelvinclaw-plugins)**: Zero. Tools shipped in plugins
  declare their `isWrite` / `isExternal` flags via existing tool
  metadata; this slice consumes those flags but doesn't change the
  ABI.
- **APIs**: Two new gateway events (`approval.requested`,
  `approval.resolved`), one new method (`approval.respond`).
  Posture additions to claw CRUD.
- **Documentation**: README + gateway-protocol.md updates.
