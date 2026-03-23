# Docker Setup for KelvinClaw

This repository includes Docker Compose configuration for running KelvinClaw services.

## Quick Start

### Basic Setup (Single Service)

```bash
# Copy environment template
cp .env.example .env

# Build and start kelvin-host
docker compose up kelvin-host
```

This starts the main `kelvin-host` service, which provides the core runtime.

### Full Setup (All Services)

```bash
# Start gateway, host, and registry
docker-compose --profile full up
```

## Services

### Core Services (always started)

- **kelvin-host**: The main Kelvin agent runtime. Interactive CLI — invoke with `docker-compose run kelvin-host kelvin-host --prompt "..."`. Not an HTTP service; exposes no ports.

- **kelvin-gateway**: WebSocket gateway for agent channel ingress.
  - Port: `34617` (WebSocket, configurable via `KELVIN_GATEWAY_WS_PORT`)
  - Port: `34618` (ingress bind, configurable via `KELVIN_GATEWAY_INGRESS_PORT`)

### Optional Services (Profile: `registry` or `full`)

- **kelvin-registry**: HTTP plugin registry for plugin discovery.
  - Port: `34718` (configurable via `KELVIN_PLUGIN_REGISTRY_PORT`)

### TUI Client (Profile: `tui`)

- **kelvin-tui**: Terminal UI client that connects to `kelvin-gateway` over the internal Docker network.
  - Usage: `docker-compose --profile tui run --rm kelvin-tui`

### Test Runner (Profile: `test`)

- **kelvin-test**: Tests execute during `docker-compose build`, not at container runtime.
  - Build: `docker-compose build kelvin-test`
  - Quick lane: `KELVIN_TEST_LANE=quick docker-compose build kelvin-test`

## Configuration

Edit `.env` to customize:

```bash
RUST_LOG=debug                      # Set logging level (debug, info, warn, error)
KELVIN_GATEWAY_WS_PORT=34617        # Change gateway WebSocket port
KELVIN_GATEWAY_INGRESS_PORT=34618   # Change gateway ingress port
KELVIN_PLUGIN_REGISTRY_PORT=34718   # Change registry port
```

### Model Provider

Set `KELVIN_MODEL_PROVIDER` and the corresponding API key in `.env`:

| Provider | `KELVIN_MODEL_PROVIDER` | API key env var |
|---|---|---|
| Echo (default, no key needed) | `kelvin.echo` | — |
| Anthropic | `kelvin.anthropic` | `ANTHROPIC_API_KEY` |
| OpenRouter | `kelvin.openrouter` | `OPENROUTER_API_KEY` |

The selected provider's plugin is installed automatically from the image at startup.
No external index or network access is required for first-party providers.

### Community Plugins

To install a community plugin at startup, set:

```bash
KELVIN_PLUGIN_INDEX_URL=https://your-host/index.json
```

The index must follow the v1 schema (`schema_version: "v1"`). See
`docs/plugins/plugin-index-schema.md` for the format.

## Common Commands

### Start kelvin-host in the foreground

```bash
docker compose up kelvin-host
```

### Start all services in the background

```bash
docker compose --profile full up -d
```

### Start the TUI client

```bash
docker-compose --profile tui run --rm kelvin-tui
```

### View logs

```bash
# All services
docker compose logs -f

# Specific service
docker compose logs -f kelvin-host

# Follow new logs
docker compose logs -f --tail=50
```

### Stop services

```bash
docker compose down
```

### Remove volumes (clean slate)

```bash
docker compose down -v
```

### Run tests in container

```bash
docker-compose build kelvin-test
```

### Quick test lane

```bash
# Inline:
KELVIN_TEST_LANE=quick docker-compose build kelvin-test

# Or export first:
export KELVIN_TEST_LANE=quick
docker-compose build kelvin-test
```

### Interactive shell in running container

```bash
docker compose exec kelvin-host bash
```

## Development Workflow

### With Live Code Changes

For development, you can mount your local source and rebuild:

```bash
# Build fresh (without cache)
docker compose build --no-cache kelvin-host

# Rebuild and start
docker compose up --build kelvin-host
```

### Volumes

- `kelvin-home`: Persistent plugin and configuration data
- `kelvin-workspace`: Agent workspace data

## Troubleshooting

### Container won't start

Check logs:
```bash
docker compose logs kelvin-host
```

### Port already in use

Change the port mapping in `.env`:
```bash
KELVIN_GATEWAY_WS_PORT=34619       # Use 34619 instead of 34617
KELVIN_GATEWAY_INGRESS_PORT=34620  # Use 34620 instead of 34618
```

### Memory issues during build

Increase Docker's memory limit in Docker Desktop settings, or use:
```bash
docker compose build --memory 4g kelvin-host
```

### Clear everything and start fresh

```bash
docker compose down -v --remove-orphans
docker system prune -a
docker compose up --build kelvin-host
```

## Network

Services are connected via the `kelvin-network` bridge network, allowing them to communicate using service names:
- `kelvin-gateway:34617` (WebSocket)
- `kelvin-gateway:34618` (ingress)
- `kelvin-registry:34718`

## Building Without Compose

To build just the runtime image:

```bash
docker build -f docker/Dockerfile.runtime -t kelvin-host:latest .
```

To run it:

```bash
docker run -it --rm \
  -e RUST_LOG=info \
  -v kelvin-home:/kelvin \
  kelvin-host:latest kelvin-host --prompt "hello"
```
