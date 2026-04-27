---
status: Draft
version: v1
audience: implementors
date: 2026-04-27
---

# Data Model — TypeScript Schemas

This doc is the source of truth for the Kelvin GUI data model. Schemas are
TypeScript-flavored (the H02 GUI is TypeScript-first) but applies to the
runtime contract too — the kelvinclaw `ModelProvider` shim and the WebSocket
gateway both speak this shape.

For the migration path from H02's existing `useGingerStore` to this model,
see [10-h02-migration.md](10-h02-migration.md).

## Conventions

- Every persistent entity carries `id`, `createdAt`, `updatedAt`,
  `ownerId`, `createdBy`. `ownerId` and `createdBy` are populated with the
  single-user value in v1 ([ADR-004](decisions/004-single-user-v1.md)) but
  exist for v2 multi-user.
- "v2:" annotations mark fields that exist for forward-compat but are not
  enforced in v1.
- Discriminated unions use a `kind: '...'` tag.
- All times are ISO-8601 strings at the wire format; `Date` in TypeScript.

## Top-level entities

```ts
// === CLAW ===
// The recursive primitive. ADR-005.
interface Claw {
  id: string;
  name: string;
  parentClawId: string | null;       // null only for the macro claw
  iconRef?: string;
  description?: string;

  // File-backed config (per ADR-006)
  soulPath: string;                   // path to soul.md
  rulesPath: string;                  // path to RULES.md

  // Per-claw collections
  sourceIds: string[];
  draftIds: string[];
  powerIds: string[];
  triggerIds: string[];
  channelIds: string[];               // bound subset of installed Channels

  // Bindings (subset invariants per ADR-005)
  boundConnectorIds: string[];        // subset of installed Connectors
  boundMcpServerIds: string[];        // subset of installed MCP servers

  // Optional spawning presets
  subAgentTemplateIds: string[];

  // Autonomy (per axis; see 05-autonomy-postures.md)
  autonomyPosture: PosturePerAxis;    // capped by parent

  // Ownership (single-user v1; ADR-004)
  ownerId: string;
  createdBy: string;
  createdAt: Date;
  updatedAt: Date;

  // v2 placeholders (forward-compat; not enforced in v1)
  privacy?: 'private' | 'shared' | 'collaborative';
  allowedUserIds?: string[];
}
```

```ts
// === SOURCE ===
type SourceType =
  | 'filesystem'
  | 'web'
  | 'api'
  | 'feed'
  | 'memory'
  | 'transcript'
  | 'connector-backed'
  | 'mcp-resource';

interface Source {
  id: string;
  clawId: string;                     // owning claw
  type: SourceType;
  name: string;
  description?: string;

  // Type-discriminated payload
  config:
    | { kind: 'filesystem'; path: string; allowList?: string[] }
    | { kind: 'web'; baseUrl: string; allowedDomains?: string[] }
    | { kind: 'api'; endpoint: string; authRef?: string }
    | { kind: 'feed'; url: string }
    | { kind: 'memory'; storePath: string }
    | { kind: 'transcript'; sessionScope: 'all' | 'recent' | 'pinned' }
    | { kind: 'connector-backed'; connectorId: string; resource: string }
    | { kind: 'mcp-resource'; mcpServerId: string; resourceUri: string };

  // v2:
  trustScore?: number;                // 0..1; 1 = highest trust

  // Hierarchical (preserved from H02 schema)
  parentSourceId?: string;
  childCount?: number;

  ownerId: string;
  createdBy: string;
  createdAt: Date;
  updatedAt: Date;
}
```

```ts
// === DRAFT ===
type DraftStatus = 'generating' | 'ready' | 'promoted';

interface Draft {
  id: string;
  clawId: string;
  title: string;
  content: string;                    // markdown / text / serialized doc
  contentType: 'markdown' | 'diff' | 'plan' | 'json' | 'binary-ref';

  status: DraftStatus;

  // Provenance
  bornFromReceiptId: string;          // immutable link to the action that produced it
  bornFromPowerId?: string;
  bornFromSubAgentInstanceId?: string;

  // Promotion (ADR-007)
  promotionTargets?: PromotionTarget[]; // candidates the user/auto can pick
  promotedToSourceIds?: string[];     // populated when status === 'promoted'
  promotedAt?: Date;
  promotedAutonomySnapshot?: PosturePerAxis;

  ownerId: string;
  createdBy: string;
  createdAt: Date;
  updatedAt: Date;
}

interface PromotionTarget {
  targetSourceId: string;
  operation: 'append' | 'overwrite' | 'connector-op' | 'mcp-op';
  operationArgs?: Record<string, unknown>;
}
```

```ts
// === POWER ===
type PowerCategory =
  | 'knowledge'
  | 'creation'
  | 'strategy'
  | 'analysis'
  | 'communication'
  | 'operations';
// (Consolidated from H02's two parallel taxonomies; see 10-h02-migration.md)

interface Power {
  id: string;
  clawId: string;                     // a Power lives in exactly one claw's library
  kind: 'skill' | 'workflow' | 'delegate-to-sub-claw';
  name: string;
  category: PowerCategory;
  description: string;
  icon?: string;

  // Optional model binding (when present, this Power runs its own inference
  // in a sub-session). Formerly modeled as "sub-agent."
  model?: {
    provider: 'anthropic' | 'openai' | 'openrouter' | string;
    id: string;                       // e.g. 'claude-sonnet-4-6'
    systemPrompt?: string;
    parameters?: Record<string, unknown>;
  };

  // Declared dependencies — a Power can only run if these are bound on the claw
  requires?: {
    sources?: string[];               // SourceId[]
    tools?: string[];                 // ToolRegistry entries
    connectors?: string[];            // ConnectorId[] (must be in claw.boundConnectorIds)
    mcps?: string[];                  // MCPServerId[] (must be in claw.boundMcpServerIds)
    powers?: string[];                // PowerId[] (for composed workflows)
  };

  // Workflow-only
  steps?: WorkflowStep[];

  // Invocation surface
  triggerSurface: ('slash' | 'auto-router' | 'mention' | 'routine')[];
  signature?: ParamSchema;            // typed args (JSON schema or zod)

  // Lifecycle
  installSource?: 'builtin' | 'plugin' | 'user-authored';
  installedFrom?: string;             // plugin id / git ref / local path
  signed?: boolean;
  trustedPublisher?: string;

  isPinned: boolean;
  lastUsedAt?: Date;

  ownerId: string;
  createdBy: string;
  createdAt: Date;
  updatedAt: Date;
}

interface WorkflowStep {
  id: string;
  powerId: string;                    // composed Power
  argsTemplate?: Record<string, unknown>;
  condition?: string;                 // expression; v2 may formalize
}
```

```ts
// === CONNECTOR ===
interface Connector {
  id: string;
  serviceId: string;                  // e.g., 'gmail', 'slack', 'github'
  displayName: string;
  description: string;
  icon?: string;

  // Authentication
  authMethod: 'oauth2' | 'api-key' | 'oauth1' | 'composio-managed';
  authRef: string;                    // opaque reference to credential storage; never plaintext
  scopes?: string[];

  // Capabilities
  exposedOperations: ConnectorOp[];

  // Lifecycle
  status: 'unauthenticated' | 'active' | 'expired' | 'revoked';
  authenticatedAt?: Date;
  expiresAt?: Date;

  ownerId: string;                    // single user in v1
  createdBy: string;
  createdAt: Date;
  updatedAt: Date;
}

interface ConnectorOp {
  id: string;                         // e.g., 'gmail.send'
  name: string;
  description: string;
  isWrite: boolean;                   // determines autonomy posture row
  signature?: ParamSchema;
}
```

```ts
// === MCP SERVER ===
interface MCPServer {
  id: string;
  name: string;
  description: string;

  endpoint:
    | { kind: 'stdio'; command: string; args?: string[]; cwd?: string }
    | { kind: 'remote-http'; url: string; authRef?: string }
    | { kind: 'remote-ws'; url: string; authRef?: string };

  // Discovered tool set (refreshed via MCP protocol)
  toolDefs: MCPToolDef[];
  resourceDefs: MCPResourceDef[];
  promptDefs: MCPPromptDef[];

  // Trust
  signed?: boolean;
  trustedPublisher?: string;
  pinnedVersion?: string;

  status: 'not-running' | 'starting' | 'active' | 'error';
  lastSeenAt?: Date;

  ownerId: string;
  createdBy: string;
  createdAt: Date;
  updatedAt: Date;
}

interface MCPToolDef {
  name: string;
  description: string;
  isWrite: boolean;
  signature?: ParamSchema;
}

interface MCPResourceDef {
  uri: string;
  name: string;
  mimeType?: string;
}

interface MCPPromptDef {
  name: string;
  description: string;
  arguments?: ParamSchema;
}
```

```ts
// === SUB-AGENT (TEMPLATE + INSTANCE) ===
// Per ADR-001, instances are runtime-only.

interface SubAgentTemplate {
  id: string;
  clawId: string;                     // a template belongs to one claw
  role: string;                       // 'Researcher', 'Critic', ...
  description?: string;
  systemPrompt: string;               // seed prompt
  defaultPowerIds: string[];          // allowlist of Powers spawned with
  defaultModel?: Power['model'];      // optional default model
  defaultBudget?: SubAgentBudget;
  category?: PowerCategory;           // for UI grouping

  ownerId: string;
  createdBy: string;
  createdAt: Date;
  updatedAt: Date;
}

interface SubAgentBudget {
  maxTokens?: number;
  maxDollars?: number;
  maxWallclockMs?: number;
  maxSubSessions?: number;            // depth or fan-out limit
}

// Runtime-only; not persisted in the store
interface SubAgentInstance {
  id: string;
  parentClawId: string;
  templateId?: string;                // optional source template
  role: string;
  systemPrompt: string;
  allowedPowerIds: string[];
  budget: SubAgentBudget;
  budgetUsed: SubAgentBudget;

  status: 'spawning' | 'running' | 'completed' | 'failed' | 'killed';
  spawnedAt: Date;
  completedAt?: Date;
  spawnedByReceiptId: string;
  resultDraftIds?: string[];
}
```

```ts
// === TRIGGER ===
type TriggerKind = 'hook' | 'heartbeat' | 'watch';

interface Trigger {
  id: string;
  clawId: string;
  kind: TriggerKind;
  name: string;
  description?: string;

  config:
    | { kind: 'hook'; eventType: string; sourceFilter?: Record<string, unknown> }
    | { kind: 'heartbeat'; cron: string; jitterMs?: number }
    | { kind: 'watch'; targetSourceId: string; predicate: string };

  // What fires when
  invokes: {
    powerId?: string;
    spawnFromTemplateId?: string;
    args?: Record<string, unknown>;
  };

  // Routines posture (separate from claw's interactive posture; ADR-008)
  enabled: boolean;
  enabledWhenUserAbsent: boolean;

  ownerId: string;
  createdBy: string;
  createdAt: Date;
  updatedAt: Date;
}
```

```ts
// === CHANNEL ===
type ChannelSurface =
  | 'web'
  | 'voice'
  | 'telegram'
  | 'discord'
  | 'email'
  | 'sms'
  | 'slack';

interface Channel {
  id: string;                         // global instance
  surface: ChannelSurface;
  displayName: string;
  config: Record<string, unknown>;    // surface-specific (bot token ref, webhook URL, ...)

  // Bi-directional permissions per ADR (channels are global; per-claw bindings live on Claw)
  inboundEnabled: boolean;
  outboundEnabled: boolean;

  status: 'configured' | 'active' | 'error';

  ownerId: string;
  createdBy: string;
  createdAt: Date;
  updatedAt: Date;
}
```

```ts
// === RECEIPT ===
// Immutable audit row. Distinct from Draft. Per ADR-007.

type ReceiptKind =
  | 'power-invocation'
  | 'sub-agent-spawn'
  | 'sub-claw-delegation'
  | 'tool-call'
  | 'connector-op'
  | 'mcp-op'
  | 'source-read'
  | 'draft-promotion'
  | 'memory-write'
  | 'posture-change';

interface Receipt {
  id: string;
  kind: ReceiptKind;
  timestamp: Date;
  clawId: string;                     // which claw's posture was in effect
  parentReceiptId?: string;           // for call-tree assembly

  actor: {
    kind: 'user' | 'macro-claw' | 'sub-claw' | 'sub-agent' | 'routine';
    id: string;
  };

  // Action specifics (discriminated by kind)
  action: Record<string, unknown>;

  // Outcome
  outcome: 'allowed' | 'denied-policy' | 'denied-posture' | 'failed' | 'completed';
  outcomeDetail?: string;

  // Posture snapshot — what posture was in effect when this happened
  posture: PosturePerAxis;

  // Open Bias trace correlation (when applicable)
  otelTraceId?: string;

  // Cost accounting
  tokensIn?: number;
  tokensOut?: number;
  costDollars?: number;
  wallclockMs?: number;

  // Cross-references
  resultDraftIds?: string[];
  resultSourceUpdates?: string[];

  ownerId: string;
}
```

```ts
// === AUTONOMY POSTURE ===
// Per ADR-008. Per-axis, named "Low" | "Medium" | "High" overall.

type PostureLevel = 'low' | 'medium' | 'high';

interface PosturePerAxis {
  // Each row of the autonomy matrix (05-autonomy-postures.md)
  toolExecution: PostureLevel;
  subClawDelegation: PostureLevel;
  subAgentSpawn: PostureLevel;
  sourceReads: PostureLevel;
  connectorWrites: PostureLevel;
  draftPromotion: PostureLevel;
  pluginInstall: PostureLevel;
  memoryWrites: PostureLevel;
  wasmEgress: PostureLevel;
  routinesUserAbsent: PostureLevel;
  crossClawPorosity: PostureLevel;
  powerModelSpend: PostureLevel;
}

interface PostureOverride {
  id: string;
  clawId: string | 'user';            // 'user' for the user-cap level
  axis: keyof PosturePerAxis;
  scope: 'once' | 'session' | 'claw' | 'forever';
  // Per-axis override is "remember this answer" for a class of action
  decision: 'allow' | 'deny';
  actionFilter?: Record<string, unknown>; // optional narrowing
  createdAt: Date;
  expiresAt?: Date;
  createdBy: string;
}
```

```ts
// === QUESTION (extended for approvals, per ADR-006) ===
// H02's existing Question type, with a kind discriminator added.

type QuestionKind = 'clarification' | 'approval';

interface Question {
  id: string;
  kind: QuestionKind;
  title: string;
  description: string;
  category: 'conflict' | 'opportunity' | 'clarification' | 'autonomy';

  clawId: string;                     // which claw raised the question
  spaceId?: string;                   // legacy field; v2 maps onto Claw
  planId?: string;
  planName?: string;

  // For both clarification and approval kinds:
  selectionType: 'single' | 'multi';
  options: QuestionOption[];
  selectedOptionIds: string[];
  urgency: 'normal' | 'high' | 'critical';
  status: 'pending' | 'answered' | 'snoozed' | 'deferred';
  traceItems: QuestionTraceItem[];
  attachments: QuestionAttachment[];

  // Approval-kind only
  actionDescriptor?: ActionDescriptor;       // what is being requested
  defaultChoiceId?: string;                  // recommended choice
  scopeOptions?: ('once' | 'session' | 'claw' | 'forever')[];
  riskLevel?: 'low' | 'medium' | 'high';     // derived from autonomy posture
  expiresAt?: Date;                          // soft auto-deny deadline

  createdAt: Date;
  answeredAt?: Date;
}

interface ActionDescriptor {
  kind: ReceiptKind;
  summary: string;
  details: Record<string, unknown>;
}

// QuestionOption, QuestionTraceItem, QuestionAttachment unchanged from H02
```

## Auxiliary types

```ts
type ParamSchema =
  | { kind: 'json-schema'; schema: Record<string, unknown> }
  | { kind: 'zod'; ref: string };

// Cost & budgets (global Settings)
interface CostBudget {
  scope: 'install' | 'claw';
  scopeId: string | null;             // null for install-wide
  periodKind: 'daily' | 'weekly' | 'monthly';
  capDollars: number;
  capTokens?: number;
  // v3: hard cutoff vs. v1 informational only
  hardCutoff: false;                  // v1 informational; v3 enables
  spentDollars: number;
  spentTokens: number;
  resetsAt: Date;
}
```

## Discriminated unions across the model

Several types appear in unified observability surfaces (Mind tabs). Keep
these in sync if you add new variants:

```ts
type FeedItem =
  | { kind: 'receipt'; data: Receipt }
  | { kind: 'draft'; data: Draft }
  | { kind: 'task'; data: Task }            // unchanged from H02
  | { kind: 'thought'; data: Thought }      // unchanged from H02
  | { kind: 'plan'; data: Plan }            // unchanged from H02
  | { kind: 'question'; data: Question };
```

## Validation rules

These are enforced by the store (write-time) AND by the runtime (server-side).

1. `Claw.parentClawId === null` for exactly one claw per install.
2. `Claw.boundConnectorIds ⊆ parentClaw.boundConnectorIds` (recursive).
3. `Claw.boundMcpServerIds ⊆ parentClaw.boundMcpServerIds` (recursive).
4. For every `Power.requires.connectors[i]`, `Power.clawId.boundConnectorIds`
   must include it.
5. For every `Power.requires.mcps[i]`, `Power.clawId.boundMcpServerIds` must
   include it.
6. `Source.config.kind` must match `Source.type`.
7. `PosturePerAxis[axis] <= parentClaw.PosturePerAxis[axis]` for every axis.
8. `Draft.promotedToSourceIds` populated implies `Draft.status === 'promoted'`.
9. A Receipt is append-only; updates produce a new Receipt with
   `parentReceiptId` link.
10. `SubAgentInstance` is never persisted to the store — only to runtime
    memory and to Receipts as a reference.

## Cross-references

- [01-claw-anatomy.md](01-claw-anatomy.md) — what each box maps to in the
  schema
- [10-h02-migration.md](10-h02-migration.md) — how H02's existing types
  migrate
- [05-autonomy-postures.md](05-autonomy-postures.md) — `PosturePerAxis`
  semantics
- [08-mind.md](08-mind.md) — `Receipt` and `FeedItem` rendering
- [ADR-001](decisions/001-sub-agents-runtime-only.md) — `SubAgentInstance`
  is runtime-only
- [ADR-007](decisions/007-drafts-promotion-edge.md) — `Draft.promotion*`
  fields
