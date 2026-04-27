## 1. Validate prose-to-spec round-trip

- [ ] 1.1 Confirm every requirement in `specs/*/spec.md` is grounded in a specific section of `docs/kelvin-spec/`.
- [ ] 1.2 Confirm every WHEN/THEN scenario maps to a testable assertion (no vague "system handles X correctly").
- [ ] 1.3 Run `openspec validate kelvin-h02-gui-architecture --strict` and resolve any issues.

## 2. Surface and resolve assumptions

- [ ] 2.1 Walk the ten Open Questions in `design.md` with a stakeholder; mark each as `accept-as-is`, `validate-during-impl`, or `revise-spec-now`.
- [ ] 2.2 For any `revise-spec-now` items, edit the corresponding capability spec and re-validate.
- [ ] 2.3 Add a v1 release-notes draft listing the `accept-as-is` items so users see them.

## 3. Runtime implementation seeds (v1, kelvinclaw)

- [ ] 3.1 Implement `OpenBiasShimProvider` (per `docs/kelvin-spec/interfaces/sidecar-integration.md`) in `crates/kelvin-providers/`.
- [ ] 3.2 Extend `ToolRegistry` with autonomy-posture enforcement (per `docs/kelvin-spec/interfaces/tool-gate-postures.md`) in `crates/kelvin-core/`.
- [ ] 3.3 Add `PostureService` with cap-chain cache.
- [ ] 3.4 Add `Receipt` append-only store with `parentReceiptId` index.
- [ ] 3.5 Add WebSocket gateway methods listed in the `gateway-protocol` capability.
- [ ] 3.6 Add `sidecar-health` event emission and aggregate state.
- [ ] 3.7 Add per-claw filesystem layout (`<KELVIN_DATA_DIR>/claws/<id>/{soul,RULES}.md`).

## 4. H02 implementation (separate H02 repo / branch)

- [ ] 4.1 Execute the slice plan in `docs/kelvin-spec/10-h02-migration.md`:
  - [ ] 4.1.1 Type-layer renames (Space→Claw, etc.)
  - [ ] 4.1.2 Store rename (`useGingerStore` → `useClawStore`)
  - [ ] 4.1.3 Constants update (`SOURCE_TYPES`, `POWER_CATEGORIES`)
  - [ ] 4.1.4 Component-layer updates under `src/components/features/`
  - [ ] 4.1.5 `Question` kind discriminator threading
- [ ] 4.2 Add new components: `claws/ClawWizard.tsx`, `autonomy/PostureMatrix.tsx`, `autonomy/ApprovalCard.tsx`, `mind/CallTreeView.tsx`, `mind/ReceiptsTab.tsx`, `settings/{Connectors,MCPServers,Sidecar}Panel.tsx`.
- [ ] 4.3 Connect to kelvinclaw via the gateway-protocol contract.

## 5. Sidecar deployment

- [ ] 5.1 Write `docker-compose.yml` pinning Open Bias to a specific image tag with `KELVIN_DATA_DIR` mounted.
- [ ] 5.2 Ship a permissive `RULES.md` template for development environments.
- [ ] 5.3 Add startup health-probe wiring; verify fail-closed is mandatory in production config.

## 6. Validation per capability

For each of the ten capabilities, write at least one integration test
that exercises a representative WHEN/THEN scenario from its spec:

- [ ] 6.1 `claw-anatomy` — create a claw tree of depth 3, verify subset invariants.
- [ ] 6.2 `concepts-taxonomy` — install a Power that requires a Connector not bound to the claw; assert rejection.
- [ ] 6.3 `delegation-call-tree` — produce a three-kind call-tree end-to-end; assert `parentReceiptId` chain is intact.
- [ ] 6.4 `composer-modes` — submit a `plan`-mode message that would otherwise call a Connector; assert no Connector op is executed.
- [ ] 6.5 `autonomy-postures` — flip parent posture; assert child effective posture recomputes.
- [ ] 6.6 `approvals-primitive` — emit an approval, resolve as `forever`; assert PostureOverride persists.
- [ ] 6.7 `security-sidecars` — kill Open Bias; assert all model calls fail-closed and `sidecar-health` emits.
- [ ] 6.8 `mind-observability` — query Receipts with filters, assert call-tree assembly walks parent links.
- [ ] 6.9 `data-model` — attempt to mutate a Receipt; assert rejection with `receipt-immutable`.
- [ ] 6.10 `gateway-protocol` — disconnect/reconnect with `fromSeq`; assert events 1235..end are replayed.

## 7. Archive

- [ ] 7.1 Once all WHEN/THEN scenarios have backing tests passing, run `openspec archive kelvin-h02-gui-architecture`. This moves the change to `openspec/changes/archive/<date>-kelvin-h02-gui-architecture/` and lifts the requirements into `openspec/specs/<capability>/spec.md` for use as the v2 baseline.
