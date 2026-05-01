# TASK-0609 — `docker-compose.yml` `bench-tcp` profile + `docs/DOCKER.md` (Phase E-2 + part of E-3)

**Phase:** E-2 + E-3 (D-011 Docker — bench-tcp service + post-D-006 documentation)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** PENDING
**Priority:** P0 (required for Phase F-2 docker bench rodada)
**Spec:** none (deployment infrastructure).
**Origin:** D-011 plan §E-2 + §E-3 (E-3 partial — the documentation portion; E-3 validation goes into TASK-0610's smoke test).
**Estimated complexity:** S (~50 LoC YAML + ~50 lines docs)
**Estimated stages duration:** Stages 2→3→4→5→6 over ~0.5 day.

---

## Context

Per D-011 plan: add a `bench-tcp` profile/service to `docker-compose.yml` that orchestrates a coordinator + N workers and runs the bench in TCP mode, parameterized via env vars (`CHUNK_SIZE`, `MAX_PENDING_LIFETIME`, `RECYCLE_POLICY`). Maintain backward-compat with the existing coordinator/worker profile.

E-3 validation in this task scope: document the post-D-006 hybrid coordinator change (coordinator now reduces a local partition, contrasting with v1) in a new `docs/DOCKER.md` (~50 lines). The actual integration test that validates the hybrid coordinator works is TASK-0610.

## Dependencies

- **TASK-0608 (E-1)** — REQUIRED. The compose file needs a working image to reference.
- **TASK-0596 (B-1)** — REQUIRED. The compose file's TCP service needs the wire protocol fix to function correctly.
- **TASK-0603 (C-3)** — RECOMMENDED. The env-var → CLI-flag mapping in the compose service depends on the C-3 flags being available.

## Files in scope

| File | Change |
|------|--------|
| `docker-compose.yml` | Add a `bench-tcp` profile (or service) that orchestrates coordinator + N workers, parameterized via `CHUNK_SIZE`, `MAX_PENDING_LIFETIME`, `RECYCLE_POLICY` env vars (mapped onto `--chunk-size`, `--max-pending-lifetime`, `--recycle-policy` CLI flags). Maintain existing coordinator/worker profile unchanged. |
| `docs/DOCKER.md` (new file) | ~50 lines describing: (a) v2 docker setup, (b) the post-D-006 hybrid coordinator (coordinator reduces a local partition, was pure dispatcher in v1), (c) `bench-tcp` profile usage and env vars, (d) common troubleshooting (port binding, healthcheck). |

## Files explicitly OUT of scope

- `Dockerfile` — TASK-0608.
- The TCP smoke test in CI — TASK-0610.
- Integration tests that verify hybrid-coordinator runtime correctness — those are unit / integration tests in `relativist-net/`, not docker-level.

## Acceptance criteria

1. `docker compose --profile bench-tcp run bench-tcp --benchmark ep_annihilation --sizes 1000 --workers 2 --chunk-size 100` succeeds (assuming TASK-0596 is landed).
2. Env vars `CHUNK_SIZE`, `MAX_PENDING_LIFETIME`, `RECYCLE_POLICY` propagate into the bench CLI flags.
3. Existing `docker compose up coordinator worker` (the v1-style profile) still works (backward-compat).
4. `docs/DOCKER.md` exists and documents the post-D-006 difference.
5. Healthcheck / port binding works in the bench-tcp profile (no startup race).

## Test floor delta expected

**+0** (test in CI is TASK-0610's scope).

## Notes

- The compose service is parameterized so Phase F-2 can run multiple configurations without editing YAML.
- The docs file is a single-pass write — Stage 5 QA reviews for clarity / accuracy (post-D-006 hybrid coordinator must be described correctly).
