## 1. Validate the spec set

- [ ] 1.1 Run `openspec validate add-kelvin-ui-runtime --strict` and resolve any issues.
- [ ] 1.2 Confirm every gateway method in the catalogue maps to a typed error path for at least one failure mode.
- [ ] 1.3 Confirm Mind tabs each have a documented data source.

## 2. Resolve the five Open Questions in `design.md`

- [ ] 2.1 Measure Receipts storage growth on a representative usage profile; decide retention policy.
- [ ] 2.2 Decide Browser tab v1.5 vs v2 ship target.
- [ ] 2.3 Catalogue real-user filter needs for `mind.query-receipts`; surface as v2 MODIFIED requirements.
- [ ] 2.4 Decide server-side streamed-result buffering policy.
- [ ] 2.5 Decide gateway method versioning strategy (handshake vs per-method existence check).

## 3. Runtime implementation seeds (kelvinclaw)

- [ ] 3.1 Add gateway methods to the existing WebSocket gateway: `claw.send-message`, `claw.list/create/update/delete`, `power.invoke`, `subagent.spawn/kill`, `delegation.invoke`, `draft.promote/discard`, `question.answer`, `posture.set`, `posture.override.add/revoke`, `connector.list/add/bind/unbind`, `mcp.list/add/bind/unbind`, `trigger.create/update/delete/fire`, `sources.list/add/read`, `mind.query-receipts`, `mind.query-call-tree`, `costs.query`, `sidecar.health`.
- [ ] 3.2 Implement monotonic per-connection `seq` numbers for events.
- [ ] 3.3 Implement reconnect-with-resume: `subscribe.fromSeq` replays; `gap-too-large` error when retention window exceeded.
- [ ] 3.4 Implement typed errors with codes.
- [ ] 3.5 Implement `throttle` backpressure response when queue depth exceeds capacity.
- [ ] 3.6 Restrict gateway WebSocket upgrade to loopback (`127.0.0.1`, `::1`); 403 on remote.
- [ ] 3.7 Implement `mind.query-receipts` filtering by claw / time / kind; opt-in `format: 'jsonl' | 'csv'`; opt-in `stream: true`.
- [ ] 3.8 Implement `mind.query-call-tree` walking `parentReceiptId`.
- [ ] 3.9 Implement `costs.query` aggregation by claw / Power / session / vs-budget.
- [ ] 3.10 Implement Receipts retention floor of 90 days minimum; surface storage notification when growth exceeds configured cap.

## 4. H02 implementation (separate H02 PR)

- [ ] 4.1 Implement mode-contract enforcement client-side: Plan/Ask/Learn modes prevent submit-error-prone calls before they leave H02.
- [ ] 4.2 Add `claws/ClawWizard.tsx` (replaces SpaceWizard).
- [ ] 4.3 Add `mind/CallTreeView.tsx` and per-node-kind renderers (`mind/CallTreeNode.{PowerInvocation,SubAgentSpawn,SubClawDelegation}.tsx`).
- [ ] 4.4 Add `mind/ReceiptsTab.tsx` with filter chain UI.
- [ ] 4.5 Add `mind/CostsTab.tsx` with aggregations.
- [ ] 4.6 Add `mind/NotificationsTab.tsx` reading the event stream.
- [ ] 4.7 Add Browser tab placeholder.
- [ ] 4.8 Implement reconnect-with-resume client logic; on `gap-too-large`, re-fetch Mind state and resume from fresh `seq`.
- [ ] 4.9 Implement client-side queue with depth cap (default 32) and `throttle` honouring.
- [ ] 4.10 Update existing components for: composer mode chip with locked mode (long-press); Sub-agent inheritance display; Routine mode override.

## 5. Integration tests

- [ ] 5.1 `composer-modes`: submit a `plan`-mode message that would otherwise call a Connector; assert no Connector op is executed and a Plan Draft is produced. Submit `ask` mode; assert no Sub-agent spawn. Plan + High autonomy still does not execute.
- [ ] 5.2 `mind-observability`: query Receipts with filters; assert correct subset returned. Walk call-tree from a leaf Receipt; assert all ancestors and siblings present. Receipt 30 days old still queryable.
- [ ] 5.3 `gateway-protocol`: disconnect, reconnect with `fromSeq`; assert events 1235..end replayed in order. Send `submit.method = 'foo.bar'`; assert `unknown-method` error. Trigger backpressure; assert `throttle` response.

## 6. Archive

- [ ] 6.1 Once foundations and security are archived AND all scenarios have backing tests passing, run `openspec archive add-kelvin-ui-runtime`. Capabilities lift into `openspec/specs/<capability>/spec.md`. v1 baseline complete.
