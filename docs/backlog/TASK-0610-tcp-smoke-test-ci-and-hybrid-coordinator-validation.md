# TASK-0610 — TCP smoke test in CI + hybrid coordinator validation (Phase E-4 + remainder of E-3)

**Phase:** E-4 + E-3 (D-011 Docker — smoke test + post-D-006 hybrid coordinator runtime validation)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** PENDING
**Priority:** P0 (the integration validation that closes QA-D009-001 / TASK-0596 on the TCP path)
**Spec:** SPEC-19 R35a (committed `c4c80b8`); SPEC-09 R18a–R18g (committed `82b2d27`); SPEC-22 R10b/R12a (free-list / next_id consistency).
**Origin:** D-011 plan §E-4 + remaining §E-3.
**Estimated complexity:** S (~30 LoC CI YAML + ~50 LoC integration test)
**Estimated stages duration:** Stages 2→3→4→5→6 over ~0.5 day.

---

## Context

Per D-011 plan §E-4 verbatim: `docker compose run bench-tcp --benchmark ep_annihilation --sizes 1000 --workers 2 --chunk-size 100 --mode tcp` MUST complete in <60s and produce a valid CSV with G1 isomorphism passing. **This is the test that validates QA-D009-001 fix (Phase B-1 / TASK-0596) actually closed the TCP path.**

Plus E-3 runtime validation: confirm the post-D-006 hybrid coordinator (coordinator now reduces a local partition) works under TCP/docker — coordinator healthcheck/logs in compose still work.

The CI workflow must run this smoke test on PRs and main pushes that touch `Dockerfile`, `docker-compose.yml`, `relativist-net/`, `relativist-core/src/protocol/`, or `relativist-core/src/partition/compact.rs`.

## Dependencies

- **TASK-0596 (B-1)** — REQUIRED. The wire format fix is what this test validates.
- **TASK-0603 (C-3)** — REQUIRED. The CLI flags must exist to be exercised.
- **TASK-0604 (C-2/C-4)** — REQUIRED. The streaming path must be live.
- **TASK-0608 (E-1)** — REQUIRED. The Dockerfile must build.
- **TASK-0609 (E-2)** — REQUIRED. The bench-tcp compose profile must exist.
- **SPEC commits `c4c80b8` and `82b2d27`** — both already landed.

## Files in scope

| File | Change |
|------|--------|
| `.github/workflows/ci.yml` (or new `.github/workflows/docker-bench-smoke.yml`) | Add a job that: (a) builds the Docker image, (b) runs `docker compose --profile bench-tcp run bench-tcp ...` with the smoke-test args, (c) parses the CSV and asserts G1 isomorphism passing + run time < 60s. |
| `relativist-net/tests/integration_tcp_bench_smoke.rs` (new — alternative or complementary) | Rust-level integration test that spins up a coordinator + 2 workers via `tokio::spawn` (no docker), runs `ep_annihilation --chunk-size=100`, asserts G1 + the SPEC-22 R10b/R12a free_list consistency check (next_id matches across coordinator/worker post-partition-transfer). |
| `docs/DOCKER.md` (extension of TASK-0609's file) | Add a "CI smoke test" section pointing at the workflow file. |

## Files explicitly OUT of scope

- The Dockerfile — TASK-0608.
- The compose profile — TASK-0609.
- The wire format fix — TASK-0596.

## Acceptance criteria

1. CI job runs the bench-tcp smoke on every PR that modifies any file listed in the dependencies above.
2. Smoke completes in <60s wall-time on the GitHub-hosted runner (gate the test budget at 90s to leave margin for runner variability).
3. Smoke asserts the CSV has a row for `ep_annihilation` size 1000 workers 2 with `g1_isomorphism: pass` (or `pass-weak` if `skip_g1` is on).
4. Smoke asserts the SPEC-22 R10b/R12a free_list-consistency check passes (next_id matches between coordinator and worker post-transfer — this is the regression guard for QA-D009-001).
5. (E-3) The hybrid coordinator's healthcheck / logs are verified to be sane: the coordinator container reports `ready` before any worker connects.
6. CI job is wired into the standard PR-blocking matrix.

## Test floor delta expected

**+2 to +4 Rust integration tests** added (in `relativist-net/tests/`). The CI smoke is not a Rust unit test; it counts as integration coverage, not toward the unit-test floor.

## Notes

- This task is the "fan-in" for Phase B-1 + Phase E and is the gate that validates the TCP path is production-ready before the Phase F-2 bench rodada.
- The plan describes E-4 as "smoke passes, G1 OK". This task expands that into a CI-enforced gate plus the optional Rust-level integration test for finer-grained debugging when CI fails.
