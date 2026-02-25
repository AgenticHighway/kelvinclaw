# Memory Controller Deployment Profiles

## Build Profiles

Controller exposes feature-based provider profiles:

- `profile_minimal`: `provider_sqlite` (+ in-memory runtime provider)
- `profile_iphone`: `provider_sqlite`, `provider_object_store`, `provider_vector_metal`
- `profile_linux_gpu`: `provider_sqlite`, `provider_object_store`, `provider_vector_nvidia`

Examples:

```bash
cargo build -p kelvin-memory-controller --no-default-features --features profile_minimal
cargo build -p kelvin-memory-controller --no-default-features --features profile_iphone
cargo build -p kelvin-memory-controller --no-default-features --features profile_linux_gpu
```

## Runtime Configuration

Controller environment:

- `KELVIN_MEMORY_CONTROLLER_ADDR`
- `KELVIN_MEMORY_PUBLIC_KEY_PEM`
- `KELVIN_MEMORY_ISSUER`
- `KELVIN_MEMORY_AUDIENCE`
- `KELVIN_MEMORY_PROFILE`
- `KELVIN_MEMORY_CLOCK_SKEW_SECS`
- `KELVIN_MEMORY_REPLAY_WINDOW_SECS`
- `KELVIN_MEMORY_DEFAULT_TIMEOUT_MS`
- `KELVIN_MEMORY_DEFAULT_FUEL`
- `KELVIN_MEMORY_MAX_MODULE_BYTES`
- `KELVIN_MEMORY_MAX_MEMORY_PAGES`
- `KELVIN_MEMORY_DEFAULT_MAX_RESPONSE_BYTES`

## Profile Guarantees

- iPhone profile excludes NVIDIA vector feature.
- Linux GPU profile includes NVIDIA vector feature.
- minimal profile stays small and excludes GPU-specialized providers.

## Module Admission

Module registration fails fast when `required_host_features` are unavailable in the current build profile.

## Operations

Runbooks:

- `docs/runbooks/memory-jwt-key-rotation.md`
- `docs/runbooks/module-publisher-trust-policy.md`
- `docs/runbooks/memory-module-denial-timeout-storms.md`
