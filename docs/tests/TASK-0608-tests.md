# TEST-SPEC-0608 — Tests for TASK-0608 — Dockerfile workspace-aware COPY

**Task:** TASK-0608 (Phase E-1, P0)
**Spec:** none (build infrastructure).
**Origin:** D-011 plan §E-1 — Dockerfile broken since workspace refactor (TASK-0320..0323).
**Test floor delta:** **+0 cargo tests** (verification is via `docker build`). Adds **+4 CI smoke tests** (not counted in the cargo `--test` floor).
**Prerequisites:** None.

---

## Test inventory

| test_id | level | target file | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| IT-0608-01 | docker-smoke | `.github/workflows/docker-smoke.yml::build_image_from_clean_checkout` (or local `scripts/test-dockerfile.sh`) | none | CI only |
| IT-0608-02 | docker-smoke | same file::`run_image_help_returns_v2_cli_help` | none | CI only |
| IT-0608-03 | docker-smoke | same file::`run_image_bench_mode_produces_csv` | none | CI only |
| IT-0608-04 | docker-smoke | same file::`build_layer_cache_does_not_invalidate_core_on_cli_only_change` | none | CI only |
| IT-0608-05 | docker-smoke | same file::`image_does_not_contain_tests_directory` | none | CI only |

Total: **5 docker-smoke tests** (not Rust unit tests; do not count toward cargo floor).

---

## Per-test specifications

### IT-0608-01 — `build_image_from_clean_checkout`

**Purpose.** Acceptance criterion #1: `docker build .` succeeds from a clean checkout of `v2-development` (no cached layers required for correctness).
**Setup.**
- A fresh shallow clone of `v2-development` into a temp dir.
- `docker system prune -af` (or use a unique image tag with `--no-cache`).
**Action.** `docker build --no-cache -t relativist:test-0608 .` from the project root.
**Assertions.**
- Exit code == 0.
- Build log contains the line `COPY relativist-core/ relativist-core/` AND `COPY relativist-cli/ relativist-cli/` (verifies the workspace COPY pattern is used, not the legacy single-`src/` pattern).
- Build log does NOT contain `COPY src/ src/` (legacy pattern absent).
- Build completes within a reasonable wall-clock (informational; ~10 min on cold cache is acceptable).
**Boundary case coverage.** Catches a regression where the Dockerfile is reverted or partially fixed.
**Why it must exist.** Acceptance criterion #1 of TASK-0608.

---

### IT-0608-02 — `run_image_help_returns_v2_cli_help`

**Purpose.** Acceptance criterion #3: the resulting image's binary executes and returns CLI help.
**Setup.** Image `relativist:test-0608` built by IT-0608-01.
**Action.** `docker run --rm relativist:test-0608 --help`. Capture stdout.
**Assertions.**
- Exit code == 0.
- Stdout contains the literal `bench` (subcommand name from v2 CLI).
- Stdout contains the literal `coordinator` OR `worker` (other v2 subcommands per `relativist-cli/src/main.rs`).
- Stdout does NOT contain Cargo error markers (e.g., `error[E`, `failed to compile`).
**Boundary case coverage.** Catches an image that builds but produces an unrunnable binary (e.g., wrong target triple, missing dynamic library).
**Why it must exist.** Acceptance criterion #3.

---

### IT-0608-03 — `run_image_bench_mode_produces_csv`

**Purpose.** Acceptance criterion #2 + smoke for Phase F-2 readiness: bench mode actually runs in the container.
**Setup.** Image from IT-0608-01.
**Action.** `docker run --rm -v $(pwd)/results:/results relativist:test-0608 bench --benchmark ep_annihilation --sizes 100 --workers 1 --output /results/test-0608.csv`. (Adjust mount + arg names per the actual CLI surface.)
**Assertions.**
- Exit code == 0.
- File `results/test-0608.csv` exists on host after the run.
- CSV contains a header row + at least 1 data row.
- The data row's `benchmark` column = `ep_annihilation`.
- Wall-clock < 60 s.
**Boundary case coverage.** Catches an image that runs `--help` but fails on actual bench dispatch (e.g., missing `/proc` mount, missing tokio runtime feature).
**Why it must exist.** Plan §E-1 — image must be usable for Phase F-2 bench rodada.

---

### IT-0608-04 — `build_layer_cache_does_not_invalidate_core_on_cli_only_change`

**Purpose.** Workspace cache discipline: changing `relativist-cli/src/main.rs` MUST NOT invalidate the `relativist-core/` build layer (keeps incremental docker builds fast for CLI-only iteration).
**Setup.**
- Build the image once (cold cache or warm).
- Modify `relativist-cli/src/main.rs` (e.g., add a comment).
- Re-build with `docker build .` (no `--no-cache`).
**Action.** Capture the build log of the second invocation.
**Assertions.**
- Build log shows the `relativist-core/` COPY layer status as `CACHED`.
- Build log shows the `relativist-cli/` COPY layer status as NOT cached (it changed).
- The `cargo build --release -p relativist-cli` step re-runs but does NOT recompile `relativist-core` from scratch (verifiable by build wall-clock < 50% of the cold-cache time, OR by parsing cargo's `Compiling relativist-core` lines being absent).
**Boundary case coverage.** Catches a Dockerfile fix that uses a single `COPY . .` (which invalidates everything on any change). The plan's three-line COPY pattern is specifically designed to give this property.
**Why it must exist.** Plan §E-1 implicit requirement (workspace cache discipline) + developer-experience floor.

---

### IT-0608-05 — `image_does_not_contain_tests_directory`

**Purpose.** Acceptance criterion #5: the image does NOT pull `tests/` into production layers (production-only sources).
**Setup.** Image from IT-0608-01.
**Action.** `docker run --rm relativist:test-0608 sh -c 'find / -name "*.rs" -path "*/tests/*" 2>/dev/null | head'`. Capture stdout.
**Assertions.**
- Stdout is empty (no `*.rs` files under any `tests/` directory in the image).
- Specifically: `relativist-core/tests/` is NOT present in the final image filesystem (verified via `docker run --rm relativist:test-0608 ls /app/relativist-core/tests/ 2>&1` returning a non-zero exit code or "No such file or directory").
**Boundary case coverage.** Catches a Dockerfile fix that uses `COPY relativist-core/ relativist-core/` BUT forgets the `.dockerignore` entry for `tests/` (currently relies on cargo's release build not packaging tests, which is correct, but the source files would still be copied and bloat the image).
**Why it must exist.** Acceptance criterion #5; image hygiene.

---

## Coverage matrix

| test_id | §AC-1 | §AC-2 | §AC-3 | §AC-4 | §AC-5 |
|---|---|---|---|---|---|
| IT-0608-01 | ✅ | ✅ | | | |
| IT-0608-02 | | ✅ | ✅ | | |
| IT-0608-03 | | ✅ | | | |
| IT-0608-04 | | | | ✅ | |
| IT-0608-05 | | | | | ✅ |

Every acceptance criterion has ≥1 test. Note: §AC-4 ("Image size comparable to v1 — informational, not a hard gate") is intentionally not a test; informational only.

---

## Implementation guidance for the developer

These are CI-side smoke tests, not cargo `#[test]`s. The recommended placement is one of:
1. **GitHub Actions workflow** at `.github/workflows/docker-smoke.yml`, executed on every PR touching `Dockerfile` or `relativist-cli/`.
2. **A shell script** at `scripts/test-dockerfile.sh` that runs all 5 assertions in sequence and exits non-zero on first failure. Invoked from the workflow.

Either way, the test floor delta on `cargo test` is **+0**. The d-011 plan accounts for this — Dockerfile validation is exit-code based, not unit-test based.

---

## Out-of-scope tests (deferred to other tasks)

- `docker-compose.yml` updates → **TASK-0609**.
- TCP smoke between coordinator + worker containers → **TASK-0610** (Phase E-4).
- Multi-arch image builds (arm64) → out of scope; single-arch (linux/amd64) is sufficient for D-011.
- Image security scan (trivy, grype) → out of scope; not a D-011 deliverable.
