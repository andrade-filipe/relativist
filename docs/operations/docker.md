---
title: Docker & deployment
summary: Build the Relativist image and run coordinator/worker plus bench profiles via Docker Compose, with scaling, env vars, and pitfalls.
keywords: [docker, docker-compose, deployment, coordinator, worker, scaling, container, hybrid]
modules: [protocol, coordinator, worker]
specs: [SPEC-07, SPEC-13, SPEC-15]
audience: [user, contributor]
status: reference
updated: 2026-06-26
---

# Docker & deployment

Operations guide for running Relativist v2 in containers: building the image,
launching coordinator + workers with Docker Compose, scaling workers, the bench
profiles, and the post-D-006 hybrid-coordinator CPU budget. All commands run from
the repo root, where `Dockerfile`, `docker-compose.yml`, and `.env.example` live.

## build-the-image

```bash
docker build -t relativist:latest .
```

Multi-stage build:

- **builder** — `rust:slim-bookworm`, runs `cargo build --release -p relativist-cli --locked`.
  Pinned to bookworm so the builder GLIBC (2.36) matches the runtime base; an
  untagged `rust:slim` tracks Trixie and breaks the runtime with a dynamic
  linker error. A dummy-source layer caches the dependency build independently
  from source changes.
- **runtime** — `debian:bookworm-slim`, copies only `/usr/local/bin/relativist`
  plus `ca-certificates`. No toolchain or source in the final image.
  `WORKDIR /data`; `ENTRYPOINT ["/usr/local/bin/relativist"]`, so compose
  `command:` entries start at the subcommand (`coordinator`, `worker`, `bench`).

Compose `build: .` builds the same image lazily on first `up`/`run`. After
changing source, force a rebuild with `docker compose build` or `--build`.

## default-profile-coordinator-worker

The default services (no `--profile` flag) reproduce the v1-style topology: one
coordinator plus N workers over TCP, sharing a `./data:/data` bind mount.

```bash
# Start coordinator + 2 workers (NUM_WORKERS default = 2)
docker compose up coordinator worker

# Override worker count
NUM_WORKERS=4 docker compose up coordinator worker
```

The coordinator binds `0.0.0.0:9000` (published as `9000:9000`); workers connect
to `coordinator:9000` via Compose DNS and declare `depends_on: coordinator`.

I/O paths are parametrised (D-017 / TASK-0730) so multi-run demos can target
per-run files without colliding:

```bash
mkdir -p data            # place your input at ./data/input.bin
docker compose up coordinator worker
# reduced net -> ./data/output.bin ; metrics -> ./data/metrics.json
```

Single-shot run without Compose:

```bash
docker run --rm -v "$(pwd)/data:/data" relativist:latest \
  coordinator --workers 2 --bind 0.0.0.0:9000 \
  --input /data/input.bin --output /data/output.bin
```

### worker-scaling

`NUM_WORKERS` drives both `coordinator --workers` and the worker
`deploy.replicas` count, keeping the expected and actual worker counts in sync —
always set it (not `--scale`) so the coordinator waits for the right number:

```bash
NUM_WORKERS=8 docker compose up coordinator worker
```

## hybrid-coordinator-cpu-budget

**v2 breaking change (post-D-006, SPEC-20).** The coordinator is no longer a pure
dispatcher. It now takes one partition for itself and reduces it locally
**alongside** the remote workers (hybrid-coordinator model), improving
utilisation when the coordinator node has spare CPU.

Deployment implications:

- Give the coordinator container the **same CPU budget as a worker** — it is no
  longer a lightweight dispatcher.
- Coordinator logs emit `local_interaction_count > 0` per round (in v1 this was
  always `0`) — a quick observable check that the hybrid path is active.
- Healthcheck / ready-probe semantics are unchanged: the coordinator reports
  ready before any worker connects.

## bench-profiles

Two opt-in profiles drive the `bench` subcommand in `tcp_localhost` mode. The
bench harness manages its own coordinator/worker lifecycle internally, so these
profiles need **no** separate `coordinator`/`worker` containers. Both mount
`./data` and `./results`, emitting `/results/detail.csv` and `/results/summary.csv`.

### bench-tcp-streaming

Exercises the streaming path with an explicit small `--chunk-size` (default
`100`) to force multi-batch assembly (SPEC-21 §3.8, SPEC-09 R18a):

```bash
mkdir -p results
docker compose --profile bench-tcp run --rm bench-tcp

# Override a single configuration
CHUNK_SIZE=500 MAX_PENDING_LIFETIME=32 RECYCLE_POLICY=border-clean \
docker compose --profile bench-tcp run --rm bench-tcp
```

### bench-tcp-eager

Exercises the v1-equivalent eager path — the `--chunk-size` flag is **omitted**
(`chunk_size = None`), so the harness materialises eagerly. CI runs both
profiles (QA-D011-004) so a regression in either path is caught:

```bash
docker compose --profile bench-tcp-eager run --rm bench-tcp-eager
```

## environment-variables

Copy `.env.example` to `.env` (Compose auto-loads it from the repo root), or pass
values inline with `-e` for ad-hoc runs.

| Variable               | Maps to                  | Default (compose)        | Scope                  |
|------------------------|--------------------------|--------------------------|------------------------|
| `NUM_WORKERS`          | `--workers` / replicas   | `2`                      | default profile        |
| `INPUT_PATH`           | `--input`                | `/data/input.bin`        | default profile        |
| `OUTPUT_PATH`          | `--output`               | `/data/output.bin`       | default profile        |
| `METRICS_PATH`         | `--metrics`              | `/data/metrics.json`     | default profile        |
| `CHUNK_SIZE`           | `--chunk-size`           | `10000` coord / `100` bench-tcp | both           |
| `MAX_PENDING_LIFETIME` | `--max-pending-lifetime` | `16`                     | both (SPEC-21 R37c)    |
| `RECYCLE_POLICY`       | `--recycle-policy`       | `disable-under-delta`    | bench (SPEC-22 R10b)   |
| `REPRESENTATION`       | `--representation`       | `dense`                  | bench                  |
| `BENCH_SIZES`          | `--sizes`                | `1000`                   | bench                  |
| `BENCH_WORKERS`        | `--workers`              | `1,2`                    | bench                  |
| `BENCH_MODE`           | `--mode`                 | `tcp_localhost`          | bench                  |

`RECYCLE_POLICY` is `disable-under-delta` (safe, baseline) or `border-clean`
(aggressive, stricter invariants). `REPRESENTATION` is `dense` (v1 baseline) or
`sparse` (construction-time micro-bench, single-worker only). `BENCH_MODE` is
`local` (in-process, fastest) or `tcp_localhost`. Note the `CHUNK_SIZE` default
differs by service: the coordinator defaults to `10000`, but the `bench-tcp`
service overrides to `100` to force the streaming path.

## pitfalls

**Coordinator shutdown race (L7).** Do **not** drive benchmark runs with
`docker compose up --abort-on-container-exit --exit-code-from coordinator`: when
the first worker exits, Compose sends SIGTERM (then SIGKILL, exit `137`) to the
coordinator before it persists `metrics.json` and `output.bin` — reduction
completes but the artefacts never land. Mitigation (used by
`reproduce_article/scripts/bench_docker_resume2.sh`): start detached and wait for
the coordinator to exit on its own:

```bash
docker compose up -d coordinator worker
docker wait relativist-coordinator-1     # blocks until the coordinator finishes
```

An internal SIGTERM handler is tracked as optional hardening in `docs/ROADMAP.md`.
See `docs/benchmarks/limitations.md` (L7).

**"connection refused" on worker start.** Workers dial `coordinator:9000` via
Compose DNS. If you change the coordinator `--bind`, update each worker
`--coordinator` to match.

**Coordinator exits immediately, no output.** Ensure `INPUT_PATH` exists inside
the container (default `./data/input.bin` on the host). The coordinator needs a
valid bincode-serialised `Net` at `--input`.

**Port 9000 already in use (Windows / WSL 2).** Remap the published port and keep
the in-container bind, e.g. `docker run -p 19000:9000 ... --bind 0.0.0.0:9000`.

**`peak_memory_during_construction` reports 0 (Windows host).** `/proc/self/VmHWM`
(SPEC-09 R18a) may be unreadable from inside the container under Docker Desktop
on Windows; the metric reads `0` there. Linux hosts report it correctly.
