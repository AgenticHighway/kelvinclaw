## ADDED Requirements

### Requirement: Claw is the recursive primitive
The system SHALL model every dispatcher (the macro Kelvin claw and every
sub-claw at any depth) as a single `Claw` schema with the same anatomy.
There SHALL NOT be a separate "macro" type; the macro claw is identified
by `parentClawId === null`.

#### Scenario: Identifying the macro claw
- **WHEN** a consumer queries for the macro claw
- **THEN** the runtime SHALL return the unique `Claw` whose
  `parentClawId` is `null`

#### Scenario: Sub-claw is structurally identical
- **WHEN** a sub-claw is rendered in the GUI
- **THEN** the same Claw component, wizard, and posture editor SHALL be
  used as for the macro claw, with no positional branching beyond
  default-value differences

#### Scenario: Exactly one macro claw per install
- **WHEN** the store contains more than one `Claw` with
  `parentClawId === null` after migration or import
- **THEN** the migration runner SHALL fail validation and refuse to
  rehydrate the store

### Requirement: Per-claw privileged anatomy
Each `Claw` SHALL own (and only this claw shall own) the following
collections: Sources, Drafts, Powers, Triggers, Channels (bound subset),
bound Connectors, bound MCP servers, optional Sub-agent templates, plus
two file-backed configs `soul.md` and `RULES.md`.

#### Scenario: Power belongs to exactly one claw
- **WHEN** a `Power` is registered
- **THEN** it SHALL carry exactly one `clawId`, and the claw at that id
  SHALL list the Power in its `powerIds`

#### Scenario: Soul and Rules are per-claw files
- **WHEN** a claw is created
- **THEN** the runtime SHALL create a directory at
  `<kelvinDataDir>/claws/<clawId>/` containing `soul.md` and `RULES.md`
  files

### Requirement: Globally shared concerns are not on Claw
Modes, Inputs, Mind, and Settings SHALL NOT be fields on the `Claw`
schema. They SHALL be exposed via the UI / runtime context layer and be
visible to every claw without being owned by any claw.

#### Scenario: Modes are not on Claw
- **WHEN** the Claw schema is validated
- **THEN** it SHALL NOT contain a `modes` or `currentMode` field

#### Scenario: Mind sees all claws
- **WHEN** the Mind UI is opened
- **THEN** it SHALL be able to filter Receipts and Drafts across every
  claw in the install, with no claw-level access control beyond
  ownership

### Requirement: Subset invariants for bindings
A child claw's `boundConnectorIds` SHALL be a subset of its parent's
`boundConnectorIds`, and the same SHALL hold for `boundMcpServerIds`.
The store SHALL reject any mutation that violates this invariant.

#### Scenario: Cannot bind unbound Connector
- **WHEN** a sub-claw attempts to bind a Connector that its parent has
  not bound
- **THEN** the store SHALL reject the mutation with error
  `subset-violation` and SHALL NOT persist the change

#### Scenario: Unbinding parent Connector cascades
- **WHEN** a parent claw unbinds a Connector
- **THEN** the runtime SHALL automatically unbind that Connector from
  every descendant claw and emit a Receipt of kind `posture-change`
  for each cascade step

### Requirement: Power.requires must match claw bindings
For every Power, every `requires.connectors[*]` SHALL reference a
Connector in the owning claw's `boundConnectorIds`, and every
`requires.mcps[*]` SHALL reference an MCP server in
`boundMcpServerIds`. The registry SHALL reject Power install when this
condition fails.

#### Scenario: Power install with missing Connector binding
- **WHEN** a Power declares `requires.connectors: ["gmail"]` and the
  owning claw does not have `gmail` in `boundConnectorIds`
- **THEN** the registry SHALL reject the install with error
  `missing-binding`

### Requirement: Drafts are the only outbound write target
A Power, Sub-agent, or Routine SHALL NOT directly mutate any `Source`.
All produced content SHALL be written as a `Draft` with status
`'generating'` initially, transitioning to `'ready'` and only then
optionally to `'promoted'` via an explicit promotion action.

#### Scenario: Power output goes to Drafts
- **WHEN** a Power produces content during its invocation
- **THEN** the runtime SHALL persist the content as a `Draft` and SHALL
  NOT directly call any Source write op

#### Scenario: Promotion is the only Source write path
- **WHEN** a Source is updated (filesystem write, memory append,
  connector op, MCP op)
- **THEN** there SHALL exist a corresponding `Draft` with `status` of
  `'promoted'` and `promotedToSourceIds` containing the updated Source's
  id, and a Receipt of kind `draft-promotion` linking them
