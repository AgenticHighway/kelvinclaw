---
status: Draft
version: v1
audience: runtime authors, plugin authors
date: 2026-04-27
---

# Tool Gate — Mapping Autonomy Postures to `ToolRegistry`

The kelvinclaw `ToolRegistry` (defined in `crates/kelvin-core`, documented
in [`OVERVIEW.md`](../../../OVERVIEW.md)) is the **tool boundary** sidecar
([07-sidecars.md](../07-sidecars.md)). The Kelvin GUI extends it with
**autonomy posture enforcement** at every tool call site.

This doc specifies the mapping from the 12-axis posture matrix
([05-autonomy-postures.md](../05-autonomy-postures.md)) to the
`ToolRegistry` enforcement points, plus the WASM sandbox preset selection
and trusted-publisher check for plugin installs.

See [ADR-008](../decisions/008-three-postures-cap-invariant.md) for the
posture model.

## Enforcement points

The `ToolRegistry` enforces autonomy at three points:

1. **Tool registration time** — a Tool registers with declared properties
   (kind, isWrite, sourceType, connectorId, mcpServerId, …). The registry
   stores these.
2. **Tool invocation time** — before dispatching, the registry consults
   the calling claw's effective posture and the Tool's properties.
3. **Plugin install time** — when adding a Tool from a plugin, the
   registry checks signing + trusted-publisher status against the
   `Plugin install` axis posture.

## Tool property → posture axis mapping

Every Tool declares properties at registration. The registry maps these
onto the matrix axes:

| Tool property | Matrix axis (key) | Notes |
|---|---|---|
| `kind: 'tool-call'` (default) | `toolExecution` | Catch-all |
| `kind: 'sub-claw-delegation'` | `subClawDelegation` | Powers of `kind: 'delegate-to-sub-claw'` |
| `kind: 'sub-agent-spawn'` | `subAgentSpawn` | Spawn handler tool |
| `kind: 'source-read'` + `sourceId: ...` | `sourceReads` | Per-source approval |
| `kind: 'connector-op'` + `isWrite: true` | `connectorWrites` | Reads pass through `toolExecution` |
| `kind: 'connector-op'` + `isWrite: false` | `toolExecution` | Reads at base axis |
| `kind: 'mcp-op'` | `toolExecution` | Plus `pluginInstall` at the install path |
| `kind: 'draft-promotion'` | `draftPromotion` | Promotion handler tool |
| `kind: 'memory-write'` | `memoryWrites` | Memory module tool |
| `kind: 'plugin-install'` | `pluginInstall` | Install/update handler |
| `kind: 'posture-change'` | (always allowed; produces Receipt) | Posture changes themselves are not gated by posture |
| Power with `model: ...` set | `powerModelSpend` | Plus `toolExecution` |

When a Tool maps to multiple axes, the **strictest** axis wins (most
restrictive posture).

## Decision algorithm (per call)

```
fn allow_call(claw: &Claw, tool: &ToolMeta, args: &Args, ctx: &Ctx) -> Decision {
  // 1. Compute effective posture per axis (cap chain: user → ancestors → claw)
  let posture = effective_posture(claw);

  // 2. Determine relevant axis (use strictest if multiple)
  let axis = relevant_axis(tool);
  let level = posture[axis];

  // 3. Check sidecar-down floor
  if sidecar_state.any_down() {
    level = PostureLevel::Low;  // floor
  }

  // 4. Check active "remember this" overrides
  if let Some(override_) = matching_override(claw, tool, args) {
    return override_.decision;  // bypasses posture check
  }

  // 5. Apply posture rules per axis
  match (axis, level) {
    // Tool execution
    (ToolExecution, Low)    => Ask,
    (ToolExecution, Medium) => if tool.is_write_or_external { Ask } else { Allow },
    (ToolExecution, High)   => if tool.is_signed_trusted { Allow } else { Ask },

    // Connector writes
    (ConnectorWrites, Low)    => Ask,
    (ConnectorWrites, Medium) => if tool.is_high_impact_write { Ask } else { Allow },
    (ConnectorWrites, High)   => Allow,

    // Source reads
    (SourceReads, Low)    => Ask /* per source per session */,
    (SourceReads, Medium) => if known_source(args.source_id) { Allow } else { Ask },
    (SourceReads, High)   => Allow,

    // Sub-agent spawn
    (SubAgentSpawn, Low)    => Ask,
    (SubAgentSpawn, Medium) => if has_template(args.template_id) { Allow } else { Ask },
    (SubAgentSpawn, High)   => Allow,

    // ... (similar for the rest)
  }
}
```

`Ask` means: emit a Question (kind=approval), block until the user
answers. `Allow` means: proceed and write a Receipt with `outcome:
'allowed'`. `Deny` (rare; sidecar-down forces some axes to default-deny)
emits a Receipt and returns an error.

## WASM egress preset selection

The `wasmEgress` axis selects which sandbox preset is in effect for WASM
Skills (see
[`docs/architecture/trusted-executive-wasm.md`](../../architecture/trusted-executive-wasm.md)):

| Posture (axis 9) | Selected preset | What's allowed |
|---|---|---|
| Low | `locked_down` | No network, no fs writes |
| Medium | `dev_local` | Local fs writes within `kelvinDataDir`; no outbound network |
| High | `hardware_control` | + selected hardware ops (audio, camera) under explicit per-Skill permission |

Per-Skill posture overrides exist via `PostureOverride` (axis-scoped); a
specific Skill can be elevated above the claw's default. Each elevated
Skill produces a posture-change Receipt at the time of override.

## Plugin install flow

Installing a Tool from a plugin (Skill, Workflow Power, or MCP server)
flows through the registry's install handler with autonomy axis
`pluginInstall`:

```
fn handle_plugin_install(claw: &Claw, plugin: &PluginManifest) -> Decision {
  let posture = effective_posture(claw);
  let level = posture[PluginInstall];

  // sidecar-down floor
  let level = if sidecar_state.any_down() { Low } else { level };

  match level {
    Low => Ask /* show full manifest, signing, scope */,
    Medium => if is_trusted_publisher(plugin.publisher) && plugin.signed { Allow } else { Ask },
    High   => if is_trusted_publisher(plugin.publisher) && plugin.signed { Allow } else { Ask /* still ask for unsigned */ },
  }
}
```

The trusted-publisher check uses the `kelvinclaw-plugins/index.json` and
`trusted_publishers.kelvin.json` manifests (existing — see
[`docs/plugins/plugin-trust-operations.md`](../../plugins/plugin-trust-operations.md)).

## Power `requires` validation (registration time)

When a Power is registered (via plugin install or user-authored
Workflow), the registry checks declared `requires`:

```
fn validate_power_install(claw: &Claw, power: &Power) -> Result<()> {
  for connector_id in &power.requires.connectors {
    if !claw.bound_connector_ids.contains(connector_id) {
      return Err(MissingBinding(connector_id));
    }
  }
  for mcp_id in &power.requires.mcps {
    if !claw.bound_mcp_server_ids.contains(mcp_id) {
      return Err(MissingBinding(mcp_id));
    }
  }
  // Recursive check: every required Power must exist on the claw
  for power_id in &power.requires.powers {
    if !claw.power_ids.contains(power_id) {
      return Err(MissingPower(power_id));
    }
  }
  Ok(())
}
```

This catches dependency mismatches at install time rather than at runtime.

## Cross-claw porosity enforcement

Sub-claw delegation tools enforce the `crossClawPorosity` axis at the
delegation handler:

```
fn handle_delegation(from: &Claw, to: &Claw, prompt: &str, ctx: &Ctx) -> Result<DelegationCtx> {
  let porosity = effective_posture(from)[CrossClawPorosity];

  let payload = match porosity {
    Low =>    DelegationPayload { prompt: prompt.into(), sources: vec![], drafts: vec![] },
    Medium => DelegationPayload { prompt: prompt.into(), sources: summarize_referenced(ctx)?, drafts: vec![] },
    High =>   DelegationPayload { prompt: prompt.into(), sources: ctx.referenced_sources()?, drafts: ctx.referenced_drafts()? },
  };

  // Receiving claw's own posture caps what it accepts
  let received = to.accept_delegation(payload, ctx)?;
  Ok(received)
}
```

The receiving claw's posture cap is checked separately on inbound — a
sub-claw at posture Low for `crossClawPorosity` won't accept full
sources/drafts even if a high-posture parent tries to send them.

## Axes not enforced at the tool gate

One axis from the matrix
([05-autonomy-postures.md](../05-autonomy-postures.md)) is **not**
enforced at the `ToolRegistry`:

- **`routinesUserAbsent`** — enforced by the runtime's Trigger/scheduler
  layer, not at per-call dispatch. The scheduler decides whether to fire
  a Trigger at all; once a Trigger fires, it goes through the normal
  tool-call dispatch (where the *other* axes apply).

All other 11 axes are enforced here.

## Posture data flow into the registry

The registry needs the calling claw's effective posture on every call.
The flow:

```
Brain creates CallContext { claw_id }
  ↓
Brain → ToolRegistry::invoke(call_ctx, tool_id, args)
  ↓
ToolRegistry resolves: effective_posture(claw_id) via PostureService
  ↓ (PostureService walks parent chain, applies overrides)
Decision(Allow | Ask | Deny)
```

`PostureService` is a small cached service over the Claw store. Posture
changes invalidate the cache for the affected claw and its descendants.

## Per-axis "remember this" override matching

A `PostureOverride` records "user said yes to this class of action."
Match algorithm:

```
fn matching_override(claw: &Claw, tool: &ToolMeta, args: &Args) -> Option<&PostureOverride> {
  let candidates = posture_overrides_for(claw);
  for o in candidates {
    if o.expires_at < now() { continue; }
    if o.axis != relevant_axis(tool) { continue; }
    if !matches(o.action_filter, tool, args) { continue; }
    return Some(o);
  }
  None
}
```

`action_filter` is a JSON object specifying which action shape this
override covers. Examples:

```jsonc
// "Allow gmail.send to advisor@example.com forever"
{
  "kind": "connector-op",
  "connectorServiceId": "gmail",
  "operationId": "gmail.send",
  "args.to": "advisor@example.com"
}

// "Allow web_search forever (any args)"
{
  "kind": "tool-call",
  "toolName": "web_search"
}
```

Matching is exact-equality plus wildcard (omitted field = any).

## OpenTelemetry tracing

Every gate decision emits an OTEL span:

- Span kind: `internal`
- Attributes: `kelvin.claw_id`, `kelvin.posture_axis`,
  `kelvin.posture_level`, `kelvin.tool_id`, `kelvin.decision`,
  `kelvin.override_id` (if matched)

These trace IDs propagate into `Receipt.otelTraceId` for forensic
correlation in Mind.

## Cross-references

- [ADR-008](../decisions/008-three-postures-cap-invariant.md) — posture invariants
- [ADR-002](../decisions/002-four-distinct-concepts.md) — four concepts; tool kind discrimination
- [05-autonomy-postures.md](../05-autonomy-postures.md) — full matrix
- [07-sidecars.md](../07-sidecars.md) — tool boundary
- [interfaces/sidecar-integration.md](sidecar-integration.md) — companion model boundary shim
- [09-data-model.md](../09-data-model.md) — `PosturePerAxis`, `PostureOverride`, `Receipt`
- [`OVERVIEW.md`](../../../OVERVIEW.md) — `Tool` / `ToolRegistry` seam
- [`docs/architecture/trusted-executive-wasm.md`](../../architecture/trusted-executive-wasm.md) — sandbox presets
- [`docs/plugins/plugin-trust-operations.md`](../../plugins/plugin-trust-operations.md) — trusted-publisher
- [`docs/security/sdk-test-matrix.md`](../../security/sdk-test-matrix.md) — security tests
