## ADDED Requirements

### Requirement: Mind tab inventory (v1)
The Mind UI SHALL expose ten tabs: Session, Tasks, Drafts, Plans,
Diffs, PRs, Browser (placeholder), Receipts, Costs, Notifications. The
Browser tab SHALL render a v2-deferred placeholder in v1.

#### Scenario: Browser placeholder visible
- **WHEN** a user opens the Browser tab in v1
- **THEN** the tab SHALL display a notice referencing the v2 roadmap
  rather than a live browsing surface

### Requirement: Filter chain
Every Mind view SHALL be filterable by claw, by time window, and by
event kind. Default filters SHALL be the active claw and last 24
hours.

#### Scenario: Default filter is active claw + 24h
- **WHEN** Mind is opened with no prior filter state
- **THEN** the claw filter SHALL match the active claw and the time
  filter SHALL be `last 24 hours`

#### Scenario: Switching active claw changes default
- **WHEN** the user switches the active claw
- **THEN** the Mind default claw filter SHALL update to the new active
  claw

### Requirement: Receipts are append-only
Receipts SHALL be append-only. Updates to a logical state SHALL produce
a new Receipt with `parentReceiptId` referencing the prior Receipt.

#### Scenario: Receipt edit produces new record
- **WHEN** a logical action's outcome is later corrected (e.g., a
  Power result becomes available after an initial provisional
  Receipt)
- **THEN** the runtime SHALL emit a new Receipt with
  `parentReceiptId` pointing to the prior one, NOT mutate the prior
  record

### Requirement: Call-tree assembly via parentReceiptId
Mind's Call-tree view SHALL assemble nodes by walking
`Receipt.parentReceiptId` links and SHALL render the three node kinds
(Power invocation, Sub-agent spawn, Sub-claw delegation) with the
corresponding action details.

#### Scenario: Tree depth from a leaf Receipt
- **WHEN** a user selects a leaf Receipt in Mind
- **THEN** the Call-tree view SHALL display all ancestors up to the
  root (the user message that originated the work) and all siblings
  at each level

### Requirement: Drafts vs Receipts distinction
Mind SHALL surface Drafts (mutable artifacts produced by work) and
Receipts (immutable audit rows logging that work happened) on
distinct tabs. A Draft SHALL be reachable from the Receipt that
created it via `Receipt.resultDraftIds`.

#### Scenario: Draft links back to Receipt
- **WHEN** a Power produces a Draft
- **THEN** the Draft's `bornFromReceiptId` SHALL match the
  `power-invocation` Receipt's id, and that Receipt's
  `resultDraftIds` SHALL include the Draft's id

### Requirement: Cost accounting per Receipt
Every Receipt that involves a model call or a tool with cost SHALL
populate `tokensIn`, `tokensOut`, `costDollars`, and `wallclockMs`.
The Costs tab SHALL aggregate from these.

#### Scenario: Model-call Receipt records cost
- **WHEN** a `power-invocation` Receipt records a model call
- **THEN** the Receipt SHALL contain non-null `tokensIn`,
  `tokensOut`, `costDollars`, and `wallclockMs` values

### Requirement: Notifications event stream
Mind's Notifications tab SHALL render a chronological stream of
async events: Routine completion, long-Task completion, RULES
violation rewrites, sidecar health changes, expired approvals.

#### Scenario: Sidecar-health event surfaces notification
- **WHEN** the gateway emits a `sidecar-health` event with
  `state: 'degraded'`
- **THEN** Mind's Notifications tab SHALL display a corresponding
  entry within 1 second

### Requirement: Receipts retention floor
Receipts SHALL be retained for at least 90 days from creation. A
maintenance task MAY compact older receipts in v2; v1 retains them
unbounded.

#### Scenario: Receipt 30 days old still queryable
- **WHEN** Mind is queried for receipts created 30 days ago
- **THEN** the runtime SHALL return all matching Receipts without
  any compaction

### Requirement: Receipts CSV / JSONL export
The `mind.query-receipts` method SHALL support an export format
parameter producing CSV or JSONL output suitable for compliance use.

#### Scenario: JSONL export streams full Receipt records
- **WHEN** `mind.query-receipts` is invoked with `format: 'jsonl'`
- **THEN** the gateway SHALL stream one Receipt JSON object per
  line, ordered by ascending `timestamp`
