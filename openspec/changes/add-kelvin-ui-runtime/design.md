## Context

The H02 GUI talks to the kelvinclaw runtime via the existing gateway
documented in `docs/gateway/gateway-protocol.md`. This change extends
that gateway with H02-specific RPC methods, formalises the Mind
observability surface, and codifies six composer modes that have
been used informally throughout the conversation.

H02 already has `OverlayState` (5 fullscreen views) and
`MindFilterStep` (5-tab filter); this change extends them into the
ten-tab Mind surface. H02 already has `MessageMode` enum
(`'auto' | 'plan' | 'ask' | 'learn' | 'play' | 'make'`); this
change makes the *contract* of each mode enforceable rather than
merely UI-labelled.

## Goals / Non-Goals

**Goals:**
- Specify mode contracts as enforceable behaviours, not UI labels.
- Define every Mind tab's data source and filter behaviour.
- Define the gateway protocol with enough rigour that any compliant
  implementation can run H02.
- Make Receipts the canonical audit substrate that every other tab
  derives from.

**Non-Goals:**
- No multi-user / per-user gateway authentication (deferred to v2;
  ADR-004). v1 authenticates via filesystem ownership of
  `KELVIN_DATA_DIR`.
- No live Browser tab — placeholder in v1.
- No plugin authoring UI — v2.
- No hard cost cutoffs — v3. v1 displays costs as Receipts.
- No mobile / native packaging — v3.

## Decisions

### D1. Modes are globally shared, not per-claw

**Context.** A mode is a per-turn intent picker. The user might
type a `plan`-mode message at the macro claw, then switch active
claw mid-conversation. The mode should follow the user, not the
claw.

**Alternatives considered.**
- Per-claw mode storage. Awkward when user switches claws mid-flow.
- Per-session-per-claw mode storage. Same complexity without clear
  benefit.
- *Chosen:* Mode is a UI/runtime context concern. The `Claw` schema
  does NOT carry a `currentMode` field.

**Consequences.** Switching active claws preserves the current
mode. Voice and channel-driven inputs default to Auto with mode
override per channel (v2).

### D2. Mode contracts are enforced separately from autonomy posture

**Context.** "Plan mode" sounds safer than "Auto mode," and it is —
but only because Plan suppresses execution by contract, not because
Plan lowers autonomy. Conflating the two confuses the threat model.

**Alternatives considered.**
- Implement modes by lowering autonomy. Undermines the explicit
  posture model.
- Make modes purely UI-labelled with no runtime enforcement.
  Surprises users when they expect Plan to actually plan.
- *Chosen:* Effective behaviour = (mode contract) ∩ (autonomy
  posture). Mode contracts and posture caps are enforced
  independently.

**Consequences.** Plan/Ask/Learn modes have hard contracts that
forbid categories of action regardless of posture. Auto/Play/Make
have softer contracts where posture does most of the gating.

### D3. Mind observability built on Receipts as the substrate

**Context.** Every action must produce an audit row. With Receipts
as the canonical source, every Mind tab is a filtered/aggregated
view over Receipts. This unifies the implementation and makes
forensic investigation trivial.

**Alternatives considered.**
- Per-tab event streams (Drafts stream, Tasks stream, Costs stream,
  …). Multiple sources of truth; sync drift; harder forensics.
- Receipts plus separate denormalised projections per tab. Faster
  reads but doubles the write path.
- *Chosen:* Single append-only Receipts substrate. Each Mind tab is
  a query/filter over Receipts (with appropriate indexes). Drafts
  remain as a separate entity (mutable, promotable) but link to
  the Receipt that created them.

**Consequences.** Mind queries are flexible (any filter combination
works). Receipts retention floor is 90 days. Indexes on
`parentReceiptId`, `clawId`, `timestamp`, `kind` are required for
performance. Cost aggregation runs incrementally on Receipt
insertion.

### D4. Gateway protocol uses three message classes (submit/wait/event)

**Context.** H02 issues commands, sometimes wants results, and
needs a continuous Mind feed. The existing kelvinclaw gateway
already uses submit/wait/event-stream semantics; this change
formalises the H02-specific surface.

**Alternatives considered.**
- Pure RPC (every call is request/response). Doesn't fit Mind's
  async event needs.
- GraphQL subscriptions. Adds dependency; not what kelvinclaw uses.
- Server-sent events for the stream + REST for commands. Two
  transports to manage; complicates reconnect.
- *Chosen:* Single WebSocket carrying submit / wait / event. Each
  event has a monotonic `seq` for reconnect-with-resume.

**Consequences.** Reconnect-with-resume is essential for offline
laptops; gap-too-large errors trigger client re-fetch. Backpressure
via `throttle` responses. Method catalogue is closed; unknown
methods rejected. Streamed results opt-in for large queries.

### D5. Loopback-only gateway in v1

**Context.** Single-user installs run H02 and kelvinclaw on the same
machine. Remote access is a v2 concern.

**Alternatives considered.**
- TLS-bound remote gateway from day one. Auth model becomes
  immediately complex.
- *Chosen:* Loopback-only WebSocket upgrade in v1.

**Consequences.** Authentication is implicit (filesystem ownership
of `KELVIN_DATA_DIR`). Same-uid sibling processes can connect; this
is the v1 trust boundary.

## Risks / Trade-offs

[Risk: 90-day Receipts retention floor produces unbounded storage
growth on heavy users] → mitigation: v2 adds a compaction
maintenance task; v1 monitors growth and surfaces a notification
if it exceeds a configurable cap (e.g., 1 GB).

[Risk: reconnect-with-resume `gap-too-large` error means client
loses some events on long disconnects] → mitigation: the client
re-fetches Mind state via `mind.query-receipts` and resumes from a
fresh `seq`. Documented as expected behaviour.

[Risk: streaming `wait-result` parts complicate client error
handling] → mitigation: only opt-in via `params.stream: true`;
default behaviour is single response. Streaming reserved for
queries that legitimately return thousands of records.

[Risk: Browser tab placeholder is a UX disappointment] →
mitigation: explicit "v2 deferred" notice referencing the roadmap
in the v1 release notes.

[Risk: mode contracts may surprise users who expect Plan to "do
research first then act"] → mitigation: in-app help text on the
mode chip; UX testing during v1 implementation.

[Risk: same-uid sibling processes can connect to the gateway and
issue commands] → not solved by this change. Documented as the v1
trust boundary; v2 adds per-user authentication.

## Migration Plan

This change archives LAST in the dependency chain. Order:

1. `add-kelvin-foundations` archives → schemas in
   `openspec/specs/`.
2. `add-kelvin-security` archives → posture matrix and sidecar
   contracts in `openspec/specs/`.
3. `add-kelvin-ui-runtime` archives → modes, Mind, and gateway
   contracts in `openspec/specs/`.

After all three archive, `openspec/specs/` contains the v1
baseline. v2 changes emit MODIFIED / ADDED / REMOVED deltas
against this baseline.

## Open Questions

1. **90-day Receipts retention floor.** Storage growth needs to be
   measured against real usage. Consider exposing the retention
   period as a user-configurable setting in v2.

2. **Browser tab placeholder.** Users who want this feature may
   pressure for an early v2. Decide whether to ship it as a
   separate change-set in late v1 or wait for v2.

3. **`mind.query-receipts` filter expressiveness.** The current
   spec allows filtering by claw, time, kind. Real users may want
   filtering by actor, posture-at-time, cost-threshold,
   trace-id. Add as MODIFIED requirements in v2 if needed.

4. **Streamed query behaviour on slow clients.** If a client is
   slow to consume a streamed result, the server must buffer or
   drop. Buffering is unbounded by spec; needs a server-side
   policy. Possibly a MODIFIED requirement adding a
   `maxBufferedParts` config.

5. **Gateway method versioning.** As v2 adds methods, how does the
   client negotiate compatibility? Options: protocol version
   handshake on connect; per-method existence-check via
   `unknown-method` error. Document the chosen path explicitly.
