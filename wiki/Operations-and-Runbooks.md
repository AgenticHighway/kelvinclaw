# Operations and Runbooks

This page groups the operator-facing scripts, background-service helpers, and runbooks that matter once KelvinClaw is running continuously.

## Local Daily-Driver Operations

Start the local profile:

```bash
scripts/kelvin-local-profile.sh start
```

Inspect and validate:

```bash
scripts/kelvin-local-profile.sh status
scripts/kelvin-local-profile.sh doctor
scripts/kelvin-doctor.sh
```

Stop:

```bash
scripts/kelvin-local-profile.sh stop
```

## Gateway Service Management

Release bundle (lifecycle manager with PID/log management):

```bash
./kelvin-gateway start             # daemon mode — PID file + log file
./kelvin-gateway start --foreground # run attached to terminal
./kelvin-gateway status            # pid, provider, uptime, log path
./kelvin-gateway stop
./kelvin-gateway restart
./kelvin-gateway start -- --bind 0.0.0.0:34617  # extra gateway flags after --
```

State files: `$KELVIN_HOME/gateway.pid`, `$KELVIN_HOME/logs/gateway.log`

Ad hoc daemon (dev/source tree):

```bash
scripts/kelvin-gateway-daemon.sh
```

Render or install a user service:

```bash
scripts/kelvin-gateway-service.sh render-systemd-user
scripts/kelvin-gateway-service.sh install-systemd-user
scripts/kelvin-gateway-service.sh render-launchd
scripts/kelvin-gateway-service.sh install-launchd
```

The service runner:

- sources an env file
- builds `kelvin-gateway` if needed
- starts the gateway in the foreground with persistent state

## Important Environment Variables

Secrets and credentials usually belong in the service env file:

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
- `scripts/first-run-success-rate.sh`
- `scripts/remote-test.sh`
- `scripts/run-runtime-container.sh`
- `scripts/runtime-entrypoint.sh`

## Reference

- [Gateway service management runbook](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/runbooks/kelvin-gateway-service-management.md)
- [Memory JWT key rotation runbook](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/runbooks/memory-jwt-key-rotation.md)
- [Memory denial/timeout storm runbook](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/runbooks/memory-module-denial-timeout-storms.md)
- [Module publisher trust-policy runbook](https://github.com/AgenticHighway/kelvinclaw/blob/main/docs/runbooks/module-publisher-trust-policy.md)
