## Why

The Kelvin GUI architecture has been specified across 23 markdown documents
under `docs/kelvin-spec/`. Those docs explain the design in prose and ADRs,
but the **behavioural assumptions** they make are not separately testable.
This change converts the spec set into OpenSpec capabilities — one
`spec.md` per capability with `## ADDED Requirements` and `WHEN/THEN`
scenarios — so each invariant becomes a concrete, archive-tracked test
case.

The exercise also surfaces assumptions that the prose docs implicitly
take for granted (single-user filesystem auth as the v1 trust boundary,
localhost-only Open Bias, sufficient expressiveness of `RULES.md`,
unbounded recursion depth, fail-closed UX acceptability, etc.). These
are documented in `design.md` under **Open Questions** and **Risks /
Trade-offs** so they are visible at review time rather than buried in
prose.

## What Changes

- **NEW** capability `claw-anatomy` — the recursive Claw primitive, its
  privileged anatomy, and the parent-caps-child invariant.
- **NEW** capability `concepts-taxonomy` — the four-way distinction
  between Powers, Connectors, MCP servers, and Sub-agents.
- **NEW** capability `delegation-call-tree` — three node kinds (Power
  invocation, Sub-agent spawn, Sub-claw delegation) and arbitration.
- **NEW** capability `composer-modes` — Auto / Plan / Ask / Learn /
  Play / Make as composer-level intent picker, orthogonal to autonomy.
- **NEW** capability `autonomy-postures` — twelve-axis posture matrix
  with named Low / Medium / High shortcuts, parent-caps-child enforcement,
  and per-action override scopes.
- **NEW** capability `approvals-primitive` — extension of H02's existing
  `Question` / `QuestionCarousel` for autonomy approvals (kind
  discriminator, scope picker, expiry).
- **NEW** capability `security-sidecars` — two-boundary defence (Open
  Bias on the model boundary, kelvinclaw `ToolRegistry` on the tool
  boundary), per-claw `RULES.md`, fail-closed semantics, sidecar-down
  floor.
- **NEW** capability `mind-observability` — tabs (session, tasks,
  drafts, plans, diffs, PRs, browser, receipts, costs, notifications),
  call-tree view, filter chain.
- **NEW** capability `data-model` — TypeScript schemas (Claw, Source,
  Draft, Power, Connector, MCPServer, SubAgentTemplate, Receipt,
  Trigger, Channel, Question, PosturePerAxis) and ten cross-cutting
  validation invariants.
- **NEW** capability `gateway-protocol` — H02 ↔ kelvinclaw WebSocket
  protocol: submit / wait / event-stream messages, RPC method catalogue,
  reconnect-with-resume.

These capabilities together specify the v1 architecture documented in
`docs/kelvin-spec/`. None of them MODIFY existing OpenSpec capabilities
because `openspec/specs/` is empty at this point — every capability is
ADDED.

The corresponding kelvin-spec ADRs (1–8) are preserved as design context
in `design.md` rather than re-rendered as separate capabilities; an ADR
records *why* a decision was taken, while an OpenSpec requirement
records *what the system must do as a result*.

## Capabilities

### New Capabilities

- `claw-anatomy`: Recursive Claw primitive — anatomy, recursion guarantee, parent-caps-child subset invariants for bound Connectors and MCP servers.
- `concepts-taxonomy`: First-class distinction between Powers, Connectors, MCP servers, and Sub-agents (templates vs runtime instances).
- `delegation-call-tree`: Three call-tree node kinds (Power invocation, Sub-agent spawn, Sub-claw delegation), their security profiles, sub-session semantics, and arbitration.
- `composer-modes`: Six modes (Auto, Plan, Ask, Learn, Play, Make) as the composer-level intent picker, orthogonal to autonomy.
- `autonomy-postures`: Twelve-axis posture matrix with three named postures (Low, Medium, High), parent-caps-child invariant, sidecar-down floor, per-action overrides with scope.
- `approvals-primitive`: UI primitive for gated actions, built as an extension of H02's `Question` / `QuestionCarousel` (kind discriminator, scope picker, expiry, audit trail).
- `security-sidecars`: Two security sidecars — Open Bias on the model boundary (per-claw `RULES.md`) and kelvinclaw `ToolRegistry` on the tool boundary (autonomy enforcement, WASM sandbox presets); fail-closed semantics.
- `mind-observability`: Mind UI surface — tabs, call-tree view, filter chain, Receipts as immutable audit log.
- `data-model`: Persistent and runtime TypeScript schemas plus cross-cutting validation invariants.
- `gateway-protocol`: H02 ↔ kelvinclaw WebSocket gateway protocol — submit / wait / event-stream messages, method catalogue, reconnect semantics.

### Modified Capabilities

None. `openspec/specs/` is empty at this point; every capability is new.

## Impact

- **Code (kelvinclaw)**: New `ModelProvider` shim implementation under
  `crates/kelvin-providers/`; `ToolRegistry` extension for autonomy
  enforcement under `crates/kelvin-core/`; gateway message types under
  `crates/kelvin-gateway/`. No deletions; adds.
- **Code (H02)**: Substantial migration documented in
  `docs/kelvin-spec/10-h02-migration.md` — `useGingerStore` rename,
  type-system changes, component updates. Tracked separately as an H02
  branch.
- **Code (kelvinclaw-plugins)**: No code changes; the
  `trusted_publishers.kelvin.json` and `index.json` manifests are read
  by the new tool gate.
- **Dependencies**: New runtime dependency on Open Bias
  (`https://github.com/open-bias/open-bias`) running at
  `http://localhost:4000/v1`. Pinned via `docker-compose.yml`.
- **APIs**: New WebSocket gateway methods (see `gateway-protocol`
  capability). Existing gateway protocol unchanged; this layers on top.
- **Operations**: Two sidecars to deploy and monitor (Open Bias +
  `ToolRegistry` health). Single "Sidecars: healthy / degraded / down"
  indicator in the GUI.
- **Documentation**: All 23 docs under `docs/kelvin-spec/` are the prose
  source of truth; this OpenSpec change makes the testable invariants
  machine-readable.
