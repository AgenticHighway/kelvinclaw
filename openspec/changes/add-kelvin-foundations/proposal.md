## Why

Kelvin is a personal AI agent platform built around a recursive "Claw"
primitive: one orchestrator (the macro claw) delegating to specialised
sub-claws for each domain of the user's life. The architecture has
been discussed in depth and now needs a concrete spec for the
foundational layer — the data shapes, the recursive primitive, the
clean separation between four agent-facing concepts (Powers,
Connectors, MCP servers, Sub-agents), and the way work flows through
the call-tree.

Without these foundations specified first, neither the security layer
(`add-kelvin-security`) nor the UI/runtime layer
(`add-kelvin-ui-runtime`) has anything to build on. This change is
strictly behavioural — it specifies what the system MUST do, not how
to implement it.

## What Changes

- **NEW** capability `claw-anatomy` — the recursive Claw primitive,
  its privileged anatomy, parent-caps-child subset invariants for
  bound Connectors and MCP servers, file-backed `soul.md` and
  `RULES.md`, and the Drafts→Sources promotion as the only outbound
  write edge from the privileged box.
- **NEW** capability `concepts-taxonomy` — the four-way distinction
  between Powers, Connectors, MCP servers, and Sub-agents (templates
  vs runtime instances). Powers stand alone (no `agentType`
  coupling). Sub-agents are runtime-only.
- **NEW** capability `data-model` — TypeScript schemas for every
  persistent entity (Claw, Source, Draft, Power, Connector, MCPServer,
  SubAgentTemplate, Receipt, Trigger, Channel) plus ten cross-cutting
  validation invariants (cap chain, subset bindings, Power.requires,
  Receipt immutability, etc.). Every entity carries forward-compat
  `ownerId` / `createdBy` fields for v2 multi-user.
- **NEW** capability `delegation-call-tree` — three call-tree node
  kinds (Power invocation, Sub-agent spawn, Sub-claw delegation),
  posture inheritance, allowlist enforcement, depth caps, cycle
  prevention, cross-claw porosity, and parent-arbitrates conflict
  resolution.

ADRs underlying these capabilities (sub-agents-runtime-only,
four-distinct-concepts, recursive-claw, drafts-promotion-edge) are
inlined into `design.md`. ADR-004 (single-user v1) is referenced
here as the scope decision that justifies forward-compat fields
without enforcement.

## Capabilities

### New Capabilities

- `claw-anatomy`: Recursive Claw primitive — anatomy, recursion guarantee, parent-caps-child subset invariants, file-backed soul/rules, Drafts→Sources outbound edge.
- `concepts-taxonomy`: First-class distinction between Powers, Connectors, MCP servers, and Sub-agents (templates vs runtime instances), with explicit layering direction (Sub-agents → Powers → Connector ops / MCP tools / built-in tools).
- `data-model`: Persistent and runtime TypeScript schemas plus ten cross-cutting validation invariants (cap chain, subset bindings, Power.requires resolution, Receipt immutability, ownership fields).
- `delegation-call-tree`: Three call-tree node kinds, posture inheritance into spawned Sub-agents, allowlist enforcement, depth and cycle caps, cross-claw porosity, parent-arbitrates conflict resolution.

### Modified Capabilities

None. `openspec/specs/` is empty; this change is foundational.

## Impact

- **Code (kelvinclaw)**: New schema definitions, store-side validation
  (cap chain, subset bindings, `Power.requires` resolution),
  append-only Receipt store with `parentReceiptId` index, Sub-agent
  runtime registry that doesn't persist instances. No deletions.
- **Code (H02)**: Major migration documented as a separate work
  stream in tasks.md — `useGingerStore` rename to `useClawStore`,
  Space→Claw renames, removal of `Power.agentType`, expansion of
  `SOURCE_TYPES`, consolidation of `AGENT_CATEGORIES` +
  `POWER_CATEGORIES`, conversion of `SubAgent` configs into
  `SubAgentTemplate`s.
- **APIs**: None at this layer; gateway / tool-gate APIs land in the
  other two changes.
- **Dependencies**: None new. This change is schema- and
  invariant-only.
- **Documentation**: Replaces the prose-heavy `docs/kelvin-spec/`
  folder (deleted in this change-set's parent commit) with
  OpenSpec-canonical content.
