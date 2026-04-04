# Git Workflow — Relativist

**Model:** GitHub Flow (simplified for solo developer + TCC)

---

## Branch Strategy

```
main (always stable, compiles, tests pass)
 ├── feat/TASK-0001-symbol-agent-types
 ├── feat/TASK-0002-net-struct
 ├── fix/TASK-0015-port-array-oob
 └── ...
```

- **`main`** — production branch. Always compiles, all tests pass, CI green.
- **Feature branches** — one per backlog task: `feat/TASK-XXXX-short-description`
- **Fix branches** — for bug fixes found during review: `fix/TASK-XXXX-description`
- **No `develop` branch** — unnecessary overhead for a single developer.

## Workflow Per Task

1. Create branch from `main`: `git checkout -b feat/TASK-XXXX-description`
2. Implement following the 7-stage pipeline (DEVELOPMENT-PIPELINE.md)
3. Push and open PR against `main`
4. CI runs (fmt, clippy, test, build)
5. Self-review the PR diff (record of what changed and why)
6. Merge via squash-merge (clean history on main)
7. Delete feature branch

## Commit Convention

[Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add Symbol and Agent types (TASK-0001)
test: add property tests for I1 bidirectionality (TASK-0003)
fix: handle ERA port array waste correctly (TASK-0012)
refactor: extract connect logic into helper (TASK-0005)
docs: update BACKLOG with SPEC-03 tasks
chore: add rayon dependency
```

- Scope is optional: `feat(net): add create_agent`
- Body for context when needed
- Footer: `Refs: SPEC-02 R11` to trace back to specs

## Version Tags

Semantic versioning tied to spec completion:

| Tag | Milestone |
|-----|-----------|
| `v0.1.0` | SPEC-02 complete (core types) |
| `v0.2.0` | SPEC-03 complete (reduction engine) |
| `v0.3.0` | SPEC-04 complete (partitioning) |
| `v0.4.0` | SPEC-05 complete (merge & grid cycle) |
| `v0.5.0` | SPEC-06 complete (wire protocol) |
| `v0.6.0` | SPEC-07+13 complete (CLI & architecture) |
| `v0.7.0` | SPEC-10 complete (security) |
| `v0.8.0` | SPEC-11 complete (observability) |
| `v0.9.0` | SPEC-12 complete (user I/O) |
| `v1.0.0` | SPEC-09 complete (benchmarks) — TCC ready |

## Rules

1. **Never push directly to `main`** — always via PR (even solo).
2. **CI must pass before merge** — no exceptions.
3. **Squash-merge** — one commit per task on main, clean history.
4. **Delete branches after merge** — no stale branches.
5. **Tag after each spec phase completes** — milestone tracking.
