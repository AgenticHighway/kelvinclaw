## MODIFIED Requirements

### Requirement: Chat composer submits via the agent method
The H02 chat composer SHALL submit user messages via the existing
gateway method `agent` (alias `run.submit`) using `request_id`,
`prompt`, and `claw_id` parameters. The H02 mock chat path SHALL be
disabled whenever a live gateway connection is open.

#### Scenario: User send produces an agent submit with claw_id
- **WHEN** the user types a message with claw "Personal" active and
  sends it from the chat composer with a live gateway connection
- **THEN** the H02 client SHALL send a `req` of method `agent`
  with a unique `request_id`, the message text as `prompt`, and
  `claw_id` matching the active claw's id

#### Scenario: Mock data disabled with live connection
- **WHEN** the chat surface renders with a live gateway connection
  open
- **THEN** the existing mock chat data path SHALL NOT be invoked,
  and rendered messages SHALL come exclusively from gateway `event`
  payloads and `res` outcomes

## ADDED Requirements

### Requirement: H02 loads claw list from the gateway
The H02 frame navigation SHALL render the user's claws sourced from
`claw.list`, NOT from H02's mock `spaces` data. Switching the active
claw SHALL update which `claw_id` is sent on subsequent `agent`
submits.

#### Scenario: Claw list rendered from gateway
- **WHEN** the H02 client connects with a live gateway
- **THEN** the frame's claw switcher SHALL display the claws
  returned by `claw.list`, replacing any mock `spaces` rendering

#### Scenario: Switch claw changes submit target
- **WHEN** the user switches the active claw from "Personal" to
  "Work"
- **THEN** the next `agent` submit SHALL carry `claw_id` matching
  Work's id
