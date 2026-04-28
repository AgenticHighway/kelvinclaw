## ADDED Requirements

### Requirement: Claw is a persisted workspace metadata record
The runtime SHALL persist `Claw` records with the fields `id`,
`name`, `parentClawId` (nullable string), optional `iconRef`,
optional `description`, `createdAt`, `updatedAt`. The store SHALL
treat Claw as workspace metadata that overlays the existing
`SessionDescriptor` shape.

#### Scenario: Created claw round-trips through the store
- **WHEN** the runtime creates a Claw with `name: "Personal"` and
  `parentClawId: null`
- **THEN** a subsequent `claw.list` SHALL return a record with that
  `name` and `parentClawId`, plus auto-populated `id`, `createdAt`,
  `updatedAt`

### Requirement: Exactly one macro claw per install
The store SHALL enforce that exactly one persisted Claw has
`parentClawId === null` at any time. Mutations that would produce
zero or two macro claws SHALL be rejected with error
`macro-claw-invariant`.

#### Scenario: Cannot delete the only macro claw
- **WHEN** an install has exactly one macro claw and a
  `claw.delete` is issued for it
- **THEN** the store SHALL reject with `macro-claw-invariant`

#### Scenario: Cannot create a second macro claw
- **WHEN** a macro claw exists and `claw.create` is issued with
  `parentClawId: null`
- **THEN** the store SHALL reject with `macro-claw-invariant`

### Requirement: parentClawId references a real claw
The store SHALL reject any `Claw` mutation where `parentClawId` is
non-null but does not reference an existing Claw `id`.

#### Scenario: Orphan parent rejected
- **WHEN** `claw.create` is issued with `parentClawId: "claw_doesnotexist"`
- **THEN** the store SHALL reject with `unknown-parent-claw`

### Requirement: New gateway methods for claw CRUD
The kelvinclaw gateway SHALL accept four new request methods,
following the existing `req`/`res` envelope and method-validation
conventions: `claw.list`, `claw.create`, `claw.update`,
`claw.delete`.

#### Scenario: claw.list returns persisted claws
- **WHEN** the gateway receives `req` of method `claw.list`
- **THEN** the gateway SHALL respond with `res` carrying `payload:
  { claws: [...] }` containing every persisted Claw record

#### Scenario: claw.create persists and returns the new claw
- **WHEN** the gateway receives `req` of method `claw.create` with
  params `{ name, parentClawId, iconRef?, description? }`
- **THEN** the gateway SHALL persist the claw, respond with `res`
  carrying `payload: { claw: <created record> }`, and the next
  `claw.list` SHALL include it

#### Scenario: claw.update mutates persisted record
- **WHEN** the gateway receives `req` of method `claw.update` with
  `claw_id` and a partial patch
- **THEN** the persisted record SHALL be updated with the patch,
  `updatedAt` SHALL be refreshed, and the response SHALL carry the
  updated record

#### Scenario: claw.delete removes claw
- **WHEN** the gateway receives `req` of method `claw.delete` with
  a `claw_id` that is not the macro claw and has no children
- **THEN** the persisted record SHALL be removed and the response
  SHALL carry `payload: { deleted: true }`

### Requirement: Claws cannot be deleted if they have children
The store SHALL reject `claw.delete` when the target claw has any
child claws (records whose `parentClawId === target.id`).

#### Scenario: Parent with children rejected
- **WHEN** claw A is the parent of claw B, and `claw.delete` is
  issued for A
- **THEN** the store SHALL reject with `claw-has-children` and
  list the blocking child ids in the error detail

### Requirement: agent submits accept claw_id
The existing `agent` (alias `run.submit`) gateway method SHALL
accept an optional `claw_id` parameter. When present, the resulting
session SHALL be associated with that claw.

#### Scenario: Submit with claw_id
- **WHEN** an `agent` submit is sent with `claw_id: "claw_personal"`
- **THEN** the resulting `session_id` SHALL be associated with that
  `claw_id` in the runtime, and operator inspection methods that
  list sessions SHALL surface the association

#### Scenario: Submit without claw_id falls back to macro claw
- **WHEN** an `agent` submit is sent with no `claw_id`
- **THEN** the resulting session SHALL be associated with the macro
  claw (the unique claw with `parentClawId === null`)

### Requirement: Macro claw is created on first runtime start
The runtime SHALL create a default macro claw with `name: "Kelvin"`,
`parentClawId: null`, on first start when no claws are persisted.

#### Scenario: Empty store gets a macro claw
- **WHEN** the runtime starts against a fresh data directory with
  no persisted Claw records
- **THEN** the runtime SHALL create one Claw with `name: "Kelvin"`
  and `parentClawId: null` before accepting `agent` submits
