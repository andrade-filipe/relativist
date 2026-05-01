# Docker — Relativist v2

## Overview

This document covers the Docker setup for Relativist v2, including the
post-D-006 hybrid coordinator change, available compose profiles, and
common troubleshooting.

## Post-D-006 Hybrid Coordinator

**v2 breaking change relative to v1:** The coordinator now reduces a local
partition in addition to dispatching work to remote workers (hybrid
coordinator model, SPEC-20 D-006). In v1 the coordinator was a pure
dispatcher — it only partitioned the input net, sent partitions to workers,
and merged the results. In v2 the coordinator takes one partition for itself
and reduces it locally alongside the remote workers. This improves utilisation
when the coordinator node has spare CPU capacity.

Practical implications for Docker deployments:

- The coordinator container requires the same CPU budget as a worker container
  (not just a lightweight dispatcher pod).
- Coordinator logs emit `local_interaction_count > 0` per round (observable
  difference from v1 where this count was always 0).
- Healthcheck and ready-probe semantics are unchanged (the coordinator
  reports ready before any worker connects).

## Building the Image

```bash
docker build -t relativist:latest .
```

The Dockerfile uses a multi-stage build:
- **Stage 1 (builder):** `rust:slim` — compiles the workspace with
  `cargo build --release -p relativist-cli --locked`.
- **Stage 2 (runtime):** `debian:bookworm-slim` — copies only the
  `/usr/local/bin/relativist` binary; no Rust toolchain or source files
  are present in the final image.

## Default Profile — coordinator + worker

Run the standard coordinator and worker services (v1-compatible invocation):

```bash
# Start coordinator and 2 workers (default NUM_WORKERS=2)
docker compose up coordinator worker

# Override worker count
NUM_WORKERS=4 docker compose up coordinator worker

# Provide an input net and collect the output
mkdir -p data
# ... copy input.bin to ./data/input.bin ...
docker compose up coordinator worker
# Reduced output lands at ./data/output.bin; metrics at ./data/metrics.json
```

Mount a local `./data` directory for I/O:

```bash
docker run --rm -v $(pwd)/data:/data relativist:latest \
  coordinator --workers 2 --bind 0.0.0.0:9000 \
  --input /data/input.bin --output /data/output.bin
```

## bench-tcp Profile

The `bench-tcp` profile runs the bench subcommand in TCP localhost mode,
parametrised via environment variables. Use it to collect reproducible
benchmark data across different streaming and recycling configurations.

```bash
# Basic invocation — ep_annihilation, sizes=1000, 2 workers
docker compose --profile bench-tcp run --rm bench-tcp

# Override env vars for a specific configuration
CHUNK_SIZE=500 MAX_PENDING_LIFETIME=32 RECYCLE_POLICY=border-clean \
docker compose --profile bench-tcp run --rm bench-tcp

# Collect results to the host
mkdir -p results
docker compose --profile bench-tcp run --rm \
  -v $(pwd)/results:/results \
  bench-tcp
# Results land at ./results/detail.csv and ./results/summary.csv
```

### Environment variables

| Variable               | CLI flag                    | Default              |
|------------------------|-----------------------------|----------------------|
| `CHUNK_SIZE`           | `--chunk-size`              | `10000`              |
| `MAX_PENDING_LIFETIME` | `--max-pending-lifetime`    | `16`                 |
| `RECYCLE_POLICY`       | `--recycle-policy`          | `disable-under-delta`|
| `REPRESENTATION`       | `--representation`          | `dense`              |
| `BENCH_SIZES`          | `--sizes`                   | `1000`               |
| `BENCH_WORKERS`        | `--workers`                 | `1,2`                |
| `BENCH_MODE`           | `--mode`                    | `tcp_localhost`      |

## CI Smoke Test

A dedicated workflow at `.github/workflows/docker-bench-smoke.yml` runs the
bench-tcp profile on every PR touching `Dockerfile`, `docker-compose.yml`,
or core protocol/partition files. The smoke validates:

- G1 isomorphism passing after a 2-worker TCP run.
- `CompactSubnet.free_list` round-trip non-empty (regression guard for
  QA-D009-001 / SPEC-19 R35a).
- Run completes within 90 s on the GitHub-hosted runner.

## Known Limitations on Windows Host

- `/proc/self/status` is Linux-only. The `peak_memory_during_construction`
  metric (SPEC-09 R18a) will always report `0` when running under Docker
  Desktop on Windows — the Linux kernel inside the VM has access to `/proc`,
  but the Rust code that reads `VmHWM` may not see it from inside the
  container depending on the Docker Desktop version.
- Port binding on Windows with WSL 2 backend: if port 9000 is already in
  use, override with `-p 19000:9000` and pass `--bind 0.0.0.0:9000` inside
  the container.

## Troubleshooting

**"connection refused" when workers start:**
The workers connect to `coordinator:9000` via Docker internal DNS. If you
override the coordinator's bind address, update the worker's
`--coordinator` flag accordingly.

**Coordinator exits immediately without output:**
Check that `./data/input.bin` exists. The coordinator requires a valid
bincode-serialised `Net` file at the `--input` path.

**bench-tcp run hangs:**
A hang usually means the bench harness could not establish a TCP listener
on the loopback. Verify there is no port conflict on 9001 (the default
bench TCP port). Set `RUST_LOG=debug` to capture connection events.
