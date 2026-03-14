# Gateway and Operator Guide

`kelvin-gateway` is the secure control-plane surface for WebSocket clients, direct channel ingress, scheduler visibility, and operator monitoring.

## Default Transport Model

- WebSocket default: `ws://127.0.0.1:34617`
- Direct HTTP ingress is separate and disabled until explicitly configured
- Non-loopback plaintext binds fail closed unless explicitly overridden
- Public binds require authentication

Recommended local start:

```bash
KELVIN_GATEWAY_TOKEN=change-me cargo run -p kelvin-gateway -- --bind 127.0.0.1:34617 --workspace "$PWD"
```

Enable direct ingress and the operator console:

```bash
KELVIN_GATEWAY_TOKEN=change-me \
KELVIN_GATEWAY_INGRESS_BIND=127.0.0.1:34618 \
KELVIN_TELEGRAM_WEBHOOK_SECRET_TOKEN=telegram-secret \
KELVIN_SLACK_SIGNING_SECRET=slack-signing-secret \
KELVIN_DISCORD_INTERACTIONS_PUBLIC_KEY=<hex-public-key> \
cargo run -p kelvin-gateway -- --bind 127.0.0.1:34617 --workspace "$PWD"
```

## Security Defaults

- connect-first handshake
- optional connect token auth
- typed request validation with fail-closed unknown method handling
- idempotent side-effecting requests via required `request_id`
- bounded connection count and message/frame sizes
- auth failure backoff by client IP
- channel adapters disabled unless explicitly enabled
- channel-specific secrets and allowlists

## Direct Ingress Surfaces

Base path defaults to `/ingress`.

Routes:

- `POST /ingress/telegram`
- `POST /ingress/slack`
- `POST /ingress/discord`

Operator console:

- `GET /operator/`

Ingress base path and limits:

- `KELVIN_GATEWAY_INGRESS_BASE_PATH`
- `KELVIN_GATEWAY_INGRESS_MAX_BODY_BYTES`
- `KELVIN_GATEWAY_INGRESS_BIND`

## Supported RPC Methods

Core:

- `health`
- `agent`
- `agent.wait`
- `agent.state`
- `agent.outcome`

Channels:

- `channel.telegram.ingest`
- `channel.telegram.pair.approve`
- `channel.telegram.status`
- `channel.slack.ingest`
- `channel.slack.status`
- `channel.discord.ingest`
- `channel.discord.status`
- `channel.route.inspect`

Operator:

- `operator.runs.list`
- `operator.sessions.list`
- `operator.session.get`
- `operator.plugins.inspect`

Scheduler:

- `schedule.list`
- `schedule.history`

## Channel Policy Highlights

Telegram:

- disabled unless `KELVIN_TELEGRAM_ENABLED=true`
- webhook verification via `KELVIN_TELEGRAM_WEBHOOK_SECRET_TOKEN`
- pairing required by default
- allowlist and host controls

Slack:

- disabled unless `KELVIN_SLACK_ENABLED=true`
- signing verification via `KELVIN_SLACK_SIGNING_SECRET`
- replay window enforcement via `KELVIN_SLACK_WEBHOOK_REPLAY_WINDOW_SECS`

Discord:

- disabled unless `KELVIN_DISCORD_ENABLED=true`
- interaction verification via `KELVIN_DISCORD_INTERACTIONS_PUBLIC_KEY`

All three inherit bounded dedupe, retry, deny, and connectivity tracking.

## Operator Console Coverage

The operator console currently surfaces:

- gateway overview and security posture
- channel ingress and delivery state
- run ledger and run inspection
- sessions and session detail
- scheduler list and history
- plugin inventory, trust policy, and registry configuration

## Routing and Health

Channel routing is driven by `KELVIN_CHANNEL_ROUTING_RULES_JSON`, with deterministic priority ordering and tie-breaking.

Per-channel status includes:

- verification state
- connectivity state
- retry counters
- deny counters
- ingest, dedupe, rate-limit, and outbound state

## Service Management

For long-running deployments, use:

- [Operations and Runbooks](Operations-and-Runbooks)

## Reference

- [Gateway protocol source doc](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/gateway-protocol.md)
- [Gateway service management runbook](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/runbooks/kelvin-gateway-service-management.md)
