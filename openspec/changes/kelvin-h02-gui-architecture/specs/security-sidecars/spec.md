## ADDED Requirements

### Requirement: Two distinct sidecars
The system SHALL deploy two security sidecars: Open Bias on the model
boundary at `http://localhost:4000/v1`, and the kelvinclaw
`ToolRegistry` on the tool boundary. The two SHALL NOT be merged into
a single process.

#### Scenario: Both sidecars are reachable
- **WHEN** the runtime probes sidecar health at startup
- **THEN** it SHALL probe Open Bias `:4000/health` (HTTP 200 within
  1 second) AND verify `ToolRegistry` configuration loads cleanly

### Requirement: Per-claw RULES.md selection via header
The `ModelProvider` shim SHALL set the request header
`X-Kelvin-Claw-Rules-Ref: <claw_id>` on every outgoing model call.
Open Bias SHALL select the matching `<kelvinDataDir>/claws/<claw_id>/RULES.md`
to evaluate against. Calls without this header SHALL be rejected by
Open Bias with HTTP 400.

#### Scenario: Header set on every model call
- **WHEN** kelvinclaw makes any model call via the shim
- **THEN** the outgoing HTTP request SHALL include
  `X-Kelvin-Claw-Rules-Ref` with a value matching the calling claw's
  id

#### Scenario: Open Bias rejects missing header
- **WHEN** a request reaches Open Bias without the
  `X-Kelvin-Claw-Rules-Ref` header
- **THEN** Open Bias SHALL respond with HTTP 400

### Requirement: Posture context header
The `ModelProvider` shim SHALL also set
`X-Kelvin-Claw-Posture: <json>` on every outgoing call, encoding the
calling claw's effective `PosturePerAxis` snapshot.

#### Scenario: Posture serialised on every call
- **WHEN** kelvinclaw makes a model call
- **THEN** the request SHALL carry `X-Kelvin-Claw-Posture` as a JSON
  string parseable as `PosturePerAxis`

### Requirement: Fail-closed on Open Bias unreachable
The `ModelProvider` shim SHALL refuse model calls when Open Bias is
unreachable. It SHALL NOT fall through to the upstream provider
(Anthropic, OpenAI, OpenRouter) directly. The configuration MUST set
`fail_closed = true`; any other value SHALL cause the runtime to
refuse to start.

#### Scenario: fail_closed = false rejected at startup
- **WHEN** kelvinclaw is started with `model_provider.fail_closed =
  false`
- **THEN** the runtime SHALL refuse to start and SHALL log a
  configuration error citing `AGENTS.md` fail-closed principle

#### Scenario: Connect error returns denied-policy
- **WHEN** a model call to Open Bias fails with a connect error or
  5xx response
- **THEN** the shim SHALL return `denied-policy` to the Brain with
  detail `'open-bias-unreachable'` and SHALL NOT attempt to call the
  upstream provider directly

### Requirement: Tool gate enforces autonomy posture
The `ToolRegistry` SHALL evaluate every tool invocation against the
calling claw's effective posture, mapping the tool's properties to
the appropriate matrix axis. A tool whose properties map to multiple
axes SHALL be evaluated against the strictest of those axes.

#### Scenario: Connector write maps to connectorWrites
- **WHEN** a Connector op is invoked with `isWrite: true`
- **THEN** the gate SHALL evaluate against `connectorWrites`, not
  `toolExecution`

#### Scenario: Strictest axis wins
- **WHEN** a tool invocation maps to both `toolExecution: 'high'`
  and `connectorWrites: 'low'`
- **THEN** the gate SHALL evaluate against `connectorWrites: 'low'`

### Requirement: WASM sandbox preset selection
The kelvinclaw runtime SHALL select the WASM sandbox preset
(`locked_down`, `dev_local`, `hardware_control`) for each Skill
invocation according to the calling claw's `wasmEgress` axis.

#### Scenario: dev_local preset under Medium
- **WHEN** a WASM Skill is invoked under `wasmEgress: 'medium'`
- **THEN** the Skill SHALL be loaded with the `dev_local` preset,
  permitting writes within `kelvinDataDir` and denying outbound
  network

### Requirement: Sidecar version pinning
Both sidecars SHALL be version-pinned in deployment configuration.
The Open Bias image tag SHALL be specified explicitly in
`docker-compose.yml`; the kelvinclaw `ToolRegistry` ships with the
runtime version.

#### Scenario: docker-compose pins Open Bias
- **WHEN** the deployment compose file references Open Bias
- **THEN** the image reference SHALL include an explicit version tag
  (not `latest`)

### Requirement: OpenTelemetry trace correlation
Open Bias SHALL emit OpenTelemetry spans for each PRE_CALL, LLM_CALL,
POST_CALL phase. The `ModelProvider` shim SHALL propagate the parent
trace id, and the resulting spans' trace id SHALL be recorded in
`Receipt.otelTraceId` for the corresponding model-call Receipt.

#### Scenario: Trace id appears on Receipt
- **WHEN** Open Bias completes a model call
- **THEN** the Receipt for that call SHALL have a non-null
  `otelTraceId` field

### Requirement: Localhost-only Open Bias (v1)
The `ModelProvider` shim SHALL verify Open Bias is reachable on
loopback (`127.0.0.1` or `::1`). Remote Open Bias deployments SHALL
NOT be supported in v1.

#### Scenario: Non-loopback Open Bias rejected
- **WHEN** the shim is configured with `model_provider.base_url =
  http://10.0.0.5:4000/v1`
- **THEN** the runtime SHALL refuse to start with error
  `non-loopback-open-bias`

### Requirement: Sidecar health is observable
The runtime SHALL emit `sidecar-health` events on the gateway event
stream whenever the aggregate state transitions among `healthy`,
`degraded`, and `down`.

#### Scenario: State transition emits event
- **WHEN** the aggregate state transitions from `healthy` to
  `degraded`
- **THEN** the gateway SHALL emit one `sidecar-health` event with
  `state: 'degraded'` and a `reason` string identifying the
  failed sidecar

### Requirement: RULES.md path conventions
Per-claw `RULES.md` SHALL live at
`<kelvinDataDir>/claws/<clawId>/RULES.md`. Open Bias SHALL be
configured with the same `KELVIN_DATA_DIR` as kelvinclaw so it can
resolve the path.

#### Scenario: Open Bias resolves path from claw id
- **WHEN** Open Bias receives a request with
  `X-Kelvin-Claw-Rules-Ref: claw_personal`
- **THEN** Open Bias SHALL load and apply
  `<KELVIN_DATA_DIR>/claws/claw_personal/RULES.md`
