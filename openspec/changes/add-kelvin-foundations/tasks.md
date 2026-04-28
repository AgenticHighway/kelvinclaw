## 1. Validate the spec set

- [ ] 1.1 Run `openspec validate add-kelvin-foundations --strict` and resolve any issues.
- [ ] 1.2 Confirm every requirement maps to at least one runtime invariant (cap-chain, subset bindings, ownership fields, etc.).
- [ ] 1.3 Confirm scenarios are concrete enough to lift into integration tests.

## 2. Resolve the four Open Questions in `design.md`

- [ ] 2.1 Decide whether to keep `ownerId` / `createdBy` in the v1 schema.
- [ ] 2.2 Decide whether to relax the Sub-agent depth cap from 1 in v1 (or document why depth=1 is permanent).
- [ ] 2.3 Confirm acceptable scope for cycle prevention (in-session only).
- [ ] 2.4 Confirm UX strategy for `delegate-to-sub-claw` Powers (hidden from default Powers UI).

## 3. Runtime implementation seeds (kelvinclaw)

- [ ] 3.1 Define schemas in `crates/kelvin-core/` (or wherever the data layer lives) for: `Claw`, `Source`, `Draft`, `Power`, `Connector`, `MCPServer`, `SubAgentTemplate`, `Receipt`, `Trigger`, `Channel`, `PosturePerAxis`, `PostureOverride`, `CostBudget`.
- [ ] 3.2 Implement `SubAgentInstance` as a runtime-only registry that emits Receipts but does not persist instance state.
- [ ] 3.3 Implement append-only Receipt store with index on `parentReceiptId`.
- [ ] 3.4 Implement validation for the ten data-model invariants (cap chain, subset bindings, `Power.requires` resolution, Source `config.kind` matches `type`, Receipt immutability, etc.).
- [ ] 3.5 Implement claw filesystem layout: `<KELVIN_DATA_DIR>/claws/<id>/{soul,RULES}.md`.

## 4. H02 migration (separate H02 PR)

- [ ] 4.1 Type-layer: `Space` → `Claw` rename + new fields (`soulPath`, `rulesPath`, `boundConnectorIds`, `boundMcpServerIds`, `subAgentTemplateIds`, `autonomyPosture`).
- [ ] 4.2 Type-layer: `Power` removes `agentType`; gains `kind`, `requires`, optional `model`, `triggerSurface`, install-source fields.
- [ ] 4.3 Type-layer: `SOURCE_TYPES` expands; legacy values map to new types.
- [ ] 4.4 Type-layer: `SubAgent` config splits into `SubAgentTemplate`; remove stored instance state.
- [ ] 4.5 Type-layer: `UserIntegration` migrates to `Connector`. New `MCPServer`, `Receipt`, `Trigger`, `Channel`, `PostureOverride`, `CostBudget` types.
- [ ] 4.6 Store: `useGingerStore` → `useClawStore`; selector renames; new actions (`bindConnector`, `bindMcpServer`, `promoteDraft`, `addReceipt`, `setPosture`).
- [ ] 4.7 Constants: collapse `AGENT_CATEGORIES` + `POWER_CATEGORIES`; update `MIND_FILTER_STEPS`; remove `Space.isHome` consumers.
- [ ] 4.8 Component layer: update `src/components/features/agents/`, `spaces/` (rename to `claws/`), `sources/`, `drafts/`, `powers/`.
- [ ] 4.9 Add migration runner that converts saved `gingerStore` state to `clawStore` state on first load.

## 5. Integration tests (one per capability scenario set)

- [ ] 5.1 `claw-anatomy`: build a claw tree depth=3; assert subset invariants on bind/unbind; assert exactly-one-macro-claw on rehydration.
- [ ] 5.2 `concepts-taxonomy`: install a Power requiring a Connector not bound to the claw; assert rejection. Spawn a Sub-agent ad-hoc without a template; assert success.
- [ ] 5.3 `data-model`: attempt to mutate a Receipt; assert `receipt-immutable` error. Attempt cap-chain violation at depth 2; assert rejection.
- [ ] 5.4 `delegation-call-tree`: produce all three node kinds in one tree; assert `parentReceiptId` chain intact. Attempt ancestor-delegation; assert `cycle-detected`.

## 6. Archive

- [ ] 6.1 Once all scenarios have backing tests passing, run `openspec archive add-kelvin-foundations`. Capabilities lift into `openspec/specs/<capability>/spec.md` to act as the baseline for `add-kelvin-security`.
