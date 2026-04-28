## Why

Once the Kelvin foundations exist (recursive Claw, four-concept
taxonomy, schemas, call-tree), the next layer is **security and
autonomy** — how the agent decides what to ask the user about, what
to do automatically, and how policy is enforced at the model and
tool boundaries.

The user wants three named autonomy postures (Low / Medium / High)
that map onto a finer-grained twelve-axis matrix. Across the model
boundary, per-claw `RULES.md` files are enforced by Open Bias
(`https://github.com/open-bias/open-bias`). Across the tool boundary,
the kelvinclaw `ToolRegistry` enforces posture per call. When either
sidecar is unreachable, autonomy floors to Low (fail-closed).

This change layers the security model on top of `add-kelvin-foundations`
without modifying it.

## What Changes

- **NEW** capability `autonomy-postures` — twelve-axis posture matrix
  (toolExecution, subClawDelegation, subAgentSpawn, sourceReads,
  connectorWrites, draftPromotion, pluginInstall, memoryWrites,
  wasmEgress, routinesUserAbsent, crossClawPorosity, powerModelSpend),
  three named shortcut postures, parent-caps-child invariant,
  user cap above macro claw, sidecar-down-floors-to-Low,
  per-action overrides with scope, Routines fire at claw posture
  not session, posture-change Receipts, WASM preset selection.
- **NEW** capability `approvals-primitive` — extension of H02's
  existing `Question` / `QuestionCarousel` for autonomy approvals.
  `kind` discriminator (`'clarification' | 'approval'`),
  approval-only fields (`actionDescriptor`, `defaultChoiceId`,
  `scopeOptions`, `riskLevel`, `expiresAt`), per-`ActionDescriptor.kind`
  renderers, scope picker (once / session / claw / forever), forever
  confirmation, termination control, expiry auto-deny, audit trail
  with posture-change Receipts on `claw` and `forever` scopes,
  carousel ordering, sidecar-down banner.
- **NEW** capability `security-sidecars` — two distinct sidecars
  (Open Bias on the model boundary, kelvinclaw `ToolRegistry` on
  the tool boundary). Per-claw `RULES.md` selection via
  `X-Kelvin-Claw-Rules-Ref` header. Posture context header
  `X-Kelvin-Claw-Posture`. Fail-closed when Open Bias unreachable
  (no fall-through to upstream provider). `fail_closed = true`
  required at startup. Tool-gate strictest-axis-wins. WASM sandbox
  preset selection per `wasmEgress`. Version pinning. OpenTelemetry
  trace correlation. Localhost-only Open Bias in v1. Sidecar-health
  events on the gateway. RULES.md path conventions.

ADRs underlying these capabilities (two-sidecars, soul-rules-files +
Question reuse, three-postures-cap-invariant) are inlined into
`design.md`.

## Capabilities

### New Capabilities

- `autonomy-postures`: Twelve-axis posture matrix with three named postures, parent-caps-child invariant, sidecar-down floor, per-action overrides with scope, WASM preset selection.
- `approvals-primitive`: UI primitive for gated actions, extending H02's `Question` / `QuestionCarousel` with kind discriminator, scope picker, expiry, audit trail.
- `security-sidecars`: Two boundary defence — Open Bias on the model boundary (per-claw `RULES.md`) and kelvinclaw `ToolRegistry` on the tool boundary, with fail-closed semantics and identity-bound header selection.

### Modified Capabilities

None directly — this change layers on `add-kelvin-foundations` but
does not modify its requirements. The `data-model` capability from
foundations defines `PosturePerAxis`, `PostureOverride`, and the
extended `Question` schema; this change specifies the *behaviour*
those structures support.

## Impact

- **Code (kelvinclaw)**: New `OpenBiasShimProvider` wrapping the
  upstream `ModelProvider`. New `PostureService` with cap-chain
  cache. Extension of `ToolRegistry` with autonomy enforcement and
  axis-mapping. WASM sandbox preset selection wired to
  `wasmEgress` axis.
- **Code (H02)**: New `PostureMatrix.tsx`, `ApprovalCard.tsx`,
  `SidecarPanel.tsx` components. Extension of `Question` consumers
  to handle `kind === 'approval'`. Sidecar-health banner.
- **Dependencies**: Open Bias as a peer-deployed sidecar
  (`https://github.com/open-bias/open-bias`) at
  `http://localhost:4000/v1`.
- **Operations**: `docker-compose.yml` pinning Open Bias to a
  specific image tag with `KELVIN_DATA_DIR` mounted; permissive
  `RULES.md` template for development.
- **Documentation**: This change is the OpenSpec-canonical home for
  every behavioural requirement of the security layer.
