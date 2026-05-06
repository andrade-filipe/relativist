# TEST-SPEC-0609 — Tests for TASK-0609 — `docker-compose.yml` `bench-tcp` profile + `docs/DOCKER.md`

**Task:** TASK-0609 (Phase E-2 + part of E-3, P0)
**Spec:** none (deployment infrastructure).
**Origin:** D-011 plan §E-2 + §E-3 (documentation portion).
**Test floor delta:** **+0 cargo tests** (verification is via `docker compose` smokes; CI-side assertions).
**Prerequisites:** TASK-0608 (Dockerfile builds), TASK-0596 (wire format fix), TASK-0603 (CLI flags exist).

---

## Test inventory

| test_id | level | target file | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| IT-0609-01 | docker-smoke | `.github/workflows/docker-smoke.yml::bench_tcp_profile_runs_basic_invocation` (or `scripts/test-compose.sh`) | TASK-0608, TASK-0596 | CI only |
| IT-0609-02 | docker-smoke | same file::`env_vars_propagate_to_cli_flags` | TASK-0603 | CI only |
| IT-0609-03 | docker-smoke | same file::`legacy_coordinator_worker_profile_unchanged` | none | CI only |
| IT-0609-04 | docker-smoke | same file::`docs_docker_md_exists_and_documents_hybrid_coordinator` | none | none (file-existence test) |
| IT-0609-05 | docker-smoke | same file::`bench_tcp_healthcheck_passes_before_workers_connect` | none | CI only |

Total: **5 docker-smoke / file-existence tests** (NOT Rust unit tests; do not count toward cargo floor).

Cargo floor delta: **+0**.

---

## Per-test specifications

### IT-0609-01 — `bench_tcp_profile_runs_basic_invocation`

**Purpose.** Acceptance criterion #1: `docker compose --profile bench-tcp run bench-tcp --benchmark ep_annihilation --sizes 1000 --workers 2 --chunk-size 100` succeeds.
**Setup.**
- Image `relativist:test-0608` available (built by TASK-0608 IT-0608-01).
- `docker-compose.yml` has the new `bench-tcp` profile with the coordinator + N workers wiring.
**Action.** `docker compose --profile bench-tcp run --rm bench-tcp --benchmark ep_annihilation --sizes 1000 --workers 2 --chunk-size 100`.
**Assertions.**
- Exit code == 0.
- Wall-clock < 90 s (margin over the 60 s gate from TASK-0610).
- Stdout contains `Bench complete` or equivalent terminal log line.
- The container's coordinator and worker logs show successful TCP handshake (`Worker connected`, `Coordinator ready` or equivalent).
- No `panic`, `error[E`, or `connection refused` strings in logs.
**Boundary case coverage.** Catches a compose file with broken service dependencies (e.g. coordinator not waiting for worker startup, port misconfiguration).
**Why it must exist.** Acceptance criterion #1 of TASK-0609.

---

### IT-0609-02 — `env_vars_propagate_to_cli_flags`

**Purpose.** Acceptance criterion #2: env vars `CHUNK_SIZE`, `MAX_PENDING_LIFETIME`, `RECYCLE_POLICY` propagate into the bench CLI flags inside the container.
**Setup.**
- `docker compose --profile bench-tcp` invocation with explicit env: `CHUNK_SIZE=200`, `MAX_PENDING_LIFETIME=5000`, `RECYCLE_POLICY=lifo`.
**Action.** Run a short bench (`--benchmark ep_annihilation --sizes 100 --workers 2`) and capture the bench process's argv as logged by tracing.
**Assertions.**
- Tracing logs (or stdout) contain `--chunk-size 200`.
- Tracing logs contain `--max-pending-lifetime 5000`.
- Tracing logs contain `--recycle-policy lifo`.
- Equivalently: the `BenchArgs` struct as logged at startup shows `chunk_size: 200, max_pending_lifetime: Some(5000), recycle_policy: Lifo`.
- A control run with the env vars UNSET produces the default values (e.g. `--chunk-size <default>`) — proving the propagation is env-driven, not hardcoded.
**Boundary case coverage.** Catches a compose file that defines env vars but forgets to `${VAR}` them into the `command:` line — silent drop of the env value, defaults are used, test would otherwise pass on Test #1.
**Why it must exist.** Acceptance criterion #2 of TASK-0609; this is the headline parameterization contract.

**Implementation note.** The exact CLI flag names (`--chunk-size` vs `--chunk_size`) MUST match the names introduced in TASK-0603. Stage 3 developer must verify after TASK-0603 lands.

---

### IT-0609-03 — `legacy_coordinator_worker_profile_unchanged`

**Purpose.** Acceptance criterion #3: backward-compat. The existing v1-style `coordinator` + `worker` services (without the `bench-tcp` profile) still work.
**Setup.** A `docker compose up -d coordinator worker` invocation (no profile flag — default services).
**Action.** Wait for healthcheck; run a small benchmark via the v1-style invocation; tear down.
**Assertions.**
- `docker compose up -d coordinator worker` exit code == 0.
- Both containers reach `running` status within 30 s.
- `docker compose ps` lists `coordinator` and `worker` (without `bench-tcp`).
- The `bench-tcp` service is NOT listed in the default `up` (it's profile-gated).
- `docker compose down` exits cleanly.
**Boundary case coverage.** Catches a compose file refactor that accidentally moves the existing coordinator/worker into the `bench-tcp` profile (which would break v1-style invocation).
**Why it must exist.** Acceptance criterion #3 of TASK-0609.

---

### IT-0609-04 — `docs_docker_md_exists_and_documents_hybrid_coordinator`

**Purpose.** Acceptance criterion #4: `docs/DOCKER.md` exists and explicitly documents the post-D-006 hybrid coordinator (coordinator now reduces a local partition, contrasting with v1's pure-dispatcher role).
**Setup.** None — file-system check.
**Action.** Read `docs/DOCKER.md`; grep for marker substrings.
**Assertions.**
- File exists at `docs/DOCKER.md`.
- File length is at least 30 lines (the task says ~50 lines; allow margin).
- Contains the substring `hybrid coordinator` (case-insensitive).
- Contains a substring referring to `D-006` OR `post-D-006` OR `coordinator reduces a local partition`.
- Contains the substring `bench-tcp` (documenting the new profile).
- Contains the substrings `CHUNK_SIZE`, `MAX_PENDING_LIFETIME`, `RECYCLE_POLICY` (documenting the env-var parameterization).
- Contains a "troubleshooting" or "common issues" section (port binding, healthcheck — per task §Files in scope item (d)).
**Boundary case coverage.** Catches a stub `docs/DOCKER.md` that exists but is empty or missing the post-D-006 documentation.
**Why it must exist.** Acceptance criterion #4 of TASK-0609. This test runs as a cargo `#[test]` (file existence + grep) so it catches missing docs at PR time.

**Implementation note.** This test CAN be a cargo `#[test]` (it just reads a file from the repo root). If implemented this way, it counts toward the cargo floor: **+1 default**. Decision: implement as a cargo test. **Floor adjustment: +1 default** (folded into the IT-0609 series above; total batch-2 floor recomputed accordingly in the summary).

---

### IT-0609-05 — `bench_tcp_healthcheck_passes_before_workers_connect`

**Purpose.** Acceptance criterion #5: the coordinator's healthcheck passes before any worker connects (no startup race).
**Setup.** `docker compose --profile bench-tcp up -d coordinator` (only the coordinator service from the bench-tcp profile, NOT the workers).
**Action.** Poll `docker compose ps` (or `docker inspect <coordinator_id> --format '{{.State.Health.Status}}'`) until the coordinator reports `healthy`. Bound the poll at 30 s.
**Assertions.**
- The coordinator reports `healthy` within 30 s.
- No worker container is running at the time the coordinator becomes healthy (proves healthcheck is independent of worker presence — required for compose dependency ordering).
- After the coordinator is healthy, starting workers via `docker compose --profile bench-tcp up -d worker_1 worker_2` succeeds and the workers connect within 10 s.
**Boundary case coverage.** Catches a healthcheck that accidentally requires a worker connection (which would be a chicken-and-egg startup deadlock).
**Why it must exist.** Acceptance criterion #5 of TASK-0609.

---

## Coverage matrix

| test_id | AC-1 (run succeeds) | AC-2 (env vars) | AC-3 (legacy compat) | AC-4 (DOCKER.md) | AC-5 (healthcheck) |
|---|---|---|---|---|---|
| IT-0609-01 | ✅ | | | | |
| IT-0609-02 | | ✅ | | | |
| IT-0609-03 | | | ✅ | | |
| IT-0609-04 | | | | ✅ | |
| IT-0609-05 | | | | | ✅ |

Every acceptance criterion has exactly 1 test (1:1 mapping).

---

## Implementation guidance for the developer

These are CI-side smoke tests, not cargo `#[test]`s — EXCEPT IT-0609-04, which is a file-grep test and can be a cargo `#[test]`. Recommended placement:
1. **GitHub Actions workflow** at `.github/workflows/docker-smoke.yml` (extends the file from TASK-0608) — IT-0609-01, 02, 03, 05.
2. **Cargo `#[test]`** at `relativist-core/tests/docs_docker_md_present.rs` — IT-0609-04 (file existence + grep).

Cargo floor delta: **+1 default** (only IT-0609-04 is a cargo test).

---

## Out-of-scope tests (deferred to other tasks)

- The TCP smoke + G1 isomorphism gate → TASK-0610.
- Hybrid coordinator runtime correctness (Rust integration test) → TASK-0610.
- Multi-arch image testing → out of scope.

---

## Known spec ambiguity (adversarial flag)

- The exact compose service / profile **name** (`bench-tcp` vs `bench_tcp` vs `bench`) is not pinned by the task — IT-0609-01 hard-codes `bench-tcp` per the task §Files in scope text. If the developer chooses a different name, regenerate the test invocations.
- The env-var → CLI-flag mapping convention is not in any spec. The test assumes uppercase env vars map to kebab-case CLI flags (`CHUNK_SIZE` → `--chunk-size`). If the production code uses snake_case CLI flags (`--chunk_size`), IT-0609-02 fails — flag for Stage 3 to verify against TASK-0603's actual flag names.
- "Healthcheck passes before workers connect" — the task §AC-5 is interpreted here as "the healthcheck does NOT require a worker." A stricter interpretation (the healthcheck waits for a worker) would invert the test. IT-0609-05 picks the looser interpretation and documents it.
