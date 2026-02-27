# Kelvin Gateway Protocol (v1)

`apps/kelvin-gateway` exposes a WebSocket control plane over Kelvin SDK runtime composition.

## Security Defaults

- connect-first handshake is required
- optional token auth on connect (`KELVIN_GATEWAY_TOKEN` or `--token`)
- typed request validation (`deny_unknown_fields`)
- fail-closed unknown method handling
- idempotency cache for side-effecting `agent` requests via required `request_id`

## Envelope

Client request:

```json
{
  "type": "req",
  "id": "req-1",
  "method": "connect",
  "params": {}
}
```

Server response:

```json
{
  "type": "res",
  "id": "req-1",
  "ok": true,
  "payload": {}
}
```

Server event:

```json
{
  "type": "event",
  "event": "agent",
  "payload": {}
}
```

## Handshake

First frame must be `connect`.

`connect` params:

- `auth.token` (required when gateway token is configured)
- `client_id` (optional)

## Methods

- `health`
- `agent` (alias: `run.submit`)
  - params: `request_id`, `prompt`, optional `session_id`, `workspace_dir`, `timeout_ms`, `system_prompt`, `memory_query`, `run_id`
- `agent.wait` (alias: `run.wait`)
  - params: `run_id`, optional `timeout_ms`
- `agent.state` (alias: `run.state`)
  - params: `run_id`
- `agent.outcome` (alias: `run.outcome`)
  - params: `run_id`, optional `timeout_ms`

## Idempotency

`agent` requires `request_id`.

- first submission stores acceptance metadata in the cache
- repeated submission with the same `request_id` returns the cached acceptance and `deduped: true`

## Errors

Response envelope uses:

- `ok: false`
- `error.code`
- `error.message`

Typical codes:

- `handshake_required`
- `unauthorized`
- `invalid_input`
- `not_found`
- `timeout`
- `backend_error`
- `io_error`
- `method_not_found`
