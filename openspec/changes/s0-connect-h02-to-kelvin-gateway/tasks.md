## 1. H02 client implementation

- [ ] 1.1 Create `src/lib/kelvinGatewayClient.ts` modelled on `apps/kelvin-tui/src/ws_client.rs` from kelvinclaw — covering: open WebSocket; send `connect` first; handle `req`/`res`/`event` envelope; expose `submitAgent({ prompt }) → AsyncIterable<event>`.
- [ ] 1.2 Add `NEXT_PUBLIC_KELVIN_GATEWAY_URL` env handling with `ws://127.0.0.1:34617` default.
- [ ] 1.3 Implement reconnect-with-backoff (2s, 4s, 8s, capped 30s).
- [ ] 1.4 Add a connection-state indicator (`disconnected | connecting | connected`) exposed via Zustand store.

## 2. Wire chat composer + chat surface to the gateway

- [ ] 2.1 Replace mock-data submit path in the chat composer with a call into `kelvinGatewayClient.submitAgent()`.
- [ ] 2.2 Adapt incoming `event` payloads to the existing chat-message render shape (assistant turn, tool result, etc.).
- [ ] 2.3 Disable mock chat data path when `connectionState === 'connected'`.
- [ ] 2.4 Render "gateway not connected" indicator when `disconnected`; lock the composer.

## 3. Developer onboarding

- [ ] 3.1 Add a section to H02's README explaining how to start kelvinclaw locally (`kelvin-host` or `kelvin-cli`) and which model plugin to install (`kpm install kelvin.echo` for offline; `kpm install kelvin.anthropic` for live).
- [ ] 3.2 Document `NEXT_PUBLIC_KELVIN_GATEWAY_URL` in `.env.example`.

## 4. Verification

- [ ] 4.1 Run kelvinclaw locally with `kelvin.echo` installed; open H02; send "hi"; verify the chat surface shows the echoed reply sourced from a gateway event.
- [ ] 4.2 Run with `kelvin.anthropic` installed (live API key); verify a real Claude response renders.
- [ ] 4.3 Kill kelvinclaw mid-session; verify the disconnected indicator appears and the composer locks.
- [ ] 4.4 Restart kelvinclaw; verify the H02 client reconnects and accepts new messages.
- [ ] 4.5 Verify the mock chat data path is NOT invoked while a live connection is open (manual code-path inspection or a feature-flag gate).

## 5. Archive

- [ ] 5.1 Once all tasks above are complete and a 5-minute end-to-end demo is recorded, run `openspec archive s0-connect-h02-to-kelvin-gateway`. Capability lifts into `openspec/specs/h02-gateway-connection/spec.md` for s1 to MODIFY.
