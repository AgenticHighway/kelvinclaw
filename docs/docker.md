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
# Start all services including memory API, gateway, and registry
docker compose --profile full up
```

## Services

### Core Service

- **kelvin-host**: The main Kelvin agent runtime. Handles request execution, memory management, and plugin loading.
  - Port: `8080` (configurable via `HOST_PORT`)
  - Health check: `GET /health`

### Additional Services (Profile: `full`)

- **kelvin-memory-api**: gRPC API for the memory backend
  - Port: `50051` (configurable via `MEMORY_API_PORT`)

- **kelvin-gateway**: Multi-channel ingress gateway (Discord, Telegram, Slack, UI)
  - Port: `3000` (configurable via `GATEWAY_PORT`)

- **kelvin-registry**: Plugin registry service
  - Port: `8888` (configurable via `REGISTRY_PORT`)

### Development Service (Profile: `test`)

- **kelvin-test**: Runs tests in a containerized environment
  - Mounts workspace for live test execution

## Configuration

Edit `.env` to customize:

```bash
RUST_LOG=debug           # Set logging level (debug, info, warn, error)
HOST_PORT=8080          # Change host port
GATEWAY_PORT=3000       # Change gateway port
REGISTRY_PORT=8888      # Change registry port
```

## Common Commands

### Start kelvin-host in the foreground

```bash
docker compose up kelvin-host
```

### Start all services in the background

```bash
docker compose --profile full up -d
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
docker compose run --rm kelvin-test cargo test
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
- `kelvin-plugins`: Plugin storage
- `memory-storage`: Memory backend data (if using persistent storage)
- `cargo-cache`: Cargo dependency cache (for test service)

## Troubleshooting

### Container won't start

Check logs:
```bash
docker compose logs kelvin-host
```

### Port already in use

Change the port mapping in `.env`:
```bash
HOST_PORT=8081  # Use 8081 instead of 8080
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
- `kelvin-host:8080`
- `kelvin-gateway:3000`
- `kelvin-registry:8888`
- `kelvin-memory-api:50051`

## Building Without Compose

To build just the runtime image:

```bash
docker build -f docker/Dockerfile.runtime -t kelvin-host:latest .
```

To run it:

```bash
docker run -it --rm \
  -e RUST_LOG=info \
  -p 8080:8080 \
  -v kelvin-home:/kelvin \
  kelvin-host:latest
```
