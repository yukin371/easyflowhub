# AGENTS.md

## Project Summary

- Project: `<PROJECT_NAME>`
- Type: `<web|desktop|backend|library|cli|scripts|hybrid>`
- Primary stack: `<stack summary>`
- Source of truth for current priorities: `docs/roadmap.md`
- Source of truth for architecture boundaries: `docs/ARCHITECTURE_GUARDRAILS.md`

## Read Order Before Any Non-Trivial Change

1. Root `AGENTS.md`
2. `docs/PROJECT_PROFILE.md`
3. `docs/roadmap.md`
4. `docs/ARCHITECTURE_GUARDRAILS.md`
5. Nearest `MODULE.md`
6. Related `ADR` / `plan` document when relevant

## Core Working Rules

1. Read before editing. Do not infer module boundaries from filenames alone.
2. Reuse before create. Search for existing utilities, services, adapters and helpers before adding a new one.
3. Keep ownership explicit. New shared logic must have a clear canonical owner package or module.
4. Verify before declaring done. Use compile, tests, browser checks or smoke tests as appropriate.
5. Sync docs when boundaries or behavior change. Update at least one of:
   - `docs/roadmap.md`
   - related `MODULE.md`
   - related `ADR`
   - related `plan`

## Architecture Guardrails

### Do Not Create Duplicates Of Shared Capabilities

Before adding any of the following, search the repo for existing owners:

- logging
- config loading
- HTTP / API clients
- persistence access
- auth / session helpers
- date / time handling helpers
- file / path helpers
- UI primitives
- error mapping
- feature flags

If an existing implementation exists, prefer extending it.  
If it cannot be reused, explain why and record the new owner.

### Respect Module Boundaries

- Do not import across forbidden layers.
- Do not move domain rules into convenience helpers.
- Do not place business logic in presentation-only modules.
- Do not create cross-module utilities without documenting ownership.

## Required Pre-Change Output For Risky Work

Before large edits, provide a short summary containing:

- target modules
- existing owner of the affected capability
- likely impact surface
- planned verification
- docs to sync

## Required Post-Change Output

After completing work, report:

- what changed
- how it was verified
- residual risks or unverified areas
- which docs were updated

## When To Stop And Ask

Stop and ask if any of these apply:

- two modules appear to own the same concern
- a new shared utility seems necessary but owner is unclear
- a change crosses package boundaries without an existing ADR
- existing docs conflict with actual code behavior
- a large migration is implied by a seemingly small request

## Completion Criteria

Work is not complete until:

- code changes are applied
- verification is run or a concrete blocker is reported
- impacted docs are synced
- new duplication or boundary drift has not been introduced
