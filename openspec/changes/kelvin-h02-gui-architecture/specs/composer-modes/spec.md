## ADDED Requirements

### Requirement: Six composer modes
The composer SHALL expose exactly six modes: `auto`, `plan`, `ask`,
`learn`, `play`, `make`. The default mode for a new session SHALL be
`auto`.

#### Scenario: New session defaults to Auto
- **WHEN** a session is opened
- **THEN** the composer's mode chip SHALL display `auto`

### Requirement: Modes are globally shared, not per-claw
The current mode SHALL be a UI / runtime context concern. The `Claw`
schema SHALL NOT carry a `currentMode` field.

#### Scenario: Switching claws preserves mode
- **WHEN** the user switches the active claw mid-session
- **THEN** the current composer mode SHALL be retained unmodified

### Requirement: Plan mode contract
In `plan` mode, the runtime SHALL NOT execute any tool call, Sub-agent
spawn, or sub-claw delegation. The output SHALL be exactly one Plan
Draft listing the steps that would be executed under `auto` mode.

#### Scenario: Plan mode does not execute
- **WHEN** the user submits a message in `plan` mode that would, in
  `auto` mode, cause a Connector op
- **THEN** the runtime SHALL produce a Plan Draft listing the Connector
  op as a step, and SHALL NOT call the Connector

### Requirement: Ask mode contract
In `ask` mode, the runtime SHALL produce a text reply only. It SHALL
NOT execute Connector writes, write to memory, modify files, or spawn
Sub-agents. It MAY read Sources subject to the `sourceReads` posture
axis.

#### Scenario: Ask mode forbids Sub-agent spawn
- **WHEN** the user submits a message in `ask` mode that would
  ordinarily spawn a Researcher
- **THEN** the runtime SHALL reply inline without spawning, and SHALL
  not emit a `sub-agent-spawn` Receipt

### Requirement: Modes are orthogonal to autonomy
A mode's contract SHALL NOT be implemented by lowering the autonomy
posture. Mode contracts and posture caps SHALL be enforced
independently and the effective behaviour SHALL be the intersection.

#### Scenario: Plan at High autonomy still does not execute
- **WHEN** a claw at autonomy `High` receives a message in `plan` mode
- **THEN** the runtime SHALL NOT execute any tool call, despite the
  high autonomy

### Requirement: Mode inheritance for spawned Sub-agents
A spawned Sub-agent SHALL inherit the mode of the spawning turn unless
the spawn explicitly overrides it.

#### Scenario: Researcher spawned in Plan mode
- **WHEN** a Researcher Sub-agent is spawned during a `plan`-mode turn
  with no explicit mode override
- **THEN** the Researcher SHALL run in `plan` mode and produce a plan
  of how it would research, without executing research steps

### Requirement: Routine default mode
Triggers (Hooks, Heartbeats, Watches) SHALL fire in `auto` mode by
default. A Trigger MAY override this via `invokes.args.mode`.

#### Scenario: Heartbeat fires in Auto by default
- **WHEN** a Heartbeat Trigger fires without specifying a mode override
- **THEN** the runtime SHALL invoke the configured Power in `auto` mode
