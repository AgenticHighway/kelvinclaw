## 1. Validate the spec set

- [ ] 1.1 Run `openspec validate add-kelvin-security --strict` and resolve any issues.
- [ ] 1.2 Confirm the 12-axis matrix labels are identical across `autonomy-postures`, `approvals-primitive` (in scenarios), and `security-sidecars`.
- [ ] 1.3 Confirm sidecar-down behaviour is consistently specified (floor to Low + banner + fail-closed model calls + denied tool calls).

## 2. Resolve the five Open Questions in `design.md`

- [ ] 2.1 Spike: verify `RULES.md` actually works as specified end-to-end against a running Open Bias instance with three example rule files (Personal / Work / Health).
- [ ] 2.2 Decide WASM-egress→preset mapping (defaults look right; confirm with a UI mockup of the posture editor).
- [ ] 2.3 Accept or mitigate the localhost bind-race risk; document in release notes either way.
- [ ] 2.4 Audit the `kelvinclaw-plugins/trusted_publishers.kelvin.json` manifest's curation policy.
- [ ] 2.5 Decide v1 mitigation for `forever`-scope override accumulation.

## 3. Sidecar deployment

- [ ] 3.1 Write `docker-compose.yml` pinning Open Bias to a specific image tag with `KELVIN_DATA_DIR` mounted.
- [ ] 3.2 Ship a permissive `RULES.md` template at `<KELVIN_DATA_DIR>/templates/RULES.dev.md` for development environments.
- [ ] 3.3 Ship example `RULES.md` for the macro claw and three seed sub-claws (Personal / Work / Health).

## 4. Runtime implementation seeds (kelvinclaw)

- [ ] 4.1 Implement `OpenBiasShimProvider` wrapping the chosen upstream `ModelProvider` in `crates/kelvin-providers/`.
- [ ] 4.2 Implement startup health probe (Open Bias `:4000/health`); refuse to start with `fail_closed = false`; refuse non-loopback URL.
- [ ] 4.3 Implement steady-state health watcher (5-second probes); emit `sidecar-health` events on state transitions.
- [ ] 4.4 Implement `X-Kelvin-Claw-Rules-Ref` and `X-Kelvin-Claw-Posture` header injection per call.
- [ ] 4.5 Implement OpenTelemetry parent-trace-id propagation; populate `Receipt.otelTraceId`.
- [ ] 4.6 Implement `PostureService` with cap-chain cache; invalidate per-claw on mutation.
- [ ] 4.7 Extend `ToolRegistry` with autonomy-posture enforcement; map tool properties → axis; strictest-axis-wins.
- [ ] 4.8 Implement WASM sandbox preset selection from `wasmEgress` axis (`low → locked_down`, `medium → dev_local`, `high → hardware_control`).
- [ ] 4.9 Implement `PostureOverride` matching at the gate ("remember this" once / session / claw / forever).
- [ ] 4.10 Implement posture-change Receipt emission on every base-posture or override mutation.

## 5. H02 implementation (separate H02 PR)

- [ ] 5.1 Add `Question.kind` discriminator threading through `QuestionCarousel` consumers.
- [ ] 5.2 Add `ApprovalCard.tsx` component: title, risk pill, action descriptor renderers (one per `ReceiptKind`), why-gated section, scope picker, decision buttons, termination control.
- [ ] 5.3 Add per-`ActionDescriptor.kind` detail-block renderers (9 total).
- [ ] 5.4 Add `PostureMatrix.tsx`: 12-axis grid, per-axis override pills, active forever-overrides list with revoke buttons, posture badge with `*` indicator.
- [ ] 5.5 Add `SidecarPanel.tsx` in Settings: Open Bias `:4000` URL, fail-closed display (read-only `true`), RULES.md inheritance toggle (placeholder for v2).
- [ ] 5.6 Add sidecar-health banner above the carousel.
- [ ] 5.7 Add forever-confirmation dialog when scope=`forever` is selected.

## 6. Integration tests (one per capability scenario set)

- [ ] 6.1 `autonomy-postures`: flip parent posture; assert child effective posture recomputes per cap-chain. Trigger sidecar-down; assert floor to Low across the install.
- [ ] 6.2 `approvals-primitive`: emit an approval; resolve as `forever`; assert PostureOverride persists and posture-change Receipt emits. Test approval expiry auto-deny.
- [ ] 6.3 `security-sidecars`: kill Open Bias; assert all model calls return `denied-policy` with detail `'open-bias-unreachable'`. Send model call; assert headers `X-Kelvin-Claw-Rules-Ref` and `X-Kelvin-Claw-Posture` present. Confirm OpenTelemetry trace id ends up on the Receipt.

## 7. Archive

- [ ] 7.1 Once foundations are archived AND all scenarios have backing tests passing, run `openspec archive add-kelvin-security`. Capabilities lift into `openspec/specs/<capability>/spec.md`.
