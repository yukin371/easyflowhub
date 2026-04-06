# <PROJECT_NAME> Architecture Guardrails

> status: active
> owner: <team-or-person>
> last_verified: <YYYY-MM-DD>
> verified_against: <commit-sha-or-release>

## 1. Purpose

This document defines hard boundaries that AI and humans must respect to avoid architecture drift, duplicate implementations and costly cleanup later.

## 2. Layer Map

| Layer / Area | Responsibility | Must Not Do |
|---|---|---|
| `<layer-or-module>` | `<one sentence>` | `<forbidden behavior>` |
| `<layer-or-module>` | `<one sentence>` | `<forbidden behavior>` |
| `<layer-or-module>` | `<one sentence>` | `<forbidden behavior>` |

## 3. Dependency Direction

Allowed dependency direction:

```text
<layer A> -> <layer B> -> <layer C>
```

Forbidden examples:

- `<module A>` must not import `<module B>`
- `<module A>` must not call `<module B>` directly
- `<layer>` must not own `<concern>`

## 4. Canonical Owners

Every cross-cutting concern must have one canonical owner.

| Concern | Canonical owner | Reuse rule | Duplicate status |
|---|---|---|---|
| Logging | `<path/package>` | `<extend, do not duplicate>` | `forbidden` |
| Config | `<path/package>` | `<extend, do not duplicate>` | `forbidden` |
| HTTP / API client | `<path/package>` | `<extend, do not duplicate>` | `forbidden` |
| Persistence | `<path/package>` | `<extend, do not duplicate>` | `forbidden` |
| Auth | `<path/package>` | `<extend, do not duplicate>` | `forbidden` |
| Shared utilities | `<path/package>` | `<only generic logic allowed>` | `restricted` |

## 5. New Shared Capability Gate

Before adding a new shared helper, service or abstraction, the implementer must answer:

1. What existing implementation was searched?
2. Why can it not be reused or extended?
3. What is the canonical owner of the new capability?
4. Which duplicate implementations should be migrated or removed later?
5. Which module docs or ADR must be updated?

If any answer is unclear, do not add the capability yet.

## 6. Naming And Placement Rules

- Shared code goes only in `<allowed shared locations>`
- Feature-specific code stays inside `<feature module pattern>`
- Test-only helpers stay inside `<test helper location>`
- Temporary migration adapters must be marked with `<tag or comment convention>`

## 7. Forbidden Patterns

- Duplicating the same utility in multiple packages
- Hiding domain logic inside generic helper folders
- Creating a new abstraction before proving repeated need
- Re-exporting architecture-violating imports through convenience modules
- Writing around existing boundaries “just for now” without a migration note

## 8. Exception Process

Temporary exceptions are allowed only if all of the following are recorded:

- reason
- owner
- expiration condition
- cleanup plan
- verification risk

Record exceptions in:

- `docs/plans/...` for short-lived work
- `docs/decisions/...` for long-lived exceptions

## 9. Verification

Recommended enforcement mechanisms:

- lint / import guards
- dependency analysis
- code review checklist
- module doc updates
- ADR for boundary changes
