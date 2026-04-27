---
status: Draft
version: v1
audience: H02 implementors, runtime authors
date: 2026-04-27
---

# H02 ↔ kelvinclaw — WebSocket Gateway Protocol

H02 talks to the kelvinclaw runtime via the existing WebSocket gateway
documented in [`docs/gateway/gateway-protocol.md`](../../gateway/gateway-protocol.md).
This doc is the kelvin-spec layer on top — the H02-specific message types
and the Mind-feed subscription model.

## Connection

H02 connects to `ws://localhost:<gateway-port>/ws` (port from kelvinclaw
config). Authentication in v1 is implicit (single-user, localhost only,
loopback-bound). v2 adds per-user JWT.

The connection is long-lived. Reconnect-with-resume: each event in the
server-to-client stream carries a monotonic `seq` number; client resumes
from `seq + 1` on reconnect.

## Submit / wait / event-stream model

Three classes of message, mirroring the existing gateway model:

### Submit (client → server)

The client sends a request. Each submit has a unique `requestId`. The
server immediately ack's with `submitted` and the work proceeds
asynchronously.

```jsonc
// Client → Server
{
  "type": "submit",
  "requestId": "req_01HXYZ...",
  "method": "claw.send-message",
  "params": {
    "clawId": "claw_personal",
    "mode": "auto",
    "message": "send a thank-you to my advisor"
  }
}
```

```jsonc
// Server → Client (immediate ack)
{
  "type": "submit-ack",
  "requestId": "req_01HXYZ...",
  "taskId": "task_01HXYZ..."
}
```

### Wait (client → server, optional)

For RPC-style flows where the client wants to block on a result:

```jsonc
// Client → Server
{
  "type": "wait",
  "requestId": "req_01HXYZ...",
  "timeoutMs": 60000
}
```

```jsonc
// Server → Client (when complete)
{
  "type": "wait-result",
  "requestId": "req_01HXYZ...",
  "outcome": "completed | failed | timeout",
  "result": { ... }
}
```

### Event stream (server → client)

The Mind feed is a single subscription on this connection that receives all
events relevant to the current user. Events:

```jsonc
// Receipt produced
{ "type": "event", "seq": 1234, "kind": "receipt", "payload": <Receipt> }

// Draft created or updated
{ "type": "event", "seq": 1235, "kind": "draft", "payload": <Draft> }

// Question raised (clarification or approval)
{ "type": "event", "seq": 1236, "kind": "question", "payload": <Question> }

// Question answered (server-side; e.g., approval expired)
{ "type": "event", "seq": 1237, "kind": "question-resolved", "payload": { id, outcome } }

// Task state change
{ "type": "event", "seq": 1238, "kind": "task", "payload": <Task> }

// Sidecar health change
{ "type": "event", "seq": 1239, "kind": "sidecar-health", "payload": { state, reason } }

// Cost update (debounced; 1 per second max)
{ "type": "event", "seq": 1240, "kind": "cost-tick", "payload": { clawId, deltaDollars } }
```

The Mind UI subscribes to this stream directly; each tab filters from the
shared event stream. See [08-mind.md](../08-mind.md).

## Methods (`submit.method` values)

H02-specific RPC methods served by the gateway:

| Method | Purpose | Params |
|---|---|---|
| `claw.send-message` | Send a chat message to a claw | `{ clawId, mode, message, attachments? }` |
| `claw.list` | List all claws | `{ filter? }` |
| `claw.create` | Create a new claw | `<Claw subset>` |
| `claw.update` | Update claw config (incl. posture) | `{ clawId, patch }` |
| `claw.delete` | Delete a claw + its directory | `{ clawId }` |
| `power.invoke` | Direct Power invocation | `{ clawId, powerId, args }` |
| `subagent.spawn` | Spawn ad-hoc Sub-agent | `{ parentClawId, role, systemPrompt, allowedPowerIds, budget }` |
| `subagent.kill` | Kill a running Sub-agent | `{ instanceId }` |
| `delegation.invoke` | Force a delegation (Power kind=delegate-to-sub-claw) | `{ fromClawId, toClawId, prompt }` |
| `draft.promote` | Promote Draft to Source(s) | `{ draftId, targets: PromotionTarget[] }` |
| `draft.discard` | Discard a Draft | `{ draftId }` |
| `question.answer` | Answer a clarification or approval | `{ questionId, selectedOptionIds, scope?, freeText? }` |
| `posture.set` | Set posture per axis | `{ clawId, axis, level }` (override only — base posture is `claw.update`) |
| `posture.override.add` | Add a "remember this" override | `<PostureOverride>` |
| `posture.override.revoke` | Revoke a "remember this" override | `{ overrideId }` |
| `connector.list` / `connector.add` / `connector.bind` / `connector.unbind` | Connector management | various |
| `mcp.list` / `mcp.add` / `mcp.bind` / `mcp.unbind` | MCP server management | various |
| `trigger.create` / `trigger.update` / `trigger.delete` / `trigger.fire` | Trigger management | various |
| `sources.list` / `sources.add` / `sources.read` | Source management | various |
| `mind.query-receipts` | Receipts query (filter, time range, limit) | `{ filter, fromSeq?, limit? }` |
| `mind.query-call-tree` | Assemble call-tree starting at a Receipt | `{ receiptId }` |
| `costs.query` | Aggregated cost query | `{ groupBy, period }` |
| `sidecar.health` | Sidecar health probe | `{}` → `{ openBias, toolRegistry }` |

For methods that return large result sets (Receipts, Drafts, Costs), the
server may stream results as multiple `wait-result` parts when the client
opts in via `params.stream: true`.

## Authorization scope (v1)

In v1 (single-user), every method runs as the implicit user. v2 adds
per-method scope checks against the user-cap and per-claw ACLs.

## Errors

Errors return:

```jsonc
{
  "type": "submit-error",
  "requestId": "req_01HXYZ...",
  "code": "denied-posture | denied-policy | not-found | invalid-args | sidecar-down | internal",
  "message": "Connector writes denied — Personal claw posture is Low",
  "detail": { ... }
}
```

`denied-posture` and `denied-policy` are NOT internal errors; they
correspond to legitimate gate refusals. The H02 client treats them as
expected outcomes and surfaces them via the appropriate UI (ApprovalCard
already-resolved-as-deny, banner, etc.).

## Subscription lifecycle

The event stream is automatic on connect. The client may filter
client-side (Mind tabs do this). v2 may add server-side filter
subscriptions.

## Reconnect / catch-up

On reconnect, the client sends:

```jsonc
{ "type": "subscribe", "fromSeq": 1234 }
```

The server replays all events with `seq > 1234`. If the gap exceeds the
server's retention window, the client receives:

```jsonc
{ "type": "subscribe-error", "code": "gap-too-large", "lastAvailableSeq": 1500 }
```

The H02 client then re-fetches Mind state from `mind.query-receipts` and
resumes from a fresh `seq`.

## Backpressure

The server may issue:

```jsonc
{ "type": "throttle", "requestId": "req_01HXYZ...", "retryAfterMs": 500 }
```

H02 honors this by deferring the next submit. Client-side queue depth is
capped (default 32) to avoid runaway submits.

## Cross-references

- [`docs/gateway/gateway-protocol.md`](../../gateway/gateway-protocol.md) — the underlying gateway
- [09-data-model.md](../09-data-model.md) — entity shapes referenced in
  params/results
- [08-mind.md](../08-mind.md) — Mind tabs as event-stream filters
- [interfaces/sidecar-integration.md](sidecar-integration.md) — sidecar
  health flows through this protocol
- [interfaces/tool-gate-postures.md](tool-gate-postures.md) — `denied-posture`
  errors originate from the tool gate
- [`OVERVIEW.md`](../../../OVERVIEW.md) — the runtime seams the gateway sits on top of
