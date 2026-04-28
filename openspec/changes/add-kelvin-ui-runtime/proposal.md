## Why

With foundations and security specified, the remaining behavioural
layer is the **user-facing surface**: the composer's mode picker, the
Mind observability tabs, and the WebSocket gateway protocol that
connects the H02 GUI to the kelvinclaw runtime.

This change specifies six composer modes (Auto / Plan / Ask / Learn
/ Play / Make) as orthogonal to autonomy, ten Mind tabs with
filterable receipts and a call-tree view, and the H02 ↔ kelvinclaw
gateway protocol with submit / wait / event-stream messages,
reconnect-with-resume, and a typed RPC method catalogue.

Together with `add-kelvin-foundations` and `add-kelvin-security`, this
completes the v1 behavioural spec.

## What Changes

- **NEW** capability `composer-modes` — six modes (Auto, Plan, Ask,
  Learn, Play, Make), default Auto on new sessions, modes are
  globally shared (not on Claw schema), Plan/Ask/Learn contracts
  forbid execution categories regardless of autonomy, modes are
  orthogonal to autonomy (intersection semantics), spawned Sub-agents
  inherit mode from spawning turn, Triggers default to Auto.
- **NEW** capability `mind-observability` — ten tabs (Session, Tasks,
  Drafts, Plans, Diffs, PRs, Browser placeholder, Receipts, Costs,
  Notifications), filter chain (claw + time + kind, defaults to
  active claw and last 24h), Receipts are append-only and parent-
  linked, call-tree assembly via `parentReceiptId`, Drafts vs
  Receipts distinction, per-Receipt cost accounting, notifications
  event stream, 90-day Receipts retention floor, CSV/JSONL export.
- **NEW** capability `gateway-protocol` — three message classes
  (submit / wait / event), monotonic per-connection event sequence,
  reconnect-with-resume via `fromSeq`, gap-too-large error, typed
  RPC method catalogue (~30 methods), typed errors
  (`denied-posture`, `denied-policy`, `cycle-detected`,
  `cap-violation`, `subset-violation`, etc.), backpressure via
  `throttle`, loopback-only authentication boundary in v1, opt-in
  streamed results for large queries.

ADR-004 (single-user v1) is referenced as the scope decision that
governs gateway authentication.

## Capabilities

### New Capabilities

- `composer-modes`: Six modes orthogonal to autonomy; mode contracts forbid categories of action regardless of posture.
- `mind-observability`: Mind UI surface — tabs, call-tree, append-only Receipts, cost accounting, notifications, retention.
- `gateway-protocol`: H02 ↔ kelvinclaw WebSocket gateway — submit/wait/event-stream messages, method catalogue, reconnect, typed errors, backpressure.

### Modified Capabilities

None directly. This change references the `data-model` capability
from `add-kelvin-foundations` for entity shapes, and the
`security-sidecars` capability from `add-kelvin-security` for the
`sidecar-health` event payload. No requirements from those
capabilities are modified here.

## Impact

- **Code (kelvinclaw)**: New gateway methods (or extensions to the
  existing gateway) for the ~30 RPC methods. Sequence-numbered event
  stream with retention window. Reconnect-with-resume handler.
  Backpressure throttle. Loopback-only WebSocket upgrade check.
  Receipt query endpoints supporting filters, time ranges,
  CSV/JSONL streaming, and call-tree assembly.
- **Code (H02)**: New components — `claws/ClawWizard.tsx`,
  `mind/CallTreeView.tsx`, `mind/ReceiptsTab.tsx`, `mind/CostsTab.tsx`,
  `mind/CallTreeNode.tsx` (one per node kind). Composer mode chip
  (already exists; ensure mode contracts are enforced
  client-side and server-side). Reconnect-with-resume client logic.
  Browser tab placeholder.
- **APIs**: This change *defines* the gateway API surface for v1.
  v2 modifications follow the OpenSpec MODIFIED workflow.
- **Operations**: 90-day Receipts retention adds storage growth;
  monitor and consider compaction in v2.
- **Documentation**: This change is the OpenSpec-canonical home for
  every behavioural requirement of the user-facing layer.
