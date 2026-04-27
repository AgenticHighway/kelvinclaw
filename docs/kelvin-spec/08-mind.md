---
status: Draft
version: v1
audience: designers, implementors
date: 2026-04-27
---

# Mind — Observability Surface

**Mind** is the global observability surface. It shows what Kelvin is
doing, what it has done, what's pending, what's costing money. It is a
[globally shared](01-claw-anatomy.md) concern — visible from every claw,
filterable down to one claw or up to the whole install.

## Tab inventory (v1)

| Tab | Shows | Source data |
|---|---|---|
| **Session** | Current chat with the active claw; live in-flight work | session state + Receipts since session start |
| **Tasks** | Active and queued Power invocations / Sub-agent runs / delegations | runtime task list |
| **Drafts** | All Drafts produced; promotion targets + status | `Draft` collection |
| **Plans** | Multi-step plans (Plan-mode outputs and Workflow plans) | `Plan` entities (preserved from H02) |
| **Diffs** | File changes pending or applied (filesystem Sources) | `Receipt`s where action is filesystem write |
| **PRs** | Pull request status (when GitHub Connector wired) | Connector-backed feed |
| **Browser** | (v2 placeholder) live browsing surface | (deferred) |
| **Receipts** | Immutable audit log; the single source of truth for "what happened" | `Receipt` collection |
| **Costs** | Token / dollar / wallclock spend per claw, per Power, per session | aggregated from Receipts |
| **Notifications** | Async pings from completed Routines, finished long Tasks, RULES violations | event stream |

Plus a **Call-tree** view (cross-cutting; not a tab — invokable from any
Receipt or Task).

## Filter chain

Every Mind view is filterable along three axes:

```
[Claw filter]   [Time filter]   [Kind filter]
  Macro            Last hour      Tool calls
  Personal         Today          Connector ops
  Health           This week      Sub-agent spawns
  Work             All time       Delegations
  …                Custom         Promotions
                                  Posture changes
                                  All
```

The default view filters to the *active claw* and *last 24h*. Switching
the active claw changes the default filter; user can override.

## Session tab

The current chat. Renders:

- Message stream (user / assistant / tool-result blobs)
- In-flight work indicators (spinning Power invocation, ongoing Sub-agent)
- Pending approvals (links into ApprovalCard view)
- Termination control (always visible)

Live updates via the WebSocket gateway event stream
([interfaces/h02-protocol.md](interfaces/h02-protocol.md)).

## Tasks tab

Active and queued work, surface for asynchronous coordination:

| Field | Meaning |
|---|---|
| Title | Power name / Sub-agent role / delegation summary |
| Status | `pending | running | waiting-on-approval | completed | failed` |
| Claw | which claw is running it |
| Started | timestamp |
| ETA | best-effort wallclock estimate |
| Spend | running token / $ count |

Click a task → opens its Call-tree view.

## Drafts tab

Renders the `Draft` collection with status indicators:

```
○ generating   ✓ ready   ↗ promoted
```

Each Draft card shows:

- Title + content preview
- Provenance: `bornFromPowerId` / `bornFromSubAgentInstanceId`
- Promotion targets list (with "Promote to..." action per
  [ADR-007](decisions/007-drafts-promotion-edge.md))
- For promoted Drafts: which Sources received it + when

## Plans tab

H02 already has a `Plan` entity. Mind's Plans tab reuses it; surfaces
plans produced in Plan-mode and Workflow-Power steps:

- Plan body (numbered steps)
- Each step's Power and prerequisites
- "Run this plan" action (subject to autonomy posture per step)

## Diffs tab

Filesystem changes — pending or applied:

- Filename + line-diff
- Change source (which Power, when, by which claw)
- Apply / discard actions for pending changes
- Filterable by directory tree

## PRs tab

When the GitHub Connector is bound to a claw and a PR-related Power is
used, Mind shows PR status:

- Open / merged / closed
- CI status (per Connector data)
- Review comments inbox

This tab is empty if the Connector isn't bound.

## Browser tab

v2 placeholder. Renders:

> "Browser tab coming in v2 — see [11-roadmap.md](11-roadmap.md)"

## Receipts tab

The audit log. Every action produces a `Receipt`. The Receipts tab is the
canonical "what happened" view:

| Column | Source |
|---|---|
| When | `Receipt.timestamp` |
| Kind | `Receipt.kind` |
| Claw | `Receipt.clawId` (link) |
| Actor | `Receipt.actor.kind / id` |
| Outcome | `Receipt.outcome` |
| Detail | summary derived from `Receipt.action` |
| Cost | `tokensIn + tokensOut + dollars` |
| Trace | OTEL trace link if present |

Receipts are append-only ([09-data-model.md](09-data-model.md)). Filtering
+ search across all fields. Export to CSV / JSONL for compliance.

## Costs tab

Aggregated spend from Receipts:

- **By claw** — bar chart of $ / day for the last 30 days
- **By Power** — top 10 most expensive Powers
- **By session** — current session vs. 7-day average
- **vs. budget** — compared to `CostBudget` config (informational in v1;
  hard cutoffs in v3)

Dollar amounts come from `Receipt.costDollars` (populated by
`ModelProvider` shim using provider pricing tables).

## Notifications tab

Asynchronous pings:

- Routine completed (e.g., "Morning brief drafted")
- Long Task finished (e.g., "Researcher done; 2 Drafts ready")
- RULES.md violation auto-rewritten or rejected (Open Bias)
- Sidecar health changed
- Approval expired

Notifications can deep-link to the underlying Receipt / Draft / Task.

## Call-tree view

A render of the three node kinds from
[03-delegation-and-call-tree.md](03-delegation-and-call-tree.md). Invoked
from any Receipt or Task; shows the full ancestry and descendants.

```
[user message → Personal claw]                     ← root
└─ [Sub-claw delegation: Health-claw]               kind=sub-claw-delegation
   ├─ [Power invocation: web_search]                kind=power-invocation
   │  └─ [MCP op: web_search.search(q=…)]            kind=mcp-op
   ├─ [Sub-agent spawn: Researcher]                 kind=sub-agent-spawn
   │  ├─ [Power invocation: web_search × 4]
   │  └─ [Power invocation: cite × 1]
   └─ [Power invocation: send_email]
      └─ [Connector op: gmail.send(...)]            kind=connector-op
```

Interactions:

- Click a node → focus + show Receipt details on the right
- Filter by outcome (only show denials, only show high-cost, …)
- Time-scrub to replay execution
- "Show as plain text" → linearized for sharing / debugging

The call-tree is assembled by walking `Receipt.parentReceiptId` links.

## Receipts vs Drafts — the distinction

A persistent confusion early in the architecture conversation:

- **Drafts** — *artifacts produced by work*. Promotable to Sources. Mutable
  during generation. Audit'd via the Receipt that created them.
- **Receipts** — *immutable rows logging that work happened*. Cannot be
  edited. Reference Drafts they produced.

A Power invocation that produces a doc creates one Receipt + one Draft.
The Draft is the content; the Receipt is the audit row. Different tabs
(Drafts shows content; Receipts shows the log).

## H02 implementation

H02 already has:

- An `OverlayState` with 5 fullscreen views — these become the primary
  Mind tabs.
- `MindFilterStep` — the current 5-tab filter; expand per this doc.
- `Plan`, `Draft`, `Thought`, `Task`, `FeedItem` types — preserved.

New components per [10-h02-migration.md](10-h02-migration.md):

- `mind/ReceiptsTab.tsx`
- `mind/CostsTab.tsx`
- `mind/CallTreeView.tsx`
- `mind/CallTreeNode.tsx` (per node kind, dispatched by `Receipt.kind`)

## Performance considerations

- Receipts can grow unbounded. v1 keeps them in IndexedDB on the H02 side,
  with a "compact older than 90 days" maintenance task (v2).
- Call-tree assembly is a join across Receipts; index on
  `parentReceiptId` is required. A virtual scroll renderer handles
  multi-thousand-node trees.
- Cost aggregation runs incrementally on Receipt insertion; full
  recomputation only on tab open + on filter change.

## Cross-references

- [01-claw-anatomy.md](01-claw-anatomy.md) — Mind is globally shared
- [03-delegation-and-call-tree.md](03-delegation-and-call-tree.md) —
  three node kinds
- [05-autonomy-postures.md](05-autonomy-postures.md) — what produces
  Receipts of each kind
- [06-approvals-primitive.md](06-approvals-primitive.md) — approval
  Receipts
- [07-sidecars.md](07-sidecars.md) — OTEL trace correlation
- [09-data-model.md](09-data-model.md) — `Receipt`, `Draft`, `Plan`,
  `FeedItem`
- [11-roadmap.md](11-roadmap.md) — Browser tab and other v2 items
