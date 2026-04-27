---
status: Draft
version: v1
audience: H02 implementors
date: 2026-04-27
---

# H02 Migration — `useGingerStore` → `useClawStore`

This doc maps the existing H02 (Ginger) codebase to the new Kelvin GUI data
model. It is the single concrete change document for the H02 repo. The
canonical schemas it migrates *to* live in
[09-data-model.md](09-data-model.md).

H02 lives at https://github.com/kmondlane/h02. All file paths in this doc are
relative to that repo's root unless otherwise noted.

## Migration philosophy

The H02 type system is **rich**. Most of the migration is **rename + extend**,
not greenfield rewrites. The Question / QuestionCarousel primitive in
particular is reused in place per
[ADR-006](decisions/006-soul-rules-files-and-question-reuse.md).

Migration is staged in slices that each leave the codebase in a working state:

1. **Type layer** — `src/types/index.ts` field renames and additions
2. **Store layer** — `useGingerStore` → `useClawStore`; selector renames
3. **Constants** — `SUB_AGENTS` → templates; `SOURCE_TYPES` expansion;
   `AGENT_CATEGORIES` consolidation into `POWER_CATEGORIES`
4. **Component layer** — `src/components/features/` updates
5. **Cross-cutting** — Question kind discriminator; Receipt as new entity

## Slice 1 — Type layer (`src/types/index.ts`)

### `Space` → `Claw`

Rename and extend the existing `Space` type. The recursive primitive is
already partially modeled (`parentSpaceId`, `chief`); finish it.

| Existing field on `Space` | Action | New field on `Claw` |
|---|---|---|
| `id` | keep | `id` |
| `name` | keep | `name` |
| `parentSpaceId` | rename | `parentClawId` |
| `chief: SpaceChief` | merge into Claw + retire SpaceChief | (charter from `chief.charter` → `soul.md`; `defaultSubAgents` → seed `subAgentTemplateIds`; `reportingTo` derives from `parentClawId`) |
| `iconRef` | keep | `iconRef` |
| `description` | keep | `description` |
| `isHome` | remove | (use derived `parentClawId === null` selector) |
| `type: SpaceType` | remove | (typed enum dropped; replaced by free naming) |
| `privacy` | keep as v2 placeholder | `privacy` (forward-compat per ADR-004) |
| `allowedUserIds` | keep as v2 placeholder | `allowedUserIds` (forward-compat) |
| `allowCrossSpaceSearch`, `inheritParentSources`, etc. (porosity flags) | merge into autonomy posture matrix | maps to `autonomyPosture.crossClawPorosity` axis |
| `subAgents: string[]` | **remove** (per ADR-001) | (replaced by `subAgentTemplateIds`; instances are runtime-only) |
| (new) | add | `soulPath: string` |
| (new) | add | `rulesPath: string` |
| (new) | add | `sourceIds`, `draftIds`, `powerIds`, `triggerIds`, `channelIds` |
| (new) | add | `boundConnectorIds`, `boundMcpServerIds` |
| (new) | add | `subAgentTemplateIds: string[]` |
| (new) | add | `autonomyPosture: PosturePerAxis` |

Migration helper (one-time, runs at store rehydration):

```ts
function migrateSpaceToClaw(space: Space): Claw {
  return {
    id: space.id,
    name: space.name,
    parentClawId: space.parentSpaceId ?? null,
    iconRef: space.iconRef,
    description: space.description,
    soulPath: `${kelvinDataDir}/claws/${space.id}/soul.md`,
    rulesPath: `${kelvinDataDir}/claws/${space.id}/RULES.md`,
    sourceIds: space.sourceIds ?? [],
    draftIds: space.draftIds ?? [],
    powerIds: space.powerIds ?? [],
    triggerIds: [],
    channelIds: [],
    boundConnectorIds: [],
    boundMcpServerIds: [],
    subAgentTemplateIds: deriveTemplatesFromChief(space.chief),
    autonomyPosture: derivePostureFromPorosity(space),
    ownerId: singleUserId,
    createdBy: singleUserId,
    createdAt: space.createdAt ?? new Date(),
    updatedAt: new Date(),
    privacy: space.privacy,
    allowedUserIds: space.allowedUserIds,
  };
}
```

### `SubAgent` → `SubAgentTemplate` + remove instances

Per [ADR-001](decisions/001-sub-agents-runtime-only.md), Sub-agents are
runtime-only. The 25 specialists in `src/constants/index.ts:SUB_AGENTS` become
seed `SubAgentTemplate` entries.

The existing `SubAgent` interface (with state fields like `lastUsedAt`,
`messageCount`) splits:

- **Configuration** → `SubAgentTemplate` (role, systemPrompt, default Powers,
  budget)
- **Runtime state** → `SubAgentInstance` (NOT persisted; runtime memory only)

### `Power` — remove `agentType` coupling

```ts
// BEFORE
interface Power {
  // ...
  agentType: SubAgentType;  // REMOVE
  // ...
}

// AFTER
interface Power {
  // ...
  // (agentType field removed; Powers stand alone)
  requires?: { sources?: string[]; tools?: string[]; connectors?: string[]; mcps?: string[]; powers?: string[] };
  model?: { provider: string; id: string; systemPrompt?: string; parameters?: Record<string, unknown> };
  kind: 'skill' | 'workflow' | 'delegate-to-sub-claw';
  triggerSurface: ('slash' | 'auto-router' | 'mention' | 'routine')[];
  installSource?: 'builtin' | 'plugin' | 'user-authored';
  installedFrom?: string;
  signed?: boolean;
  trustedPublisher?: string;
}
```

### `Source.type` expansion

```ts
// BEFORE  (src/constants/index.ts)
const SOURCE_TYPES = ['document', 'web', 'feed', 'database', 'text'];

// AFTER  (per 09-data-model.md)
type SourceType =
  | 'filesystem'
  | 'web'
  | 'api'
  | 'feed'
  | 'memory'
  | 'transcript'
  | 'connector-backed'
  | 'mcp-resource';
```

Migration of legacy values:

- `'document'` → `'filesystem'`
- `'web'` → `'web'` (unchanged)
- `'feed'` → `'feed'` (unchanged)
- `'database'` → `'connector-backed'` (most likely use; user-confirmed at
  migration time)
- `'text'` → `'memory'` (most likely use; user-confirmed)

Each migrated Source gains a `config` field with the discriminated payload
shape from [09-data-model.md](09-data-model.md).

### `Draft` — minor field adds

```ts
// BEFORE
type DraftStatus = 'generating' | 'ready' | 'exported';

// AFTER
type DraftStatus = 'generating' | 'ready' | 'promoted';
// 'exported' renames to 'promoted' for symmetry with the promotion edge (ADR-007)
```

Add fields:

- `bornFromReceiptId: string`
- `bornFromPowerId?: string`
- `bornFromSubAgentInstanceId?: string`
- `promotionTargets?: PromotionTarget[]`
- `promotedToSourceIds?: string[]`
- `promotedAt?: Date`
- `promotedAutonomySnapshot?: PosturePerAxis`

### `Question` — kind discriminator

Per [ADR-006](decisions/006-soul-rules-files-and-question-reuse.md), the
existing Question type gains a kind field. This is non-breaking for existing
consumers (default is `'clarification'`).

```ts
type QuestionKind = 'clarification' | 'approval';

interface Question {
  // ... all existing fields ...
  kind: QuestionKind;                        // default 'clarification'
  actionDescriptor?: ActionDescriptor;       // approval kind only
  defaultChoiceId?: string;                  // approval kind only
  scopeOptions?: ('once' | 'session' | 'claw' | 'forever')[];
  riskLevel?: 'low' | 'medium' | 'high';
  expiresAt?: Date;
}
```

### New entities

Add entirely new types (see [09-data-model.md](09-data-model.md) for full
shapes):

- `Connector` (was partially modeled as `UserIntegration`)
- `MCPServer` (new)
- `SubAgentTemplate` (replaces stored `SubAgent` config)
- `Receipt` (new — immutable audit row)
- `Trigger` (new — Hooks/Heartbeats/Watches)
- `Channel` (was partially modeled in user settings; now first-class)
- `PostureOverride` (new)
- `CostBudget` (new)

### `UserIntegration` → `Connector`

The existing `UserIntegration` is the closest H02 has to a Connector. Migrate:

| `UserIntegration` field | `Connector` field |
|---|---|
| `id` | `id` |
| `service` | `serviceId` |
| `displayName` | `displayName` |
| `oauthRefreshTokenRef` (assumed) | `authRef` |
| `scopes` | `scopes` |
| (new) | `authMethod`, `exposedOperations`, `status`, `authenticatedAt`, `expiresAt` |

## Slice 2 — Store layer

### Rename: `useGingerStore` → `useClawStore`

Mechanical rename across the codebase. Search/replace `useGingerStore` →
`useClawStore` in:

- `src/stores/gingerStore.ts` → `src/stores/clawStore.ts` (file rename)
- All component imports

Selector renames:

| Old | New |
|---|---|
| `useGingerStore.getState().spaces` | `useClawStore.getState().claws` |
| `useGingerStore.getState().subAgents` | (removed; templates instead) |
| `useGingerStore.getState().userIntegrations` | `useClawStore.getState().connectors` |
| `useGingerStore.getState().drafts` | `useClawStore.getState().drafts` |
| (new) | `useClawStore.getState().mcpServers` |
| (new) | `useClawStore.getState().receipts` |
| (new) | `useClawStore.getState().triggers` |
| (new) | `useClawStore.getState().postureOverrides` |
| (new derived) | `useClawStore.getState().macroClaw` (the claw with `parentClawId === null`) |

### Action renames

| Old | New |
|---|---|
| `addSpace`, `updateSpace`, `removeSpace` | `addClaw`, `updateClaw`, `removeClaw` |
| `setActiveSpace` | `setActiveClaw` |
| `addSubAgent` (config) | `addSubAgentTemplate` |
| `assignSubAgentToSpace` | (removed; templates live on the claw directly) |

New actions:

- `bindConnector(clawId, connectorId)` — with subset-of-parent validation
- `bindMcpServer(clawId, mcpServerId)` — with subset-of-parent validation
- `promoteDraft(draftId, targetSourceIds, autoOrApproval)`
- `addReceipt(receipt)` — append-only
- `setPosture(clawId, axis, level)` — with parent-cap validation
- `addPostureOverride(override)` — for "remember this choice"

### Migration runner

A `migrate()` function in the store handles the one-time rehydration step:

```ts
// src/stores/clawStore.ts
async function migrateFromGinger(legacy: GingerState): Promise<ClawState> {
  return {
    claws: legacy.spaces.map(migrateSpaceToClaw),
    sources: legacy.sources.map(migrateSource),
    drafts: legacy.drafts.map(migrateDraft),
    powers: legacy.powers.map(migratePower),  // strips agentType
    connectors: legacy.userIntegrations.map(migrateUserIntegration),
    mcpServers: [],
    subAgentTemplates: legacy.subAgents.map(migrateSubAgent),
    receipts: [],
    triggers: [],
    channels: deriveChannelsFromSettings(legacy.settings),
    postureOverrides: [],
    questions: legacy.questions.map(q => ({ ...q, kind: q.kind ?? 'clarification' })),
    // ... other passthroughs ...
  };
}
```

## Slice 3 — Constants (`src/constants/index.ts`)

### `SUB_AGENTS` → seed templates

The 25 specialist definitions seed the initial `SubAgentTemplate` collection
on each macro claw. They map through the consolidated category set:

| Old `SubAgentType` enum value | New `category` |
|---|---|
| Researcher, Analyst, Synthesizer | `knowledge` |
| Writer, Designer, Composer | `creation` |
| Planner, Strategist, Architect | `strategy` |
| Critic, Auditor, Reviewer | `analysis` |
| Editor, Translator, Presenter | `communication` |
| Developer, Operator, Coordinator | `operations` |
| (others) | mapped per closest fit; user override at migration time |

### `AGENT_CATEGORIES` + `POWER_CATEGORIES` → single `POWER_CATEGORIES`

Merge into one taxonomy:

```ts
type PowerCategory =
  | 'knowledge'
  | 'creation'
  | 'strategy'
  | 'analysis'
  | 'communication'
  | 'operations';
```

The two parallel category lists in H02 collapse into this single set.
Existing values in either list map onto the unified set during migration
(largest fit wins; user can re-categorize).

### `MESSAGE_MODES` extension

Existing modes (`auto`, `plan`, `ask`, `learn`, `play`, `make`) are kept;
add `MessageMode` semantics in [04-modes.md](04-modes.md). No constant
changes needed beyond docs.

### `SOURCE_TYPES` — replace

```ts
// BEFORE
const SOURCE_TYPES = ['document', 'web', 'feed', 'database', 'text'] as const;

// AFTER
const SOURCE_TYPES = [
  'filesystem',
  'web',
  'api',
  'feed',
  'memory',
  'transcript',
  'connector-backed',
  'mcp-resource',
] as const;
```

### `OVERLAY_STATES` — keep

H02's `OverlayState` (5 fullscreen views) maps directly onto Mind tabs;
unchanged in v1.

### `MIND_FILTER_STEPS` — extend

The 5-tab Mind filter expands per [08-mind.md](08-mind.md). Old values
remain; new values added.

## Slice 4 — Component layer (`src/components/features/`)

### Affected directories

Search across `src/components/features/` for the following old terms;
update each occurrence:

| Old | New |
|---|---|
| `space.subAgents` | (remove; use template selector) |
| `space.chief` | derived from `claw.soulPath` content |
| `space.isHome` | derived selector `isMacroClaw(claw)` |
| `power.agentType` | (remove all references) |
| `userIntegration.*` | `connector.*` |

### Components requiring direct migration

Based on H02's directory layout (`src/components/features/`):

- `agents/` — All sub-agent UI. Refactor "agent roster on space" → "template
  picker on claw + ad-hoc spawn UI."
- `spaces/` (or whatever the equivalent) — Rename to `claws/`. Update home
  detection to derived selector.
- `sources/` — Update Source type rendering for the expanded taxonomy.
- `drafts/` — Add "Promote to…" target picker; update status enum.
- `powers/` — Remove `agentType` from forms; add `kind`, `requires`,
  `triggerSurface` editors.
- `mind/` — Add Receipts tab; Browser tab placeholder (v2); Costs tab
  populated.
- `settings/` — Add Connectors panel; MCP servers panel; Sidecar panel;
  Autonomy user-cap panel.
- `questions/` (carousel) — Add `kind === 'approval'` rendering branch.

### New components

- `claws/ClawWizard.tsx` — replaces SpaceWizard; same shape, recursive
- `autonomy/PostureMatrix.tsx` — the 12-axis matrix UI
- `autonomy/ApprovalCard.tsx` — inside QuestionCarousel for kind===approval
- `mind/CallTreeView.tsx` — three-node-kind renderer
- `mind/ReceiptsTab.tsx`
- `settings/ConnectorsPanel.tsx`
- `settings/MCPServersPanel.tsx`
- `settings/SidecarPanel.tsx` (Open Bias `:4000` config)

## Slice 5 — Cross-cutting

### `Question` consumers audit

All sites reading `Question` fields directly need a kind check (or a
default). Search for `Question` type usage in `src/components/features/` and
`src/stores/`. The default branch is `'clarification'` so existing UX is
unchanged for non-approval questions.

### Receipt insertion points

Every action that previously updated state without an audit row now MUST
emit a `Receipt`. Insertion points:

- After every Power invocation
- After every tool/connector/MCP op
- After every Source read (gated by autonomy posture)
- After every Draft creation, edit, promotion
- After every Sub-agent spawn / completion
- After every posture change
- After every plugin install / uninstall

Receipts are append-only. Updates to a logical state produce a new Receipt
with `parentReceiptId`.

### Routing pre-existing `cognitive-architecture-v2.md`

H02's existing `docs/cognitive-architecture-v2.md` is preserved (not
deleted). Add a header note: "This document predates the v3 architecture
specced in
[kelvinclaw/docs/kelvin-spec/](https://github.com/agentichighway/kelvinclaw/tree/main/docs/kelvin-spec).
Refer to that for current shapes."

## Verification

Before merging the migration to H02 main:

1. **Type-check passes** — `tsc --noEmit` clean across all `src/`.
2. **Store rehydration test** — load a saved Ginger state; verify migration
   produces a valid `ClawState` (validation rules from
   [09-data-model.md](09-data-model.md)).
3. **No `agentType` references** — `grep -rn 'agentType' src/` is empty.
4. **No `gingerStore` references** — `grep -rn 'gingerStore' src/` is empty.
5. **No `Space.isHome` references** — `grep -rn 'isHome' src/` is empty.
6. **`SOURCE_TYPES` legacy values absent** — no `'document' | 'database' |
   'text'` literals in code.
7. **Question consumers handle approval kind** — every Question read site
   either branches on `kind` or has a default-to-clarification fallback.
8. **Tests pass** — existing test suite green (unit + integration).
9. **Visual regression** — chat composer, Sources panel, Drafts panel render
   the same on equivalent inputs.

## Out-of-scope for this migration

- **Multi-user** — deferred to v2 ([ADR-004](decisions/004-single-user-v1.md)).
- **Mobile** — out of scope for v1.
- **Plugin authoring UI** — v2.
- **Browser tab in Mind** — v2.

## Cross-references

- [09-data-model.md](09-data-model.md) — target schemas
- [ADR-001](decisions/001-sub-agents-runtime-only.md) — Sub-agent migration
- [ADR-002](decisions/002-four-distinct-concepts.md) — four concepts
- [ADR-005](decisions/005-recursive-claw.md) — Claw recursion
- [ADR-006](decisions/006-soul-rules-files-and-question-reuse.md) — Question reuse
- [ADR-007](decisions/007-drafts-promotion-edge.md) — Draft promotion
- [ADR-008](decisions/008-three-postures-cap-invariant.md) — Posture invariants
- H02 source: https://github.com/kmondlane/h02/blob/main/src/types/index.ts
- H02 constants: https://github.com/kmondlane/h02/blob/main/src/constants/index.ts
