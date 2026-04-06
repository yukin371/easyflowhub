# <PROJECT_NAME> Project Profile

> status: active
> owner: <team-or-person>
> last_verified: <YYYY-MM-DD>
> verified_against: <commit-sha-or-release>

## 1. Summary

- Project type: `<web|desktop|backend|library|cli|scripts|hybrid>`
- Primary goal: `<one sentence>`
- Primary users: `<internal|external|ops|developers|mixed>`
- Current phase: `<prototype|v1|scaling|maintenance>`

## 2. Stack

- Primary languages: `<ts/go/rust/python/...>`
- Frameworks / runtimes: `<react, tauri, gin, fastapi, ...>`
- Storage: `<sqlite/postgres/redis/none>`
- Infrastructure / deployment: `<desktop/local/docker/k8s/serverless/...>`

## 3. Verification

- Install command: `<command or TBD>`
- Dev command: `<command or TBD>`
- Build command: `<command or TBD>`
- Type check / compile command: `<command or TBD>`
- Unit test command: `<command or TBD>`
- Integration / E2E command: `<command or TBD>`
- Smoke test path: `<url, cli flow, screen flow, or TBD>`

## 4. Repo Topology

| Path | Role | Notes |
|---|---|---|
| `<path>` | `<app/service/lib/docs/tests>` | `<one sentence>` |
| `<path>` | `<app/service/lib/docs/tests>` | `<one sentence>` |

## 5. Runtime Entry Points

| Entry point | Type | Notes |
|---|---|---|
| `<path or command>` | `<http|cli|desktop|worker|cron>` | `<one sentence>` |
| `<path or command>` | `<http|cli|desktop|worker|cron>` | `<one sentence>` |

## 6. Architecture Shape

- Architecture style: `<layered|modular-monolith|hexagonal|feature-sliced|mixed>`
- Core layers: `<ui -> app -> domain -> infra>` or `<list actual layers>`
- Module boundary strategy: `<by feature|by layer|by package|by workspace|mixed>`
- Shared capability strategy: `<which dirs/packages may host shared logic>`

## 7. Canonical Shared Capability Owners

| Concern | Canonical owner | Notes |
|---|---|---|
| Logging | `<path/package>` | `<rules>` |
| Config | `<path/package>` | `<rules>` |
| HTTP / API client | `<path/package>` | `<rules>` |
| Persistence | `<path/package>` | `<rules>` |
| Auth | `<path/package>` | `<rules>` |
| UI primitives / shared widgets | `<path/package>` | `<rules or N/A>` |
| Utilities | `<path/package>` | `<rules>` |

## 8. Non-Negotiable Constraints

- `<constraint>`
- `<constraint>`
- `<constraint>`

Examples:

- No business logic in UI components
- No direct database access outside repository layer
- No new shared utility without checking existing owners

## 9. Known Risks / Recurring Pitfalls

- `<pitfall and consequence>`
- `<pitfall and consequence>`

## 10. Unknowns

- `TBD`: `<unknown fact>`  
  Confirm via: `<file / command / owner / test>`
- `TBD`: `<unknown fact>`  
  Confirm via: `<file / command / owner / test>`

## 11. AI Initialization Checklist

- [ ] Read root `AGENTS.md` if present
- [ ] Read `docs/roadmap.md` if present
- [ ] Confirm install / build / test commands
- [ ] Identify shared capability owners
- [ ] Identify 1 to 3 highest-risk modules
- [ ] Mark unknowns as `TBD`, do not fabricate
