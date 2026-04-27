---
status: Draft
version: v1
audience: designers, implementors
date: 2026-04-27
---

# Approvals Primitive — UI Spec

When the autonomy posture matrix gates an action, an **approval** surfaces
to the user. This doc specs the approval UI primitive.

Per [ADR-006](decisions/006-soul-rules-files-and-question-reuse.md), the
approval primitive is an **extension** of H02's existing `Question` /
`QuestionCarousel` types — not a new component. This doc describes the
extension.

## Why reuse `Question`

H02's `Question` already has every field an approval needs:

- `options` with selection types (single / multi)
- `selectedOptionIds`
- `urgency` (`normal | high | critical`)
- `traceItems` — the chain of reasoning leading to the question
- `attachments` — referenced material (drafts, source excerpts)
- `category` — already includes `'autonomy'`
- `confidence` — used to size the urgency
- `clawId` (formerly `spaceId`) — which claw raised it
- Carousel UI for stacking pending questions

Adding a `kind: 'clarification' | 'approval'` discriminator and a small set
of approval-specific fields gives the full primitive.

## Question schema extension (recap)

From [09-data-model.md](09-data-model.md):

```ts
type QuestionKind = 'clarification' | 'approval';

interface Question {
  // ... existing fields ...
  kind: QuestionKind;
  // approval-only:
  actionDescriptor?: ActionDescriptor;
  defaultChoiceId?: string;
  scopeOptions?: ('once' | 'session' | 'claw' | 'forever')[];
  riskLevel?: 'low' | 'medium' | 'high';
  expiresAt?: Date;
}

interface ActionDescriptor {
  kind: ReceiptKind;             // tool-call, connector-op, ..., posture-change
  summary: string;               // "Send email to advisor@example.com"
  details: Record<string, unknown>;
}
```

## ApprovalCard — visual layout

When `kind === 'approval'`, the QuestionCarousel renders the question with
the `ApprovalCard` template:

```
┌──────────────────────────────────────────────────────────────────────┐
│  ⚠ Connector write — Personal claw  (medium risk)                    │
│                                                                      │
│  Send email via Gmail                                                │
│  to: advisor@example.com                                             │
│  subject: Thank you                                                  │
│                                                                      │
│  ▾ Body preview                                                      │
│    Hi Dr. Smith, thanks for the time today. ...                      │
│                                                                      │
│  ▾ Why this is gated                                                 │
│    Personal claw → connectorWrites = Low (override)                  │
│    Default for Medium would be "Auto for low-impact"                 │
│                                                                      │
│  ▾ Trace (3 steps)                                                   │
│    1. User said "send a thank-you to my advisor"                     │
│    2. Personal claw drafted message                                  │
│    3. Power `send_email` invoked → gmail.send                        │
│                                                                      │
│  Decision:                                                           │
│  ◉ Allow once         ○ For session   ○ For this claw   ○ Forever    │
│  [ Allow ]  [ Edit body first ]  [ Deny ]                            │
└──────────────────────────────────────────────────────────────────────┘
```

Mandatory components:

- **Title** — one-line summary derived from `actionDescriptor.summary`
- **Risk pill** — color-coded by `riskLevel` (green / yellow / red)
- **Action details block** — discriminated by `actionDescriptor.kind`
- **Body / payload preview** — for diff-able actions, show the diff
- **"Why this is gated" section** — what posture rule triggered the approval
- **Trace** — collapsible; mirrors `traceItems`
- **Scope picker** — one option from `scopeOptions`
- **Decision buttons** — `Allow`, `Deny`. Optionally `Edit before allowing`
  for reversible payload edits (email body, file diff).

## Action descriptor renderers

Each `ReceiptKind` has a renderer for the action details block:

| Kind | Detail block render |
|---|---|
| `tool-call` | tool name, args (JSON pretty); show signing status if Skill |
| `sub-agent-spawn` | role, allowed Powers, budget, parent template |
| `sub-claw-delegation` | target claw, prompt, porosity snapshot |
| `connector-op` | service, op, args; reversibility note |
| `mcp-op` | server, tool, args |
| `source-read` | source name, type, what's being read |
| `draft-promotion` | source destination, write op, diff preview |
| `memory-write` | memory store, write preview, append/overwrite |
| `posture-change` | axis, before / after, scope |

Renderers are thin React components mounted on the Question detail panel.

## Scope semantics

The `scope` choice governs persistence of the user's decision:

| Scope | Effect | Stored as |
|---|---|---|
| `once` | This action only | nothing (not persisted) |
| `session` | All future actions matching the same descriptor for this session | `PostureOverride` with session lifetime |
| `claw` | All future actions on this claw matching the descriptor (until claw posture changes) | `PostureOverride` with `clawId` scope |
| `forever` | All future actions matching the descriptor across claws (subject to ancestor caps) | `PostureOverride` with no expiry |

A `forever` choice is special: a separate confirmation step is required
before it commits ("Are you sure you want to allow this forever? Listed at
top of posture screen."). This is to prevent silent posture loosening.

## Termination control

Every approval surface includes a **termination button** ("Stop this
session") that:

- Cancels in-flight Sub-agent runs.
- Marks pending approvals as `denied` with reason "user terminated."
- Returns control to the macro claw with a stop summary.

Termination is also surfaced in:

- The Mind session tab (always visible)
- The persistent chat composer area (a small "stop" icon when work is
  in-flight)

## Carousel behavior

When multiple approvals are pending (e.g., a Sub-agent is firing several
gated actions in parallel):

- They stack in the existing QuestionCarousel.
- The most urgent (`urgency: 'critical'` or highest `riskLevel`) sorts first.
- Sub-agents pause after their first gate-required action; do not pile up
  dozens of approvals.
- The user can use a "batch decide" button: "Allow all from this Sub-agent
  for the rest of its run." This commits a session-scoped `PostureOverride`
  and clears the carousel.

## Approval expiry

`expiresAt` causes a soft auto-deny. By default, approvals don't expire
unless the autonomy posture or trigger configuration requests it. When set:

- 60 seconds before expiry, the card flashes amber.
- At expiry, the card shows "expired — auto-denied" and the underlying
  action is denied as if the user had clicked Deny.
- A Receipt is written with `outcome: 'denied-policy'` and detail
  `'approval-expired'`.

Use cases:

- Routines fire-and-decay — if user isn't around to approve in 5 min, deny.
- High-stakes actions where long delay implies user disengagement.

## Sidecar-down behavior

If sidecars are down (per [ADR-008](decisions/008-three-postures-cap-invariant.md)):

- Pending approvals remain visible.
- New actions auto-deny (sidecar-down floors to Low; Low for that axis may
  already be deny-by-default; if not, the floor adds the gate).
- A sidecar-down banner explains the state.

## The "always show me" pattern

Per the user's request during architecture discussion, the approval
primitive emphasizes giving the user the full picture, not eliding details.

By default, all renderers expand action details and "why gated" sections.
Collapsed-by-default applies only to long body previews and trace items
beyond 5 entries.

## Audit trail

Every approval decision produces a `Receipt`:

```
Receipt {
  kind: 'tool-call' | 'connector-op' | ...,  // the underlying action
  outcome: 'allowed' | 'denied-policy' | 'denied-posture',
  outcomeDetail: 'user-approved-once' | 'user-approved-session' | ... | 'user-denied' | 'expired',
  posture: <snapshot at time of approval>,
}
```

If the decision was `forever` or `claw`, an additional `Receipt` of kind
`posture-change` records the persistent override.

## Consistency invariants

The same axes/wording from the matrix in
[05-autonomy-postures.md](05-autonomy-postures.md) MUST appear in:

- ApprovalCard "Why this is gated" copy
- The PostureMatrix UI labels
- This doc

Verification: a mechanical diff between the table in
[05-autonomy-postures.md](05-autonomy-postures.md) and the labels used in
ApprovalCard renderers should produce zero differences. See the verification
list in the spec plan.

## Cross-references

- [ADR-006](decisions/006-soul-rules-files-and-question-reuse.md) — Question reuse rationale
- [ADR-008](decisions/008-three-postures-cap-invariant.md) — posture invariants
- [05-autonomy-postures.md](05-autonomy-postures.md) — matrix
- [07-sidecars.md](07-sidecars.md) — sidecar-down behavior
- [08-mind.md](08-mind.md) — Receipts produced by approvals
- [09-data-model.md](09-data-model.md) — Question, PostureOverride
- [10-h02-migration.md](10-h02-migration.md) — Question consumers audit
