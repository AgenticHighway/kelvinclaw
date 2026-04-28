## MODIFIED Requirements

### Requirement: Claw is a persisted workspace metadata record
The runtime SHALL persist `Claw` records with the fields `id`,
`name`, `parentClawId` (nullable string), optional `iconRef`,
optional `description`, `posture` (object), `createdAt`,
`updatedAt`. The store SHALL treat Claw as workspace metadata that
overlays the existing `SessionDescriptor` shape. The `posture`
field is an open-ended object whose only currently-defined axis is
`toolExecution: 'low' | 'medium' | 'high'`. Future axes are added
in later slices.

#### Scenario: Created claw round-trips through the store
- **WHEN** the runtime creates a Claw with `name: "Personal"` and
  `parentClawId: null` and no explicit posture
- **THEN** a subsequent `claw.list` SHALL return a record with that
  `name` and `parentClawId`, plus auto-populated `id`, `createdAt`,
  `updatedAt`, AND a `posture.toolExecution` of `'medium'`

#### Scenario: Posture round-trips
- **WHEN** the runtime creates a Claw with explicit `posture: {
  toolExecution: 'low' }`
- **THEN** a subsequent `claw.list` SHALL return that claw with
  `posture.toolExecution` of `'low'`

### Requirement: New gateway methods for claw CRUD
The kelvinclaw gateway SHALL accept four request methods,
following the existing `req`/`res` envelope and method-validation
conventions: `claw.list`, `claw.create`, `claw.update`,
`claw.delete`. `claw.create` SHALL accept an optional `posture`
parameter; if absent, defaults to `{ toolExecution: 'medium' }`.
`claw.update` SHALL accept partial `posture` patches.

#### Scenario: claw.list returns persisted claws with posture
- **WHEN** the gateway receives `req` of method `claw.list`
- **THEN** every returned claw SHALL include its `posture` field
  with `toolExecution` set

#### Scenario: claw.create with explicit posture
- **WHEN** `claw.create` is sent with `posture: { toolExecution:
  'high' }`
- **THEN** the persisted record SHALL have `posture.toolExecution
  === 'high'`

#### Scenario: claw.update patches posture
- **WHEN** `claw.update` is sent with `claw_id` and `patch: {
  posture: { toolExecution: 'low' } }`
- **THEN** the persisted record's `posture.toolExecution` SHALL
  become `'low'` and the response SHALL carry the updated record

#### Scenario: claw.delete removes claw and directory
- **WHEN** the gateway receives `req` of method `claw.delete` with
  a `claw_id` that is not the macro claw and has no children
- **THEN** the persisted record SHALL be removed, the per-claw
  directory SHALL be removed, and the response SHALL carry
  `payload: { deleted: true }`
