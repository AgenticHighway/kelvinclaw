## 1. kelvinclaw — Posture data shape

- [ ] 1.1 Define `Posture` struct in `crates/kelvin-core/` with field `tool_execution: PostureLevel` where `PostureLevel = Low | Medium | High`.
- [ ] 1.2 Extend the `Claw` record from s1/s2 to include `posture: Posture` with default `tool_execution: Medium`.
- [ ] 1.3 Migration on runtime start: any persisted Claw without `posture` gets defaulted to `{ tool_execution: Medium }`.

## 2. kelvinclaw — Tool metadata flags

- [ ] 2.1 Add `is_write: bool` and `is_external: bool` to the `Tool` trait or `ToolDefinition`.
- [ ] 2.2 Audit every built-in tool in kelvinclaw and tag flags. (kelvin-cli-toolpack, websearch, wiki, etc.)
- [ ] 2.3 Document the flags in `docs/plugins/tool-plugin-abi.md` and update the plugin index schema (coordinate with `agentichighway/kelvinclaw-plugins` for ABI extension; default flags for legacy plugins = `is_write: true, is_external: true` so unknown defaults to "ask").

## 3. kelvinclaw — Posture gate

- [ ] 3.1 Create a `posture_gate` module that wraps `ToolRegistry::dispatch`. Signature: `fn evaluate(claw: &Claw, tool: &dyn Tool) -> GateOutcome` where `GateOutcome = AutoAllow | Ask | AutoDeny`.
- [ ] 3.2 Implement the gate logic per spec scenarios (Low → ask always; Medium → ask if write or external; High → auto-allow).
- [ ] 3.3 Wire the gate in front of `Tool::call` from the brain's tool-loop.
- [ ] 3.4 Implement suspension: when `Ask`, store an `ApprovalRecord { id, claw_id, run_id, tool_name, arguments, ... }` in a runtime-only registry; emit `approval.requested` event; block the tool-call future on a oneshot channel.

## 4. kelvinclaw — Approval gateway flow

- [ ] 4.1 Register `approval.respond` method in `apps/kelvin-gateway/`. Params: `{ approval_id, decision: 'allow' | 'deny' }`.
- [ ] 4.2 Implement `approval.respond` handler: look up suspended call, fire the oneshot channel with the user's decision, then emit `approval.resolved` event.
- [ ] 4.3 On `decision: 'allow'`, the suspended `Tool::call` proceeds. On `decision: 'deny'`, the suspended call returns a `denied-by-approval` error to the brain.
- [ ] 4.4 Implement 5-minute timeout: if no `approval.respond` arrives, emit `approval.resolved` with `outcome: 'timeout'` and treat as deny.
- [ ] 4.5 Brain treats `denied-by-approval` as a hard stop (no retry).
- [ ] 4.6 Update `gateway-protocol.md` with the new event shapes + method.

## 5. H02 — Question kind + ApprovalCard

- [ ] 5.1 Extend the existing `Question` type with `kind: 'clarification' | 'approval'` defaulting to `'clarification'`.
- [ ] 5.2 Add `actionDescriptor` and related fields to Question for kind=approval.
- [ ] 5.3 Create `ApprovalCard.tsx` component: title (tool name), risk pill, JSON-pretty arguments, why-gated section, Allow / Deny buttons, loading state during respond.
- [ ] 5.4 Wire `QuestionCarousel` to render approval cards alongside clarification cards.
- [ ] 5.5 Add gateway event listener for `approval.requested`: create approval-kind Question; for `approval.resolved`: remove the Question and inline an outcome message in chat.
- [ ] 5.6 Add gateway client method `respondToApproval(approval_id, decision)` that sends `req` of method `approval.respond`.
- [ ] 5.7 Add a termination control to the chat surface that denies all open approvals + cancels the originating run.

## 6. H02 — Posture editor

- [ ] 6.1 Add a posture section to the claw settings overlay (created in s2). Single radio group: Low / Medium / High for `toolExecution`.
- [ ] 6.2 Wire the posture selector to `claw.update` patches.
- [ ] 6.3 Display the active posture as a small badge near the claw switcher.

## 7. Verification

- [ ] 7.1 Set "Personal" claw to Low; ask the model to do a web search; verify ApprovalCard appears; click Allow; verify the tool runs and the reply renders.
- [ ] 7.2 Click Deny on a different attempt; verify the brain reports the failure and the user sees an inline outcome.
- [ ] 7.3 Set Personal to Medium; trigger a `memory_search` (read-only internal) tool call; verify NO approval card appears and the tool runs.
- [ ] 7.4 Set Personal to Medium; trigger `web_fetch` (external); verify approval card DOES appear.
- [ ] 7.5 Set Personal to High; verify NO approval cards for any tool.
- [ ] 7.6 Trigger an approval; do nothing for 5+ minutes; verify timeout fires and the tool call is denied.
- [ ] 7.7 Trigger an approval; click termination control; verify the approval is denied AND the run is cancelled.
- [ ] 7.8 Restart kelvinclaw; verify posture survives.

## 8. Archive

- [ ] 8.1 Once tasks above are green and a 5-minute "set posture, see approval, allow/deny" demo is recorded, run `openspec archive s3-add-tool-execution-posture-axis`.
