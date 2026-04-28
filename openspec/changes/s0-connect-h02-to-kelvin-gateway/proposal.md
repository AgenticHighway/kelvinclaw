## Why

H02 (`https://github.com/kmondlane/h02`) is at v0.1.0 with a complete
Next.js frontend and mock data, sitting waiting to be wired to a
real backend. kelvinclaw is at v0.2.7 with a working WebSocket
gateway (`apps/kelvin-gateway`), `agent` submit semantics, channel
adapters, and a plugin system that ships model providers
(`kelvin.anthropic`, `kelvin.openai`, `kelvin.openrouter`,
`kelvin.echo`, `kelvin.ollama`) via the `agentichighway/kelvinclaw-plugins`
distribution repo.

Slice s0 is the smallest demoable step toward Kelvin: connect the
two. After this slice, a user can open H02 in their browser, type a
message, and receive a real reply from a model running through the
existing kelvinclaw runtime.

This slice introduces NO Claw, NO posture, NO sidecars, NO sub-agents.
Those each get their own slice. The point of s0 is to prove the
seam works end-to-end with the smallest possible surface area.

## What Changes

- **NEW** capability `h02-gateway-connection` — H02 implements a
  WebSocket client (modelled on `apps/kelvin-tui/src/ws_client.rs`)
  that:
  - Connects to `ws://127.0.0.1:34617` with the gateway handshake
  - Sends `connect` as the first frame, parses `supported_methods`
  - Submits chat composer messages via the existing `agent` method
  - Renders streamed `event` payloads in the chat surface
  - Disables H02's mock chat data when a real connection is up

ModelProvider selection is whatever plugin Fabro has installed via
`kpm install` — likely `kelvin.echo` for offline development or
`kelvin.anthropic` for live demos. No new plugin work is required
in this slice.

## Capabilities

### New Capabilities

- `h02-gateway-connection`: H02 connects to the kelvinclaw WebSocket gateway and uses the existing `agent` method to submit chat messages. The mock chat path is disabled when a live connection is present.

### Modified Capabilities

None — `openspec/specs/` is empty.

## Impact

- **Code (kelvinclaw)**: Zero. The gateway exposes everything
  needed already.
- **Code (H02)**: New `src/lib/kelvinGatewayClient.ts` (or similar)
  modelled on `apps/kelvin-tui/src/ws_client.rs`. New environment
  variable `NEXT_PUBLIC_KELVIN_GATEWAY_URL`. Hook in chat composer
  + chat surface to use the real client when configured.
- **Code (kelvinclaw-plugins)**: Zero. Existing model plugins
  (`kelvin.echo`, `kelvin.anthropic`) cover the demo cases.
- **APIs**: Uses the existing gateway protocol exactly as
  documented in `docs/gateway/gateway-protocol.md`.
- **Documentation**: A short H02 README addition pointing at
  `KELVIN_GATEWAY_URL` configuration.
