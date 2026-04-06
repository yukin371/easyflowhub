# <MODULE_NAME> MODULE

> status: active
> owner: <team-or-person>
> last_verified: <YYYY-MM-DD>
> verified_against: <commit-sha-or-release>

## 1. Responsibility

One sentence:

`<what this module exists to do>`

## 2. Owns

This module is the canonical owner of:

- `<owned capability>`
- `<owned capability>`
- `<owned capability>`

## 3. Must Not Own

This module must not contain:

- `<forbidden concern>`
- `<forbidden concern>`

## 4. Entry Points

| Entry point | Role | Notes |
|---|---|---|
| `<file / symbol / route / command>` | `<role>` | `<notes>` |
| `<file / symbol / route / command>` | `<role>` | `<notes>` |

## 5. Key Dependencies

| Dependency | Why it exists | Risk if changed |
|---|---|---|
| `<module / package>` | `<reason>` | `<risk>` |
| `<module / package>` | `<reason>` | `<risk>` |

## 6. Dependents

Main callers / consumers:

- `<module / package>`
- `<module / package>`

## 7. Invariants

These must remain true after changes:

- `<invariant>`
- `<invariant>`
- `<invariant>`

## 8. Common Pitfalls

- `<pitfall>`
- `<pitfall>`

## 9. Reuse Rules

Before adding new code here, check:

- whether `<existing symbol / package>` already solves it
- whether the change belongs in this module or in another canonical owner
- whether a similar helper already exists nearby

## 10. Verification

- Primary verification: `<tests / smoke flow / compile command>`
- Secondary verification: `<optional>`

## 11. Doc Sync Triggers

Update this file when any of these change:

- module responsibility
- owner capability
- dependency rules
- invariants
- common pitfalls
