---
status: Accepted
version: v1
audience: architects, implementors
date: 2026-04-27
---

# ADR-001 — Sub-agents are runtime-only, not stored per-claw

## Status

Accepted.

## Context

Early drafts of the Kelvin GUI architecture modeled "sub-agents" as a privileged box
inside every Claw, alongside Sources, Drafts, and Powers. This mirrored the existing
H02 (Ginger) data model in `H02/src/types/index.ts`, where each `Space` carries a
`subAgents: string[]` and `defaultSubAgents: SubAgentType[]`, and a fixed enum of 25
specialist roles (Researcher, Editor, Writer, Developer, Critic, Planner, …) is
defined in `H02/src/constants/index.ts` as `SUB_AGENTS`.

That model coupled three concerns into one entity:

1. **Capability** (what the agent can do — a Power)
2. **Persona / charter** (the system prompt, the role identity)
3. **Lifecycle** (when does the agent come into being, when does it go away)

It also forced every claw to maintain a roster of specialists even when most of them
would never run, and made the existing `Power.agentType` field necessary as a coupling
hint — a Power had to be associated with a specific specialist to make sense.

As the architecture conversation progressed, two facts became clear:

- Most "specialist" roles are short-lived: a Researcher exists for the duration of a
  single research task and is discarded.
- The boundary between "a Power that uses a model" and "a Sub-agent that uses Powers"
  is artificial. A Power-with-bound-model can do everything a stored Sub-agent can do.

A separate but related decision (ADR-002) clarified that Powers, Connectors, MCP
servers, and Sub-agents are four distinct concepts. This ADR settles the lifecycle
question for Sub-agents specifically.

## Alternatives Considered

### Alternative A — Keep Sub-agents as stored entities per claw (status quo)

Each claw maintains a roster of installed specialists. Powers are coupled to a
specialist via `agentType`. The Auto router picks among installed specialists.

**Pros:** No migration; matches Ginger's existing schema; users can curate a stable
team per space.

**Cons:** Forces a redundant taxonomy alongside Powers; specialists are dead-weight
state when not running; the coupling between Powers and specialists is rigid and
makes Powers harder to compose.

### Alternative B — Collapse Sub-agents into Powers entirely

A "Researcher" is just a Power named Researcher with a model binding and a system
prompt. No Sub-agent concept at all. The Auto router picks among Powers.

**Pros:** Simplest data model — one taxonomy. Powers compose freely.

**Cons:** Loses the conceptual distinction between a *capability* (Power) and an
*acting persona running in its own sub-session* (Sub-agent). Conflates a function
definition with a process invocation. Makes call-trees harder to read because there
is no notion of a transient identity to attribute work to.

### Alternative C (chosen) — Sub-agents are runtime-only

Sub-agents exist as runtime instances spawned for a sub-session. Each spawn carries
a role name, a system prompt, an allowed-Powers list, and a budget. When the
sub-session ends, the Sub-agent is gone. Sub-agent **templates** (presets for
common spawns) MAY exist per claw as optional convenience; they are not required
for spawning and are not a privileged anatomy box.

**Pros:** Clean lifecycle separation (Power = function, Sub-agent = process);
preserves call-tree legibility (every sub-session has a named identity); reduces
stored state per claw; templates remain available for the curated-team UX.

**Cons:** Spawning logic must decide role + Powers + budget at runtime; templates
are an optional convenience layer that needs its own UI affordance.

## Decision

**Sub-agents are runtime-only.** They are spawned by a parent claw for one
sub-session, carry a transient identity (role + system prompt + allowed Powers +
budget), and cease to exist when the sub-session ends. They are NOT stored as a
privileged anatomy box on a Claw.

A separate concept — **Sub-agent templates** — MAY live per claw as optional
spawning presets (default role, default Powers allowlist, default budget). Templates
are convenience, not architecture: a Claw with zero templates can still spawn
Sub-agents on the fly.

The 25 specialists currently defined in `H02/src/constants/index.ts:SUB_AGENTS`
migrate to **Sub-agent templates** as seed presets, mapped through the
six-category taxonomy (knowledge / creation / strategy / analysis / communication /
operations).

`Power.agentType` is removed as a field; Powers stand alone and are selected by
the parent claw or by a spawned Sub-agent independently of any specialist
coupling. See [10-h02-migration.md](../10-h02-migration.md) for the concrete
field-removal map.

## Consequences

### Positive

- The data model loses one taxonomy. `AGENT_CATEGORIES` and `POWER_CATEGORIES`
  consolidate into a single Power category set.
- Per-claw stored state shrinks. Claws own Powers, Sources, Drafts, Triggers,
  Channels, Soul, Rules, and bindings — not a roster of specialists.
- Powers compose freely across spawn contexts. A Researcher Sub-agent can use
  the same `web_search` Power that a Critic Sub-agent uses; neither owns it.
- The Mind call-tree gains a clean three-node-kind taxonomy (Power invocation,
  Sub-agent spawn, Sub-claw delegation) — see
  [03-delegation-and-call-tree.md](../03-delegation-and-call-tree.md).
- Sub-agent templates become a curation surface, not a dependency. Power users
  curate templates; new users spawn ad-hoc.

### Negative

- Existing H02 components that read `Space.subAgents` / `Space.chief.defaultSubAgents`
  must migrate (see [10-h02-migration.md](../10-h02-migration.md) for the impacted
  components under `H02/src/components/features/agents/`).
- A new runtime concept (`SubAgentInstance`) appears in the data model that is NOT
  persisted, requiring a clear distinction in the store between persisted entities
  and session-scoped state.
- Onboarding flows must teach two concepts (template vs spawn) instead of one
  (installed specialist). See [11-roadmap.md](../11-roadmap.md) on onboarding
  in v1.

### Security

- A spawned Sub-agent inherits the parent claw's autonomy posture as a cap; see
  ADR-008 and [05-autonomy-postures.md](../05-autonomy-postures.md). The runtime
  nature of Sub-agents means the cap MUST be applied at spawn time and re-checked
  on each Power invocation within the sub-session, since the parent's posture can
  change mid-flight.
- Sub-agent templates are stored config and are subject to the same `RULES.md`
  and signing posture as Powers (see ADR-008 for plugin install posture rules).

## References

- ADR-002 — Powers, Connectors, MCP servers, Sub-agents are four distinct concepts
- ADR-005 — Recursive Claw primitive
- ADR-008 — Three autonomy postures with parent-caps-child invariant
- [01-claw-anatomy.md](../01-claw-anatomy.md)
- [02-concepts-disambiguated.md](../02-concepts-disambiguated.md)
- [03-delegation-and-call-tree.md](../03-delegation-and-call-tree.md)
- [09-data-model.md](../09-data-model.md)
- [10-h02-migration.md](../10-h02-migration.md)
