---
title: v2 features
summary: Compact guide to the five opt-in v2 features — delta protocol, zero-copy wire, elastic grid, streaming generation, arena management.
keywords: [delta protocol, zero-copy, rkyv, elastic grid, hybrid coordinator, streaming generation, chunk-size, arena, free-list, sparse, recycle, SPEC-18, SPEC-19, SPEC-20, SPEC-21, SPEC-22]
modules: [merge, partition, protocol, net]
specs: [SPEC-18, SPEC-19, SPEC-20, SPEC-21, SPEC-22]
audience: [user, contributor, llm]
status: guide
updated: 2026-06-26
---

# v2 features

The v2 line adds five orthogonal features on top of the frozen v1 baseline. Each one targets a distinct bottleneck of distributed reduction:

| Feature | Cuts | Default in v2 |
|---------|------|---------------|
| Delta protocol (SPEC-19) | network rounds (number of messages) | opt-in |
| Zero-copy wire (SPEC-18) | deserialize CPU (cost per message) | opt-in (build feature) |
| Elastic grid (SPEC-20) | idle coordinator, rigid worker set | opt-in (experimental over TCP) |
| Streaming generation (SPEC-21) | coordinator peak memory | **on by default** |
| Arena management (SPEC-22) | per-partition memory growth | **on by default** (safe auto-decisions) |

Delta and zero-copy compose (fewer messages × cheaper messages). Streaming and arena interact tightly through the free-list (see those two sections). All features preserve **G1** (final net isomorphic to sequential `reduce_all`); none changes v1 behaviour unless its flag is set. Flags below are verified against [../reference/cli.md](../reference/cli.md); each section points to its `../specs/SPEC-NN-*.md` for the formal detail.

## Delta protocol (SPEC-19)

Replaces v1's full-partition BSP. Instead of re-packing and re-sending whole partitions every round (O(N) network per round), workers stay **stateful**: they hold their partition across rounds and exchange only **border deltas** with the coordinator (O(|deltas|) per round, typically `<< N`). The coordinator tracks inter-partition connectivity in a lightweight `BorderGraph` and only rebuilds the full net once, at convergence. Global Normal Form (DC-C5) requires three conditions in the same round: every worker reports zero local redexes, the `BorderGraph` has zero active pairs, and no new delta was reported.

**Use it** when partitions are large (MB-GB) but the active border is small (hundreds of ports), round counts are high (`cascade_cross`, `dual_tree`), and the network is the bottleneck. **Skip it** when a partition fits in a few KB and rounds are CPU-bound — the gain is marginal there.

| Flag | Default | Notes |
|------|---------|-------|
| `--delta-mode` | `false` | Stateful-worker delta BSP. **coordinator only** — currently rejected on `local` (needs a coordinator runtime, SPEC-19 R20). Workers detect the mode via handshake; no worker flag. |

**Status: opt-in.** Default stays v1 full-partition (zero regression). Correctness rests on amendment D3d (incremental border-redex detection equals full `merge()+findBorderRedexes()`); a formal proof is still pending (SPEC-19 §8), backed today by an informal argument plus extensive tests. Worker and coordinator must share the same version. See [../specs/SPEC-19-delta-protocol.md](../specs/SPEC-19-delta-protocol.md).

## Zero-copy wire format (SPEC-18)

The v2 wire format replaces v1 bincode (fixed-int) with bincode v2 (varint), shrinks `PortRef` from 8 bytes to a 2-5 byte varint, and adds optional LZ4 (frames over the compression threshold). On top of that it offers a **zero-copy** path via rkyv for hot-path messages (`AssignPartition`, `PartitionResult`): the receiver reads fields directly off the received, LZ4-decompressed buffer with no deserialize pass and no allocation — just pointer arithmetic over archived offsets. A frame-header flag bit distinguishes bincode (0) from rkyv (1), so the sender chooses and the receiver adapts. The varint `PortRef` shrink applies to the plain bincode v2 path too, independent of zero-copy.

**Use it** when partitions are large (>1 MB) and deserialize shows up as a hotspot, the network is fast (10 GbE, loopback) so receiver CPU is the bottleneck, or you want clean Phase 3 LAN timings uncontaminated by deserialize variance. **Cost:** rkyv archives are slightly larger on the wire (explicit offsets/metadata), which LZ4 largely closes; touched types carry both serde and rkyv derives (schema changes must keep both consistent); rkyv needs 16-byte buffer alignment (handled by `AlignedVec` in `src/protocol/frame.rs`). There is no silent downgrade — if one peer lacks the feature, the handshake fails early.

| Flag | Default | Notes |
|------|---------|-------|
| `cargo build --features zero-copy` | off | Compile-time gate; required on **both** peers to read rkyv archives. |
| `--use-zero-copy` | `false` | Runtime request for the rkyv path on hot-path messages; effective only when built with `--features zero-copy`. |
| `--compression-threshold <BYTES>` | `1024` | LZ4 frame threshold; `0` compresses every frame. |

**Status: opt-in (build feature).** Not default; the benefit only materialises when deserialize cost dominates. Upgrading a live mesh requires a rolling restart with new binaries. See [../specs/SPEC-18-wire-format-v2.md](../specs/SPEC-18-wire-format-v2.md).

## Elastic grid (SPEC-20)

Makes the participating-node set dynamic during a run. Three independent capabilities: **hybrid** — the coordinator also runs an in-process worker (`WorkerId 0`, via `ChannelTransport`) instead of idling between merges, using `K_eff = K+1` slots; **dynamic join** — between `merge` and the next `partition` the coordinator drains pending connections within the join window, so new workers get partitions next round; **dynamic departure** — a worker can send `LeaveRequest` or simply go silent past a timeout, and the coordinator redispatches its retained partition (`retained_last_acked`, or `retained_initial` if nothing was acked) to survivors. With no workers connected past `--initial-wait-timeout`, a hybrid coordinator enters `SoloReducing`, running `reduce_n` batches of `--solo-budget` interactions while polling for joins.

**Use it** on a single machine (`--hybrid` reclaims the coordinator core), with an intermittent/heterogeneous pool (`--elastic-join` lets workers arrive mid-run), or for minimal resilience (`--elastic-departure`). **Skip it** for a fixed, pre-sized mesh — the flags add FSM complexity (`SoloReducing`, `Departing`, `Joining`) with no gain. Recoverability is justified by strong confluence (T4) + ARG-006: CLOSED for v1 mode, CONDITIONAL in delta mode (relies on `RecyclePolicy::DisableUnderDelta` as the conservative fallback — see Arena management).

| Flag | Default | Notes |
|------|---------|-------|
| `--hybrid` | off | Coordinator self-partition (`WorkerId 0`). Auto-enables `--elastic-join`. |
| `--elastic-join` | off | Drain pending joins between rounds. |
| `--elastic-departure` | off | Recover departing-worker partitions. Auto-enables `--retain-partitions`. |
| `--retain-partitions` | off | Keep `retained_initial` + `retained_last_acked`. |
| `--checkpoint-partitions` | off | Persist retained partitions — **placeholder** (flag parsed, disk persistence in backlog). |
| `--initial-wait-timeout <SECS>` | `30` | Wait before `SoloReducing` (hybrid only). |
| `--join-window-min-ms` / `--join-window-max-ms` | `50` / `500` | Join-drain window. |
| `--solo-budget <N>` | `10000` | Interactions per solo batch; `u32::MAX` disables polling. |

**Status: experimental over TCP.** Flags parse and propagate to `GridConfig`; in-process `local --hybrid` exercises 100% of the elastic transitions via `ChannelTransport` and is the recommended mode for experiments. Full `coordinator`/`worker` (real TCP) coverage — catastrophic mid-round departure, M5-scale recovery — exists in tests but not in locked benchmarks. `delta_mode`/`strict_bsp` are immutable per run (R0c). WAN split-brain is out of scope (deferred to SPEC-24). Default stays the v1 static mesh. See [../specs/SPEC-20-elastic-grid.md](../specs/SPEC-20-elastic-grid.md).

## Streaming generation (SPEC-21)

Replaces v1's eager `generate-whole-net -> partition -> dispatch` with an incremental `generate-chunk -> partition-chunk -> dispatch-chunk` pipeline, bounding coordinator peak memory to `O(chunk_size + border_state + pending)` instead of `O(total_agents)`. Forward references (e.g. `dual_tree` roots before children) are buffered as `PendingConnection`s and resolved when the target appears; a pending unresolved longer than `--max-pending-lifetime` batches aborts the run (leak guard). Partitioning strategy is `round-robin` (zero state, exact match with v1 `ContiguousIdStrategy`) or `fennel` (keeps an `AgentId->WorkerId` cache ~8 bytes/agent — 8× smaller than the materialised net — improving locality when generators emit neighbours in the same batch). Adding the streaming `Message` variants moved `PROTOCOL_VERSION` to 5.

**Use it** for very large nets (>10M agents) where `~64 B × total_agents` won't fit on the coordinator before first dispatch, or when the coordinator has less RAM than the workers. **Skip it** for small nets (<100k agents) — chunk-management overhead outweighs the saving; the `10000` default is already reasonable there. Trade-offs: round-robin streaming matches eager exactly; fennel streaming may lose ~5-10% locality (heuristic sees only data up to the current batch); coordinator CPU overhead stays under 5% of wall-clock at `chunk_size=10000`.

| Flag | Default | Notes |
|------|---------|-------|
| `--chunk-size <N>` | `10000` (on `coordinator`/`local`); eager on `bench` | Agents per `AgentBatch`. `4294967295` (`u32::MAX`) forces the eager v1-equivalent path. On `bench`, omit the flag for eager; pass it to stream. |
| `--streaming-strategy <round-robin\|fennel>` | `round-robin` | Partition heuristic. |
| `--fennel-alpha <F>` | `1.0` (fennel) | Fennel balance factor. |
| `--dispatch-mode <auto\|push\|pull>` | `auto` | Pull-dispatch control. |
| `--max-pending-lifetime <N>` | `16` | Max batches a forward-ref may stay unresolved. |

**Status: default on** (`coordinator`/`local` since v0.20-pre). Correctness depends on the free-list being protected under streaming (SPEC-22 R10b/c — see next section). The eager path stays valid for v1-baseline reproduction and bisects. Note: `generate -o file.bin` still materialises the full net to disk; streaming applies only to dispatch (disk -> workers). See [../specs/SPEC-21-streaming-generation.md](../specs/SPEC-21-streaming-generation.md).

## Arena management (SPEC-22)

Two independent mechanisms keep per-partition memory near the live-agent count. **Free-list:** each `Net` carries `free_list: Vec<AgentId>`; `remove_agent` pushes the freed id, `create_agent` pops it (LIFO, for temporal locality) before growing `next_id`. The key invariant (R7): an id on the free-list must not be referenced by any `PortRef::AgentPort`, or recycling breaks pointers. **SparseNet:** a `HashMap`-backed representation with no tombstones, used for construction/partitioning only — never in the reduction hot path (R23, CI-linted), since dense arenas win on cache. The dense-vs-sparse choice is automatic via `effective_arena_size = max_live_id + 1 > 4 × live_agent_count` (the D-011 fix that replaced the over-eager `id_range` metric and removed a +83% regression); evaluated once per `build_subnet`.

Recycling is the subtle case: under delta mode **or** streaming the coordinator holds long-lived `AgentId` references (BorderGraph / border_map), so recycling a still-referenced id breaks G1 (SPEC-21 §3.8 A6 generalised the R10b/c trigger to `delta_mode || streaming_active`). The default `disable-under-delta` policy is the safe choice and matters mainly to benchmarks/experiments — typical users never touch these flags. Free-list adoption bumped `PROTOCOL_VERSION` 2->3, so pre-SPEC-22 binaries can't read nets with a non-empty free-list (frozen v1 `.bin` baselines become unreadable to v3+ — regenerate with `relativist generate` if needed).

| Flag | Default | Notes |
|------|---------|-------|
| `--recycle-policy <P>` | `disable-under-delta` | `disable-under-delta` (no pop while delta/streaming active — safe), `border-clean` (pop but validate id not border-referenced — for measuring recycle gain). The CLI also documents `disable` in guide context; bench accepts the first two. |
| `--representation <R>` | `dense` | `dense` or `sparse` — **bench-harness only**; `coordinator`/`local` decide automatically via the threshold. |
| `cargo build --features streaming-no-recycle` | off | Compile-time: always falls back to `next_id` under streaming/delta, ignoring the runtime policy (SPEC-21 R37b closure). |

**Status: default on** with safe automatic decisions (`disable-under-delta` recycle, `dense` with auto-fallback to sparse above the threshold). The defaults cover 100% of the locked v2 benchmarks (preserve G1, avoid the D-011 regression, zero crashes across the 32/32 distributed slots of the canonical baseline). Change them only to deliberately measure an alternative, reproduce an old bug, or run a net so large dense always trips the threshold. See [../specs/SPEC-22-arena-management.md](../specs/SPEC-22-arena-management.md).

---

This is the last stop in the guide trail (getting-started -> first-reduction -> local-grid -> distributed-tcp -> church-arithmetic -> **v2-features**). For how these pieces fit together internally, read [../architecture/overview.md](../architecture/overview.md); for what's planned next (Phase 3 LAN, break-even, topology-aware Fennel, WAN), see [../roadmap.md](../roadmap.md).
