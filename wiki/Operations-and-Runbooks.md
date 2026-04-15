# Operations and Runbooks

This page groups the operator-facing commands, background-service helpers, and runbooks that matter once KelvinClaw is running continuously.

## Local Daily-Driver Operations

Start the local profile:

```bash
scripts/kelvin-dev-stack.sh start
```

Inspect and validate:

```bash
scripts/kelvin-dev-stack.sh status
scripts/kelvin-dev-stack.sh doctor
kelvin doctor
```

Stop:

```bash
scripts/kelvin-dev-stack.sh stop
```

## Gateway Service Management

Release bundle (lifecycle manager with PID/log management):

```bash
kelvin gateway start             # daemon mode — PID file + log file
kelvin gateway start --foreground # run attached to terminal
kelvin gateway status            # pid, provider, uptime, log path
kelvin gateway stop
kelvin gateway restart
kelvin gateway start -- --bind 0.0.0.0:34617  # extra gateway flags after --
```

State files: `$KELVIN_HOME/gateway.pid`, `$KELVIN_HOME/logs/gateway.log`

Render or install a user service:

```bash
kelvin service render-systemd
kelvin service install-systemd
kelvin service render-launchd
kelvin service install-launchd
```

The service runner:

- reads configuration from `~/.kelvinclaw/.env`
- starts the gateway in the foreground with persistent state

## Important Environment Variables

Secrets and credentials belong in `~/.kelvinclaw/.env`:

- `KELVIN_GATEWAY_TOKEN`
- `KELVIN_GATEWAY_TLS_CERT_PATH`
- `KELVIN_GATEWAY_TLS_KEY_PATH`
- `KELVIN_TELEGRAM_WEBHOOK_SECRET_TOKEN`
- `KELVIN_SLACK_SIGNING_SECRET`
- `KELVIN_DISCORD_INTERACTIONS_PUBLIC_KEY`

Common runtime defaults:

- `KELVIN_GATEWAY_WORKSPACE`
- `KELVIN_GATEWAY_STATE_DIR`
- `KELVIN_GATEWAY_BIND`
- `KELVIN_GATEWAY_INGRESS_BIND`

## Cache Hygiene

Shared Docker caches live under `./.cache/docker`.

Inspect and prune:

```bash
scripts/docker-cache-prune.sh --dry-run
scripts/docker-cache-prune.sh --max-age-days 14
```

## Runbooks

- gateway service lifecycle and environment model
- JWT signing key rotation for memory RPC
- memory-module denial or timeout storm response
- module publisher trust-policy operations

## Useful Operational Scripts

- `scripts/memory-rollout-check.sh`

## Reference

- [Gateway service management runbook](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/runbooks/kelvin-gateway-service-management.md)
- [Memory JWT key rotation runbook](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/runbooks/memory-jwt-key-rotation.md)
- [Memory denial/timeout storm runbook](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/runbooks/memory-module-denial-timeout-storms.md)
- [Module publisher trust-policy runbook](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/runbooks/module-publisher-trust-policy.md)
