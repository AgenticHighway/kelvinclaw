## Context

After s2, claws have identity (`soul.md`) but no enforcement layer.
The kelvinclaw `ToolRegistry` (in `crates/kelvin-core/src/tools.rs`)
already has the `Tool` trait with `name`, `description`,
`input_schema`, and async `call` methods, plus `ToolCallInput {
run_id, session_id, workspace_dir, arguments }`. What's missing is
a *posture-aware gate* that sits between the brain's tool-call
decision and the actual `Tool::call` invocation.

H02 already has a `Question` / `QuestionCarousel` UI primitive used
for asking the user clarifications. Extending it with a `kind`
discriminator and approval-specific fields is non-disruptive and
re-uses the carousel layout, urgency, attachment, and trace
machinery.

## Goals / Non-Goals

**Goals:**
- Land the gate-and-approve loop end-to-end on ONE axis
  (`toolExecution`).
- Reuse `Question` / `QuestionCarousel` rather than building a new
  approval surface.
- Establish the event shape (`approval.requested` / `approval.resolved`)
  and method (`approval.respond`) so future axes can extend the
  same plumbing.

**Non-Goals:**
- No remember-this scopes (`session` / `claw` / `forever`). Only
  Allow-once and Deny in this slice.
- No other axes (sub-claw delegation, connector writes, etc.).
- No PostureOverride records, because there's nothing to remember
  yet.
- No cross-claw posture cap chain (only relevant when sub-claws
  with their own posture exist — s5+).
- No Open Bias (s6).

## Decisions

### D1. ToolRegistry posture gate sits ABOVE the existing dispatch

The cleanest seam is a wrapper: the existing `ToolRegistry` keeps
its `dispatch(tool, input)` semantics; a new posture gate intercepts
at the brain's call-into-registry boundary and either dispatches,
suspends, or denies.

**Alternative:** modify each `Tool` implementation to consult the
gate.
**Rejected because:** that pushes policy into every tool author's
code. Wrapping at the registry boundary keeps tools posture-agnostic.

### D2. tools declare isWrite / isExternal flags themselves

The posture gate at Medium needs to distinguish "internal read" from
"external write" classes of tool. The most reliable place to know
that is the tool itself.

**Alternative:** maintain a side-table mapping tool name to flags.
**Rejected because:** drifts when tools change behaviour; new
plugins ship without entries; a tool author who wants to mark
their own tool as low-risk-write can't.

This means existing tool plugins (websearch, wiki, fs ops, etc.)
need to be audited and tagged. That's a one-time cost listed in
tasks.md.

### D3. Approval flows through the existing event channel

The gateway already has `req`/`res`/`event` envelope. Approval
requests are events; responses come via a new `req` method. This
matches how `agent` flows already work and keeps wire-format
discipline.

**Alternative:** a new dedicated approval channel.
**Rejected because:** adds a parallel transport for no semantic
gain.

### D4. Reuse Question / QuestionCarousel

The user's design and H02's existing primitive align almost
perfectly. The kind discriminator is a non-breaking extension.

**Alternative:** new `Approval` type and `ApprovalCarousel`
component.
**Rejected because:** duplicates carousel UI work; loses
benefits like cross-question urgency sorting.

### D5. Default posture is Medium, not Low

Low for every claw on first creation would surprise users with
constant approvals. Medium is the "ask only when it matters"
default that's pragmatic. Power users can lower to Low per claw.

### D6. Approval timeout = 5 minutes

If the user steps away mid-conversation, we don't want suspended
tool calls clogging memory forever. 5 minutes balances "user just
got distracted" vs "user closed the laptop." Configurable via
runtime config.

## Risks / Trade-offs

[Risk: existing tool plugins ship without `isWrite` / `isExternal`
flags] → mitigation: tasks.md includes the audit + tagging step.
Default for unknown tools is `isWrite: true, isExternal: true`
(treat as risky). New plugins must declare flags explicitly.

[Risk: Allow-once is the only scope, so users will see lots of
approvals and develop fatigue] → mitigation: this is a known
limitation of s3. Scope expansion (`session` / `claw` / `forever`)
is in a near-term slice. Document in the s3 release notes that
"remember this" is coming.

[Risk: approval timeout could fire during legitimate user think
time] → mitigation: timeout is configurable; 5 min is a
conservative default; future slice may add "expiresAt indicator"
on the card so users see the deadline ticking.

[Risk: H02 Question carousel may not handle cards with very
different sizes (clarification cards are short, approval cards
with arg JSON can be tall)] → mitigation: existing carousel
already handles variable heights via the trace items. UX
verification in tasks.md.

[Risk: brain's tool-call retry logic could re-issue the same call
on deny, creating loops] → mitigation: `denied-by-approval` is a
distinct error class the brain treats as a hard stop, not a
retry-eligible failure. Tested in tasks.md.

## Migration Plan

1. s2 archives → soul/rules files exist per claw; CRUD methods
   accept posture-less Claw records.
2. s3 implements: posture struct, ToolRegistry wrapper, tool flag
   audit, gateway events + method, H02 ApprovalCard.
3. Existing claws (created under s1/s2) get auto-defaulted to
   `posture: { toolExecution: 'medium' }` on first runtime start
   under s3.
4. s3 archives → autonomy-posture-tool-execution and
   approvals-primitive-basic capabilities exist in
   `openspec/specs/`. Future axis slices ADD to
   autonomy-posture-tool-execution OR create new
   `autonomy-posture-<axis>` capabilities (open question).

## Open Questions

1. **One capability per axis, or one umbrella `autonomy-postures`
   capability?** I lean per-axis: each new axis gets its own
   capability with its own `## ADDED Requirements` section. Easier
   to review per slice; harder to see the matrix at a glance.
   Alternative: single capability `autonomy-postures` that grows
   via `MODIFIED` over slices. Less idiomatic OpenSpec but more
   matrix-shaped. Decide before s5.

2. **Approval timeout default — 5 min or shorter/longer?**
   No data yet. 5 min is a guess. Revisit after early usage.

3. **Should `auto-deny` ever fire in s3, or is everything either
   `auto-allow` or `ask`?** In s3, sidecars don't exist so
   sidecar-down floors don't apply. There's no current path to
   `auto-deny`. The trichotomy is in the spec for forward-compat.

4. **What's the `risk pill` colour for `isWrite: true, isExternal:
   false`?** isExternal=true is "external" (red). isWrite=true and
   not external is "write" (yellow). Both false is "read" (green
   or neutral). H02's existing color tokens decide.

5. **Plugin `isWrite` / `isExternal` declaration ABI extension.**
   Existing tools in `agentichighway/kelvinclaw-plugins` need new
   metadata fields. The plugin index schema (`v1`) may need a
   `v1.1` bump or backward-compat default. Decide ABI strategy
   with the plugins repo maintainers; could be in this slice or
   the next.
