## ADDED Requirements

### Requirement: H02 connects via the existing kelvinclaw gateway
The H02 client SHALL connect to the kelvinclaw WebSocket gateway at
the URL configured by `NEXT_PUBLIC_KELVIN_GATEWAY_URL` (default
`ws://127.0.0.1:34617`) using the existing protocol envelope (`req`
/ `res` / `event`) documented in
`docs/gateway/gateway-protocol.md`.

#### Scenario: First frame is connect
- **WHEN** the H02 client opens its WebSocket connection
- **THEN** the first frame it sends SHALL be a `req` of method
  `connect`, and the client SHALL wait for the `res` containing
  `protocol_version` and `supported_methods` before sending any
  further frames

#### Scenario: Default URL on loopback
- **WHEN** `NEXT_PUBLIC_KELVIN_GATEWAY_URL` is unset
- **THEN** the H02 client SHALL default to `ws://127.0.0.1:34617`

### Requirement: Chat composer submits via the agent method
The H02 chat composer SHALL submit user messages via the existing
gateway method `agent` (alias `run.submit`) using `request_id` and
`prompt` parameters. The H02 mock chat path SHALL be disabled
whenever a live gateway connection is open.

#### Scenario: User send produces an agent submit
- **WHEN** the user types a message and sends it from the chat
  composer with a live gateway connection
- **THEN** the H02 client SHALL send a `req` of method `agent`
  with a unique `request_id` and the message text as `prompt`

#### Scenario: Mock data disabled with live connection
- **WHEN** the chat surface renders with a live gateway connection
  open
- **THEN** the existing mock chat data path SHALL NOT be invoked,
  and rendered messages SHALL come exclusively from gateway `event`
  payloads and `res` outcomes

### Requirement: Gateway events render in the chat surface
The H02 client SHALL render assistant messages and tool-result
payloads from gateway `event` frames as they arrive, in the order
they arrive, in the active chat surface for the originating
`request_id`.

#### Scenario: Assistant text renders incrementally
- **WHEN** the gateway emits a sequence of `event` frames carrying
  assistant text for a `request_id` the H02 client originated
- **THEN** the chat surface SHALL render those frames in arrival
  order, attached to the user message that originated the
  `request_id`

#### Scenario: Final outcome closes the turn
- **WHEN** the gateway emits a `res` for the originating `request_id`
  indicating completion
- **THEN** the H02 client SHALL mark the turn complete in the chat
  surface and re-enable the composer for the next message

### Requirement: Connection failure surfaces honestly
The H02 client SHALL display a non-modal "gateway not connected"
indicator when it cannot reach the gateway, and SHALL NOT silently
fall back to mock data while the indicator is shown.

#### Scenario: Gateway unreachable on connect
- **WHEN** the H02 client cannot establish a WebSocket connection
  to the configured URL
- **THEN** the chat surface SHALL display a "gateway not connected"
  indicator AND the composer SHALL NOT accept new submissions
  AND the mock chat path SHALL NOT be re-enabled

### Requirement: Reconnect on transient disconnect
The H02 client SHALL attempt to reconnect with exponential backoff
(2s, 4s, 8s, capped at 30s) when the WebSocket closes unexpectedly.
On reconnect, the client SHALL re-send `connect` as the first frame.

#### Scenario: Reconnect after transient drop
- **WHEN** the WebSocket closes with no clean shutdown
- **THEN** the H02 client SHALL retry the connection at 2s, then
  4s, then 8s, then every 30s, until either reconnected or the
  user closes the tab

### Requirement: Existing plugin model providers work end-to-end
The slice SHALL function with any installed model plugin that
satisfies the existing `wasm_model_v1` ABI in
`agentichighway/kelvinclaw-plugins/index.json`, including
`kelvin.echo` (offline) and `kelvin.anthropic` / `kelvin.openai` /
`kelvin.openrouter` (live).

#### Scenario: Offline demo with kelvin.echo
- **WHEN** the runtime has only `kelvin.echo` installed and the
  user submits the message "hi"
- **THEN** the chat surface SHALL render the assistant turn echoing
  "hi" back, sourced from gateway events

#### Scenario: Live demo with kelvin.anthropic
- **WHEN** the runtime has `kelvin.anthropic` installed with a
  valid `ANTHROPIC_API_KEY` and the user submits a real prompt
- **THEN** the chat surface SHALL render Claude's response sourced
  from gateway events
