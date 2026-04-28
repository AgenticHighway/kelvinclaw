## ADDED Requirements

### Requirement: Persistent entity ownership fields
The store SHALL ensure that every persistent entity (Claw, Source,
Draft, Power, Connector, MCPServer, SubAgentTemplate, Receipt, Trigger,
Channel, PostureOverride, CostBudget) carries `id`, `createdAt`,
`updatedAt`, `ownerId`, and `createdBy` fields. In v1 single-user
installs, `ownerId` and `createdBy` SHALL be populated with the single
implicit user id.

#### Scenario: New Power record has all ownership fields
- **WHEN** a Power is added through `claw.update`
- **THEN** the persisted record SHALL contain non-empty `id`,
  `createdAt`, `updatedAt`, `ownerId`, and `createdBy` fields

### Requirement: Source type and config alignment
Each `Source` SHALL declare a `type` from the closed set
{`filesystem`, `web`, `api`, `feed`, `memory`, `transcript`,
`connector-backed`, `mcp-resource`}, and its `config.kind` SHALL
match `type`. The store SHALL reject Sources whose `config.kind`
does not match `type`.

#### Scenario: Mismatched config rejected
- **WHEN** a Source is added with `type: 'filesystem'` and
  `config: { kind: 'web', baseUrl: '...' }`
- **THEN** the store SHALL reject the mutation with error
  `source-config-kind-mismatch`

### Requirement: SubAgentInstance is not persisted
A `SubAgentInstance` SHALL NEVER be written to the persistent store.
It SHALL exist only in runtime memory and SHALL be referenced by
Receipts only via id, never by embedding the full record.

#### Scenario: Store rehydration omits instances
- **WHEN** the runtime exports its persistent state
- **THEN** the export SHALL contain no `SubAgentInstance` records

### Requirement: Draft promotion bookkeeping
A `Draft` whose `status === 'promoted'` SHALL have a non-empty
`promotedToSourceIds`, a non-null `promotedAt`, and a
`promotedAutonomySnapshot` capturing the posture in effect at
promotion time.

#### Scenario: Promoted Draft carries snapshot
- **WHEN** a Draft is promoted to a memory Source under posture
  `Medium`
- **THEN** the resulting persistent record SHALL have `status:
  'promoted'`, `promotedToSourceIds` containing the memory source id,
  `promotedAt` set, and `promotedAutonomySnapshot` containing
  `medium` for `draftPromotion`

### Requirement: Receipts append-only and parent-linked
Every `Receipt` SHALL be append-only and MAY carry
`parentReceiptId`. Updates to a logical state SHALL produce a new
Receipt with `parentReceiptId` linking to the prior one.

#### Scenario: Receipt mutation rejected
- **WHEN** a write op attempts to update an existing Receipt's
  `outcome` or `action` field
- **THEN** the store SHALL reject the mutation with error
  `receipt-immutable`

### Requirement: PosturePerAxis schema
The store SHALL require that every persistent posture-bearing entity
(Claw, PostureOverride, Receipt) serialises its `PosturePerAxis` with
exactly the twelve keys defined in the autonomy-postures capability,
each set to one of `low`, `medium`, `high`.

#### Scenario: PosturePerAxis with extra key rejected
- **WHEN** a Claw is mutated with a `PosturePerAxis` containing an
  unrecognised axis key
- **THEN** the store SHALL reject the mutation with error
  `unknown-axis`

### Requirement: Question kind discriminator default
A `Question` record without an explicit `kind` field SHALL be loaded
with `kind: 'clarification'`.

#### Scenario: Legacy question loads as clarification
- **WHEN** a Question record is loaded from H02 legacy storage
  without a `kind` field
- **THEN** the in-memory representation SHALL have `kind:
  'clarification'`

### Requirement: Cap chain validation
The store SHALL enforce: for every claw and every axis,
`claw.autonomyPosture[axis] <= parent.autonomyPosture[axis]` (where
`low < medium < high` for cap purposes), recursively to root, and
SHALL also enforce `macroClaw.autonomyPosture[axis] <=
userCap[axis]`.

#### Scenario: Two-level cap violation rejected
- **WHEN** the macro claw is at `Medium`, a sub-claw is at `Medium`,
  and the sub-claw's child is mutated to `High` on any axis
- **THEN** the store SHALL reject the mutation with error
  `cap-violation`

### Requirement: Subset binding validation
The store SHALL enforce, for every claw, `claw.boundConnectorIds ⊆
parent.boundConnectorIds` and `claw.boundMcpServerIds ⊆
parent.boundMcpServerIds`.

#### Scenario: Binding subset enforced on add
- **WHEN** a sub-claw attempts to add a Connector binding the parent
  has not bound
- **THEN** the store SHALL reject with error `subset-violation`

### Requirement: Power.requires must resolve
For every Power, every `requires.connectors[*]` SHALL match an entry
in `claw.boundConnectorIds`, every `requires.mcps[*]` SHALL match
`claw.boundMcpServerIds`, and every `requires.powers[*]` SHALL
resolve to a Power on the same claw. The store SHALL reject Power
install when these conditions fail.

#### Scenario: Composed Power requires sibling Power
- **WHEN** a Workflow Power declares `requires.powers: ["pwr_search"]`
  and `pwr_search` does not exist on the same claw
- **THEN** the registry SHALL reject install with error
  `missing-power`
