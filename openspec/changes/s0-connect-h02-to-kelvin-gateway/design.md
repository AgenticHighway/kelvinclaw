## Context

H02 is a Next.js 14 / React 18 / Zustand 4 application. Its existing
chat surface is wired to mock data in `src/lib/mock*` files. The
kelvinclaw runtime exposes a WebSocket gateway at
`ws://127.0.0.1:34617` (loopback default) with documented envelope,
handshake, and the `agent` (alias `run.submit`) method for submitting
LLM turns. The TUI app at `apps/kelvin-tui/src/ws_client.rs` is a
working reference client.

ModelProvider plugins (`kelvin.anthropic`, `kelvin.openai`,
`kelvin.openrouter`, `kelvin.echo`, `kelvin.ollama`) ship signed via
`agentichighway/kelvinclaw-plugins` and install with
`kpm install <id>`. `kelvin.echo` is the obvious offline development
plugin since it requires no API key and no network egress.

## Goals / Non-Goals

**Goals:**
- Wire H02 to a real backend with the smallest possible diff.
- Use the existing gateway protocol exactly as-documented; no new
  methods, no new envelope shapes.
- Keep `kelvin.echo` available as the zero-dependency demo path.

**Non-Goals:**
- No Claw concept yet (s1).
- No Soul / Rules / per-claw config (s2).
- No autonomy posture or approvals (s3).
- No Mind tabs beyond chat (s4).
- No sub-claw delegation (s5).
- No Open Bias (s6).

## Decisions

### D1. Use the existing `agent` method, not new H02-specific methods

The kelvinclaw gateway already accepts `agent` with `request_id`,
`prompt`, optional `session_id`, `workspace_dir`, `timeout_ms`,
`system_prompt`, `memory_query`, `run_id`. That covers the s0 chat
turn fully.

**Alternative:** add a new `claw.send-message` method tuned for H02.
**Rejected because:** that's a future-slice concern (s1+ when claws
exist). For s0 there's no claw, so `agent` is sufficient and correct.

### D2. Default to loopback; no auth in s0

The gateway runs on loopback by default (`KELVIN_GATEWAY_TOKEN`
unset = no auth required for loopback). H02 in dev mode runs on the
same machine. v1 trust boundary is filesystem ownership (per
ADR-004 in earlier exploration).

**Alternative:** require a token from day one.
**Rejected because:** premature; would block Fabro on operational
config that's a v0.6+ concern.

### D3. Reconnect-with-backoff but no event-stream resume in s0

The gateway has no `seq` numbers in its current `event` envelope
(it's name-keyed: `agent.delta`, `agent.outcome`, etc.). Resuming
mid-stream isn't part of the existing protocol. For s0, on
reconnect the chat surface accepts that any in-flight turn may be
lost (an honest "gateway dropped — retry your message" prompt).

**Alternative:** introduce `seq` + resume now.
**Rejected because:** that's a protocol change; out of scope. Can be
added later as a `MODIFIED` requirement against s0's spec.

## Risks / Trade-offs

[Risk: dropped turns on reconnect surprise users] → mitigation:
explicit toast / inline indicator showing "connection dropped — last
message may be incomplete." Better UX added in later slices.

[Risk: H02 mock-data path is still in the repo and could re-engage
unexpectedly] → mitigation: the spec requires the mock path be
disabled when a live connection exists, AND a feature flag
documented in H02's README. Long-term: delete the mock path
entirely once s1 lands a real claw store.

[Risk: existing H02 chat surface may have assumptions tied to mock
data shape that don't match gateway events] → mitigation: this is
the actual integration work in tasks.md. Likely a thin adapter layer
between gateway events and the existing chat-message renderer.

## Migration Plan

This slice ships as a single H02 PR. kelvinclaw is unchanged.
Existing kelvinclaw deployments work without modification.

After this slice archives, `openspec/specs/h02-gateway-connection/spec.md`
contains the requirements that future slices may MODIFY (e.g.,
s1 will MODIFY this capability to include `claw_id` in `agent`
calls).

## Open Questions

1. **What's the right env-variable name for the gateway URL in H02?**
   `NEXT_PUBLIC_KELVIN_GATEWAY_URL` is the spec's choice. Alternatives:
   `NEXT_PUBLIC_KELVIN_WS_URL`, `KELVIN_GATEWAY_URL`. Confirm with
   Fabro at implementation time.

2. **Should s0 ship with `kelvin.echo` pre-installed in the H02
   dev workflow, or document the install step?**
   Documenting feels lighter; pre-installing requires shipping a
   `kpm install` step in H02's `npm run dev:setup`. Defer to Fabro.

3. **Existing H02 chat surface streaming model.**
   H02 may render messages all-at-once or token-by-token. The spec
   says "render in arrival order" which works either way. Verify
   the existing component supports streaming chunks before assuming.
