## ADDED Requirements

### Requirement: Question type gains a kind discriminator
H02's existing `Question` type SHALL be extended with a `kind` field
of type `'clarification' | 'approval'`. Existing questions
without an explicit `kind` SHALL be loaded as `'clarification'` so
existing carousel renderers continue working unchanged.

#### Scenario: Legacy question loads as clarification
- **WHEN** a question record produced before this change is loaded
  into the H02 store
- **THEN** the in-memory representation SHALL have `kind:
  'clarification'`

### Requirement: Approval-kind questions carry an action descriptor
Questions of `kind: 'approval'` SHALL carry an `actionDescriptor`
with at least: `tool_name`, `arguments`, `isWrite`, `isExternal`,
`postureSnapshot.toolExecution`, plus a `claw_id` and `run_id` for
correlation.

#### Scenario: Action descriptor populated from event
- **WHEN** the H02 client receives an `approval.requested` event
  from the gateway
- **THEN** it SHALL create a Question of `kind: 'approval'` whose
  `actionDescriptor` is populated from the event's payload fields

### Requirement: ApprovalCard renders inside QuestionCarousel
The H02 chat surface SHALL render approval-kind questions in the
existing `QuestionCarousel` using a new `ApprovalCard` component.
The card SHALL display: title (tool name), risk pill (derived from
`isWrite`/`isExternal`), arguments (pretty-printed JSON), why-gated
section (the posture snapshot), and two buttons: Allow / Deny.

#### Scenario: Approval card appears in carousel
- **WHEN** an approval-kind Question is added to the H02 store
- **THEN** the existing QuestionCarousel SHALL render an
  ApprovalCard for it, distinguishable from clarification cards

#### Scenario: Risk pill colours by flags
- **WHEN** the action descriptor has `isExternal: true`
- **THEN** the risk pill SHALL display "external" in a color
  distinct from internal-tool actions

### Requirement: Allow / Deny send approval.respond
Clicking Allow or Deny on an ApprovalCard SHALL send a
`approval.respond` request to the gateway carrying the
`approval_id` and the user's `decision`. The card SHALL show a
loading state until the corresponding `approval.resolved` event
arrives.

#### Scenario: Allow click sends respond
- **WHEN** the user clicks Allow on an ApprovalCard
- **THEN** the H02 client SHALL send `req` of method
  `approval.respond` with `{ approval_id: <card's id>, decision:
  'allow' }`

#### Scenario: Resolve event clears the card
- **WHEN** an `approval.resolved` event arrives for an open
  ApprovalCard
- **THEN** the card SHALL be removed from the carousel and the
  outcome SHALL be reflected inline in the chat surface
  (e.g., "Allowed `web_fetch(...)`" or "Denied `web_fetch(...)`")

### Requirement: Termination control while awaiting approval
While an approval is pending, the chat surface SHALL display a
termination control that, when activated, sends `approval.respond`
with `decision: 'deny'` for every open approval AND additionally
issues a `run.cancel` (or equivalent existing cancellation) for the
originating run.

#### Scenario: Terminate denies all pending approvals
- **WHEN** two ApprovalCards are open and the user clicks the
  termination control
- **THEN** the H02 client SHALL send `approval.respond` with
  `decision: 'deny'` for both AND issue cancellation for the
  originating run(s)

### Requirement: Scope picker is deferred to a later slice
This slice SHALL NOT implement the scope picker (once / session /
claw / forever). Every Allow click SHALL behave as `once`. Future
slices will MODIFY this requirement to add the picker.

#### Scenario: Allow is treated as once
- **WHEN** the user clicks Allow on any approval in this slice
- **THEN** no PostureOverride is created, and the next equivalent
  tool call SHALL re-prompt
