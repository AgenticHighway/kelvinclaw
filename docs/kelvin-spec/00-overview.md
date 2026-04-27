---
status: Draft
version: v1
audience: everyone
date: 2026-04-27
---

# Kelvin / H02 GUI Architecture — Overview

This is the canonical home for the Kelvin GUI architecture spec. It describes
how **H02** (the Next.js front-end, formerly "Ginger") becomes the GUI of the
**kelvinclaw** runtime, with two security sidecars, a recursive Claw primitive,
three autonomy postures, and a clear separation between Powers, Connectors,
MCP servers, and Sub-agents.

## What Kelvin is

A **personal AI agent platform** organized around the metaphor of a "claw":
one orchestrator you talk to (the **macro claw**, named Kelvin) that delegates
work to specialized **sub-claws** for each domain of your life (Work, Health,
Personal, Finance, Learning, Creative, …). Every claw — macro and sub — has
the same anatomy: an identity, a policy file, sources of information,
mutable drafts, capabilities (Powers), triggers (hooks/heartbeats/watches),
and bound communication channels.

Kelvin is implemented as four cooperating components:

```
┌─────────────────────────────────────────────────────────────────┐
│                         User                                     │
└──────────────┬──────────────────────────────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────────────────────────────┐
│  H02 GUI (Next.js)                                               │
│  - Chat composer, voice input, channels                          │
│  - Mind (observability tabs + call-tree)                         │
│  - Approvals tray, autonomy posture screens                      │
│  - Per-claw soul.md / RULES.md editing                           │
└──────────────┬──────────────────────────────────────────────────┘
               │ WebSocket gateway (submit/wait/event-stream)
               ▼
┌─────────────────────────────────────────────────────────────────┐
│  kelvinclaw runtime (Rust workspace)                             │
│  - KelvinBrain orchestration loop                                │
│  - Tool / ToolRegistry  ← TOOL BOUNDARY GATE (autonomy posture)  │
│  - ModelProvider        ← model-call shim (per-claw RULES.md)    │
│  - Memory + WASM sandbox + plugin registry                       │
└────────────┬───────────────────────────────────────────┬────────┘
             │                                           │
             ▼                                           ▼
┌────────────────────────────────┐    ┌────────────────────────────────┐
│  Open Bias sidecar (Python)    │    │  kelvinclaw-plugins            │
│  http://localhost:4000/v1      │    │  (trusted publisher registry)  │
│  ← MODEL BOUNDARY GATE         │    │                                │
│  Per-claw RULES.md enforcement │    │                                │
└──────────────┬─────────────────┘    └────────────────────────────────┘
               │
               ▼
       Anthropic / OpenAI
```

H02 is a **front-end**; the runtime, security gates, and plugin registry all
live in this repo and `kelvinclaw-plugins`. The decision to put the spec here
rather than in H02 reflects that architecture is runtime-led: H02 is one of
N possible front-ends.

## Concept index

Every concept named below has a deeper doc. Use this section as the spec's
table of contents.

### Foundations

| Concept | One-line definition | Deeper doc |
|---|---|---|
| **Claw** | A recursive dispatcher (macro or sub) that owns Sources, Drafts, Powers, Triggers, Channels, Soul, and Rules. | [01-claw-anatomy.md](01-claw-anatomy.md) |
| **Macro claw** | The root claw (Kelvin); the one you talk to first. | [01-claw-anatomy.md](01-claw-anatomy.md) |
| **Sub-claw** | Any claw with a parent. Same anatomy as macro. | [01-claw-anatomy.md](01-claw-anatomy.md) |
| **Soul** | Per-claw identity + charter + style; lives in `soul.md`. | [01-claw-anatomy.md](01-claw-anatomy.md), [ADR-006](decisions/006-soul-rules-files-and-question-reuse.md) |
| **Rules** | Per-claw enforceable policy; lives in `RULES.md`; enforced by Open Bias. | [07-sidecars.md](07-sidecars.md), [ADR-006](decisions/006-soul-rules-files-and-question-reuse.md) |

### Capabilities and lifecycle

| Concept | One-line definition | Deeper doc |
|---|---|---|
| **Power** | A capability the agent can use (Skill or Workflow), optionally bound to a model. Lives in the claw's library. | [02-concepts-disambiguated.md](02-concepts-disambiguated.md) |
| **Connector** | An authenticated integration with one external service (Composio app, OAuth provider). Persistent, in global Settings. | [02-concepts-disambiguated.md](02-concepts-disambiguated.md) |
| **MCP server** | A protocol-level provider of tools / resources / prompts. Persistent, in global Settings. | [02-concepts-disambiguated.md](02-concepts-disambiguated.md) |
| **Sub-agent** | A transient specialist instance spawned for one sub-session. Runtime-only, not stored. | [ADR-001](decisions/001-sub-agents-runtime-only.md), [03-delegation-and-call-tree.md](03-delegation-and-call-tree.md) |
| **Sub-agent template** | Optional per-claw preset for spawning common Sub-agents. | [ADR-001](decisions/001-sub-agents-runtime-only.md) |

### Data and state

| Concept | One-line definition | Deeper doc |
|---|---|---|
| **Sources** | Privileged read inputs to a claw: filesystem, web, APIs, memories, transcript, connector-backed, MCP resources. | [01-claw-anatomy.md](01-claw-anatomy.md), [09-data-model.md](09-data-model.md) |
| **Drafts** | Privileged mutable outputs. The only outbound promotion edge to Sources. | [ADR-007](decisions/007-drafts-promotion-edge.md) |
| **Receipt** | Immutable audit row for one action. Distinct from Drafts. | [08-mind.md](08-mind.md) |
| **Triggers** | Hooks (event), Heartbeats (timer), Watches (state). Per-claw. | [01-claw-anatomy.md](01-claw-anatomy.md) |
| **Channels** | Bi-directional surfaces for inbound and outbound communication (Telegram, Discord, email, SMS, web, voice). Per-claw subset of installed channels. | [01-claw-anatomy.md](01-claw-anatomy.md) |

### Globally shared

| Concept | One-line definition | Deeper doc |
|---|---|---|
| **Modes** | Auto / Plan / Ask / Learn / Play / Make. Composer-level intent picker; orthogonal to autonomy. | [04-modes.md](04-modes.md) |
| **Inputs** | Composer (text), voice, and channel ingress. | [01-claw-anatomy.md](01-claw-anatomy.md) |
| **Mind** | Global observability surface; tabs (session, tasks, browser, diffs, PRs, drafts, plans, receipts, costs, notifications) + call-tree. | [08-mind.md](08-mind.md) |
| **Settings** | Global config: Identity, Channels installs, Connectors, MCP servers, Sidecar, Autonomy user-cap, Cost & budgets, Versioning. | [01-claw-anatomy.md](01-claw-anatomy.md) (settings layout) |

### Security

| Concept | One-line definition | Deeper doc |
|---|---|---|
| **Autonomy posture** | Low / Medium / High default for "how much can the agent do without asking?" Per claw. | [05-autonomy-postures.md](05-autonomy-postures.md) |
| **Approvals primitive** | UI tray for gated actions; extends H02's existing `Question`/`QuestionCarousel`. | [06-approvals-primitive.md](06-approvals-primitive.md), [ADR-006](decisions/006-soul-rules-files-and-question-reuse.md) |
| **Two sidecars** | Open Bias on the model boundary + kelvinclaw `ToolRegistry` on the tool boundary. | [07-sidecars.md](07-sidecars.md), [ADR-003](decisions/003-two-sidecars.md) |
| **Parent caps child** | A sub-claw cannot exceed its parent's posture. User caps macro. | [ADR-008](decisions/008-three-postures-cap-invariant.md), [ADR-005](decisions/005-recursive-claw.md) |
| **Sidecar-down floors to Low** | If either sidecar is unreachable, effective posture floors to Low. | [ADR-008](decisions/008-three-postures-cap-invariant.md) |

## Decisions log

The architecture is anchored by 8 ADRs. Read these first if you want to
understand *why* things are shaped the way they are.

| ADR | Decision |
|---|---|
| [001](decisions/001-sub-agents-runtime-only.md) | Sub-agents are runtime-only, not stored per-claw |
| [002](decisions/002-four-distinct-concepts.md) | Powers, Connectors, MCP servers, Sub-agents are four distinct concepts |
| [003](decisions/003-two-sidecars.md) | Two security sidecars (model boundary + tool boundary) |
| [004](decisions/004-single-user-v1.md) | Single-user v1; multi-user / shared spaces deferred to v2 |
| [005](decisions/005-recursive-claw.md) | Recursive Claw primitive — macro and sub-claws are structurally identical |
| [006](decisions/006-soul-rules-files-and-question-reuse.md) | Per-claw `soul.md` + `RULES.md`; reuse `Question`/`QuestionCarousel` for approvals |
| [007](decisions/007-drafts-promotion-edge.md) | Drafts → Sources is the only outbound promotion edge from privileged |
| [008](decisions/008-three-postures-cap-invariant.md) | Three autonomy postures with parent-caps-child invariant; sidecar-down floors to Low |

## Spec navigation

A reading order from foundation to implementation:

1. **Decisions** ([decisions/](decisions/)) — start here for the why
2. **[01-claw-anatomy.md](01-claw-anatomy.md)** — the recursive primitive
3. **[02-concepts-disambiguated.md](02-concepts-disambiguated.md)** — terminology
4. **[09-data-model.md](09-data-model.md)** — TypeScript schemas
5. **[10-h02-migration.md](10-h02-migration.md)** — concrete H02 store migration
6. **[03-delegation-and-call-tree.md](03-delegation-and-call-tree.md)** — how claws hand work off
7. **[04-modes.md](04-modes.md)** — composer intent picker
8. **[05-autonomy-postures.md](05-autonomy-postures.md)** — security matrix
9. **[06-approvals-primitive.md](06-approvals-primitive.md)** — UI primitive
10. **[07-sidecars.md](07-sidecars.md)** — Open Bias + ToolRegistry
11. **[08-mind.md](08-mind.md)** — observability surface
12. **[11-roadmap.md](11-roadmap.md)** — v1/v2/v3

Interface contracts (the boundary with the runtime) live under
[interfaces/](interfaces/):

- [interfaces/h02-protocol.md](interfaces/h02-protocol.md) — WebSocket gateway
- [interfaces/sidecar-integration.md](interfaces/sidecar-integration.md) — `ModelProvider` shim for per-claw `RULES.md`
- [interfaces/tool-gate-postures.md](interfaces/tool-gate-postures.md) — autonomy → `ToolRegistry`

## Cross-references to existing kelvinclaw docs

This spec layers on top of the runtime architecture already documented in
this repo. Specifically:

- `OVERVIEW.md` — Kelvin Core SDK seams (`Brain`, `MemorySearchManager`,
  `ModelProvider`, `SessionStore`, `EventSink`, `Tool`/`ToolRegistry`)
- `AGENTS.md` — engineering principles (file-size limits, fail-closed defaults,
  security frameworks list)
- `docs/architecture/architecture.md` — system architecture overview
- `docs/architecture/trusted-executive-wasm.md` — WASM sandbox presets
  referenced by the autonomy matrix's WASM egress row
- `docs/gateway/gateway-protocol.md` — existing WebSocket protocol the H02
  protocol builds on
- `docs/security/sdk-owasp-top10-ai-2025.md` — security test mapping
- `docs/plugins/plugin-author-kit.md` — Power authoring path
- `docs/plugins/plugin-trust-operations.md` — trusted-publisher tiers
  referenced by the autonomy matrix's "Plugin install" row

## What this spec does NOT cover

Kept explicit so the scope stays honest:

- **Multi-user and shared spaces** — deferred to v2 (ADR-004).
- **Plugin authoring UI in H02** — deferred to v2.
- **Mobile / native packaging** — out of scope for v1.
- **On-device model fallback** — v3.
- **Cost-budget hard cutoffs** — v3 (v1 displays costs in Mind; v2 adds soft
  warnings).
- **Conflict-resolution UX beyond "parent arbitrates"** — v3.
