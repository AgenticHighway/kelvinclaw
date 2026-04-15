# Kelvin Gateway Service Management

KelvinClaw ships two supported service-management layers for `kelvin-gateway`:

- ad hoc local background management via `kelvin gateway start/stop/status`
- user-service definitions for `systemd` and `launchd` via `kelvin service`

## Ad Hoc Background Management

Start the gateway in daemon mode:

```bash
kelvin gateway start
```

Run attached to the terminal (foreground):

```bash
kelvin gateway start --foreground
```

Status, restart, and stop:

```bash
kelvin gateway status
kelvin gateway restart
kelvin gateway stop
```

Pass extra flags to the gateway binary after `--`:

```bash
kelvin gateway start -- --bind 0.0.0.0:34617
```

State files: `$KELVIN_HOME/gateway.pid`, `$KELVIN_HOME/logs/gateway.log`

## systemd user service

Render the unit:

```bash
kelvin service render-systemd
```

Install the unit into `~/.config/systemd/user/kelvin-gateway.service`:

```bash
kelvin service install-systemd
systemctl --user daemon-reload
systemctl --user enable --now kelvin-gateway.service
```

## launchd user agent

Render the plist:

```bash
kelvin service render-launchd
```

Install the LaunchAgent into `~/Library/LaunchAgents/dev.kelvinclaw.gateway.plist`:

```bash
kelvin service install-launchd
launchctl bootout gui/$(id -u) dev.kelvinclaw.gateway 2>/dev/null || true
launchctl bootstrap gui/$(id -u) "$HOME/Library/LaunchAgents/dev.kelvinclaw.gateway.plist"
```

If installed via Homebrew, `brew services start kelvin` is also available.

## Environment model

Put secrets and channel credentials in `~/.kelvinclaw/.env` (or `KELVIN_HOME/.env`), for example:

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

## Cache hygiene

Shared Docker build caches live under `./.cache/docker`. To keep them bounded:

```bash
scripts/docker-cache-prune.sh --dry-run
scripts/docker-cache-prune.sh --max-age-days 14
```
