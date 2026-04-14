# KelvinClaw Agent Instructions

This file defines default expectations for coding agents working in this repository.

## Priorities

1. Security

- Protect secrets and sensitive information.
- Avoid introducing vulnerabilities or attack vectors.
- Ensure safe defaults and fail-closed behavior.
- Avoid over-permissioning or excessive access.

2. Stability
3. Reliability
4. Simplicity
5. Size/maintainability

## Safety Rules

- Never commit secrets, keys, tokens, hostnames, or private IPs.
- Keep `.env` / local machine details out of commits.
- Avoid destructive git commands unless explicitly requested.
- Do not revert user-authored unrelated changes.

## Architectural Principles

- All Crates must be self-contained and not directly reference each other, except for the SDK which can reference all.
- The Core SDK and the Memory SDK should be the only interfaces to the crates from the outside.
- All WASM plugins must be loaded through the SDK and not directly from the root or other crates.
- All network access must be mediated through the SDK with explicit allowlists and not directly from the root or other crates.
- All configuration must be validated and fail closed on missing or invalid values, with clear error messages.
- Keep everything as simple as possible, but no simpler. Avoid unnecessary complexity or abstractions.
- Bear in mind at all times the OWASP Top 10, NIST CSF / AI, MITRE ATT&CK, ISO 42001, and other relevant security frameworks and best practices.
- Follow Rust best practices for safety, error handling, and code quality.
- Follow general software engineering best practices for testing, documentation, and maintainability.
- Prioritize security and stability over new features or optimizations.
- Always consider the potential impact of changes on users and the ecosystem.
- Communicate clearly and proactively about changes, especially breaking ones, with users and stakeholders.
- Continuously monitor and improve the security, stability, and reliability of the system over time.

## Build and Test

- Only run tests/builds relevant to the change being made, but ensure all tests pass before finalizing.
- Prefer Docker-based verification, remote server first if available, only fallback to local if needed.
- Remote Docker preflight (required when using a vanilla `rust:*` image):
    - `rustup component add rustfmt clippy`
    - If missing: `cargo install cargo-audit --locked`
    - If missing: `cargo install cargo-outdated --locked`
- Docker cache policy:
    - Iteration/testing: use cached layers (`scripts/test-docker.sh`).
    - Final push validation: run a clean rebuild from zero (`scripts/test-docker.sh --final`).
- Standard SDK lane:
    - `scripts/test-sdk.sh`
- Targeted Rust lane:
    - `cargo test -p kelvin-core -p kelvin-wasm -p kelvin-brain -p kelvin-sdk --lib`
- Run formatting checks before finalizing:
    - `cargo fmt --all -- --check`
- Run linting checks before finalizing:
    - `cargo clippy --all -- -D warnings`
- Run security checks before finalizing:
    - `cargo audit`
- Run dependency checks before finalizing:
    - `cargo outdated`
- Run integration tests before finalizing:
    - `cargo test --workspace --tests`
- Run end-to-end tests before finalizing:
    - `scripts/test-e2e.sh`
- Run Docker-based tests before finalizing:
    - `scripts/test-docker.sh`
- If a referenced script is not present in the repo, report it as `MISSING` and continue with the remaining checks.

## Plugin Architecture Guardrails

- Keep model/tool plugins on the SDK path, not direct root coupling.
- Fail closed on missing or invalid plugin configuration.
- Enforce manifest capability/runtime parity and import allowlists.
- Keep network access host-mediated with explicit allowlists.

## Commit Discipline

- Keep commits scoped and atomic.
- Only stage files relevant to the requested change.
- Use clear commit messages that describe intent.

## Branching and Delivery Philosophy

- `main` must stay releasable and working at all times.
- Prefer short-lived branches over long-running divergence.
- Merge small, reviewable changes frequently instead of batching large rewrites.
- Treat CI as a hard quality gate for anything headed to `main`.
- If a branch stops getting frequent merges, either shrink its scope or merge it behind a safe default.
- When in doubt, optimize for fast feedback, green builds, and easy rollback.

Below is an AI-optimized AGENTS.md designed specifically for agentic coding environments (Cursor, Claude Code, OpenAI agents, etc.).

It focuses on things that AI coding systems consistently struggle with:
• unclear structure
• hidden side effects
• inconsistent patterns
• giant files
• ambiguous interfaces

This version enforces deterministic structure, which dramatically improves AI-generated code quality.

⸻

AGENTS.md

Agentic Highway Engineering Guidelines

This repository is designed to be worked on by humans and AI agents.

Code must be:
• predictable
• modular
• easy to reason about
• safe to modify automatically

The goal is fast iteration with minimal breakage.

⸻

Core Principles

Priorities in order: 1. Deterministic structure 2. Small composable modules 3. Explicit interfaces 4. Minimal hidden behavior 5. Safe automated refactoring

Readable, boring code is preferred over clever code.

⸻

Rules:
• core must be pure logic
• tools contain all external effects
• agents coordinate behavior
• workflows orchestrate systems

This separation allows AI agents to modify components safely.

Make sure that Verbs, Nouns, Actions, etc are properly grammatically named and consistent across the codebase.

⸻

File Size Rules

Large files degrade AI performance.

Limits:
• files: ≤ 400 lines
• functions: ≤ 50 lines
• classes: ≤ 200 lines

If a file grows too large:

split it.

⸻

Function Design

Functions must:
• do one thing
• have clear inputs
• have predictable outputs

Avoid hidden dependencies.

Bad:

def process_task():
user = get_current_user()
data = requests.get(API).json()

Good:

def process_task(user, external_data):

Pass dependencies explicitly.

⸻

Pure Logic vs Side Effects

All side effects must be isolated.

Side effects include:
• LLM calls
• HTTP requests
• database operations
• filesystem access
• environment variables

Pure logic should never directly call external services.

⸻

Typed Data Contracts

All shared data structures should use typed models.

Prefer:
• Pydantic models
• dataclasses
• typed dictionaries

Example:

class AgentTask(BaseModel):
id: str
prompt: str
tools_allowed: list[str]

Avoid passing raw dictionaries between modules.

Typed contracts reduce AI-generated bugs.

⸻

Error Handling

Errors must include context.

Rules:
• never swallow exceptions
• always add debugging context
• propagate meaningful errors upward

Bad:

except Exception:
pass

Good:

except Exception as e:
raise WorkflowExecutionError(
f"Step {step_id} failed"
) from e

Logs should contain enough information to reproduce failures.

⸻

Logging

Every agent execution should log:
• task ID
• tools invoked
• model calls
• latency
• errors

Example:

logger.info("agent_step_start", step_id=step_id)

Logs should be structured.

⸻

Tool Integration Pattern

External systems must be wrapped in tool adapters.

Example:

tools/
openai_client.py
slack_client.py
github_client.py

Agents must not call external APIs directly.

Adapters improve:
• testability
• mocking
• debugging
• observability

⸻

Testing Expectations

Focus tests on logic that could break.

Test:
• reasoning logic
• data transformations
• workflow orchestration

Avoid heavy testing of:
• thin wrappers
• temporary experiments

Minimum test types:

tests/
unit/
workflows/

⸻

Performance Guidelines

Prioritize major improvements:
• fewer LLM calls
• caching
• batching
• smaller prompts

Ignore premature micro-optimizations.

Measure before optimizing.

⸻

Security Rules

Always:
• validate inputs
• avoid unsafe deserialization
• never commit secrets
• use environment variables for credentials
• sanitize external data

Security issues must be treated as bugs.
.env files should never be committed, but `.env.example` with placeholder values is encouraged.
and `.env` should be in `.gitignore`.

⸻

Pull Request Guidelines

Changes should be easy to review.

Preferred:
• small PRs
• focused changes
• clear commit messages

Separate:
• refactors
• feature additions
• bug fixes

⸻

Refactoring Rules for AI Agents

AI-generated refactors must:
• maintain test coverage
• avoid large multi-file rewrites unless requested

Prefer incremental improvements.

⸻

Avoid Over-Engineering

Do not build:
• generic frameworks
• premature abstractions
• complex plugin systems

Duplicate small patterns until real reuse emerges.

⸻

Ownership Mindset

Write code assuming:

you will debug this system during production incidents.

Code should make failures easy to understand and fix.

⸻

Summary

The system should always be:
• readable
• modular
• observable
• easy to test
• safe to refactor

Fast iteration is the goal.

Not theoretical perfection.

⸻
