---
status: Draft
version: v1
audience: architects, implementors
date: 2026-04-27
---

# Delegation and the Call-Tree

When work flows through Kelvin, it produces a **call-tree** with three
distinct kinds of nodes. This doc specifies the three kinds, their security
profiles, sub-session semantics, and arbitration rules.

The Mind UI renders these trees ([08-mind.md](08-mind.md)). Receipts capture
each node ([09-data-model.md](09-data-model.md), `Receipt` type).

## Three node kinds

| Kind | What it is | Persistence | Identity | Children |
|---|---|---|---|---|
| **Power invocation** | A claw calls one of its Powers | Receipt only | Power id | Tool calls, Connector ops, MCP ops |
| **Sub-agent spawn** | A claw spawns a transient specialist instance for a sub-session | Receipt only (instance is runtime-only — [ADR-001](decisions/001-sub-agents-runtime-only.md)) | role + sub-session id | Power invocations within the sub-session |
| **Sub-claw delegation** | A claw hands off a request to a peer claw (typically a child claw) | Receipt only | target claw id | The sub-claw's own call-tree |

A typical call-tree mixes all three:

```
[user message → macro claw]
└─ [Sub-claw delegation: Health-claw]                        ← kind 3
   ├─ [Power invocation: web_search]                         ← kind 1
   │  └─ [MCP op: web_search.search(q=…)]
   ├─ [Sub-agent spawn: Researcher (60k tok / 10min budget)] ← kind 2
   │  ├─ [Power invocation: web_search × 4]
   │  ├─ [Power invocation: summarize]
   │  └─ [Power invocation: cite]
   └─ [Power invocation: send_email]
      └─ [Connector op: gmail.send(to=...)]
```

## Power invocation

The simplest node. The current claw calls one of its own Powers
(see [02-concepts-disambiguated.md](02-concepts-disambiguated.md)).

### Security profile

- Gated by autonomy axis **Tool execution** (and possibly **Power model
  spend** if the Power has a bound model).
- `Power.requires` must be satisfied — referenced Connectors must be in
  `claw.boundConnectorIds`; referenced MCP servers in `claw.boundMcpServerIds`.
- Each constituent Tool call / Connector op / MCP op produces its own
  Receipt under the Power invocation's Receipt as parent.

### Sub-session semantics

A Power invocation runs in the **calling claw's session**. It does not
create a new sub-session unless the Power has a bound model AND its system
prompt explicitly asks for one. (Bound-model Powers are functionally
identical to Sub-agent spawns when they take that path; the distinction
lives in *whether the runtime treats them as a new conversational turn*.)

### Arbitration

Powers do not arbitrate; they execute. If a Power fails (denied by posture,
denied by RULES.md, ran out of budget), the Receipt records the failure and
control returns to the calling claw, which decides what to do next.

## Sub-agent spawn

A claw creates a transient specialist instance for a multi-step task that
benefits from its own identity and budget. Per
[ADR-001](decisions/001-sub-agents-runtime-only.md), instances are
runtime-only.

### Security profile

- Gated by autonomy axis **Sub-agent spawn**.
- The spawned Sub-agent inherits the spawning claw's posture as a **hard cap**
  on every axis. The cap is re-checked on each Power invocation within the
  sub-session because the parent's posture can change mid-flight.
- The Sub-agent's `allowedPowerIds` is a strict subset of the spawning claw's
  Powers. Powers not in the allowlist are denied even if they exist on the
  claw.
- Budget enforcement (tokens, $, wallclock) is hard. Exceeding budget kills
  the sub-session.

### Sub-session semantics

- A new conversation thread is created with the Sub-agent's `systemPrompt`
  as the seed.
- The Sub-agent runs to completion, returns a result Draft (typically), and
  ceases to exist.
- The Sub-agent does NOT have access to the parent claw's transcript by
  default; it sees only what the spawn act explicitly passes to it.
- A Sub-agent CANNOT spawn further Sub-agents in v1. (v2 may relax this with
  a depth cap.)

### Arbitration

If a Sub-agent's output conflicts with another Sub-agent's output (e.g., a
Critic disagrees with a Writer), the **spawning claw's chief arbitrates**.
Arbitration is the spawning claw's responsibility — not the runtime's.
Mind shows the arbitration step as a Receipt under the spawning claw.

## Sub-claw delegation

A claw passes a request to a peer claw (typically a child). This is how the
macro claw routes work to domain-specific sub-claws.

### Security profile

- Gated by autonomy axis **Sub-claw delegation**.
- The receiving claw runs at **its own** posture, capped by its parent
  ([ADR-005](decisions/005-recursive-claw.md), [ADR-008](decisions/008-three-postures-cap-invariant.md)).
- A delegate-to-sub-claw is implemented as a special **Power** of kind
  `'delegate-to-sub-claw'` ([09-data-model.md](09-data-model.md)) — so
  delegations always go through the Power layer, which means they always
  produce Receipts.
- Cross-claw porosity (the autonomy axis) determines what data passes from
  delegating claw to receiving claw.

### Sub-session semantics

- The receiving claw treats the delegated request as a new top-level prompt
  in its own session context. It uses its own Soul, RULES, Sources, Powers,
  Triggers.
- The receiving claw may itself spawn Sub-agents or delegate to its own
  child claws — recursion is unlimited (depth caps optional in v2).
- The result returned to the delegating claw is the receiving claw's final
  output (Draft, message, action confirmation).

### Arbitration

If two child claws produce conflicting outputs (a delegating claw asked
both Health-claw and Personal-claw, and they returned different
recommendations), the **delegating claw's chief arbitrates**. Same rule as
Sub-agent arbitration, one level up.

> Quote from the user during architecture discussion:
> "if there's a conflict between sub-chiefs, the macro chief judges."

## Cross-claw porosity

The autonomy axis **Cross-claw porosity** governs what data flows between
claws during delegation:

| Posture | Effect |
|---|---|
| Low | Only the explicit prompt content passes. No source contents, no transcript, no draft contents. The receiving claw must use its own Sources only. |
| Medium | Explicit prompt + summarized referenced material. The delegating claw produces a one-paragraph context block; receiving claw sees it. |
| High | Explicit prompt + full referenced Sources/Drafts (subject to receiving claw's own posture and Source bindings). |

Cross-claw porosity is per-direction (delegating → receiving). The
receiving claw's posture also gates what *inbound* it accepts.

## Receipt structure for the call-tree

Every node produces exactly one `Receipt` ([09-data-model.md](09-data-model.md)).
Receipts link via `parentReceiptId` to assemble the tree. Mind's call-tree
view ([08-mind.md](08-mind.md)) is a depth-first render of these.

For a Sub-agent spawn:

```
Receipt {
  kind: 'sub-agent-spawn',
  clawId: <spawning claw>,
  actor: { kind: 'macro-claw' | 'sub-claw', id: ... },
  action: { templateId, allowedPowerIds, budget },
  outcome: 'completed' | 'failed' | 'killed',
  resultDraftIds: [...],
  // children Receipts have parentReceiptId = this.id
}
```

For a Sub-claw delegation:

```
Receipt {
  kind: 'sub-claw-delegation',
  clawId: <delegating claw>,
  actor: { kind: 'sub-claw' | 'macro-claw', id: ... },
  action: { targetClawId, prompt, porositySnapshot },
  outcome: 'completed' | 'failed' | 'denied-posture',
  // children Receipts (the receiving claw's invocations) have parentReceiptId = this.id
}
```

## Cycle prevention

Sub-claw delegation could in principle cycle (A delegates to B, B delegates
back to A). v1 prevents cycles via:

1. A claw cannot delegate to its own ancestor in the parent chain.
2. A claw cannot delegate to a sibling that has already delegated to it in
   the same top-level session.

These are enforced at the runtime (Tool gate). v2 may relax this for
explicitly-bidirectional flows with explicit user opt-in.

## Cross-references

- [01-claw-anatomy.md](01-claw-anatomy.md) — the recursive primitive
- [02-concepts-disambiguated.md](02-concepts-disambiguated.md) — Power vs Sub-agent vs Sub-claw delegation
- [05-autonomy-postures.md](05-autonomy-postures.md) — axes governing each kind
- [08-mind.md](08-mind.md) — call-tree rendering
- [09-data-model.md](09-data-model.md) — Receipt schema
- [ADR-001](decisions/001-sub-agents-runtime-only.md) — Sub-agents runtime-only
- [ADR-005](decisions/005-recursive-claw.md) — Recursive Claw primitive
- [ADR-008](decisions/008-three-postures-cap-invariant.md) — Posture caps
