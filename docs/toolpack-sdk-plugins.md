# Kelvin Core Tool Pack (SDK Plugins)

Kelvin ships a default first-party SDK tool pack through the `Kelvin Core` plugin path
(`InMemoryPluginRegistry -> SdkToolRegistry`), not direct root wiring.

## Included Tools

- `fs_safe_read`
  - workspace-relative read
  - traversal blocked
  - sensitive paths denied (`.env`, `.git/`, plugin home)
- `fs_safe_write`
  - workspace-relative write
  - only allowed roots: `.kelvin/sandbox/`, `memory/`, `notes/`
  - explicit approval required
- `web_fetch_safe`
  - host-mediated HTTP(S) fetch
  - strict host allowlist
  - explicit approval required
  - payload bounds enforced
- `schedule_cron`
  - local scheduler state registry
  - add/remove require explicit approval
- `session_tools`
  - session-local notes state
  - clear requires explicit approval

## Sensitive Operation Controls

Sensitive operations are deny-by-default unless an explicit approval payload is present:

```json
{
  "approval": {
    "granted": true,
    "reason": "user-authorized maintenance operation"
  }
}
```

Runtime policy toggles:

- `KELVIN_TOOLPACK_ENABLE_FS_WRITE` (default `true`)
- `KELVIN_TOOLPACK_ENABLE_WEB_FETCH` (default `true`)
- `KELVIN_TOOLPACK_ENABLE_SCHEDULER_WRITE` (default `true`)
- `KELVIN_TOOLPACK_ENABLE_SESSION_CLEAR` (default `true`)
- `KELVIN_TOOLPACK_WEB_ALLOW_HOSTS` (default: `docs.rs,crates.io,raw.githubusercontent.com,api.openai.com`)

## Security/Stability Notes

- Tool failures degrade gracefully: run continues, tool error is emitted as payload/event.
- Every tool execution emits an audit receipt (`who/what/why/result_class/latency_ms`).
- Deterministic OWASP/NIST suites cover tool sandbox behavior:
  - `crates/kelvin-sdk/tests/tool_sandbox_owasp_top10_ai_2025.rs`
  - `crates/kelvin-sdk/tests/tool_sandbox_nist_ai_rmf_1_0.rs`
