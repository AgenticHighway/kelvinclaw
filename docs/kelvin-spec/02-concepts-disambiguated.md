---
status: Draft
version: v1
audience: architects, plugin authors, designers
date: 2026-04-27
---

# Powers, Connectors, MCP servers, Sub-agents — Disambiguated

These four words have been used interchangeably in earlier drafts. This doc
is the single source of truth for what each is, what each is not, and how
they nest. See [ADR-002](decisions/002-four-distinct-concepts.md) for the
rationale.

## The four concepts

| Concept | What it is | Lifecycle | What it has | What it isn't |
|---|---|---|---|---|
| **Connector** | Authenticated integration with one external service (Composio app, OAuth provider — Gmail, Slack, Notion, GitHub) | Persistent; installed + auth'd in **global Settings** | credentials, scopes, exposed operations | a tool definition, a model, a capability |
| **MCP server** | Protocol-level provider of tools / resources / prompts. Local or remote. | Persistent; installed + configured in **global Settings** | tool defs, resource refs, prompt templates | an integration with one app; a model |
| **Power** | A *capability* the agent can use — Skill (atomic) or Workflow (composed). May bind its own model. May *use* Connector ops or MCP tools to do its work. | Persistent; lives in the **claw's library** | name, charter/system-prompt, optional model, requires (sources/tools/connectors/MCPs/powers), trigger surface | an active execution; an integration |
| **Sub-agent** | A *transient specialist instance* spawned for one sub-session with its own identity, system prompt, and Power allowlist. May or may not invoke Powers during its run. | **Ephemeral** — exists only for the sub-session | role, system prompt, parent claw, allowed Powers, budget | a stored entity, a Power |

## Layering

The four concepts compose in a defined direction:

```
┌──────────────────────────────────────────────────────────────┐
│  Sub-agent (transient)                                       │
│  - has a role + system prompt + budget                       │
│  - invokes Powers during its sub-session                     │
└────────────────────────┬─────────────────────────────────────┘
                         │ uses
                         ▼
┌──────────────────────────────────────────────────────────────┐
│  Power (persistent, in claw's library)                       │
│  - Skill (atomic) or Workflow (composed)                     │
│  - may bind its own model                                    │
│  - declares requires.connectors[] and requires.mcps[]        │
└────────────────────────┬─────────────────────────────────────┘
                         │ uses
                         ▼
┌──────────────────────────────────────────────────────────────┐
│  Connector op             OR             MCP tool             │
│  (Gmail.send,                            (filesystem.read,    │
│   Slack.post, ...)                        web.search, ...)    │
│  Auth'd via OAuth.                       Protocol-defined.    │
└──────────────────────────────────────────────────────────────┘
```

**Sub-agents use Powers; Powers use Connector ops or MCP tools.** A Power
that needs no Connector and no MCP is fine — it just runs against the claw's
own Sources and the model.

## Three worked examples

### Example 1 — "Search the web for X"

A simple model-driven query:

- **Sub-agent**: none required. The macro claw's chat handles it inline.
- **Power**: `web_search` (Skill, model-bound). Lives in the claw's library.
- **MCP tool**: the `web_search` tool exposed by an MCP server (e.g.,
  `kelvin-mcp-search`).
- **Connector**: none.

Call-tree (in Mind):

```
[Power invocation: web_search]
  └─ [MCP tool: web_search.search(q="X")]
```

### Example 2 — "Research and write me a brief on solar regulation in CA"

A multi-step task best handled by a transient specialist:

- **Sub-agent**: spawned by macro claw with role `Researcher`, system prompt
  seeded from the Health-or-Personal claw's `Researcher` template, allowed
  Powers `[web_search, summarize, cite]`, budget 50k tokens / 10 min.
- **Powers used during sub-session**:
  - `web_search` (Skill, MCP-backed)
  - `summarize` (Skill, model-bound, no MCP)
  - `cite` (Skill, model-bound, no MCP)
- **Connectors**: none.
- **Output**: a Draft "CA Solar Regulation Brief" in the spawning claw's
  Drafts collection.

Call-tree:

```
[Sub-agent spawn: Researcher (budget 50k tok / 10min)]
  ├─ [Power invocation: web_search × 4]
  │    └─ [MCP tool: web_search.search × 4]
  ├─ [Power invocation: summarize × 1]
  └─ [Power invocation: cite × 1]
[Receipt: Draft "CA Solar Regulation Brief" created]
```

### Example 3 — "Send the brief to my advisor"

A connector-backed action:

- **Sub-agent**: none.
- **Power**: `send_email` (Skill, no model bound — just maps args to connector
  op).
- **Connector**: Gmail (installed in global Settings, bound to the macro
  claw or a specific sub-claw).
- **Connector op**: `gmail.send(to=..., subject=..., body=...)`.

Call-tree:

```
[Power invocation: send_email]
  └─ [Connector op: gmail.send(to="advisor@example.com", ...)]
[Receipt: Email sent via Gmail connector]
```

## Why each distinction matters

### Sub-agent ≠ Power

Because a Sub-agent *consumes* Powers. A Sub-agent has its own identity,
sub-session, budget, and role; a Power is a function it can call.

If you collapsed Sub-agents into Powers, you'd lose:

- The transient identity attribution in Mind's call-tree
- The budget concept (Sub-agents have budgets; Powers don't)
- The role allowlist (a Researcher Sub-agent might be allowed `web_search`
  but not `send_email`; a Power doesn't have its own allowlist of OTHER
  Powers)

### Power ≠ Connector

Because a Power *consumes* Connector ops. A Connector is the integration
itself (with credentials and scopes); a Power is the agent-facing capability
that uses one or more Connector ops.

If you collapsed Powers into Connectors, you'd lose:

- The agent-facing capability layer (a Power composes multiple ops with logic)
- The model-bound Power case (a Power with no Connector at all)
- Workflows that span multiple Connectors (one Power can call Gmail.send and
  then Slack.post)

### Connector ≠ MCP server

Because a Connector is identity/auth-bearing while an MCP server is
protocol-bearing. A Connector has credentials and a single service relationship;
an MCP server exposes a tool/resource/prompt protocol over which many tools
may be defined.

If you collapsed them, you'd lose:

- The OAuth / refresh-token storage concern (Connectors)
- The protocol versioning concern (MCP servers)
- The trust class distinction (a Connector is a trust relationship with a
  specific provider; an MCP server is a trust relationship with a protocol
  implementation)

## Where each lives

| Concept | Storage location | UI surface |
|---|---|---|
| Connector | Global Settings → Connectors panel | Settings → Connectors |
| MCP server | Global Settings → MCP servers panel | Settings → MCP |
| Power | Per-claw library (`<claw>/powers/`) | Claw view → Powers tab |
| Sub-agent template (optional) | Per-claw config | Claw view → Templates |
| Sub-agent instance | Runtime memory only (not persisted) | Mind call-tree |

## Trust and autonomy mapping

Each concept maps onto specific autonomy matrix axes (full matrix in
[05-autonomy-postures.md](05-autonomy-postures.md)):

| Concept | Primary autonomy axis | Notes |
|---|---|---|
| Connector | Connector writes; Plugin install | Different rows for the auth/install vs. the per-call gate |
| MCP server | Plugin install; Tool execution | Install is the trust event; tool calls flow through Tool execution gate |
| Power (no model) | Tool execution | Most Powers fall here |
| Power (model-bound) | Tool execution + Power model spend + Plugin install (at install time) | Bring-your-own-model Powers carry extra axis |
| Sub-agent | Sub-agent spawn | Distinct from delegation to a peer claw |

## Where each is documented for plugin authors

Each concept has a different plugin-authoring path (some already documented
in this repo's existing plugins/ subdir):

| Concept | Authoring guide |
|---|---|
| Connector | (planned, v2; meanwhile Composio docs apply) |
| MCP server | Standard MCP server authoring (external) |
| Power (Skill, WASM-backed) | [`docs/plugins/build-a-tool-plugin.md`](../plugins/build-a-tool-plugin.md) |
| Power (Workflow, in-claw) | [10-h02-migration.md](10-h02-migration.md) (post-v1) |
| Sub-agent template | [01-claw-anatomy.md](01-claw-anatomy.md) (per-claw config; no plugin path) |

## Cross-references

- [ADR-002](decisions/002-four-distinct-concepts.md) — rationale
- [ADR-001](decisions/001-sub-agents-runtime-only.md) — Sub-agents are
  runtime-only
- [01-claw-anatomy.md](01-claw-anatomy.md) — anatomy with bindings
- [03-delegation-and-call-tree.md](03-delegation-and-call-tree.md) — call-tree
- [09-data-model.md](09-data-model.md) — schemas
- [05-autonomy-postures.md](05-autonomy-postures.md) — autonomy axes
- `docs/plugins/plugin-author-kit.md` — existing plugin author kit
