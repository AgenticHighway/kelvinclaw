## ADDED Requirements

### Requirement: Twelve posture axes
The autonomy posture matrix SHALL define exactly twelve axes:
`toolExecution`, `subClawDelegation`, `subAgentSpawn`, `sourceReads`,
`connectorWrites`, `draftPromotion`, `pluginInstall`, `memoryWrites`,
`wasmEgress`, `routinesUserAbsent`, `crossClawPorosity`,
`powerModelSpend`. Each axis SHALL accept exactly the levels `low`,
`medium`, `high`.

#### Scenario: Posture serialises with all twelve axes
- **WHEN** a `PosturePerAxis` is serialised
- **THEN** the resulting object SHALL contain exactly the twelve
  named keys, each set to one of `low`, `medium`, `high`

### Requirement: Three named postures
The system SHALL expose three named shortcut postures (`Low`,
`Medium`, `High`). Selecting a named posture SHALL set every axis to
the corresponding level. Per-axis overrides MAY then be applied within
parent caps.

#### Scenario: Selecting Medium populates all axes
- **WHEN** a user selects posture `Medium` for a claw with no overrides
- **THEN** every axis on that claw SHALL be set to `medium`

#### Scenario: Posture badge with overrides
- **WHEN** a claw has at least one per-axis override active
- **THEN** the posture badge SHALL render as the base name with a
  trailing `*` (e.g., `Medium*`)

### Requirement: Parent-caps-child invariant
For every claw and every axis, `effective[axis]` SHALL equal the
strictest level among the user cap, every ancestor's level, and the
claw's own level. The store SHALL reject mutations that would violate
the cap.

#### Scenario: Child posture exceeds parent rejected
- **WHEN** a parent claw is at `Low` for `connectorWrites` and a child
  claw is set to `High` for the same axis
- **THEN** the store SHALL reject the mutation with error
  `cap-violation`

#### Scenario: Cap re-evaluated when parent tightens
- **WHEN** a parent claw lowers its `toolExecution` from `High` to
  `Low`
- **THEN** every descendant's effective `toolExecution` SHALL be
  recomputed and capped at `Low`

### Requirement: User cap above macro claw
The install SHALL maintain a single user-cap `PosturePerAxis` that
caps the macro claw on every axis.

#### Scenario: User cap caps macro
- **WHEN** the user cap is `Medium` and the macro claw's posture is
  `High`
- **THEN** the macro claw's effective posture SHALL be `Medium` per
  axis

### Requirement: Sidecar-down floors to Low
The runtime SHALL floor the effective posture across every claw and
every axis to `Low` whenever Open Bias `:4000` is unreachable or
`ToolRegistry` is misconfigured, and SHALL maintain the floor until
both sidecars recover.

#### Scenario: Open Bias unreachable triggers floor
- **WHEN** the `ModelProvider` shim's health probe to Open Bias fails
  for two consecutive 5-second intervals
- **THEN** the effective posture across the install SHALL be `Low` and
  a `sidecar-health` event SHALL be emitted with state `degraded`

#### Scenario: ToolRegistry misconfigured denies tool calls
- **WHEN** the `ToolRegistry` cannot resolve the calling claw's
  posture
- **THEN** every tool call SHALL be denied with a Receipt
  `outcome: 'denied-posture'` and detail `'tool-gate-misconfigured'`

### Requirement: Per-action overrides ("remember this")
A user SHALL be able to commit a `PostureOverride` with scope `once`,
`session`, `claw`, or `forever`. Overrides at scope `forever` SHALL
require a separate explicit confirmation step before commit.

#### Scenario: Forever scope requires confirmation
- **WHEN** a user picks `forever` in an approval card and clicks
  Allow
- **THEN** the GUI SHALL present a secondary confirmation dialog
  before persisting the `PostureOverride`

#### Scenario: Session scope expires on disconnect
- **WHEN** a `PostureOverride` is created at scope `session` and the
  client disconnects
- **THEN** the override SHALL be invalidated for any subsequent
  reconnect

### Requirement: Routines posture independent of session
Triggers SHALL fire at the claw's posture, NOT at the user's current
session posture. The `routinesUserAbsent` axis MAY further restrict
what routines are permitted to do when no user is present.

#### Scenario: Routine fires at claw posture
- **WHEN** a Heartbeat Trigger fires while the user is logged out and
  the claw's `connectorWrites` is `Medium`
- **THEN** the Routine's tool calls SHALL be evaluated against
  `Medium`, not against any user-session posture

#### Scenario: routinesUserAbsent further restricts
- **WHEN** the claw has `connectorWrites: 'medium'` but
  `routinesUserAbsent: 'low'`, and a Trigger fires while user-absent
- **THEN** Connector write calls within that Trigger's run SHALL be
  evaluated as if `connectorWrites` were `low`

### Requirement: Posture changes are recorded
The runtime SHALL produce a Receipt of kind `posture-change` for every
change to a claw's posture (base posture or per-axis override
add/revoke).

#### Scenario: Posture change emits Receipt
- **WHEN** a user lowers a claw's `connectorWrites` from `Medium` to
  `Low`
- **THEN** a Receipt of kind `posture-change` SHALL be appended with
  the before/after axis values

### Requirement: WASM egress maps to sandbox preset
The `wasmEgress` axis SHALL select the kelvinclaw WASM sandbox preset
in effect for WASM-backed Skills: `low → locked_down`, `medium →
dev_local`, `high → hardware_control`.

#### Scenario: Low maps to locked_down
- **WHEN** a WASM Skill is invoked under `wasmEgress: 'low'`
- **THEN** the runtime SHALL load the Skill with the `locked_down`
  preset, denying network and filesystem writes
