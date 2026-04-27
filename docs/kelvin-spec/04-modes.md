---
status: Draft
version: v1
audience: designers, implementors, end-users
date: 2026-04-27
---

# Modes — Composer Intent Picker

**Modes** are the composer-level intent picker: a small switcher in the chat
input that tells Kelvin *what kind of turn* the user is starting. Modes are
**globally shared** ([01-claw-anatomy.md](01-claw-anatomy.md)) — they live
in the UI layer, not on any individual claw.

Modes are **orthogonal to autonomy postures**. Autonomy is *how much* the
agent can do without asking; Mode is *what kind of work* the user wants in
this turn. A "Plan" mode in a "high autonomy" claw still produces a plan;
the autonomy posture just affects how freely the planning happens.

## The six modes

| Mode | Intent | Default render | Autonomy interaction |
|---|---|---|---|
| **Auto** | "Just handle it." Kelvin chooses how. | Conversational reply or action result | Full posture matrix in effect |
| **Plan** | "Don't act; produce a plan I can review." | Numbered Plan in Drafts | Suppresses tool execution; produces plan only |
| **Ask** | "Just answer; don't take action." | Text reply, no tools | Suppresses tool execution and Connector writes |
| **Learn** | "Teach me about this." | Explanation with citations | Allows Source reads; suppresses writes |
| **Play** | "Be exploratory; brainstorm." | Free-form, low-stakes | Same as Auto but with looser style |
| **Make** | "Produce an artifact." | Draft (markdown / diff / plan / …) | Allows Drafts creation; promotion still gated |

Modes are persistent only for the current turn unless the user explicitly
locks one (rare — a power user might lock Ask for a longer review session).

## Mode vs autonomy — the orthogonality rule

A common confusion: "Plan mode" sounds like it should be safer than "Auto
mode." It is, but only because Plan mode *suppresses tool execution by
contract*, not because Plan mode lowers autonomy.

The model is:

```
Effective behavior = Mode contract  ∩  Autonomy posture
```

- **Mode contract**: what kinds of operations the mode allows.
- **Autonomy posture**: among allowed operations, how many require approval.

Examples:

| Mode | Autonomy | Effective |
|---|---|---|
| Auto | Low | Reply or act, but ask before any write |
| Auto | High | Reply or act freely; only most-sensitive writes ask |
| Plan | Low | Produce plan; nothing executes regardless |
| Plan | High | Produce plan; nothing executes regardless |
| Ask | Low | Text reply only; no tools regardless |
| Ask | High | Text reply only; no tools regardless |

Plan and Ask have **harder contracts** than Auto — they don't just lower
the autonomy threshold; they remove categories of action entirely.

## Per-mode behavior

### Auto

The default. Kelvin reads the message, decides whether a tool call /
delegation / sub-agent spawn is appropriate, and acts (subject to autonomy
posture). Most messages should be Auto.

If the model decides "I should make a plan first," it can produce one
internally (a planning step inside the Auto turn) without switching modes.
That's distinct from the user explicitly choosing Plan mode.

### Plan

The user wants a plan, not an action. Kelvin:

- Produces a numbered Plan as a Draft.
- Lists each step with the Power(s) it would invoke.
- Lists prerequisites (Sources to read, Connectors needed, approvals it
  would request).
- Does NOT execute any step. Plan mode is a contract.

The user can then "Run this plan" from the Drafts UI, which switches the
plan to Auto execution (subject to posture per step).

### Ask

The user wants an answer to a question, not an action. Kelvin:

- Replies with text.
- May read Sources (subject to autonomy axis Source reads).
- Does NOT execute Connectors, write to memory, or modify files.
- Does NOT spawn Sub-agents (because Sub-agents typically take action).

Ask is the safest mode and is appropriate when in doubt.

### Learn

The user wants explanation with citations. Kelvin:

- Reads relevant Sources.
- Produces a structured explanation: definitions, examples, citations
  back to specific Sources.
- May produce a Draft if the explanation is long.
- Does NOT take other actions.

Learn differs from Ask in *output shape*: Learn produces structured,
citation-rich explanations; Ask produces direct conversational replies.

### Play

Exploratory / brainstorming mode. Kelvin:

- Replies in a more relaxed, exploratory style.
- May suggest multiple alternatives.
- May ask follow-up questions to riff with the user.
- Otherwise behaves like Auto (subject to posture).

Play exists for brainstorming sessions where the user wants Kelvin to be
more conversational and less determined.

### Make

The user wants a concrete artifact. Kelvin:

- Produces a Draft as the primary output (markdown, diff, plan, code, …).
- Lists the Draft's promotion candidates (where it could go: filesystem,
  memory, connector, …).
- Does NOT auto-promote; promotion is the user's next step.

Make differs from Auto in *commitment*: Make's contract is "an artifact will
exist after this turn," whereas Auto might just reply.

## Mode persistence and switching

- Default mode is Auto on every new session.
- The mode switcher is a small chip near the composer.
- The user can lock a mode for the session by long-pressing the chip or via
  a session-settings panel (v2 polish).
- Voice input defaults to Auto unless the user has locked another mode.
- Channel-driven inputs (Telegram, email, etc.) default to Auto with
  per-channel override (v2).

## Mode and Sub-agent spawning

A Sub-agent spawn inherits the mode of the spawning turn. A "Researcher"
spawned in Auto runs Auto. Spawned in Plan, the Researcher produces a plan
of how it would research, not actual research.

Per-spawn override is allowed (a Workflow Power can spawn a Sub-agent in a
different mode for a specific step), but defaults to inheritance.

## Mode and Triggers

Triggers (Hooks, Heartbeats, Watches) fire in **Auto mode** by default.
The trigger's `invokes.args` MAY override the mode if a Routine wants to
"draft a brief but never act" (Plan or Make).

Trigger-driven Auto-mode actions are subject to the autonomy axis
**Routines firing user-absent**, which can be stricter than the claw's
interactive posture.

## H02 implementation

The existing H02 `MessageMode` enum (`'auto' | 'plan' | 'ask' | 'learn' |
'play' | 'make'`) and `MESSAGE_MODES` constants in `src/constants/index.ts`
already implement this. No changes needed to the type or the constant
beyond adding doc comments referencing this spec.

The composer's mode chip already exists in H02; the only change is to ensure
the **mode contract** (what each mode does/doesn't do) is enforced at the
*runtime layer*, not just labeled in the UI.

## Cross-references

- [01-claw-anatomy.md](01-claw-anatomy.md) — Modes are globally shared
- [05-autonomy-postures.md](05-autonomy-postures.md) — autonomy is orthogonal
- [03-delegation-and-call-tree.md](03-delegation-and-call-tree.md) — modes
  inherit through Sub-agent spawns
- [10-h02-migration.md](10-h02-migration.md) — `MessageMode` is unchanged
