---
status: Draft
version: v1
audience: architects, security engineers, designers
date: 2026-04-27
---

# Autonomy Postures — The 12-Axis Matrix

The user picks **Low**, **Medium**, or **High** autonomy per claw. That
choice populates a 12-axis matrix with defaults; per-axis overrides are
allowed within parent-imposed caps. This doc is the **source of truth for
the matrix**. The same axes/wording must appear in
[06-approvals-primitive.md](06-approvals-primitive.md) and
[07-sidecars.md](07-sidecars.md).

See [ADR-008](decisions/008-three-postures-cap-invariant.md) for the
rationale and invariants.

## The matrix

| # | Axis | Low | Medium | High |
|---|---|---|---|---|
| 1 | **Tool execution** | Ask before every tool call | Ask for write/external; auto for read/internal | Auto for all; ask only on signed-but-untrusted Tools |
| 2 | **Sub-claw delegation** | Ask before every delegation | Auto to known children; ask for new | Auto to all bound children |
| 3 | **Sub-agent spawn** | Ask before every spawn | Auto from templates; ask for ad-hoc | Auto for all spawns within budget |
| 4 | **Source reads** | Ask per source per session | Auto for known sources; ask for new | Auto for all bound sources |
| 5 | **Connector writes** | Ask before every write op | Ask before high-impact writes (sends, deletes); auto for low (drafts, tags) | Auto for all writes |
| 6 | **Drafts → Sources promotion** | Always ask; show diff | Auto-promote to internal (memory, fs); ask for external (connector, MCP) | Auto-promote anywhere allowed by destination |
| 7 | **Plugin / Skill install** | Ask + show signing/trust info | Ask + auto for trusted-publisher signed | Auto for trusted-publisher signed; ask for others |
| 8 | **Memory writes** | Ask per write | Auto append; ask for overwrite/delete | Auto for all writes |
| 9 | **WASM egress** | `locked_down` only | `dev_local` allowed for trusted skills | `hardware_control` allowed for explicitly opted-in skills |
| 10 | **Routines firing user-absent** | Disabled when user absent | Allowed for read-only routines; disabled for writes | Allowed for all routines per claw config |
| 11 | **Cross-claw porosity** | Prompt only; no source/draft passthrough | Prompt + summarized context | Prompt + full referenced sources/drafts |
| 12 | **Power model spend** | Ask before any model-bound Power (>$0.01) | Ask above per-task threshold (default $0.50) | Auto up to per-day budget (default $20) |

### Axis index

For typed cross-references, the canonical key per axis (matching
`PosturePerAxis` in [09-data-model.md](09-data-model.md)):

| # | Axis name | TypeScript key |
|---|---|---|
| 1 | Tool execution | `toolExecution` |
| 2 | Sub-claw delegation | `subClawDelegation` |
| 3 | Sub-agent spawn | `subAgentSpawn` |
| 4 | Source reads | `sourceReads` |
| 5 | Connector writes | `connectorWrites` |
| 6 | Drafts → Sources promotion | `draftPromotion` |
| 7 | Plugin / Skill install | `pluginInstall` |
| 8 | Memory writes | `memoryWrites` |
| 9 | WASM egress | `wasmEgress` |
| 10 | Routines firing user-absent | `routinesUserAbsent` |
| 11 | Cross-claw porosity | `crossClawPorosity` |
| 12 | Power model spend | `powerModelSpend` |

## Posture selector — UX

A claw's Settings shows:

```
Autonomy: ◉ Low   ○ Medium   ○ High        [⚙ Edit per-axis]

▾ Per-axis overrides (3 active)
   • Connector writes:    Medium  ← override (default Low)
   • Source reads:        High    ← override (default Medium)
   • WASM egress:         Low     ← override (default Medium)

▾ Active "remember this" approvals (2)
   • Allow web_search forever                    [revoke]
   • Allow gmail.send to advisor@... (session)   [revoke]

User cap on this install: Medium  ← prevents High overrides
```

The posture badge shown elsewhere (chat header, Mind, etc.) is the *base
posture name* with an asterisk if any per-axis overrides are active:
`Low*` means "Low base with overrides."

## Invariants

The cap chain (per [ADR-008](decisions/008-three-postures-cap-invariant.md)):

```
User-cap (global, per install)
   ⤓ caps
Macro claw posture
   ⤓ caps
Sub-claw posture
   ⤓ caps
Grand-sub-claw posture
   ⤓ caps
… (recursion unbounded)
```

Per axis:

`effective[axis] = min(user_cap[axis], all_ancestors[axis], claw[axis])`

where `Low < Medium < High` for cap purposes (Low is the strictest).

Additional invariants:

1. Routines fire at the claw's posture, NOT the user's session posture.
2. The "Routines firing user-absent" axis can FURTHER restrict trigger-driven
   actions below the claw's interactive posture.
3. A spawned Sub-agent inherits the spawning claw's posture as a hard cap;
   re-checked on each Power invocation.
4. Per-action "remember forever" overrides require a separate explicit
   confirmation step in the UI.
5. Per-claw posture changes are recorded as Receipts.

## Sidecar-down floor

Per [ADR-003](decisions/003-two-sidecars.md) and
[ADR-008](decisions/008-three-postures-cap-invariant.md):

| Sidecar state | Effective posture | Behavior |
|---|---|---|
| Both healthy | as configured | normal |
| Open Bias `:4000` down | floors to Low | model calls refused (fail-closed); banner |
| `ToolRegistry` misconfigured | floors to Low | tool calls denied; banner |
| Both down | denied | "Safe mode" — no model, no tools |

The floor is global (not per-claw). The banner is visible on every screen
until the sidecar(s) recover.

## Per-axis playbook

For each axis, the implementation reference: which gate enforces it, where
in the runtime it sits, and what the approval surface looks like.

### 1. Tool execution

- **Gate**: kelvinclaw `ToolRegistry`
  ([interfaces/tool-gate-postures.md](interfaces/tool-gate-postures.md))
- **Approval surface**: ApprovalCard in QuestionCarousel
- **Receipt kind**: `tool-call`

### 2. Sub-claw delegation

- **Gate**: kelvinclaw `ToolRegistry` (delegations are Powers — see
  [03-delegation-and-call-tree.md](03-delegation-and-call-tree.md))
- **Approval surface**: ApprovalCard with target claw + cross-claw porosity
  preview
- **Receipt kind**: `sub-claw-delegation`

### 3. Sub-agent spawn

- **Gate**: kelvinclaw runtime spawn handler
- **Approval surface**: ApprovalCard with role, allowed Powers, budget
- **Receipt kind**: `sub-agent-spawn`

### 4. Source reads

- **Gate**: kelvinclaw `ToolRegistry` Source-read tools
- **Approval surface**: ApprovalCard per Source per session (Low) or once
  (Medium)
- **Receipt kind**: `source-read`

### 5. Connector writes

- **Gate**: kelvinclaw `ToolRegistry` per-Connector write op
- **Approval surface**: ApprovalCard with op summary + reversibility note
- **Receipt kind**: `connector-op`

### 6. Drafts → Sources promotion

- **Gate**: GUI promotion action + ToolRegistry for the underlying write
- **Approval surface**: ApprovalCard with diff preview (text diff for
  filesystem; structured diff for memory; preview-render for connector ops)
- **Receipt kind**: `draft-promotion`

### 7. Plugin / Skill install

- **Gate**: kelvinclaw plugin install path with trust check (see
  `docs/plugins/plugin-trust-operations.md`)
- **Approval surface**: ApprovalCard with publisher, signature status,
  scoped permissions, sample manifest
- **Receipt kind**: `tool-call` (kind=`plugin-install`) — written by install
  handler

### 8. Memory writes

- **Gate**: kelvinclaw memory module
- **Approval surface**: ApprovalCard with target memory + write preview
- **Receipt kind**: `memory-write`

### 9. WASM egress

- **Gate**: kelvinclaw WASM sandbox preset selection
  (`locked_down` / `dev_local` / `hardware_control` —
  `docs/architecture/trusted-executive-wasm.md`)
- **Approval surface**: at install time only (per-Skill); changing posture
  changes the preset
- **Receipt kind**: `tool-call` records the active preset

### 10. Routines firing user-absent

- **Gate**: kelvinclaw scheduler + presence detector
- **Approval surface**: per-Trigger setting (`enabledWhenUserAbsent`); no
  per-fire approval (the user is absent)
- **Receipt kind**: matches the action the Routine performs

### 11. Cross-claw porosity

- **Gate**: kelvinclaw delegation handler — strips/transforms context per
  posture
- **Approval surface**: shown as preview in delegation ApprovalCard
- **Receipt kind**: `sub-claw-delegation` (porosity snapshot in `action`)

### 12. Power model spend

- **Gate**: kelvinclaw `ModelProvider` shim (cost accounting per call) +
  ToolRegistry threshold check
- **Approval surface**: ApprovalCard with estimated $ and remaining daily
  budget
- **Receipt kind**: `power-invocation` (cost in Receipt)

## Worked example — a Medium-posture Personal claw

User's macro claw (Kelvin) is at User cap **Medium**. Personal sub-claw
inherits Medium with two overrides:

- `connectorWrites: 'low'` (Personal claw shouldn't auto-send messages)
- `routinesUserAbsent: 'low'` (no Routines fire when user is away)

Effective matrix on Personal sub-claw:

| Axis | Effective | Source |
|---|---|---|
| toolExecution | medium | inherited |
| subClawDelegation | medium | inherited |
| subAgentSpawn | medium | inherited |
| sourceReads | medium | inherited |
| **connectorWrites** | **low** | per-axis override |
| draftPromotion | medium | inherited |
| pluginInstall | medium | inherited |
| memoryWrites | medium | inherited |
| wasmEgress | medium | inherited |
| **routinesUserAbsent** | **low** | per-axis override |
| crossClawPorosity | medium | inherited |
| powerModelSpend | medium | inherited |

Posture badge: `Medium*`.

User asks Personal claw to "send a thank-you to my advisor." Trace:

1. Personal claw runs the request (mode: Auto).
2. Personal claw drafts a message (Draft created — no gate).
3. Personal claw needs to call `gmail.send` (Connector write, axis 5).
4. `connectorWrites` is Low — ApprovalCard shown: "Allow `gmail.send` to
   `advisor@...` with body preview?" with scope options
   `once / session / claw / forever`.
5. User clicks "once."
6. Power invocation runs; Receipt written; Draft promoted to "sent"
   status; user notified.

## Cross-references

- [ADR-008](decisions/008-three-postures-cap-invariant.md) — invariants
- [ADR-003](decisions/003-two-sidecars.md) — sidecar-down floor
- [06-approvals-primitive.md](06-approvals-primitive.md) — approval UI
- [07-sidecars.md](07-sidecars.md) — sidecar topology
- [interfaces/tool-gate-postures.md](interfaces/tool-gate-postures.md) — runtime mapping
- [09-data-model.md](09-data-model.md) — `PosturePerAxis`, `PostureOverride`
