---
title: Testing tiers
summary: Two-tier test strategy — a fast essential CI gate (library unit tests) and an optional, non-blocking integration suite.
keywords: [testing, ci, gate, cargo test, --lib, integration tests, ignored, essential, optional, extended tests, g1, run_grid, benchmark, horner, codec]
modules: [reduction, merge, partition, net, bench, encoding]
specs: [SPEC-08, SPEC-01, SPEC-09]
audience: [contributor, llm]
status: reference
updated: 2026-06-26
---

# Testing tiers

Relativist will change a lot: contributors will add new encoders/decoders, new
partition strategies, new transports. So CI enforces the **core that must never
break**, and keeps everything else available but **optional**. Three tiers.

## 1. Essential — the required CI gate (`ci.yml`)

Fast (~2-3 min). Blocks merges. Run on every push/PR:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib                              # core engine + G1
cargo test --lib --features streaming-no-recycle
cargo build --release
```

`cargo test --lib` runs the **library unit tests** (~1700). These cover the
reduction engine (the six rules), partition/merge, the SPEC-01 invariants, and —
critically — the **fundamental property G1** (`reduce_all ≅ run_grid`), which is
exercised by ~90 `run_grid` tests in the `merge` module. This is the research
contribution; it is the line CI defends.

It deliberately **does not** run the `tests/` integration binaries.

## 2. Optional — the full integration suite (`extended-tests.yml`)

The `tests/` integration binaries (54 of them): example codecs (Horner, Church),
the benchmark harness, CLI smokes, wire/elastic/streaming end-to-end. Useful, but
**not the core correctness contract** — an example codec or a benchmark is not
something every change must keep green. These are slow (the Horner round-trip
alone is ~4 min).

They run **non-blocking**:

- automatically **weekly** + on manual **workflow_dispatch** (Actions tab), and
- locally any time with plain `cargo test`.

Do **not** mark `extended-tests` as a required status check.

## 3. Environment-fragile — manual only (`#[ignore]`)

A few D-014 stress-curve smokes depend on runner speed, RAM, or python/bash +
matplotlib (wall-budget trips, RSS measurement, plotting). They are `#[ignore]`d
so they never gate anything. Run them deliberately:

```bash
cargo test -- --ignored
```

## Choosing a tier when you add a test

- **Core engine / distribution / invariant** (something every change must keep
  true) → a **library unit test** in the relevant `src/` module, so it runs in the
  gate.
- **A new codec, a benchmark, a CLI/e2e smoke** → an **integration test** under
  `tests/` (optional tier). It runs in `extended-tests` and locally, not in the
  per-PR gate.
- **Depends on machine speed / RAM / external tools** → `#[ignore]` it with a
  one-line reason.

## Local shortcuts

```bash
cargo test --lib        # the gate — fast, run this before pushing (pre-push hook does it)
cargo test              # everything except #[ignore]d
cargo test -- --ignored # the environment-fragile smokes
```
