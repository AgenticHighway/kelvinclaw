# Kelvin GUI Implementation Roadmap

This document outlines the slice plan for incrementally building the
Kelvin GUI architecture on top of the existing kelvinclaw runtime
and the H02 (Ginger) frontend at `https://github.com/kmondlane/h02`.

**Each slice is a single OpenSpec change** with its own
`proposal.md` / `design.md` / `specs/<capability>/spec.md` /
`tasks.md`. Each slice is **independently demoable** — by the end of
each slice, the user can do something they couldn't do before.

## Status

| Slice | Title | OpenSpec change | Status |
|---|---|---|---|
| s0 | Connect H02 to kelvin gateway | `s0-connect-h02-to-kelvin-gateway` | scaffolded |
| s1 | Claw as workspace metadata | `s1-add-claw-workspace-metadata` | scaffolded |
| s2 | Per-claw `soul.md` + `RULES.md` | `s2-add-claw-soul-and-rules-files` | scaffolded |
| s3 | Tool-execution posture axis | `s3-add-tool-execution-posture-axis` | scaffolded |
| s4 | Receipts + Mind session/audit tabs | _outline below_ | not yet scaffolded |
| s5 | Sub-claw delegation | _outline below_ | not yet scaffolded |
| s6 | Open Bias as ModelProvider profile | _outline below_ | not yet scaffolded |
| s7 | More posture axes | _outline below_ | not yet scaffolded |
| s8 | Drafts → Sources promotion edge | _outline below_ | not yet scaffolded |
| s9 | Sub-agent runtime registry | _outline below_ | not yet scaffolded |
| s10 | Hardening + remaining axes | _outline below_ | not yet scaffolded |

s0-s3 are full OpenSpec changes ready for `/opsx:apply`. s4-s10 are
described below as outlines; full changes are scaffolded as their
predecessors archive.

## Slice outlines (s4 — s10)

### s4 — `add-receipts-and-mind-session-tab`

**Demo:** scroll H02's Mind tab, see a chronological audit of every
tool call, every model call, every approval decision since the
session started.

**New capabilities:**
- `receipts-substrate` — append-only Receipt store with
  `parentReceiptId` linking. Every tool call, model call, posture
  change, and approval decision produces a Receipt.
- `mind-session-tab` — H02 Mind UI with Session and Receipts tabs;
  filter chain (claw + time + kind); call-tree assembly via
  `parentReceiptId`.

**Modified capabilities:**
- `autonomy-posture-tool-execution` — gate emits Receipt for
  Allow / Deny outcomes.

**New gateway methods:** `mind.query-receipts`,
`mind.query-call-tree`.

**Plugin repo:** zero impact.

### s5 — `add-sub-claw-delegation`

**Demo:** from the macro Kelvin claw, ask "have Health summarise my
recent runs"; the request delegates to the Health sub-claw; Mind
shows the delegation and the response in a call-tree.

**New capabilities:**
- `sub-claw-delegation` — `delegate-to-sub-claw` Power kind. Cycle
  prevention (ancestor + same-session sibling). Cross-claw porosity
  axis with three levels (low = prompt only; medium = summarised
  context; high = full referenced material).
- `autonomy-posture-sub-claw-delegation` — second axis on the matrix.

**Modified capabilities:**
- `autonomy-posture-tool-execution` — generalise gate to "axis-aware";
  add posture-cap-chain so a child claw cannot exceed parent's
  posture on any axis.
- `claw-workspace-metadata` — `posture` extends with
  `subClawDelegation`.

**New gateway methods:** `delegation.invoke` (or as a Power kind
through the existing `power.invoke` introduced in this slice if
not earlier).

**Plugin repo:** zero impact.

### s6 — `add-open-bias-modelprovider-profile`

**Demo:** install Open Bias as a sidecar; switch a claw to use it;
edit the claw's `RULES.md` to redact PII; chat; the Open Bias
PRE_CALL/POST_CALL evaluators apply rules against the claw's
RULES.md per request.

**New capabilities:**
- `open-bias-passthrough-profile` — new ModelProvider profile
  routing through `http://localhost:4000/v1`. Per-call header
  `X-Kelvin-Claw-Rules-Ref` carrying `claw_id` so Open Bias can
  load the right RULES.md. Header `X-Kelvin-Claw-Posture` carrying
  the effective posture snapshot. Sidecar-health probe + event.
- `rules-md-enforcement` — RULES.md content (created in s2) is now
  meaningful; the profile shim consumes it.

**Modified capabilities:**
- `claw-soul-rules-files` — `RULES.md` is consumed when the active
  ModelProvider profile is the Open Bias passthrough; otherwise
  unchanged.

**Plugin repo:** New package candidate `kelvin.open-bias` (or
profile shipped built-in to kelvin-brain). If shipped as a plugin,
update `agentichighway/kelvinclaw-plugins/index.json`.

**Important:** Open Bias is OPT-IN. s0-s5 work without it. v1
ships with this profile available but not required.

### s7 — `add-more-posture-axes`

**Demo:** in the claw posture editor, see a 6-axis matrix instead of
1-2; each axis can be set independently; the gate behaviour
matches per-axis posture.

**New capabilities:**
- `autonomy-posture-connector-writes`
- `autonomy-posture-source-reads`
- `autonomy-posture-draft-promotion`
- `autonomy-posture-plugin-install`

**Modified capabilities:**
- All previous posture capabilities — generalise the gate to handle
  multiple axes; "strictest axis wins" when a tool maps to several.

**Plugin repo:** Plugins may need to declare `posture_axis_hint`
in their tool metadata (e.g., a Connector op tool declares "this
is connectorWrites"). Coordinate with plugin index schema.

### s8 — `add-drafts-promotion-edge`

**Demo:** the agent produces a Draft (a markdown file, a diff, an
email body); H02's Drafts tab shows it; user clicks "Promote to
filesystem" / "Promote via Gmail"; the destination is updated.

**New capabilities:**
- `drafts-as-write-target` — All Power outputs land as Drafts;
  Sources are not directly writable from the agent layer. Promotion
  is a gated action (axis from s7).
- `mind-drafts-tab` — Drafts surface in Mind with promotion targets.

**Modified capabilities:**
- `receipts-substrate` — Promotion produces a Receipt of kind
  `draft-promotion` linking the Draft, the destination, the posture
  snapshot.

**Plugin repo:** zero impact (Drafts → Connector ops use existing
plugin tools).

### s9 — `add-sub-agent-runtime`

**Demo:** chat with a complex multi-step request; Mind shows the
macro claw spawning a transient "Researcher" sub-agent on the fly;
the sub-agent invokes Powers (web_search, summarise, cite); when
done, it ceases to exist, leaving only Receipts as the trace.

**Important per user-clarified semantics:** sub-agent delegation
creates specialized agents on the fly that have access to powers
*as they need or plan for them*. The spawning claw does not lock
a strict allowlist at spawn time; instead, the sub-agent can
request additional powers within the parent's allowed set, gated
by the same posture matrix. Each requested-and-granted power is
an audit row.

**New capabilities:**
- `sub-agent-runtime` — runtime-only `SubAgentInstance` registry
  (NOT persisted). Spawn handler creates instances per delegation
  with role + system prompt + budget. Power-request handshake:
  the sub-agent asks the runtime for a power; the runtime gates
  against the parent claw's posture before granting.
- `autonomy-posture-sub-agent-spawn` — third or later posture axis.

**Modified capabilities:**
- `receipts-substrate` — sub-agent spawn and per-power-grant produce
  Receipts.
- `mind-session-tab` — call-tree renders sub-agent spawn nodes.

**Plugin repo:** zero impact.

### s10 — `hardening-and-remaining-axes`

**Demo:** the system is robust under failure modes (sidecar crash,
connection drop, posture cap depth). The full 12-axis matrix is
implemented. v1 polish and release.

**New capabilities (the remaining axes):**
- `autonomy-posture-memory-writes`
- `autonomy-posture-wasm-egress` — maps to existing kelvinclaw WASM
  sandbox presets (`locked_down` / `dev_local` / `hardware_control`)
- `autonomy-posture-routines-user-absent`
- `autonomy-posture-power-model-spend`
- `autonomy-posture-cross-claw-porosity` (formal axis)
- `composer-modes-contract-enforcement` — the existing six modes
  (Auto/Plan/Ask/Learn/Play/Make) get hard contracts (Plan never
  executes; Ask never spawns; etc.)
- `cost-accounting-receipts` — token / dollar / wallclock per
  Receipt; Mind Costs tab.

**Modified capabilities:**
- All previous posture capabilities — finalise the matrix.

**Plugin repo:** WASM egress maps tools to presets; coordinate
with plugin trust/sandbox metadata.

## Cross-cutting principles

These hold for every slice:

1. **Each slice is demoable in 5 minutes.** If a slice's demo
   doesn't fit in 5 minutes, it's too big and should be split.

2. **Each slice's `tasks.md` lists explicit verification scenarios**
   that an implementer (Fabro) can execute to confirm the slice
   ships.

3. **Each slice can MODIFY earlier capabilities**, but new behaviour
   prefers ADDED-on-top rather than rewriting prior contracts.

4. **No Open Bias dependency before s6.** The architecture must
   work end-to-end without it.

5. **Plugin repo extensions accumulate.** Most slices touch only
   kelvinclaw + H02. Slices that touch
   `agentichighway/kelvinclaw-plugins` (the distribution repo)
   call this out explicitly in their proposal Impact section.

6. **Sub-agent power access is dynamic.** Sub-agents request
   powers at runtime as they need them; the parent's posture
   gates each grant. Per-power grants are auditable Receipts.

## Reference architecture (single-page summary)

```
┌────────────────────────────────────────────────────────────────┐
│  H02 (Next.js, kmondlane/h02)                                   │
│  • Chat composer, Mind tabs, Approvals carousel                │
│  • Talks to kelvinclaw via WS gateway                          │
└──────────────────────┬─────────────────────────────────────────┘
                       │  ws://127.0.0.1:34617
                       ▼
┌────────────────────────────────────────────────────────────────┐
│  kelvinclaw (this repo, agentichighway/kelvinclaw)              │
│  • apps/kelvin-gateway   ← WS protocol, methods, events        │
│  • apps/kelvin-host      ← runtime composition                 │
│  • crates/kelvin-brain   ← orchestration loop                  │
│  • crates/kelvin-core    ← Tool/ToolRegistry, ModelProvider,   │
│                            SessionDescriptor, Posture,         │
│                            Claw, Receipt                       │
│  • crates/kelvin-memory  ← memory subsystem                    │
│  • crates/kelvin-wasm    ← WASM sandbox (locked_down, etc.)   │
└──────────────────────┬─────────────────────────────────────────┘
                       │  WASM ABIs + plugin index
                       ▼
┌────────────────────────────────────────────────────────────────┐
│  kelvinclaw-plugins (agentichighway/kelvinclaw-plugins)         │
│  • index.json                                                   │
│  • signed packages: kelvin.{cli,echo,openai,anthropic,         │
│                              openrouter,ollama,websearch,wiki} │
│  • s6 may add kelvin.open-bias                                 │
└────────────────────────────────────────────────────────────────┘
```

Open Bias (s6+, optional):

```
                       ┌─────────────────────────────┐
                       │ Open Bias (Python proxy)     │
                       │ http://localhost:4000/v1     │
                       │ (a ModelProvider profile)    │
                       └──────────────┬──────────────┘
                                      ▼
                              Anthropic / OpenAI
```

## Versioning convention

This roadmap uses `s0`, `s1`, … as slice ids in OpenSpec change
names because OpenSpec change names cannot contain dots. The
slice numbers correspond to the conceptual versions a contributor
might call `v0.0`, `v0.1`, … in informal discussion.

## Documents

- Each slice lives in `openspec/changes/sN-<name>/` with its own
  proposal/design/specs/tasks.
- This roadmap (`docs/architecture-reference/roadmap.md`) is the
  cross-slice navigator.
- Slice s0 archives → `openspec/specs/h02-gateway-connection/spec.md`
  becomes the canonical baseline; s1 MODIFIES it; etc.

## When to extend this roadmap

Update this roadmap when:
- A slice is fully scaffolded (mark "scaffolded" in the status
  table)
- A slice archives (mark "shipped" in the status table)
- The slice plan changes structurally (add/remove/reorder slices —
  capture rationale)
