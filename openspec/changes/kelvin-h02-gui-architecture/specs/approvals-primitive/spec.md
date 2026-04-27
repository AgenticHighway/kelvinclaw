## ADDED Requirements

### Requirement: Question kind discriminator
The existing `Question` type SHALL gain a `kind` field with values
`'clarification' | 'approval'`. The default for existing records SHALL
be `'clarification'` so existing consumers continue to render
unchanged.

#### Scenario: Existing question defaults to clarification
- **WHEN** a `Question` record produced before this change is loaded
- **THEN** it SHALL be treated as `kind: 'clarification'` and rendered
  through the existing carousel renderer without modification

### Requirement: Approval-kind fields
A `Question` of `kind: 'approval'` SHALL carry the additional fields
`actionDescriptor`, `defaultChoiceId`, `scopeOptions`, `riskLevel`, and
`expiresAt` (all optional except `actionDescriptor`).

#### Scenario: Approval has actionDescriptor
- **WHEN** a `Question` of `kind: 'approval'` is emitted
- **THEN** `actionDescriptor` SHALL be present and SHALL contain a
  `kind` matching one of the `ReceiptKind` values

### Requirement: Approval card renderers per ActionDescriptor.kind
The system SHALL provide a specific detail-block renderer for each
`ActionDescriptor.kind` (`tool-call`, `sub-agent-spawn`,
`sub-claw-delegation`, `connector-op`, `mcp-op`, `source-read`,
`draft-promotion`, `memory-write`, `posture-change`) that surfaces the
relevant context (args, target, diff, posture snapshot, …).

#### Scenario: Connector op approval shows reversibility note
- **WHEN** an approval is rendered for a `connector-op` of an
  outbound write op
- **THEN** the card SHALL include a note indicating the op is
  irreversible from the agent's side

### Requirement: Scope picker
An approval card SHALL render scope options exactly as listed in the
question's `scopeOptions`. Selecting `forever` SHALL trigger a
secondary confirmation dialog.

#### Scenario: Forever requires confirmation
- **WHEN** the user selects `forever` and clicks Allow
- **THEN** the GUI SHALL display a confirmation dialog and SHALL not
  commit a `PostureOverride` until the dialog is confirmed

### Requirement: Termination control
Every approval surface SHALL include a termination button that, when
activated, cancels in-flight Sub-agent runs and marks pending approvals
as `denied` with reason `'user-terminated'`.

#### Scenario: Termination cancels in-flight Sub-agent
- **WHEN** a user clicks the termination control while a Researcher
  Sub-agent is running
- **THEN** the runtime SHALL kill the `SubAgentInstance` and emit
  `Receipt`s with `outcome: 'killed'` for any in-flight tool calls

### Requirement: Approval expiry
The system SHALL auto-deny an approval whose `expiresAt` deadline has
passed and SHALL emit a Receipt with `outcomeDetail:
'approval-expired'`. `expiresAt` is optional on a `Question` of
`kind: 'approval'`.

#### Scenario: Expired approval auto-denies
- **WHEN** the wall-clock time exceeds an approval's `expiresAt`
- **THEN** the underlying action SHALL be denied automatically and
  the Question SHALL be marked `status: 'answered'` with
  `selectedOptionIds` empty

### Requirement: Approval audit trail
Every resolved approval (allowed, denied, or expired) SHALL produce a
Receipt of the appropriate kind. If the resolution scope is `claw` or
`forever`, an additional Receipt of kind `posture-change` SHALL be
emitted recording the persistent override.

#### Scenario: Allow forever produces posture-change Receipt
- **WHEN** a user resolves an approval as Allow with scope `forever`
- **THEN** the runtime SHALL emit one Receipt of the action's kind
  (e.g., `connector-op`) and one Receipt of kind `posture-change`
  carrying the new `PostureOverride`

### Requirement: Carousel ordering
When multiple approvals are pending, the carousel SHALL sort by
descending urgency (`critical > high > normal`); ties broken by
descending `riskLevel`; ties broken by ascending `createdAt`.

#### Scenario: Critical before normal regardless of arrival order
- **WHEN** a `normal`-urgency approval is created at t=1 and a
  `critical`-urgency approval is created at t=2
- **THEN** the carousel SHALL display the `critical` approval first

### Requirement: Sidecar-down approval visibility
When sidecars are degraded or down, the approval surface SHALL remain
visible but new actions SHALL auto-deny per the sidecar-down floor
defined in the autonomy-postures capability.

#### Scenario: Sidecar-down banner above carousel
- **WHEN** the install enters `sidecar-down` state
- **THEN** the GUI SHALL render a banner above the carousel
  identifying which sidecar is unavailable, and the underlying gates
  SHALL deny new actions
