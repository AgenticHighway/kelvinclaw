## Context

Kelvin must defend against several failure classes simultaneously:
prompt injection, off-policy model output, unauthorised tool
execution, network egress from skills, cross-claw exfiltration. No
single chokepoint catches all of these because they happen at
different boundaries.

Recent industry incidents (March 2026: Claude Code Security Review
action, Gemini CLI Action, GitHub Copilot Agent all leaking secrets
via PR-title prompt injection) demonstrate that model-output
evaluation alone does not stop tool-execution risks, and tool gates
alone do not stop policy violations in model output.

The user has named **Open Bias**
(`https://github.com/open-bias/open-bias`) as the model-boundary
sidecar — a Python proxy at `http://localhost:4000/v1` that reads
`RULES.md` files and intervenes at PRE_CALL / LLM_CALL / POST_CALL.
The kelvinclaw runtime already provides `ToolRegistry` and
`Tool` traits for the tool-boundary side, plus WASM sandbox presets
(`locked_down` / `dev_local` / `hardware_control`).

This change specifies how the two boundaries cooperate, how the
twelve-axis posture matrix maps onto enforcement points, and how the
approvals UI primitive surfaces gated actions.

## Goals / Non-Goals

**Goals:**
- Define the twelve-axis posture matrix as the *single source of
  truth* for "what requires the user's approval and what doesn't."
- Specify two distinct sidecars with non-overlapping responsibilities
  and a clear fail-closed contract.
- Reuse H02's existing `Question` / `QuestionCarousel` primitive for
  approvals; do not build a parallel approval system.
- Make every gated decision produce a Receipt with posture snapshot,
  so the audit trail in Mind (specced in `add-kelvin-ui-runtime`) is
  complete.

**Non-Goals:**
- No multi-user posture caps (deferred to v2; ADR-004).
- No remote Open Bias deployments — localhost-only in v1.
- No automated upgrade flow for Open Bias version pinning. Manual
  ops concern.
- No trust-score gating beyond signed/unsigned (v2).
- No GUI surfaces beyond what's in `approvals-primitive` — Mind UI is
  in `add-kelvin-ui-runtime`.

## Decisions

### D1. Two security sidecars, not one (model boundary + tool boundary)

**Context.** Five failure classes spread across two boundaries:
prompt injection and off-policy output are model-side; unauthorised
tool execution, WASM egress, and cross-claw exfiltration are
tool-side. No single chokepoint can defend both.

**Alternatives considered.**
- One sidecar at the model boundary (Open Bias only). Cannot stop
  policy-compliant model outputs that request unauthorised tool
  calls, miss WASM egress.
- One sidecar at the tool boundary (kelvinclaw native only).
  Cannot stop model from producing prompt-injected text that leaks
  PII directly in the assistant turn (no tool involved). Loses
  Open Bias's OTel trace data.
- One unified in-house sidecar. Reinvents Open Bias; mixes two
  concerns; substantial build cost.
- *Chosen:* Two sidecars, one per boundary. Both must be healthy
  for normal operation; if either is down, autonomy floors to Low
  (fail-closed).

**Consequences.** Two processes to deploy and monitor;
`docker-compose.yml` ships both. Each gate produces an audit signal
that flows into Mind's Receipts. Open Bias adds one local in-process
hop per model call (small but non-zero latency). Per-claw `RULES.md`
selection requires a header passthrough shim in `ModelProvider`.

### D2. Per-claw `soul.md` + `RULES.md` as file-backed config; reuse `Question` for approvals

**Context.** Two things must be configured per claw: identity/charter
(Soul) and enforceable policy (Rules). The user explicitly said "the
soul can be one `.md` for now." Open Bias already reads plain
`RULES.md` files. H02 already has a sophisticated user-clarification
primitive (`Question` / `QuestionCarousel`) with options, trace
items, attachments, urgency, confidence — every field an approvals
tray needs.

**Alternatives considered.**
- Store Soul and Rules as JSON in the database. Loses git versioning;
  loses Open Bias's native compatibility; requires GUI editing.
- Three files (`soul.md`, `style.md`, `rules.md`). Splits a small
  thing too early; user said "one `.md` for now."
- Build a new approvals primitive from scratch. Reinvents the
  carousel wheel; throws away tested H02 code.
- *Chosen:* Two files per claw (`soul.md` + `RULES.md`). Approvals
  extend `Question` with a `kind` discriminator (`'clarification' |
  'approval'`).

**Consequences.** Filesystem layout
`<KELVIN_DATA_DIR>/claws/<id>/{soul,RULES}.md` is part of the trust
boundary in v1. Existing `Question` consumers must be audited to
handle the new `kind`. Approvals carousel UX is the same as
clarifications carousel — minimal new design surface.

### D3. Three autonomy postures with parent-caps-child invariant; sidecar-down floors to Low

**Context.** The user requested three postures (Low / Medium / High).
A single slider proved too coarse — different concerns warrant
different defaults at the same overall level (auto-write memory,
ask before sending email). Many independent sliders would be a
config nightmare. The sweet spot: three named shortcuts that
populate a 12-axis matrix; per-axis overrides allowed within parent
caps.

**Alternatives considered.**
- Single global slider. Forces all-or-nothing.
- Per-action approval only, no posture concept. Approval fatigue;
  appearance of safety is not safety.
- Many independent sliders (one per axis). Configuration nightmare.
- *Chosen:* Three named postures populate matrix defaults; per-axis
  overrides within cap-chain.

**Consequences.** Twelve axes (toolExecution, subClawDelegation,
subAgentSpawn, sourceReads, connectorWrites, draftPromotion,
pluginInstall, memoryWrites, wasmEgress, routinesUserAbsent,
crossClawPorosity, powerModelSpend). Cap chain: User cap → Macro →
Sub → ... Strictest level wins per axis. Sidecar-down floors to Low
across all axes. Routines fire at claw posture (not session
posture); `routinesUserAbsent` may further restrict. Posture changes
produce Receipts.

### D4. Fail-closed when Open Bias is unreachable

**Context.** If Open Bias is unreachable, falling through to the
upstream provider would silently bypass `RULES.md` enforcement —
worse than refusing service. `AGENTS.md` enshrines fail-closed as a
core principle.

**Alternatives considered.**
- Fall through to upstream on unreachable. Silently bypasses every
  RULES.md.
- Best-effort retry then fall through. Same risk, just delayed.
- *Chosen:* Refuse model calls; surface sidecar-down banner;
  posture floors to Low across the install. `fail_closed = true`
  required at startup; any other value rejected.

**Consequences.** Open Bias becomes a hard install dependency.
Development uses a permissive `RULES.md` template (still routed
through Open Bias). Health probes run at startup and steady-state.

## Risks / Trade-offs

[Risk: Open Bias unreachable = system unusable] → mitigation: peer
dependency, ship in `docker-compose.yml`, permissive dev `RULES.md`
template. Acceptable trade vs. silent policy bypass.

[Risk: localhost bind-race — a malicious process may bind
`127.0.0.1:4000` before Open Bias and impersonate it] → not solved
by this change. The `security-sidecars` capability requires
loopback-only but does not verify Open Bias process identity.
Documented for explicit acceptance; mitigation deferred to a
follow-up change (e.g., unix socket + process-launch verification).

[Risk: `RULES.md` expressiveness unverified end-to-end] → mitigation:
Tasks include a spike to validate the format claims against actual
Open Bias before treating the capability as production-ready.

[Risk: 12 axes is too many for novice users] → mitigation: three
named postures populate defaults; per-axis overrides are progressive
disclosure. Posture badge shows base name with `*` when overrides
are active.

[Risk: Sub-agent posture cap re-checked per Power invocation has
mid-flight tightening but not loosening] → mitigation: this is
intentional. A user can tighten posture mid-flight to interrupt
overly-aggressive Sub-agents. Loosening would create a TOCTOU race.

[Risk: `Question` kind discriminator change breaks existing H02
consumers that read fields without checking `kind`] → mitigation:
the `data-model` capability in `add-kelvin-foundations` requires the
default to be `'clarification'` so legacy records remain valid.
H02 migration tasks include a consumer audit.

[Risk: forever-scope overrides accumulate over time and silently
loosen posture] → mitigation: the matrix UI lists active forever
overrides prominently with one-tap revoke; forever requires a
secondary confirmation step.

## Migration Plan

This change has cross-process dependencies:

1. `add-kelvin-foundations` archives FIRST so the `data-model`
   capability is in `openspec/specs/`.
2. Open Bias runs as a peer container; `docker-compose.yml` pins
   the version.
3. The kelvinclaw `OpenBiasShimProvider` wraps the chosen upstream
   `ModelProvider` so `base_url` points at Open Bias.
4. The `ToolRegistry` extension reads `PosturePerAxis` per call.

Archive order: this change archives AFTER `add-kelvin-foundations`
and BEFORE `add-kelvin-ui-runtime` (which references `sidecar-health`
events).

## Open Questions

1. **Is `RULES.md` expressive enough for typical per-claw policies?**
   Untested end-to-end. The capability assumes Open Bias's
   plain-markdown rules format covers PII redaction, off-topic
   refusal, charter alignment. Validate against three real claws
   (Personal / Work / Health) during implementation; if gaps appear,
   add a kelvinclaw-side post-filter or migrate to Open Bias's
   programmatic API.

2. **WASM egress mapping (`low → locked_down, medium → dev_local,
   high → hardware_control`).** `hardware_control` is the broadest
   preset; mapping it to `high` is sensible for trusted personal-AI
   use cases but may surprise users who expect a stricter default.
   Document the mapping in the posture editor's UI.

3. **Localhost bind-race.** A malicious process can bind
   `127.0.0.1:4000` first and impersonate Open Bias. The
   `security-sidecars` capability mandates loopback-only but does
   not verify Open Bias identity. Accept the risk for v1, document
   in release notes, plan a follow-up change with unix socket +
   process-launch verification.

4. **Trusted-publisher manifest curation.** "Auto-allow signed-by-
   trusted-publisher" at Medium posture is only as safe as the
   `kelvinclaw-plugins/trusted_publishers.kelvin.json` manifest.
   If publishers with weak signing practice land on the list,
   Medium becomes silently riskier. Add a v2 trust-score capability.

5. **`forever`-scope overrides accumulation.** Even with secondary
   confirmation, a user might add many forever-overrides over months
   and not realise their effective posture has drifted. Prominent
   list + one-tap revoke is the v1 mitigation; v2 may add periodic
   review prompts.
