# What's next for Relativist

A short, contributor-facing map of where the software should go — distilled from
[`../ROADMAP.md`](../ROADMAP.md) (which has the full rationale and the per-feature
status tags). If you're a curious reader or a would-be contributor, start here.

## The one result that frames everything

Relativist's v1 campaigns (4490 executions, **0 correctness failures**) proved the
research claim — distributed reduction is bit-for-bit equivalent to sequential
reduction (`reduce_all(net) ≅ run_grid(net, n)`) — but also produced a **clean
negative performance result on local/localhost hardware**:

> No configuration with workers ≥ 2 achieved speedup > 1.0. The per-agent
> distribution overhead `c_o` is ~2.2× the per-agent reduction cost `c_r`. The
> speedup formula `1 / (c_o/c_r + 1/w)` then caps below 1 — and **N cancels**, so
> the limit is architectural, not a matter of scaling the problem up.

For speedup at `w=2` you need `c_o/c_r < 0.5` — a **~77% overhead reduction**. That
single number is the engineering target. Full derivation:
[`../ROADMAP.md` §2.40](../ROADMAP.md).

## The critical path to break-even

These three items, together, are projected to take `c_o/c_r` from 2.2 down to ~0.1
(speedups of ~1.7/2.9/4.4 at w=2/4/8). They're the highest-leverage work:

| # | Item | Status | Idea |
|---|------|--------|------|
| 2.26 | **Delta-only protocol** (stateful workers) | ✅ shipped (D-005) | Workers keep partition state; only border deltas cross the wire (~30% fewer bytes/round measured). |
| 2.34 | **Coordinator-free round** | ⚠️ helper exists, integration pending | When no worker reports border activity, skip the merge entirely. |
| 2.35 | **Delta-based merge** | ◻️ not yet a distinct module | Replace O(N) merge with O(border_changes) merge. |

The structural saving of 2.26 is already proven on the wire; what's missing is the
empirical payoff, because on **TCP-localhost** the CPU cost of the abstraction
layers eats the bytes-saved. Which leads to the next milestone.

## The next empirical milestone: Phase 3 LAN

The break-even crossover is expected at LAN bandwidth ≤ ~156 MB/s (≈1.2 Gbps) —
i.e. **every real LAN/WAN qualifies**, but `localhost` does not. The harness is
ready ([`../../reproduce_article/scripts/bench_phase3_lan.sh`](../../reproduce_article/scripts/bench_phase3_lan.sh),
[`../benchmarks/phase-3-lan.md`](../benchmarks/phase-3-lan.md)); the **real
cross-machine run has not been done**. Running it on real cabling and publishing
the crossover is the single most valuable next contribution. This is a
**good first heavy contribution** if you have a couple of networked machines.

## Other directions (lower on the critical path)

Pulled from `ROADMAP.md`; see it for status tags and complexity notes.

- **Confluence-enabled elasticity** — dynamic worker join/departure is partially
  shipped (D-006); full mid-session departure recovery (reclaim from retained
  partitions) is deferred to v2.1.
- **Recipe-based distributed generation** (SPEC-25, draft) — coordinator emits a
  compact recipe; each worker materialises its own partition. Needed for
  genuinely M+-agent distributed benchmarks without a full-net bottleneck.
- **Memory-bounded coordinator** — streaming generation + partitioning shipped
  (D-010); the MVP for "coordinator handles nets larger than its RAM" is
  structurally in place but not yet validated empirically (Phase 3 scope).
- **Label support for extended IC** (HVM/Bend compatibility) — a *decision-pending*
  architectural fork: pure Lafont IC (current) vs. labelled CON/DUP. Touches all 6
  rules, the wire protocol, and ARG-001 (would need a labelled-IC confluence
  citation, e.g. Mazza 2006). ~1000+ LoC + invariant revision.
- **WAN hardening** (SPEC-24, draft) — TLS-mandatory, session reconnect, RTT-aware
  batching, Tailscale/WireGuard integration. Prerequisite for any "Relativist on
  the open internet" claim.
- **GPU workers, WASM target, GUI (Tauri)** — explicitly out of the TCC's scope;
  long-range research / UX directions.

## How to pick something up

1. Read [`../ROADMAP.md`](../ROADMAP.md) for the item's full context and status tag.
2. Open an issue to discuss (especially for anything touching the model — see
   [`../../GOVERNANCE.md`](../../GOVERNANCE.md)).
3. Use the [RPI workflow](../../CONTRIBUTING.md) and keep the three gates green.
