# TASK-0608 — Dockerfile workspace-aware COPY (Phase E-1)

**Phase:** E-1 (D-011 Docker fix — broken since workspace refactor)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** PENDING
**Priority:** P0 (blocks Phase E-4 TCP smoke and Phase F-2 docker bench rodada)
**Spec:** none (build infrastructure).
**Origin:** D-011 plan §E-1 — `Dockerfile:5-6` does `COPY src/ src/` and `COPY benches/ benches/`, but the repo is now a workspace (`relativist-core/`, `relativist-cli/`).
**Estimated complexity:** S (~10 LoC production + ~10 LoC test in CI)
**Estimated stages duration:** Stages 2→3→4→5→6 over ~0.25 day. (Light: build verification is the test.)

---

## Context

The workspace restructure (TASK-0320..0323, landed pre-D-011) moved `src/`, `tests/`, `benches/` into `relativist-core/` and added `relativist-cli/`. The Dockerfile was not updated. Today's `docker build` fails on the `COPY src/ src/` step.

Fix per D-011 plan:
```dockerfile
COPY Cargo.toml Cargo.lock ./
COPY relativist-core/ relativist-core/
COPY relativist-cli/ relativist-cli/
RUN cargo build --release -p relativist-cli
```

Binary must remain at `/app/target/release/relativist`.

## Dependencies

- None on D-011 spec amendments.
- Independent — can run in parallel with Phase B/C/D.

## Files in scope

| File | Change |
|------|--------|
| `Dockerfile` (lines 5-6 and surrounding RUN) | Replace single-`src/` copy with the 3-line workspace COPY + the workspace-aware `cargo build`. |

## Files explicitly OUT of scope

- `docker-compose.yml` — TASK-0609.
- Smoke test in CI — TASK-0610.

## Acceptance criteria

1. `docker build .` succeeds from a clean checkout of `v2-development` (no cached layers required for correctness).
2. The resulting image contains `/app/target/release/relativist` (workspace bin output for `relativist-cli`).
3. `docker run <image> --help` returns the v2 CLI help (smoke that the binary executes).
4. Image size is comparable to v1 (no major bloat — informational, not a hard gate).
5. Build does NOT pull `tests/` into the image (only production sources are copied).

## Test floor delta expected

**+0** (no Rust unit-test additions; verification is via `docker build`).

## Notes

- This is a "build infrastructure" task — Stage 5 QA scope is "did the build break for any subset of the workspace dependencies (e.g., feature flags, target-OS)".
- After this lands, TASK-0609 can author the new bench-tcp compose service against a known-good image.
