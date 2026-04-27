## Context

This change codifies the architecture documented across 23 markdown
files at `docs/kelvin-spec/` (the prose source of truth) into OpenSpec
capabilities. The architecture treats H02 (a Next.js GUI in
`https://github.com/kmondlane/h02`) as the front-end of the kelvinclaw
runtime in this repository, with two security sidecars: Open Bias
(`https://github.com/open-bias/open-bias`) at the model boundary and
kelvinclaw `ToolRegistry` at the tool boundary. See `OVERVIEW.md` for
runtime seams and `AGENTS.md` for engineering principles (file-size
limits, fail-closed defaults).

The architecture has eight ADRs at `docs/kelvin-spec/decisions/`. ADRs
record *why* a decision was taken; OpenSpec requirements record *what
the system must do as a result*. The two are complementary, not
duplicative.

## Goals / Non-Goals

**Goals:**
- Convert every behavioural assumption in `docs/kelvin-spec/` into a
  testable requirement with at least one `WHEN/THEN` scenario.
- Surface implicit assumptions that the prose docs take for granted
  (single-user filesystem auth, localhost-only sidecars, etc.) so they
  are visible at review time.
- Keep capabilities organised so v2 can be expressed as `MODIFIED` /
  `ADDED` deltas against this baseline.

**Non-Goals:**
- This change is documentation-only. No code in `crates/`, no edits to
  `H02/src/`, no edits to existing kelvinclaw docs outside
  `docs/kelvin-spec/`.
- No multi-user / shared-claw enforcement (deferred to v2 per ADR-004).
- No browser tab implementation, no plugin authoring UI, no hard cost
  cutoffs (deferred per `docs/kelvin-spec/11-roadmap.md`).
- No validation of Open Bias's `RULES.md` expressiveness against
  real-world rule sets — that needs a running deployment.

## Decisions

### Single change, ten capabilities

The kelvin-spec docs cover one coherent architecture; expressing it as
a single OpenSpec change keeps the proposal/specs/design/tasks anchored
together. Ten capabilities is the seam cut that aligns with the prose
docs without exploding into thirty micro-capabilities.

**Alternative:** decompose into ~10 separate changes, one per capability.
**Rejected because:** OpenSpec changes are reviewed as units and the
capabilities here are deeply interlinked (autonomy postures reference
the data model; approvals reference autonomy; sidecars reference both).
A single change keeps cross-references tractable.

### ADRs preserved as design context, not re-specified

The eight kelvin-spec ADRs already capture decision rationale in the
Nygard form (Status / Context / Alternatives / Decision /
Consequences). Re-rendering them as OpenSpec requirements would
duplicate prose without adding testability. Instead, this change
references the ADRs from the proposal and uses the requirements to
encode the *consequences* of each decision as testable invariants.

**Alternative:** re-render every ADR as a `decision-NNN` capability with
"the system shall implement decision NNN" requirements.
**Rejected because:** that conflates *why* (ADR) and *what* (spec), and
makes specs harder to validate (you can't test "the system implements
the rationale for picking Open Bias over a custom sidecar"; you can
test "the system fails closed when Open Bias is unreachable").

### Empty existing-specs baseline

`openspec/specs/` is empty when this change is created, so every
capability is `ADDED`. There are no `MODIFIED` or `REMOVED` deltas in
this change. Future v2 changes will treat the requirements here as the
baseline and emit deltas against them.

**Trade-off:** the `MODIFIED Requirements` workflow is untested in this
change. v2 work must verify the archive flow lands the requirements
in `openspec/specs/<capability>/spec.md` correctly so that v2 deltas
have something to MODIFY.

### Scenarios as test seeds

Every requirement in the ten capability spec files carries at least
one `WHEN/THEN` scenario. The scenarios are written as test seeds — a
human or AI implementer can lift them into unit/integration tests with
minimal transformation.

**Trade-off:** scenarios cover the happy and primary failure paths but
do not exhaustively enumerate every edge case. The verification list
below covers gap analysis.

## Risks / Trade-offs

[Risk: Over-specification of UI details locks design before validation]
→ Mitigation: keep UI specs to the *contract* of each surface (what
data is shown, what actions exist) rather than visual layout. Layout
remains in prose at `docs/kelvin-spec/06-approvals-primitive.md` and
the implementer's discretion.

[Risk: Scenario count drift across capabilities (some over-specified,
others under)]
→ Mitigation: target 5-10 requirements per capability, 1-3 scenarios
per requirement. Counts after this change: claw-anatomy 6 reqs;
concepts-taxonomy 7; delegation-call-tree 8; composer-modes 7;
autonomy-postures 9; approvals-primitive 9; security-sidecars 11;
mind-observability 9; data-model 10; gateway-protocol 8.

[Risk: Single-user assumption baked into requirements that v2 must
later loosen]
→ Mitigation: ownership fields (`ownerId`, `createdBy`) are required
in v1 schemas even though they're not enforced. v2 changes will MODIFY
the cap-chain requirement to include per-user caps, and ADD an ACL
capability. The data model is forward-compatible.

[Risk: Open Bias as the model-boundary sidecar may not be
expressive enough for all per-claw policies users will want]
→ Mitigation: this is an Open Question (see below) for live validation
during v1 implementation. If insufficient, the security-sidecars
capability can be MODIFIED to add a kelvinclaw-side post-filter.

[Risk: `Question` kind discriminator may break existing H02 consumers
that read fields directly without checking `kind`]
→ Mitigation: default `kind: 'clarification'` is required by the
data-model capability so legacy records remain valid. The H02
migration doc lists every consumer that needs an audit; the migration
slice in `tasks.md` calls this out explicitly.

[Risk: 12-axis matrix may be too granular for novice users or too
coarse for power users]
→ Mitigation: the three named postures (Low / Medium / High) populate
defaults so beginners interact with one knob; per-axis overrides are
available for power users. v2 can ADD axes without breaking the
schema (but adding requires updating every claw's posture default
mapping).

[Risk: Recursive Claw cap-chain at unbounded depth has unknown
performance characteristics]
→ Mitigation: the `PostureService` cache (referenced in
`interfaces/tool-gate-postures.md`) limits the cost to once per
posture mutation. Tests at depth ≥ 5 should be added during v1
implementation.

## Migration Plan

This change is documentation-only; there is no runtime migration. The
H02-side migration from `useGingerStore` to `useClawStore` is documented
in `docs/kelvin-spec/10-h02-migration.md` and tracked separately as an
H02 PR.

When `openspec apply` and `openspec archive` run, the requirements
land in `openspec/specs/<capability>/spec.md`. v2 changes can then
emit `MODIFIED` / `REMOVED` / `RENAMED` deltas against them.

## Open Questions

These are assumptions in the kelvin-spec prose that this change
exposes but does not resolve. Each needs validation during v1
implementation.

1. **Is single-user filesystem ownership a sufficient v1 trust
   boundary?** The data-model capability assumes `ownerId === <single
   user>` everywhere. If two users share a machine, they share Kelvin.
   Document this assumption in the v1 release notes; explicit
   multi-user is v2.

2. **Is `RULES.md` expressive enough for typical per-claw policies?**
   The security-sidecars capability assumes Open Bias's
   plain-markdown rules format covers PII redaction, off-topic
   refusal, charter alignment, and similar. Validate against three
   real claws (Personal / Work / Health) during implementation; if
   gaps appear, the capability should be MODIFIED to add a
   kelvinclaw-side post-filter or to migrate to Open Bias's
   programmatic API.

3. **Is `localhost-only Open Bias` workable for users who want
   Kelvin on a laptop and Open Bias on a workstation?** v1 explicitly
   forbids non-loopback Open Bias. v2 should evaluate adding TLS-bound
   trust between the two processes if there is signal for remote
   sidecar deployments.

4. **Is the `wasmEgress → preset` mapping (`low → locked_down, medium
   → dev_local, high → hardware_control`) the right defaulting?**
   `hardware_control` is the broadest preset; mapping it to `high`
   makes sense for trusted personal-AI use cases but may surprise
   users who expect a stricter default. Document the mapping in the
   posture editor's UI and revisit if user feedback indicates
   surprise.

5. **Are the twelve axes the right set?** Untested. Some plausible
   future axes: per-Channel autonomy (Telegram vs. SMS may want
   different gates), per-Source trust scores (v2), per-Sub-agent budget
   override per spawn. v2 can add axes without breaking; deletions are
   harder.

6. **Is `parent-caps-child` recursion at unbounded depth a real
   constraint or a theoretical one?** No depth limit in v1. Real-world
   personal-AI deployments are unlikely to exceed depth 3 (macro →
   domain → sub-domain). If depth-5+ shows up, the cap-chain cache may
   need re-tuning.

7. **Is `fail-closed when Open Bias is down` the right default vs.
   degraded UX?** Forces the user to install Open Bias before
   anything works. Mitigation: the install path includes Open Bias as
   a peer dependency; the dev environment provides a permissive
   `RULES.md` template.

8. **Does the `Question` kind discriminator scale to additional kinds
   in v2?** Plausible additions: `notification` (one-way), `prompt`
   (free-text input), `picker` (multi-step). The current
   discriminator union is forward-compatible.

9. **Does the single-claw RULES.md design lose value when claws share
   policies?** Some users may want one rule shared across all claws
   ("no profanity"). v1 requires duplication; RULES.md inheritance is
   a v2 ADD.

10. **Is the trusted-publisher manifest in `kelvinclaw-plugins/`
    curated enough that "auto-allow signed-by-trusted-publisher" is a
    safe Medium-posture default?** The manifest is a small file; if
    it grows large or includes publishers with weak signing practice,
    Medium becomes silently riskier. Add a v2 trust-score capability
    and tighten Medium accordingly.
