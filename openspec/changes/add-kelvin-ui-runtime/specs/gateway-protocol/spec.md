## ADDED Requirements

### Requirement: Three message classes
The H02 ↔ kelvinclaw gateway SHALL define three classes of message:
`submit` (client → server, fire-and-forget RPC), `wait` (client →
server, optional block-on-result), and `event` (server → client,
push stream). Every server response other than `event` SHALL carry
the originating `requestId`.

#### Scenario: Submit returns ack with taskId
- **WHEN** the client sends `{ type: 'submit', requestId: r1, method:
  'claw.send-message', params: ... }`
- **THEN** the server SHALL respond with
  `{ type: 'submit-ack', requestId: r1, taskId: t1 }` before any
  long-running work begins

### Requirement: Monotonic event sequence
Every `event` message from server to client SHALL include a
monotonically increasing `seq` number, scoped to the current
WebSocket connection.

#### Scenario: Sequence number strictly increasing
- **WHEN** a client receives two `event` messages on the same
  connection
- **THEN** the second's `seq` SHALL be greater than the first's

### Requirement: Reconnect with resume
On reconnect, the client SHALL be able to send `{ type:
'subscribe', fromSeq: <last seen> }`. The server SHALL replay all
events with `seq > fromSeq`. If the gap exceeds the server's
retention window, the server SHALL respond with `{ type:
'subscribe-error', code: 'gap-too-large', lastAvailableSeq: <n> }`.

#### Scenario: Resume from last seen seq
- **WHEN** a client reconnects with `subscribe.fromSeq = 1234` and
  events 1235..1500 are within the retention window
- **THEN** the server SHALL replay events 1235 through 1500 in
  order before any new events

#### Scenario: Gap too large
- **WHEN** a client reconnects with `subscribe.fromSeq` older than
  the retention window allows
- **THEN** the server SHALL respond with `subscribe-error` carrying
  `code: 'gap-too-large'` and the lowest available `seq`

### Requirement: Method catalogue
The gateway SHALL accept the following `submit.method` values:
`claw.send-message`, `claw.list`, `claw.create`, `claw.update`,
`claw.delete`, `power.invoke`, `subagent.spawn`, `subagent.kill`,
`delegation.invoke`, `draft.promote`, `draft.discard`,
`question.answer`, `posture.set`, `posture.override.add`,
`posture.override.revoke`, `connector.list`, `connector.add`,
`connector.bind`, `connector.unbind`, `mcp.list`, `mcp.add`,
`mcp.bind`, `mcp.unbind`, `trigger.create`, `trigger.update`,
`trigger.delete`, `trigger.fire`, `sources.list`, `sources.add`,
`sources.read`, `mind.query-receipts`, `mind.query-call-tree`,
`costs.query`, `sidecar.health`. Methods outside this set SHALL be
rejected with error `unknown-method`.

#### Scenario: Unknown method rejected
- **WHEN** the client sends `submit.method: 'foo.bar'`
- **THEN** the server SHALL respond with
  `{ type: 'submit-error', code: 'unknown-method', ... }`

### Requirement: Typed errors
A failed `submit` SHALL produce a `submit-error` carrying one of the
codes: `denied-posture`, `denied-policy`, `not-found`,
`invalid-args`, `sidecar-down`, `cycle-detected`, `cap-violation`,
`subset-violation`, `missing-binding`, `missing-power`,
`unknown-method`, `internal`.

#### Scenario: Posture denial code
- **WHEN** a tool gate denies a Power invocation due to posture
- **THEN** the resulting `submit-error` SHALL have `code:
  'denied-posture'`

### Requirement: Backpressure
Clients SHALL honour `throttle` responses by deferring the next
submit by at least `retryAfterMs` milliseconds before retrying. The
server MAY respond with `{ type: 'throttle', requestId: ...,
retryAfterMs: <n> }` when its queue depth exceeds capacity.

#### Scenario: Throttle defers submit
- **WHEN** the client receives a `throttle` response
- **THEN** the client SHALL not retry the same `requestId` for at
  least `retryAfterMs` milliseconds

### Requirement: Authentication boundary (v1)
In v1 the gateway SHALL accept connections only from loopback
(`127.0.0.1` or `::1`). It SHALL NOT accept remote connections.
Authentication is implicit (filesystem ownership of
`KELVIN_DATA_DIR`).

#### Scenario: Non-loopback connection refused
- **WHEN** a connection is initiated from a non-loopback address
- **THEN** the gateway SHALL refuse the WebSocket upgrade with HTTP
  403

### Requirement: Stream-format opt-in
The server SHALL emit multiple `wait-result` parts when a streamed
method (`mind.query-receipts`, `mind.query-call-tree`, `costs.query`)
is invoked with `params.stream: true`. Each part SHALL carry a
`final` boolean that is `false` for intermediate parts and `true`
for the last.

#### Scenario: Streamed Receipts query
- **WHEN** the client sends `mind.query-receipts` with `params.stream
  = true` and a filter that matches 5,000 Receipts
- **THEN** the server SHALL emit multiple `wait-result` parts; only
  the final one SHALL set `final: true`
