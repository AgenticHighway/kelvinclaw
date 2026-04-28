## ADDED Requirements

### Requirement: Posture record per claw with toolExecution axis
Every persisted Claw SHALL carry a `posture` record. In this slice
the record SHALL contain exactly one axis, `toolExecution`, with
value `'low'`, `'medium'`, or `'high'`. Default for new claws SHALL
be `'medium'`.

#### Scenario: New claw posture defaults to medium
- **WHEN** `claw.create` is issued without an explicit `posture`
  parameter
- **THEN** the persisted Claw SHALL have `posture.toolExecution`
  set to `'medium'`

#### Scenario: Posture round-trips
- **WHEN** `claw.create` is issued with `posture: { toolExecution:
  'low' }` and a subsequent `claw.list` is issued
- **THEN** the listed claw SHALL have `posture.toolExecution` set
  to `'low'`

### Requirement: ToolRegistry gates every tool call against toolExecution
The kelvinclaw `ToolRegistry` SHALL consult the calling claw's
`posture.toolExecution` before dispatching any tool call. The result
SHALL be one of: `auto-allow` (proceed), `ask` (suspend the call
and emit an approval request), or `auto-deny` (refuse the call
without asking).

#### Scenario: Low posture asks for every tool call
- **WHEN** a tool call originates from a session whose claw is at
  `posture.toolExecution: 'low'`
- **THEN** the gate SHALL return `ask` regardless of the tool's
  `isWrite` / `isExternal` flags

#### Scenario: Medium posture asks for write or external tools
- **WHEN** a tool call originates from a claw at
  `posture.toolExecution: 'medium'` for a tool whose metadata sets
  `isWrite: true` OR `isExternal: true`
- **THEN** the gate SHALL return `ask`

#### Scenario: Medium posture auto-allows pure-read internal tools
- **WHEN** a tool call originates from a claw at
  `posture.toolExecution: 'medium'` for a tool whose metadata sets
  `isWrite: false` AND `isExternal: false`
- **THEN** the gate SHALL return `auto-allow` and the call
  proceeds without surfacing an approval

#### Scenario: High posture auto-allows all tools
- **WHEN** a tool call originates from a claw at
  `posture.toolExecution: 'high'`
- **THEN** the gate SHALL return `auto-allow` regardless of tool
  metadata

### Requirement: Tool metadata carries isWrite and isExternal flags
Tools registered with the `ToolRegistry` SHALL declare two boolean
metadata flags: `isWrite` (true if the tool mutates state, makes
outbound calls, or has any side effect outside the runtime) and
`isExternal` (true if the tool reaches outside `KELVIN_DATA_DIR` or
makes any network egress). Existing built-in tools SHALL be
audited and tagged.

#### Scenario: Filesystem-write tool is isWrite=true
- **WHEN** a tool registered as `fs_safe_write` is inspected
- **THEN** its metadata SHALL set `isWrite: true`

#### Scenario: Web-fetch tool is isExternal=true
- **WHEN** a tool registered as `web_fetch` is inspected
- **THEN** its metadata SHALL set `isExternal: true`

#### Scenario: Memory-search tool is isWrite=false, isExternal=false
- **WHEN** a tool registered as `memory_search` is inspected
- **THEN** its metadata SHALL set both `isWrite: false` and
  `isExternal: false`

### Requirement: Approval requests are emitted as gateway events
When the gate returns `ask`, the runtime SHALL suspend the tool
call AND emit an `event` of kind `approval.requested` with payload
containing `{ approval_id, claw_id, run_id, tool_name, arguments,
isWrite, isExternal, posture: { toolExecution } }`.

#### Scenario: Suspension blocks the call
- **WHEN** the gate returns `ask` for a tool call
- **THEN** the underlying `Tool::call` invocation SHALL NOT execute
  until an `approval.respond` arrives with a decision

#### Scenario: Event payload is complete
- **WHEN** an `approval.requested` event is emitted
- **THEN** the payload SHALL include all of: `approval_id`,
  `claw_id`, `run_id`, `tool_name`, `arguments`, `isWrite`,
  `isExternal`, `posture.toolExecution`

### Requirement: approval.respond resumes or denies the suspended call
The kelvinclaw gateway SHALL accept a method `approval.respond`
with params `{ approval_id, decision: 'allow' | 'deny' }`. On
`allow`, the suspended tool call SHALL execute. On `deny`, the
suspended call SHALL be aborted with a typed error.

#### Scenario: Allow resumes the call
- **WHEN** `approval.respond` is sent with `decision: 'allow'` for
  a pending approval
- **THEN** the suspended tool call SHALL execute and a
  corresponding `approval.resolved` event SHALL fire with
  `outcome: 'allowed'`

#### Scenario: Deny aborts the call
- **WHEN** `approval.respond` is sent with `decision: 'deny'`
- **THEN** the suspended tool call SHALL NOT execute, the brain
  SHALL receive a tool-call error of kind `denied-by-approval`,
  AND an `approval.resolved` event SHALL fire with `outcome:
  'denied'`

### Requirement: Approval timeout fails closed
The runtime SHALL deny a suspended call automatically and emit an
`approval.resolved` event with `outcome: 'timeout'` if no
`approval.respond` arrives within a configurable timeout (default
5 minutes).

#### Scenario: 5-minute default timeout denies
- **WHEN** an `approval.requested` event has no corresponding
  `approval.respond` within 5 minutes
- **THEN** the suspended call SHALL be denied with a tool-call
  error AND `approval.resolved` SHALL fire with `outcome:
  'timeout'`
