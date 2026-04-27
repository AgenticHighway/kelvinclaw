---
status: Accepted
version: v1
audience: architects, implementors, plugin authors
date: 2026-04-27
---

# ADR-002 — Powers, Connectors, MCP servers, Sub-agents are four distinct concepts

## Status

Accepted.

## Context

Across the Kelvin GUI design conversation, four words have been used to describe
"things the agent can use to get work done": **Powers**, **Connectors**, **MCP
servers**, and **Sub-agents**. The terms have been used interchangeably in some
discussions and as nested categories in others. The H02 codebase reflects this
muddiness:

- `Power` (`H02/src/types/index.ts`) carries an `agentType: SubAgentType` field,
  coupling Powers to Sub-agents.
- `UserIntegration` (`H02/src/types/index.ts`) is a Connector-shaped entity but
  lives in user settings rather than a first-class concept.
- MCP servers do not appear in H02's type system at all; they are referenced
  conceptually.
- Sub-agents are a fixed enum of 25 specialists.

In the runtime (kelvinclaw), the picture is partially separated:
`crates/kelvin-core` defines `Tool` / `ToolRegistry` and `PluginFactory` /
`PluginRegistry` as distinct seams (see `OVERVIEW.md`). But the GUI-facing
mental model has not caught up.

This conflation matters because:

1. Each concept has a different **lifecycle** (persistent installation vs. transient
   spawn).
2. Each carries a different **trust profile** (credentials vs. protocol vs. policy
   vs. transient identity).
3. Each shows up on a different **settings surface** (global Connectors panel,
   global MCP panel, per-claw Powers library, ad-hoc spawn UI).
4. Each has a different **upgrade and audit story** (signed plugin updates vs.
   OAuth re-consent vs. MCP version bumps vs. session-scoped spawn audit).

Without disambiguation, the autonomy matrix axes blur (which row gates which
thing?), and the data model accumulates fields that try to be all four things
at once.

## Alternatives Considered

### Alternative A — One taxonomy: "tools"

Collapse all four into a single "tool" concept with type discriminators.

**Pros:** Smallest data model. Fewest concepts to teach.

**Cons:** Loses every distinction that matters operationally. A user installing
a Composio Gmail integration (which prompts an OAuth flow and stores refresh
tokens) is doing something materially different from spawning a Researcher
Sub-agent (which has no credentials and no install). Single taxonomy makes the
autonomy matrix unworkable.

### Alternative B — Two taxonomies: "installed extensions" vs. "runtime instances"

Group Connectors + MCP servers + Powers under "extensions"; Sub-agents under
"instances."

**Pros:** Captures the persistence vs. lifecycle distinction.

**Cons:** Still flattens the install/auth/protocol distinctions among the three
"extensions." Connectors carry credentials; MCP servers carry a protocol contract;
Powers carry an agent-facing capability shape. Squashing them obscures the
trust-and-audit story.

### Alternative C (chosen) — Four distinct concepts with explicit nesting

- **Connector** — authenticated integration with one external service.
  Persistent, installed in global Settings.
- **MCP server** — protocol-level provider of tools / resources / prompts.
  Persistent, installed in global Settings.
- **Power** — capability the agent can use (Skill or Workflow), optionally bound
  to a model. Persistent, lives in the claw's library. Powers MAY require
  Connector ops or MCP tools to function (declared in `requires`).
- **Sub-agent** — transient specialist instance for a sub-session. Ephemeral.
  Optionally spawned from a Sub-agent template (per-claw preset).

Nesting is layered: **Sub-agents use Powers; Powers use Connector ops or MCP
tools.**

**Pros:** Each concept has a clear lifecycle, trust profile, and settings home.
Autonomy matrix axes map cleanly. Plugin author paths are different and that's
correct (a Power author has different responsibilities than a Connector author).

**Cons:** Four concepts to teach. Mitigated by treating most users' interaction
as "Powers" and surfacing Connectors/MCP servers only in Settings.

## Decision

The four concepts are first-class and distinct. The disambiguation table in
[02-concepts-disambiguated.md](../02-concepts-disambiguated.md) is the
single source of truth for what each is, what each is not, and how they nest.

Concrete consequences for the data model
([09-data-model.md](../09-data-model.md)):

- `Connector`, `MCPServer`, `Power`, `SubAgentTemplate`, `SubAgentInstance` are
  five distinct TypeScript schemas. (Connector and MCPServer are Settings-level;
  Power and SubAgentTemplate are per-claw; SubAgentInstance is runtime-only per
  ADR-001.)
- `Power.requires` declares dependencies on `sources`, `tools` (registry entries),
  `connectors` (connector ids), `mcps` (MCP server ids), and `powers` (composed).
- A claw must **bind** Connectors and MCP servers from the global pool before
  Powers in that claw can use them (privileged subset).
- The Settings UI has separate panels for Connectors and MCP servers. The Powers
  panel is per-claw.
- `Power.agentType` is removed (consequence of ADR-001).

## Consequences

### Positive

- The autonomy matrix in [05-autonomy-postures.md](../05-autonomy-postures.md)
  has clean per-row semantics: "Plugin install" gates Powers and MCP servers
  with bring-their-own-model risk; "Connector writes" gates Connector ops;
  "Sub-agent spawn" gates new sub-sessions.
- Plugin author paths in `kelvinclaw/docs/plugins/` correctly differ — building
  a model plugin, a tool plugin, or a channel plugin all already exist as
  separate guides; this ADR ratifies the GUI-side mirror.
- Trust posture is per-concept: Connectors require OAuth and credential storage
  policy; MCP servers require protocol version compatibility; Powers (with
  optional model) require signing and trusted-publisher status; Sub-agents
  inherit posture from spawning claw.
- The Mind call-tree
  ([08-mind.md](../08-mind.md)) renders the layering visibly: a Sub-agent node
  contains Power invocation children; a Power node contains Connector op or
  MCP tool children.

### Negative

- More entities to model and migrate. See
  [10-h02-migration.md](../10-h02-migration.md) for the concrete impact on
  H02's `useGingerStore`.
- Settings UI grows from a single Integrations panel to three (Connectors,
  MCP servers, plus the existing user-integrations panel as legacy or merged).
- Documentation and onboarding must explain the layering. Mitigated by making
  most beginner interactions stay at the Power level.

### Security

- Each concept has its own trust class in the autonomy matrix; collapsing them
  would make the matrix incoherent.
- Connectors are the highest-credential class (refresh tokens, scopes); they
  warrant the strictest add/auth flow.
- MCP servers introduce protocol-level risk (an MCP server can return arbitrary
  tool definitions); require version pinning and trusted-publisher status.
- Powers with bound models are gated under "Plugin install" because installing
  one implies new outbound model API calls.

## References

- ADR-001 — Sub-agents are runtime-only
- ADR-008 — Three autonomy postures with parent-caps-child invariant
- [02-concepts-disambiguated.md](../02-concepts-disambiguated.md)
- [09-data-model.md](../09-data-model.md)
- [05-autonomy-postures.md](../05-autonomy-postures.md)
- [10-h02-migration.md](../10-h02-migration.md)
- `kelvinclaw/docs/plugins/plugin-author-kit.md` (existing — referenced)
