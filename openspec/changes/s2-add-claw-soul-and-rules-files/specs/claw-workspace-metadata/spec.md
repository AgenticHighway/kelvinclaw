## MODIFIED Requirements

### Requirement: New gateway methods for claw CRUD
The kelvinclaw gateway SHALL accept four new request methods,
following the existing `req`/`res` envelope and method-validation
conventions: `claw.list`, `claw.create`, `claw.update`,
`claw.delete`. `claw.create` SHALL additionally create the per-claw
directory and seed `soul.md` and `RULES.md`. `claw.delete` SHALL
additionally remove the per-claw directory and contents.

#### Scenario: claw.list returns persisted claws
- **WHEN** the gateway receives `req` of method `claw.list`
- **THEN** the gateway SHALL respond with `res` carrying `payload:
  { claws: [...] }` containing every persisted Claw record

#### Scenario: claw.create persists, returns the new claw, and seeds files
- **WHEN** the gateway receives `req` of method `claw.create` with
  params `{ name, parentClawId, iconRef?, description? }`
- **THEN** the gateway SHALL persist the claw, create the per-claw
  directory with seeded `soul.md` and `RULES.md`, respond with
  `res` carrying `payload: { claw: <created record> }`, and the
  next `claw.list` SHALL include it

#### Scenario: claw.update mutates persisted record
- **WHEN** the gateway receives `req` of method `claw.update` with
  `claw_id` and a partial patch
- **THEN** the persisted record SHALL be updated with the patch,
  `updatedAt` SHALL be refreshed, and the response SHALL carry the
  updated record

#### Scenario: claw.delete removes claw and directory
- **WHEN** the gateway receives `req` of method `claw.delete` with
  a `claw_id` that is not the macro claw and has no children
- **THEN** the persisted record SHALL be removed, the per-claw
  directory SHALL be removed (including `soul.md` and `RULES.md`),
  and the response SHALL carry `payload: { deleted: true }`
