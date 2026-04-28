## ADDED Requirements

### Requirement: Per-claw filesystem directory layout
The runtime SHALL maintain a directory at
`<KELVIN_DATA_DIR>/claws/<claw_id>/` for every persisted Claw. The
directory SHALL contain at minimum two files: `soul.md` and
`RULES.md`.

#### Scenario: Directory created on claw.create
- **WHEN** `claw.create` succeeds with the resulting `claw_id`
- **THEN** the runtime SHALL create the directory
  `<KELVIN_DATA_DIR>/claws/<claw_id>/` (if absent) and write both
  `soul.md` and `RULES.md` with default content

#### Scenario: Directory removed on claw.delete
- **WHEN** `claw.delete` succeeds for a `claw_id`
- **THEN** the runtime SHALL remove the directory
  `<KELVIN_DATA_DIR>/claws/<claw_id>/` and its contents

### Requirement: Default soul.md content
The runtime SHALL seed a new claw's `soul.md` with a minimal default
that uses the claw's `name`. The default SHALL contain the claw's
`name` at least once and SHALL be valid markdown describing
identity and style placeholders.

#### Scenario: Default soul mentions claw name
- **WHEN** a claw is created with `name: "Personal"`
- **THEN** the resulting `soul.md` SHALL contain the string
  "Personal" at least once and SHALL be valid markdown

### Requirement: Default RULES.md is a stub
The runtime SHALL seed a new claw's `RULES.md` with a placeholder
header indicating the file exists for future policy enforcement.
The stub SHALL NOT contain any active rules.

#### Scenario: Default RULES.md is empty of rules
- **WHEN** a claw is created
- **THEN** the resulting `RULES.md` SHALL contain a placeholder
  header (for example `# Rules for {name}`) AND no other rule-like
  sections

### Requirement: Soul read/write gateway methods
The kelvinclaw gateway SHALL accept two methods, `claw.soul.read`
and `claw.soul.write`, that read and write the `soul.md` of a
specified `claw_id`.

#### Scenario: Read returns current soul content
- **WHEN** the gateway receives `req` of method `claw.soul.read`
  with params `{ claw_id }`
- **THEN** the gateway SHALL respond with `payload: { content: <utf-8 text> }`
  containing the current contents of that claw's `soul.md`

#### Scenario: Write replaces soul content
- **WHEN** the gateway receives `req` of method `claw.soul.write`
  with params `{ claw_id, content }`
- **THEN** the runtime SHALL replace the file's contents with
  `content`, respond with `payload: { written: true }`, and the
  next `claw.soul.read` SHALL return the new content

### Requirement: Rules read/write gateway methods
The kelvinclaw gateway SHALL accept two methods, `claw.rules.read`
and `claw.rules.write`, with the same shape as the soul methods.

#### Scenario: Read returns current rules content
- **WHEN** the gateway receives `req` of method `claw.rules.read`
  with `{ claw_id }`
- **THEN** the response SHALL carry `payload: { content }` with the
  current contents of that claw's `RULES.md`

#### Scenario: Write replaces rules content
- **WHEN** the gateway receives `req` of method `claw.rules.write`
  with `{ claw_id, content }`
- **THEN** the runtime SHALL persist the new content and the next
  read SHALL return it

### Requirement: Brain seeds claw-specific system prompt from soul.md
The kelvin-brain SHALL, on every `agent` turn associated with a
`claw_id`, read that claw's `soul.md` and use its content as the
system prompt seed for the model call. If `soul.md` cannot be read,
the brain SHALL fall back to the existing default system prompt and
SHALL emit a warning event.

#### Scenario: Custom soul changes assistant tone
- **WHEN** a claw's `soul.md` is rewritten to "You speak only in
  haiku" and an `agent` turn is submitted with that `claw_id`
- **THEN** the model call's `system_prompt` SHALL include the new
  soul content, and the assistant response SHALL exhibit the
  haiku constraint (within model adherence)

#### Scenario: Missing soul.md falls back gracefully
- **WHEN** a claw's `soul.md` is unreadable (file removed, perms
  error)
- **THEN** the brain SHALL use the runtime's default system prompt
  and SHALL emit an `event` of kind `warning` with detail
  `soul-md-unreadable` and the offending `claw_id`

### Requirement: RULES.md exists but is not enforced in this slice
The runtime SHALL persist `RULES.md` per claw and accept reads /
writes to it, but SHALL NOT enforce its content during `agent`
turns in this slice. Enforcement is introduced in a later slice
(s6) via a model-boundary sidecar profile. The file's existence
serves as a forward-compatibility anchor.

#### Scenario: RULES.md content does not affect agent output today
- **WHEN** a claw's `RULES.md` contains a rule like "never mention
  bananas" and an `agent` turn is submitted asking about bananas
- **THEN** the assistant response SHALL be whatever the model would
  produce ignoring the rules file (RULES.md is not consulted in
  this slice)

#### Scenario: RULES.md is preserved across runtime restarts
- **WHEN** `claw.rules.write` persists content for a claw and the
  runtime restarts
- **THEN** a subsequent `claw.rules.read` SHALL return exactly the
  written content
