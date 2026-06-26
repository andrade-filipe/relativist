---
title: Troubleshooting
summary: Lookup reference for Relativist install, run, Docker, TCP, memory, and test errors.
keywords: [troubleshooting, error, windows, smartscreen, docker, tcp, memory, build, cargo]
modules: [protocol, config]
specs: [SPEC-07, SPEC-13]
audience: [user]
status: reference
updated: 2026-06-26
---

# Troubleshooting

Symptom-to-fix lookup. For campaign-specific limits see [benchmarks/limitations.md](../benchmarks/limitations.md) (L1-L7) and the per-campaign notes in [benchmarks/campaigns/](../benchmarks/campaigns/).

## install

### windows-smartscreen-blocks-binary

**Symptom.** Running the downloaded binary shows "Windows protected your PC" / "unrecognized app".

**Cause.** The binary is unsigned (no EV cert); SmartScreen flags new, low-reputation executables.

**Fix.** Click "More info" -> "Run anyway". Or build locally with `cargo build --release` — a compiler-produced binary does not trip SmartScreen. Tracked as an open UX issue (ROADMAP 2.37-2.39, Tauri GUI + distribution).

### rustc-not-found-or-outdated

```bash
rustup update stable
rustc --version    # must satisfy MSRV in Cargo.toml
```

### linker-not-found-windows

`cargo build` fails with `link.exe not found`. Install Visual Studio Build Tools (workload "Desktop development with C++"), or MSYS2 with `mingw-w64`.

## run-local

### command-not-found

Add `target/release/` to `PATH`, or run with an explicit path: `./target/release/relativist`.

### inspect-shows-not-normal-form

`inspect` reports `Normal Form: no` after `reduce`. Should not happen on terminating nets. Check:

1. `--max-steps <N>` too low on `reduce`.
2. Net has non-terminating cycles (out of TCC scope; premise P6).
3. Stale items in `redex_queue` after an agent delete (bug — file an issue).

### compute-exp-wrong-or-decode-failed

Expected. `compute exp` reduces correctly, but the current decoder does not walk a cyclic DUP. See [L5](../benchmarks/limitations.md#l5). Use `inspect` to confirm the structure is correct.

## docker

### input-bin-not-found

```bash
ls data/input.bin    # ensure the volume is mounted and the file exists
```

### windows-git-bash-path-conversion

**Symptom.** Git Bash rewrites `/data` to `C:\Program Files\Git\data`; volume fails to mount.

**Fix.** Set `MSYS_NO_PATHCONV=1` for `docker compose` commands carrying unix-style paths, or run from PowerShell (no MSYS conversion):

```bash
MSYS_NO_PATHCONV=1 docker compose up
```

### coordinator-exits-before-metrics-flush

**Symptom.** `metrics.json` missing after `docker compose up --abort-on-container-exit --exit-code-from coordinator`.

**Cause.** `--abort-on-container-exit` SIGKILLs the coordinator as soon as the first worker exits.

**Fix.** Use `docker compose up -d` + `docker wait relativist-coordinator-1` instead. Pattern used by `reproduce_article/scripts/bench_docker_resume2.sh`. See [L7](../benchmarks/limitations.md#l7).

### docker-desktop-out-of-space

Restart Docker Desktop. If the disk filled (`no space left on device`):

```bash
docker system df
docker builder prune -af    # free build cache
docker image prune -af      # free unused images
```

## tcp-distributed

### worker-rejected-on-connect

**Symptom.** Worker log: `authentication failed` or `version mismatch`. Common causes:

1. `--auth-token` mismatch between coordinator and worker.
2. Different binary versions (incompatible wire protocol). Run `relativist --version` on both sides.
3. `--features zero-copy` enabled on only one side (see [v2-features.md](../guides/v2-features.md)).

### worker-cannot-reach-coordinator

Test low-level connectivity from the worker:

```bash
nc -vz <COORD_HOST> 9000    # expect: (host) 9000 open
```

On timeout: firewall (Windows Defender Firewall, iptables). Open TCP 9000 (or your chosen port) on the coordinator host.

### peak-memory-missing-in-metrics

Expected on Windows/macOS. `peak_memory_bytes` is Linux-only (read from `/proc/self/status`).

## memory

### coordinator-oom-large-sizes

The default WSL2 VM has 15 GiB. At 50M agents, bincode v1 uses ~3-4 GB for the serialized partition alone; holding two copies (input + merge) blows the budget. Mitigations:

- Raise WSL2 memory (`.wslconfig` on Windows, e.g. `memory=24GB`).
- Use `--features zero-copy` (SPEC-18 avoids the deserialize copy).
- Use `--delta-mode` (SPEC-19) — coordinator does not hold the merged net until the end.

### worker-swapping-linux

Check `free -h` + `vmstat 1`. If `si/so` > 0 during a run you are swapping. Add RAM, reduce `workers`, or reduce `size`.

### frame-cap-exceeded

`protocol: frame size exceeds limit` (1 GiB cap). A serialized partition exceeded 1 GiB under bincode v1 + CompactSubnet; occurs at stress sizes (50M with w=1/w=2). See [L6](../benchmarks/limitations.md#l6); future mitigation in ROADMAP 2.23 (wire compaction) and [SPEC-18](../specs/SPEC-18-wire-format-v2.md).

## build-tests

### cargo-test-fails-after-branch-switch

```bash
cargo clean
cargo test
```

If it still fails, confirm `Cargo.lock` is committed. For hotfixes you may need to regenerate it: `rm Cargo.lock && cargo build`.

### flaky-single-test

```bash
RUST_BACKTRACE=full cargo test -- --nocapture --test-threads=1 <TEST_NAME>
```

`--test-threads=1` isolates concurrency. File an issue if it reproduces deterministically single-threaded.

## benchmarks

### correct-false-in-csv

**Stop immediately** — this is a G1 regression. Then:

1. Run `cargo test` — 690+ tests must pass.
2. Inspect `raw/phase1/<bench>.log` (or `raw/phase2/metrics_*.json`) for the affected config.
3. File an issue with the datapoint + log.

### wall-clock-too-high

1. Power plan reverted to Balanced? `powercfg /getactivescheme` (Windows).
2. Thermal throttling? Cool down ~30 min.
3. Background load (browser, IDE, AV scan)? See [v1-local-baseline §1.4](../benchmarks/campaigns/v1-local-baseline.md#14-environment-hygiene-windows-11).

### high-cv-many-configs

CV > 0.15 across configs means the machine had background load. Re-run Phase 1 clean. Marking single runs `keep` does not fix a systemic pattern. See [v1-local-baseline §4.2](../benchmarks/campaigns/v1-local-baseline.md#42-triagem-cv).

## reporting-a-bug

- Attach: `relativist --version`, `rustc --version`, OS + version, full command, output, relevant `raw/` log.
- Correctness bug (G1): include the input `.bin` (or the `generate` command that produced it).
- Issues: [github.com/andrade-filipe/relativist/issues](https://github.com/andrade-filipe/relativist/issues).
