---
status: Draft
version: v1, v2, v3
audience: product, architects
date: 2026-04-27
---

# Roadmap — v1 / v2 / v3

The Kelvin GUI architecture is intentionally versioned. v1 ships a
single-user, locally-deployed personal AI agent with the recursive Claw
primitive, three autonomy postures, two sidecars, and the H02 GUI. v2 adds
features that need a v1 to anchor against. v3 is "nice to have."

This roadmap is decision-rationale-per-item, not a project plan.

## v1 — Ship

The v1 surface is the entire kelvin-spec set in this directory **except**
items explicitly noted as v2/v3 below. The verification list in the spec
plan describes the cross-link integrity criteria.

v1 commitments:

- Single-user personal AI agent
- Recursive Claw primitive (macro + N levels of sub-claws)
- All four concepts distinct (Power / Connector / MCP server / Sub-agent)
- 12-axis autonomy matrix with three named postures
- Two sidecars (Open Bias model boundary + ToolRegistry tool boundary)
- H02 GUI fully migrated from `useGingerStore`
- Mind observability with all v1 tabs
- Per-claw `soul.md` + `RULES.md` files
- Approvals primitive extending QuestionCarousel
- Drafts → Sources promotion as the only outbound write edge
- Cost / token tracking (informational)

v1 explicitly does not include:

- Multi-user / shared spaces (v2)
- Plugin authoring UI in H02 (v2)
- Browser tab in Mind (v2)
- Cross-claw porosity beyond binary (v2)
- Trust-score gating beyond signed/unsigned (v2)
- Hard cost cutoffs (v3)
- Mobile / native packaging (v3)
- On-device model fallback (v3)
- Conflict-resolution UX beyond parent-arbitrates (v3)

## v2 — Versioned features

These need v1 to ship first because they layer on the foundation.

### Multi-user

**What**: Per-user accounts with login, ACLs on Claws and Sources,
invitation flows, per-user posture caps, audit attribution by user.

**Why deferred**: Substantial scope; requires a v1 to anchor identity
plumbing against. Personal-AI users don't need it on day one. See
[ADR-004](decisions/004-single-user-v1.md).

**Migration path**: v1 already populates `ownerId` / `createdBy` fields;
v2 adds enforcement, not schema.

### Exportable / shareable claw bundles (templates)

**What**: Export a claw (or Sub-agent template) as a portable bundle
(`tar` of the claw directory + dependencies manifest). Import on another
install.

**Why deferred**: Needs trust model for imported `RULES.md` (does the
target user inherit the source's rules verbatim?). Solved when multi-user
trust model lands.

**Migration path**: v1's per-claw directory layout is already
`tar`-friendly; the v2 work is the trust + import flow.

### Plugin authoring UI in H02

**What**: GUI for authoring Workflow Powers (composed from existing
Powers + arguments + conditional steps). Visual, no-code-required.

**Why deferred**: Complex UX surface; v1 ships with the existing
`docs/plugins/build-a-tool-plugin.md` flow (Rust/TS/WASM authoring). v2
adds a flow for non-developers.

### Browser tab in Mind

**What**: Live browsing surface — when an agent is doing web research,
show the actual rendered pages it's looking at.

**Why deferred**: Requires headless browser sidecar (Playwright, Puppeteer).
Significant ops complexity. v1 shows web search results as text in
Receipts; v2 shows them as live page renders.

### Cross-claw porosity beyond binary

**What**: Today the porosity axis is per-posture (low / medium / high =
prompt-only / summarized / full-passthrough). v2 adds:

- Per-Source porosity overrides ("This Source is OK to share with Health
  but not Work")
- Per-receiving-claw rules ("Health can read Personal's transcript but
  only the last 7 days")

**Why deferred**: Real cases for fine-grained porosity emerge from v1
usage; v1 doesn't have the data to know what knobs to add.

### Trust-score gating

**What**: Today the autonomy matrix's "Plugin install" axis distinguishes
"signed by trusted publisher" vs. "not." v2 adds a continuous trust score:

- Publisher reputation (verified org, age, install count)
- Plugin reputation (downloads, signed reviews, security audits)
- Posture rule "Auto only if trust score ≥ X"

**Why deferred**: Requires a registry trust signal beyond what
`kelvinclaw-plugins/index.json` provides today. Builds on the
`docs/plugins/plugin-trust-operations.md` framework.

### RULES.md inheritance

**What**: A child claw's `RULES.md` can `include` a parent's rules. v1
each claw is self-contained.

**Why deferred**: Inheritance semantics need careful spec — "what does
override mean? What if parent and child rules conflict?" Mitigation in
v1: users can copy or symlink rules manually.

## v3 — Nice to have

These are improvements to UX or capability that aren't needed for either
v1 or v2 to be useful.

### Conflict-resolution UX beyond "parent arbitrates"

**What**: When sub-claws disagree, today the parent chief arbitrates
([03-delegation-and-call-tree.md](03-delegation-and-call-tree.md)). v3
adds:

- Side-by-side comparison UI in Mind
- User can pick a sub-claw's answer or merge
- "Always prefer Health's recommendation on medical topics" rules

**Why deferred**: v1 + v2 establish the conflict pattern; v3 polishes the
UX once usage shows what users actually want.

### Hard cost cutoffs

**What**: Today `CostBudget.hardCutoff = false` in v1 (informational).
v3 enables cutoffs:

- Per-claw daily cap → block at threshold
- Per-Power per-task cap → block expensive single calls
- Install-wide monthly cap → safety net

**Why deferred**: Risk of blocking the user mid-task is real; need v1+v2
data to set sensible defaults. Manual user attention is enough until
costs run away.

### Mobile / native packaging

**What**: H02 as native iOS / Android / desktop apps. Today H02 is
Next.js, Electron-able but not packaged.

**Why deferred**: Pure packaging work; orthogonal to architecture. v1
focuses on web GUI quality; native is a follow-on.

### On-device model fallback

**What**: When sidecars are reachable but the upstream LLM is rate-limited
or down, fall back to a local model (e.g., Llama via Ollama).

**Why deferred**: Quality of small local models is improving but not yet
seamless. v1 fails-closed; v3 fails-degraded with explicit user opt-in.

### Multi-region / team deployment

**What**: A hosted Kelvin deployment for teams (companies, families,
co-living groups).

**Why deferred**: v1 is local-first by design (privacy stance). v2 adds
multi-user. v3 is the ops + hosting story for "Kelvin as a service."

## Decision rationale summary

| Item | v | Why this v |
|---|---|---|
| Recursive Claw + 12-axis posture + 2 sidecars | v1 | The architecture itself; without these, Kelvin doesn't work |
| Single-user | v1 | Smallest scope to ship; ADR-004 |
| H02 migration | v1 | No GUI = no Kelvin |
| Multi-user | v2 | Layers on v1; ADR-004 |
| Exportable templates | v2 | Needs multi-user trust model |
| Plugin authoring UI | v2 | Underlying APIs exist in v1; UI is the gap |
| Browser tab | v2 | Ops complexity; v1 provides text receipts as substitute |
| Trust-score gating | v2 | Builds on registry trust work |
| Hard cost cutoffs | v3 | v1+v2 produce data to set thresholds |
| Mobile / native | v3 | Pure packaging; orthogonal |
| On-device fallback | v3 | Local model quality not there yet |
| Team deployment | v3 | Layer above multi-user |

## Cross-references

- [00-overview.md](00-overview.md) — what's in scope for v1
- [ADR-004](decisions/004-single-user-v1.md) — single-user v1 decision
- [10-h02-migration.md](10-h02-migration.md) — v1 migration scope
- [09-data-model.md](09-data-model.md) — v2 placeholder fields
