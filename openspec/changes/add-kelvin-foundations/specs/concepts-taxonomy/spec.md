## ADDED Requirements

### Requirement: Four distinct concepts
The system SHALL model Powers, Connectors, MCP servers, and Sub-agents
as four first-class, distinct concepts. Each SHALL have its own
TypeScript schema and SHALL NOT be collapsed into a unified "tools"
type.

#### Scenario: Schemas are distinct
- **WHEN** the data model is loaded
- **THEN** five distinct types SHALL be present: `Connector`,
  `MCPServer`, `Power`, `SubAgentTemplate`, `SubAgentInstance`

### Requirement: Connector lifecycle and home
A `Connector` SHALL be persistent, installed and authenticated through
the global Settings → Connectors panel. It SHALL carry credentials
(opaquely via `authRef`), scopes, and an `exposedOperations` list, but
SHALL NOT carry a model binding or a per-claw scope.

#### Scenario: Connector is global
- **WHEN** a Connector is added through Settings
- **THEN** the Connector SHALL be visible to the install at-large and
  SHALL NOT be scoped to a specific claw until explicitly bound via
  `claw.boundConnectorIds`

#### Scenario: Connector credentials never plaintext
- **WHEN** a Connector is serialised in any persisted form
- **THEN** `authRef` SHALL be an opaque reference to credential storage
  and the credential value itself SHALL NOT appear in the persisted
  record

### Requirement: MCP server lifecycle and home
An `MCPServer` SHALL be persistent, installed and configured through
the global Settings → MCP servers panel. It SHALL expose
`toolDefs`, `resourceDefs`, and `promptDefs` discovered via the MCP
protocol, and SHALL declare an endpoint of kind `stdio`,
`remote-http`, or `remote-ws`.

#### Scenario: MCP tools are discovered
- **WHEN** an MCP server reaches `status: 'active'`
- **THEN** the runtime SHALL populate `toolDefs`, `resourceDefs`,
  and `promptDefs` from the server's protocol responses

### Requirement: Power as agent-facing capability
A `Power` SHALL be persistent, owned by exactly one claw, and SHALL be
either a Skill (atomic), a Workflow (composed), or a
`delegate-to-sub-claw` Power. A Power MAY bind its own model. A Power's
`requires` field SHALL declare any Connectors, MCP servers, or other
Powers it depends on.

#### Scenario: Power declares its requirements explicitly
- **WHEN** a Power is registered
- **THEN** `requires` SHALL list every Connector, MCP server, and
  Power it uses, and SHALL NOT depend on any external resource not
  listed there

### Requirement: Sub-agents are runtime-only
A `SubAgentInstance` SHALL exist only at runtime, scoped to a single
sub-session, and SHALL NOT be persisted to the store. A
`SubAgentTemplate` MAY be persisted as a per-claw spawning preset but
SHALL NOT be required for spawning.

#### Scenario: Instance is not persisted
- **WHEN** a sub-session ends
- **THEN** the corresponding `SubAgentInstance` SHALL no longer be
  reachable from the persisted store; only the spawn `Receipt` SHALL
  remain

#### Scenario: Ad-hoc spawn without a template
- **WHEN** a parent claw invokes `subagent.spawn` with explicit role,
  systemPrompt, allowedPowerIds, and budget
- **THEN** the spawn SHALL succeed without referencing any
  `SubAgentTemplate`

### Requirement: Layering direction
Sub-agents SHALL invoke Powers; Powers SHALL invoke Connector ops or
MCP tools or built-in tools. A Connector SHALL NOT invoke a Power, an
MCP tool SHALL NOT invoke a Power, and a Sub-agent SHALL NOT directly
invoke a Connector op or MCP tool without going through a Power.

#### Scenario: Sub-agent cannot bypass Power layer
- **WHEN** a Sub-agent runtime attempts to invoke a Connector op
  directly
- **THEN** the tool gate SHALL deny the call with error
  `must-go-through-power`

### Requirement: Powers stand alone (no agentType coupling)
A `Power` SHALL NOT carry an `agentType` field coupling it to a
specific Sub-agent. Powers SHALL be selectable independently of any
Sub-agent context.

#### Scenario: Migration removes agentType
- **WHEN** the H02 migration runs
- **THEN** the resulting `Power` schema SHALL NOT contain an
  `agentType` field, and a `grep -rn 'agentType' src/` over the H02
  source SHALL yield no matches
