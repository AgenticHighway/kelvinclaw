## ADDED Requirements

### Requirement: Three call-tree node kinds
The runtime SHALL produce a call-tree with exactly three node kinds:
`power-invocation`, `sub-agent-spawn`, and `sub-claw-delegation`. Every
node SHALL produce exactly one `Receipt` linking to its parent via
`parentReceiptId`.

#### Scenario: Power invocation produces a Receipt
- **WHEN** a Power is invoked by a claw or Sub-agent
- **THEN** the runtime SHALL emit one Receipt of kind
  `power-invocation` with the calling actor and posture snapshot

#### Scenario: Sub-agent spawn produces a Receipt
- **WHEN** a claw spawns a Sub-agent
- **THEN** the runtime SHALL emit one Receipt of kind
  `sub-agent-spawn` carrying the role, allowedPowerIds, and budget

#### Scenario: Children link to parents
- **WHEN** a Sub-agent invokes a Power within its sub-session
- **THEN** the resulting `power-invocation` Receipt SHALL set
  `parentReceiptId` to the `sub-agent-spawn` Receipt's id

### Requirement: Sub-agent posture cap
A spawned Sub-agent SHALL inherit the spawning claw's posture as a
hard cap on every axis. The cap SHALL be re-checked on each Power
invocation within the sub-session, allowing the spawning claw's posture
to tighten mid-flight.

#### Scenario: Inherited cap at spawn
- **WHEN** a claw at posture `Medium` spawns a Sub-agent
- **THEN** the Sub-agent's effective posture SHALL be `Medium` (or
  lower per any further cap), and tool calls requiring `High` SHALL be
  denied

#### Scenario: Mid-flight tightening
- **WHEN** the spawning claw's posture is lowered while a Sub-agent is
  running
- **THEN** the next Power invocation by the Sub-agent SHALL be
  evaluated against the new lower cap

### Requirement: Sub-agent allowlist enforcement
A Sub-agent SHALL only invoke Powers listed in its `allowedPowerIds`,
even if other Powers exist on the spawning claw.

#### Scenario: Power outside allowlist
- **WHEN** a Sub-agent attempts to invoke a Power not in
  `allowedPowerIds`
- **THEN** the tool gate SHALL deny the call with error
  `not-in-allowlist` and emit a Receipt with `outcome: 'denied-policy'`

### Requirement: Sub-agent depth cap (v1)
A `SubAgentInstance` SHALL NOT itself spawn further Sub-agents in v1.

#### Scenario: Recursive spawn rejected
- **WHEN** a running Sub-agent invokes `subagent.spawn`
- **THEN** the spawn handler SHALL deny the request with error
  `subagent-depth-cap`

### Requirement: Sub-claw delegation goes through a Power
Cross-claw delegation SHALL be implemented as a Power of kind
`delegate-to-sub-claw`. There SHALL NOT be a separate "delegation"
runtime concept that bypasses the Power / Tool gate / Receipt path.

#### Scenario: Delegation is gated like any Power
- **WHEN** the macro claw delegates to a sub-claw
- **THEN** the runtime SHALL invoke a Power of kind
  `delegate-to-sub-claw`, gated by the autonomy axis
  `subClawDelegation`, producing a Receipt of kind
  `sub-claw-delegation`

### Requirement: Cycle prevention in delegation
A claw SHALL NOT delegate to its own ancestor in the parent chain, and
SHALL NOT delegate to a sibling that has already delegated to it within
the same top-level session.

#### Scenario: Ancestor delegation rejected
- **WHEN** sub-claw A attempts to delegate to its parent macro claw
- **THEN** the delegation handler SHALL deny the call with error
  `cycle-detected`

#### Scenario: Sibling cycle within session rejected
- **WHEN** sibling A has already delegated to sibling B in the current
  top-level session, and sibling B attempts to delegate back to A
- **THEN** the delegation handler SHALL deny the call with error
  `cycle-detected`

### Requirement: Cross-claw porosity governs delegation payload
The delegating claw's `crossClawPorosity` posture axis SHALL determine
what is passed to the receiving claw: prompt-only at Low, prompt plus
summarised context at Medium, prompt plus full referenced
Sources/Drafts at High.

#### Scenario: Low porosity strips context
- **WHEN** a claw with `crossClawPorosity: 'low'` delegates
- **THEN** the receiving claw SHALL receive only the prompt text and
  SHALL NOT receive any sources or drafts from the delegating claw

#### Scenario: High porosity passes referenced material
- **WHEN** a claw with `crossClawPorosity: 'high'` delegates with
  references to specific Sources and Drafts
- **THEN** the receiving claw SHALL receive the referenced material,
  subject to the receiving claw's own porosity cap on inbound

### Requirement: Arbitration belongs to the parent
The spawning claw or the delegating claw SHALL arbitrate when two
Sub-agents or two child claws produce conflicting outputs. The runtime
SHALL NOT attempt automatic merging or voting.

#### Scenario: Conflicting Sub-agent outputs
- **WHEN** a Researcher Sub-agent and a Critic Sub-agent return
  contradictory recommendations to the spawning claw
- **THEN** the spawning claw's chief SHALL produce one resolved Draft,
  and a Receipt SHALL record the arbitration step
