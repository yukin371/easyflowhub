# EasyFlowHub AGENTS

## Project Summary

- Project: `EasyFlowHub`
- Type: `hybrid desktop app (React + Tauri + Go sidecar)`
- Primary stack: `React 19 + TypeScript 5 + Tauri 2 + Rust + Go + SQLite`
- Source of truth for version requirements: `docs/PRD-v1.0.0.md`
- Source of truth for current priorities: `docs/roadmap.md`
- Source of truth for project facts: `docs/PROJECT_PROFILE.md`
- Source of truth for architecture boundaries: `docs/ARCHITECTURE_GUARDRAILS.md`

## Read Order Before Any Non-Trivial Change

1. Root `AGENTS.md`
2. `docs/README.md`
3. `docs/PROJECT_PROFILE.md`
4. `docs/PRD-v1.0.0.md`
5. `docs/roadmap.md`
6. `docs/ARCHITECTURE_GUARDRAILS.md`
7. Nearest `MODULE.md`
8. Related `ADR` / `plan` document when relevant

## Repo-Specific Owners

- Frontend native command wrappers: `easyflowhub-app/src/lib/tauri`
- Native desktop shell and Tauri commands: `easyflowhub-app/src-tauri/src`
- Frontend module registration: `easyflowhub-app/src/modules`
- Manager workspace shell: `easyflowhub-app/src/components/manager`
- Script discovery / execution / tasks: `scriptmgr-go/internal/{api,discovery,executor,runtime,store}`
- MCP server protocol and dynamic tools: `scriptmgr-go/internal/mcp`
- External MCP client wrappers: `scriptmgr-go/internal/mcpcli`
- OpenAI-compatible API relay and provider routing: `scriptmgr-go/internal/relay`
- Manifest-based extension discovery: `scriptmgr-go/internal/extensions`

## Core Working Rules

1. Read before editing. Do not infer architecture from filenames alone.
2. Reuse before create. Search for existing helpers, wrappers, services and owners before adding new ones.
3. Keep ownership explicit. New shared logic must have a clear canonical owner package or module.
4. Verify before declaring done. For non-trivial work, run the relevant TS / Rust / Go checks.
5. Sync docs when boundaries or behavior change. Update at least one of:
   - `docs/roadmap.md`
   - related `MODULE.md`
   - related `docs/decisions/ADR-*.md`
   - related `docs/plans/*.md`

## EasyFlowHub Guardrails

### Do Not Duplicate Shared Capabilities

Before adding any of the following, search the repo for existing owners:

- Tauri command wrappers
- module registration / enable-disable logic
- notes persistence helpers
- settings persistence helpers
- script discovery / execution helpers
- MCP schema / routing logic
- relay provider selection / failover logic
- extension manifest scanning / contribution loading
- shared parsers for markdown / todo / image assets

If an existing implementation exists, prefer extending it.  
If it cannot be reused, explain why and record the new owner.

### Respect Layer Boundaries

- UI components should not scatter raw Tauri `invoke()` command names when `src/lib/tauri` can own them.
- `src-tauri/src` should not duplicate Go-side script runtime logic.
- `scriptmgr-go/internal/mcp` should stay protocol-focused, not absorb discovery / executor business logic.
- `scriptmgr-go/internal/relay` should own provider routing and failover, not `internal/http` or ad hoc CLI helpers.
- `scriptmgr-go/internal/extensions` should stay declarative and manifest-based in v1, not execute arbitrary plugin code.
- New manager panels should enter through `src/modules` rather than ad hoc sidebar wiring.

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

## Verified Commands

Verified locally on 2026-04-07:

```bash
cd easyflowhub-app && bun run test
cd easyflowhub-app && bunx tsc --noEmit
cd easyflowhub-app && cargo check --manifest-path src-tauri/Cargo.toml
cd scriptmgr-go && go test ./...
```

## When To Stop And Ask

Stop and ask if any of these apply:

- two modules appear to own the same concern
- a new shared utility seems necessary but owner is unclear
- a change crosses TS / Rust / Go boundaries without a clear source of truth
- existing docs conflict with actual code behavior
- a large migration is implied by a seemingly small request

## Completion Criteria

Work is not complete until:

- code or docs changes are applied
- relevant verification is run or a concrete blocker is reported
- impacted docs are synced
- new duplication or boundary drift has not been introduced
