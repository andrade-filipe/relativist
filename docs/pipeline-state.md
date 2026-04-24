# Pipeline State

**Last updated:** 2026-04-23 (D-005 Option A Stages 0-3 SHIPPED; Stage 4 REVIEW queued on the 1/12 CON-DUP asymmetric failure. Three rounds of spec-critic closed Stage 0 SIGN-OFF. Stage 1 TASK-SPLITTER decomposed into TASK-0400..0403 (strict linear DAG). Stage 2 TEST-GENERATOR produced 26 mandatory UTs + optional PTs across TEST-SPEC-0400..0403. Stage 3 DEV shipped: 13 src files modified, 4 new TASK/TEST-SPEC markdowns, spec amendment landed. Test counts: **1168 / 1211** (+22 / +25 over D-004 baseline 1146/1186). Clippy + fmt clean both configs. Gate 11/12 green; 1 failure on `UT-0385-08 CON-DUP strict=false` — v1 produces the expected 4-agent commutation residue (2 Con + 2 Dup cross-wired to border FreePorts), v2 delta yields empty net via `run_grid_delta_final_collect`. Symmetric rules all green. Stage 4 REVIEW narrows to `run_grid_delta_final_collect` / `dispatch_final_state_request` / `merge::core::merge` / `cleanup_t1_violations` diagnostic.)
**Maintained by:** sdd-pipeline agent (do not edit manually)

---

## Active Bundle

**Bundle:** **D-005 Option A — Worker-side application of `CommutationBatch.local_wiring` for minted agents (production, wire-level)**
**Stage:** 4 — REVIEW (queued on the 1/12 CON-DUP asymmetric failure; diagnostic-first review)
**Opened:** 2026-04-23 (immediately post-D-004 close, commit `89492db`)
**Option elected:** A (production). Option B (test-only) explicitly rejected by user: would produce throw-away plumbing; Option A fixes the real wire+worker bug once, and keeps G1 asymmetric parity proof rooted in the same codepath a LAN worker would run — central for the TCC thesis claim.

**Stage 0 — SPEC-CRITIC (DONE 2026-04-23, 3 rounds):**
- **R1 (BLOCK, 12 findings):** `docs/spec-reviews/SPEC-REVIEW-19-section-3.4-D-005-2026-04-23.md`. SC-001..SC-012 spanning encoding ambiguity, slot-marker range collision, wire-to-memory mapping, rkyv-zero-copy interaction, error-path coverage, missing invariants. All 12 closed by specialista-em-specs Round 2 redraft (inline spec edits to §3.3 R23a, §3.4 R31-R36, §3.6 R48/R48a/R48b).
- **R2 (BLOCK, 5 new findings):** `docs/spec-reviews/SPEC-REVIEW-19-section-3.4-D-005-2026-04-23-REREVIEW.md`. NF-001 CRITICAL (`target_symbols` reconstruction gap), NF-002 HIGH (HandshakeAck bidirectionality), NF-003 MEDIUM (HashSet detection site for duplicate wiring), NF-004 MEDIUM (arity==0 handling — new `ZeroArity` case), NF-005 LOW (pipeline-state housekeeping). All 5 closed by Round 3 redraft — spec extended with R37 (`ProtocolError::MalformedLocalWiring` + 7-case enum) and R33c case taxonomy.
- **R3 (SIGN-OFF):** `docs/spec-reviews/SPEC-REVIEW-19-section-3.4-D-005-2026-04-23-REREVIEW-R3.md`. 0 CRITICAL / 0 HIGH new. NF-001 propagation audited across 10 call-sites (zero legacy `pc.symbol_type` / `pc.arity` refs remain). `Symbol` already has `#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]` at `relativist-core/src/net/types.rs:34-38` — build safe under `--features zero-copy`. **3 LOW NR3 findings non-blocking** (absorbable in Stage 3 DEV or next spec-touch):
  - NR3-001: prose edit `arity` → `pc.target_symbols.len()` in one R23a clause.
  - NR3-002: R37 wording sharpen — explicit mention of `ProtocolError::DeserializationFailed` vs `MalformedLocalWiring` dispatch boundary.
  - NR3-003: optional 8th enum case `MalformedLocalWiringReason::TargetSymbolsTooLong` (coordinator-side bound guard).

**Spec in effect:** `specs/SPEC-19-delta-protocol.md` §3.3 R23/R23a/R24, §3.4 R31-R37, §3.6 R48/R48a/R48b. `PendingCommutation` Shape A: `{ request_id, target_symbols: Vec<Symbol>, local_wiring: Vec<LocalWiringHint> }`. `PROTOCOL_VERSION` bump 2→3 mandated by R37.

**Acceptance gate (identical to D-003/D-004 full closure):** UT-0385-08 passes on all 6 fixtures × 2 strict modes with `SKIP_ASYMMETRIC = false`; `canonicalize(out_delta) == canonicalize(out_v1)` AND `metrics.total_interactions == metrics_v1.total_interactions` on every case. `cargo test --workspace --lib` ≥ **1146** default / ≥ **1186** `--features zero-copy`. Clippy + fmt clean both feature configs.
**Files expected to touch:** `relativist-core/src/protocol/types.rs` (Shape A refactor + new error variant + version bump) · `relativist-core/src/merge/border_resolver.rs` (resolver-to-wire transport) · `relativist-core/src/worker.rs` (mint-then-wire in `handle_round_start`) · `relativist-core/src/merge/grid_delta_integration_tests.rs` (LocalDeltaDispatch forwarding + `SKIP_ASYMMETRIC=false` flip) · (read-only reference) `relativist-core/src/net/types.rs` (Symbol rkyv derives L34-38) · `relativist-core/src/net/core.rs` (`Net::connect`).
**Branch:** `v2-development`
**Test baseline (start of D-005):** 1146 lib default / 1186 lib `--features zero-copy`.

---

## Prior Bundle (archived — reference for traceability)

**Bundle:** CLOSED — D-004 Coordinator-Side Round-N+2 Finalizer (plumbing-only scope) CLOSED as of 2026-04-23. `SKIP_ASYMMETRIC` flip gated on D-005.
**Stage:** DONE. Six SDD stages collapsed to 4 per reviewer endorsement (scope was pure-core plumbing + helpers + test-only T1 cleanup, no behavior change on symmetric rules):
  1. SPLITTING — TASK-0398 + TASK-0399 authored directly (Option B "cheap and equally formal" path per user directive).
  2. TESTS — TEST-SPEC-0398 + TEST-SPEC-0399 authored inline; UT-0398-01..08 cover encode/decode, enqueue, register, lenient duplicates, R48 stray, DC-B6 preserve-existing-border.
  3. DEV — TASK-0398 shipped inline (pure-core plumbing); TASK-0399 shipped inline (wire + revert SKIP_ASYMMETRIC).
  4. REVIEW — general-purpose reviewer agent (Opus-7), 2026-04-23 — ALIGNED, 0 Must-Fix, 2 Should-Fix (one docstring drift fixed inline; one release-mode overflow handling deferred to D-005 follow-up), several NITs, explicit endorsement of direct close without Stage 5/6.
  5. QA — SKIPPED per reviewer endorsement. UT-0398-01..08 cover the 4 critical Q-probes (R48 stray, duplicate-lenient, DC-B6 preserve, partial resolution).
  6. REFACTOR — SKIPPED (0 Must-Fix from REVIEW).
**Branch:** `v2-development`
**Test baseline (start of D-004):** 1138 lib default / 1178 lib `--features zero-copy`.
**Test counts at close:** **1146** lib default (+8: UT-0398-01..08) / **1186** lib `--features zero-copy` (+8, same set).
**Clippy:** clean both feature configs.
**fmt:** clean.
**D-003:** still PARTIALLY CLOSED — symmetric rules (CON-CON, DUP-DUP, ERA-ERA) G1 parity verified; asymmetric rules under `const SKIP_ASYMMETRIC: bool = true;` now pending D-005.
**D-004:** **PARTIALLY SHIPPED** (plumbing only) — `RoundResultPayload.minted_agents` extended, `BorderGraph::{enqueue_pending_borders, register_minted_agents}` implemented with R48 validation and DC-B6 preserve-existing-border path, `encode_request_id`/`decode_request_id` codec shared between resolver and LocalDeltaDispatch, `run_grid_delta_inner` wired, `package_resolutions_with_pending` exposes pending borders to coordinator. Step (5) `SKIP_ASYMMETRIC = false` flip blocked by D-005.
**D-005:** NEW — worker-side application of `CommutationBatch.local_wiring` for minted agents. Root cause: `PendingCommutation` wire message does not carry `local_wiring`, so workers mint agents but leave internal edges DISCONNECTED. Option B test-only workaround unblocks integration tests; Option A wire-level fix required for real LAN runs in delta mode. Blocks full D-003 AND full D-004 closure.
**Previous bundle:** SPEC-19 §3.3 Refactor (2026-04-23) — closed MF-001/MF-002/SF-001/SF-002; opened D-004.

## Next Action

**Stage 0 gate passed 2026-04-23.** Spec signed off on Round 3. 3 LOW NR3 findings (NR3-001/002/003) explicitly marked non-blocking — absorbable in Stage 3 DEV or in a future spec-touch (see Active Bundle block above for NR3 details).

**Next agent:** `task-splitter` (Stage 1, decompose spec into atomic tasks).

**Invocation scope:** Decompose SPEC-19 §3.3 R23/R23a/R24 + §3.4 R31-R37 + §3.6 R48/R48a/R48b + §9 Change Log into atomic tasks (<200 LoC each) forming a DAG. Target decomposition (splitter may adjust):

1. **TASK-0400** (~40-60 LoC): Wire struct rewrite. Refactor `protocol/types.rs::PendingCommutation` → Shape A; add `LocalWiringHint` struct; add `ProtocolError::MalformedLocalWiring { request_id, reason }` + 7-case `MalformedLocalWiringReason` enum; derive rkyv conditionally; bump `PROTOCOL_VERSION` 2→3; round-trip tests (bincode default + rkyv zero-copy). Absorbs NR3-001 (prose), NR3-002 (R37 error-path wording), NR3-003 (optional 8th `TargetSymbolsTooLong` case).
2. **TASK-0401** (~60-80 LoC): Resolver-to-wire transport. Extend `border_resolver.rs` `CommutationBatch`→`PendingCommutation` conversion (likely via `package_resolutions_with_pending`) to populate `target_symbols` + `local_wiring`. Pure transport, zero new resolver logic. UTs per rule (CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA).
3. **TASK-0402** (~80-100 LoC): Worker-side mint-then-wire. Implement R24.1.6a/b/c in `worker.rs::handle_round_start`: (a) allocate `target_symbols.len()` agents; (b) apply `local_wiring` via `Net::connect` post-mint with R23a clause 6 HashSet pre-pass, clause 3 slot-marker decoding, clause 4 concrete-id pass-through, R33c cases 1/2/3/5/6/7 rejection; (c) echo `MintedAgent { request_id, minted_agent_id: slot_0 }`. Protect R24 ordering invariant (no `reduce_all` drain until step 2). UT per R33c case.
4. **TASK-0403** (~30-50 LoC): LocalDeltaDispatch forwarding + canary. Update `grid_delta_integration_tests.rs::LocalDeltaDispatch` to propagate `target_symbols` + `local_wiring` from resolver into wire PC; flip `const SKIP_ASYMMETRIC: bool = false`. **Gate: UT-0385-08 green on 6 fixtures × 2 strict modes.**

**DAG:** `TASK-0400 → TASK-0401 → TASK-0402 → TASK-0403` (strict linear — each consumes the preceding). TASK-0403 is the bundle acceptance gate.

**Splitter inputs:** `specs/SPEC-19-delta-protocol.md` §3.3 + §3.4 + §3.6 + §9 · `docs/DEFERRED-WORK.md` D-005 row (lines 98-121) · all 3 spec-review artifacts (R1/R2/R3) for historical context and NR3 absorption notes · `relativist-core/src/merge/border_resolver.rs` · `relativist-core/src/protocol/types.rs` · `relativist-core/src/worker.rs` (`handle_round_start` mint loop) · `relativist-core/src/merge/grid_delta_integration_tests.rs` (`LocalDeltaDispatch`, `SKIP_ASYMMETRIC`) · `relativist-core/src/net/core.rs` (`Net::connect`) · `relativist-core/src/net/types.rs` (Symbol rkyv L34-38).

**Output:** `docs/backlog/TASK-0400.md` .. `TASK-0403.md` + `docs/backlog/BACKLOG.md` entries. Each TASK file: acceptance criteria, files to touch, DAG links, LoC estimate, NR3 absorption notes.

**Queued after D-005 closes:**
1. M1 exit measurement (Passo 6): `ep_con 5M w=2` baseline v1 vs current `run_grid`; `c_o/c_r` drop to CSV.
2. Phase 3 LAN preparation (orthogonal — uses v1 `run_grid`).

## Stage 5 QA Summary (SPEC-18 §3.5 / item 2.24)

- **Probes implemented:** 10 (Q1, Q2-on, Q2-off, Q3-on, Q3-off, Q4, Q5, Q6, Q7, Q8) per
  `docs/reviews/REVIEW-SPEC-18-section-3.5-2026-04-16.md` §7.
- **Location:**
  - Feature-gated (8 probes): `relativist-core/src/protocol/zero_copy_tests.rs` under
    `#[cfg(all(test, feature = "zero-copy"))]`.
  - Default-build (2 probes: Q2-off, Q3-off): `relativist-core/src/protocol/frame.rs`
    inside the existing `#[cfg(test)]` module under `#[cfg(not(feature = "zero-copy"))]`.
- **Q1 tightening:** Initial "mutate trailing 4 bytes" strategy landed on POD u32 fields
  rather than relative pointers (rkyv 0.8 places the root struct at the buffer end, so
  the tail bytes are fields, not the root RelPtr). Switched to a deterministic
  front-half truncation (`payload.drain(0..len/2)`) which moves the buffer base so every
  internal RelPtr now references pre-buffer memory — the rkyv validator rejects
  reliably (`ArchiveValidationFailed`).
- **Bug count:** 0. All 10 probes PASS.
- **Gate status:** `cargo test --workspace` GREEN (905+4), `cargo test --workspace
  --features zero-copy` GREEN (945+4), `cargo clippy --workspace --all-targets -- -D
  warnings` GREEN (both feature configs), `cargo fmt --check` GREEN.
- **Q6 status:** Intentional no-op traceability test (ARM alignment witness deferred —
  no ARM CI runner). Q7 documents rkyv 0.8.x cross-patch fixture coverage as deferred.
- **Stage 6 REFACTOR:** No-op (0 bugs). Ready to ship.

---

## Current Work

**Current spec:** SPEC-26 §3.1 (Cargo Workspace Restructure — Layer 0) — **COMPLETE**
**Current stage:** DONE (mechanical refactor, no functional changes)
**v2 branch:** v2-development
**v1 tests baseline:** 690 passing
**Final test count:** 716 (712 unit + 4 integration) — unchanged
**Clippy status:** clean (0 warnings, including --all-targets)
**Formatting:** clean (cargo fmt --check passes)

## Stage History (SPEC-26 §3.1)

- [x] SPLITTING: 2026-04-16 — 5 atomic tasks (workspace skeleton, move src/, thin CLI, verify, docs)
- [x] DEV: 2026-04-16 — workspace created (relativist-core + relativist-cli), src/tests/benches moved via git mv
- [x] VERIFY: 2026-04-16 — 716 tests pass, clippy --workspace --all-targets clean, fmt clean, release binary at target/release/relativist.exe (scripts + Dockerfile path preserved)
- [x] REVIEW: 2026-04-16 — refactor-only change, all 7 requirements (R1-R7) satisfied
- [x] QA: 2026-04-16 — smoke test `compute add 3 5 = 8` passes
- [x] REFACTOR: 2026-04-16 — fixed pre-existing clippy warning in protocol/config.rs (bool_assert_comparison)

## Layer 0 Verification (SPEC-26 R1-R7)

| Req | Status | Evidence |
|-----|--------|----------|
| R1 | ✅ | Layout: relativist/{Cargo.toml, relativist-core/, relativist-cli/} |
| R2 | ✅ | Workspace manifest with members + resolver = "2" |
| R3 | ✅ | All src/ files moved verbatim to relativist-core/src/ via git mv |
| R4 | ✅ | All deps + features (tls, metrics, otel, full) preserved in relativist-core/Cargo.toml |
| R5 | ✅ | relativist-cli/src/main.rs delegates to relativist_core::commands; binary name "relativist" preserved |
| R6 | ✅ | 716 tests pass in relativist-core; [[bench]] section in relativist-core/Cargo.toml |
| R7 | ✅ | `cargo test --workspace` runs all tests across both crates |

## Backward Compatibility

- Binary path unchanged: `target/release/relativist.exe`
- All 6 bench scripts (`scripts/bench_*.sh`) work without modification
- Dockerfile `COPY` line works unchanged
- CLI flags and subcommands identical to pre-restructure

## SPEC-27 Progress

**Phase 1: Traits — COMPLETE** (2026-04-16)
- `relativist_core::encoding::traits` module created (~150 LoC inc. tests)
- `Encoder`, `Decoder`, `Codec` traits + `EncodeError`, `DecodeError` types (R1-R4)
- `validate_encoded_net()` checks E1 (T1 linearity subset) + E2 (at least one redex) (R5, R6)
- 6 inline tests; test count 716 → 722 (+6)

**Phase 2: Church refactoring — COMPLETE** (2026-04-16, commit `a601c5e`)
- `ChurchArithmeticCodec` (4 ops: add/mul/exp/sum_of_squares) implements `Codec`
- Wraps existing `build_*` + `decode_nat_or_shared`; zero changes to public API (R7)
- 8 inline tests (round-trips, schema validation, object safety); test count 722 → 726 (+4)

**Phase 3: LambdaCodec — COMPLETE** (2026-04-16, all 6 stages)
- TASK-0333 (encoder), TASK-0334 (decoder), TASK-0335 (edge cases) created in `docs/backlog/`.
- TEST-SPEC-0333/0334/0335 specified in `docs/tests/`.
- `relativist-core/src/encoding/codec_lambda.rs` implements parser + pretty-printer +
  Mackie/Pinto encoder + port-directed readback decoder; ~900 LoC inc. 32 inline tests
  (26 spec'd + 6 QA adversarial).
- Engine fix: `interact_comm` self-loop guard added (mirrors `interact_anni` /
  `interact_eras`); required for sound reduction of identity-typed arguments
  (e.g., `(λx. x x) (λy. y)`). No regression on the 690 v1 tests.
- Final test count: 726 → 758 (+32). Clippy + fmt clean.
- REVIEW notes in `docs/reviews/REVIEW-SPEC-27-Phase3-LambdaCodec.md`.
- Stage 6 REFACTOR: no Must-Fix items from REVIEW. Should-Fix items
  (free-var name preservation, DUP-cycle readback algorithm) deferred as
  out of T5-T9 scope; will be re-evaluated when consumed by Phase 5 CLI.
- Driven by Option B: SDD pipeline managed manually from TCC root
  (relativist-local agents not invocable from TCC cwd).

**Phase 4: EncoderRegistry — COMPLETE** (2026-04-16, all 6 stages)
- TASK-0336 (registry struct + ops, R17/R18/R20), TASK-0337 (default_registry, R19) created.
- TEST-SPEC-0336 (8 cases R1-R8) and TEST-SPEC-0337 (5 cases D1-D5) specified.
- `relativist-core/src/encoding/registry.rs` (~310 LoC inc. 19 inline tests):
  - `EncoderRegistry` (HashMap<String, Box<dyn Codec>>) with 6 ops: new, register,
    get, list (sorted), encode_and_validate (calls validate_encoded_net), decode.
  - `RegistryError` enum (DuplicateName, NotFound, Encode #[from], Decode #[from]).
  - `default_registry()` registering 4 ChurchArithmeticCodec ops + LambdaCodec.
  - 13 spec-driven tests + 6 QA adversarial tests (case-sensitive lookup,
    failed-register doesn't poison, empty input, NotFound preserves name,
    Default impl, no-redex net rejected by E2).
- Re-exported from `encoding/mod.rs` as `EncoderRegistry`, `RegistryError`,
  `default_registry`.
- Note: round-trip test for `church_exp` reduced to encode-only because decode
  hits the SPEC-14 DUP-cycle readback limitation (pre-existing, not caused by
  this work). add/mul/sum_of_squares fully round-trip.
- Final test count: 758 → 777 (+19). Clippy + fmt clean.
- REVIEW notes in `docs/reviews/REVIEW-SPEC-27-Phase4-EncoderRegistry.md` (APPROVE).
- Stage 5 QA: 6 adversarial tests added, all passed first try — no Must-Fix items.
- Stage 6 REFACTOR: no code changes needed (zero bugs found in QA).

**Phase 5: CLI integration — COMPLETE** (2026-04-16, all 6 stages)
- TASK-0338 (compute --encoder dispatch, R21/R23), TASK-0339 (encoders list, R22) created.
- TEST-SPEC-0338 (5 cases C1-C5) and TEST-SPEC-0339 (3 cases E1-E3) specified.
- `relativist-core/src/config.rs`: ComputeArgs extended (operation/a/b → Option,
  added `--encoder` and `--input` with clap `requires`); `Encoders(EncodersArgs)`
  variant with `EncodersAction::List` subcommand added.
- `relativist-core/src/commands.rs`:
  - `run_compute_command` early-returns through `run_compute_with_encoder` when
    `--encoder` is set; legacy SPEC-14 path validates positional args at runtime
    and is otherwise unchanged.
  - `run_compute_with_encoder` runs the full SPEC-27 R23 pipeline:
    `encode_and_validate → reduce_all → decode → serde_json pretty-print`.
  - `run_encoders_command` enumerates `default_registry().list()` aligned with
    the longest name (deterministic, sorted output).
- `relativist-core/src/error.rs`: added `RelativistError::Encoding(String)` and
  `From<RegistryError>` impl so `?` propagates registry errors with exit code 3.
- `relativist-cli/src/main.rs`: `Command::Encoders(args)` dispatch wired.
- Smoke (release binary): `compute add 3 5 → Result: 8`;
  `compute --encoder church_add --input '{"a":3,"b":5}' → {"result":8}`;
  `compute --encoder lambda --input '{"term":"(λx. x) (λy. y)"}' → λv0. v0`;
  unknown encoder → exit 3 with clear message; `encoders list` → 5 codecs.
- Final test count: 777 → 789 (+12: 4 spec-driven C/E + 4 QA arg-parsing + 4
  dispatch unit tests). Clippy + fmt clean.
- REVIEW notes in `docs/reviews/REVIEW-SPEC-27-Phase5-CLI.md` (APPROVE).
- Stage 5 QA: 8 adversarial tests added (parsing edge cases + dispatch error
  paths), all passed first try — no Must-Fix items.
- Stage 6 REFACTOR: no code changes needed.

**Phase 6: RecipeEncoder mínimo (R24+R25 only) — COMPLETE** (2026-04-16, all 6 stages)
- TASK-0340 created with explicit "minimal scope" note: ships only R24 (trait
  definition) and R25 (non-coupling with the `Codec` registry). R26/R27/R28
  deferred until SPEC-25 (item 2.29, milestone M7) is implemented.
- TEST-SPEC-0340 (R1-R8) specified.
- `relativist-core/src/encoding/recipe.rs` (~250 LoC inc. 12 inline tests):
  - `RecipeEncoder: Encoder` trait with `type Recipe: Serialize + DeserializeOwned + Send + Sync`
    and 3 methods (`is_decomposable`, `make_recipes`, `generate_partition`).
  - `MinimalRecipe` struct + `MinimalRecipeEncoder` demo implementation (Con/Era pairs).
  - Demo encoder is **not** registered in `default_registry()` — it exists only to
    exercise the trait in tests.
- 8 spec-driven tests + 4 QA adversarial tests (more-workers-than-size, zero-pair
  recipe, Send+Sync compile-check, centralized-encode validation).
- Final test count: 789 → **801** (+12). Clippy + fmt clean.
- REVIEW notes in `docs/reviews/REVIEW-SPEC-27-Phase6-RecipeEncoder.md` (APPROVE).
- Stage 5 QA: 4 adversarial tests added, all passed first try — no Must-Fix items.
- Stage 6 REFACTOR: no code changes needed.
- **Deferral tracker:** see `docs/DEFERRED-WORK.md` row D-001 for the unblock
  checklist that must be processed when SPEC-25 ships in milestone M7.

**SPEC-27 status:** Phases 1-5 fully complete. Phase 6 partially complete
(R24+R25 shipped; R26-R28 deferred to M7). M10 milestone in V2-FEATURE-MATRIX
remains open until D-001 is resolved.

**Next focus (per V2-FEATURE-MATRIX Tier 1 break-even path):**
- ~~Item 2.22 — TCP Transport Tuning~~ DONE (commit c360fe5, 2026-04-15)
- ~~Item 2.25 — Unix Domain Socket Fast Path~~ DONE (commit c360fe5, 2026-04-15)
- **Item 2.23 — Wire Format v2 (M1, ~450 LoC, 2-3d) — IN PROGRESS**
- Item 2.34 — Coordinator-Free Round (next after 2.23)
- Item 2.24 — rkyv zero-copy (deferred per DEFERRED-WORK D-002, sequenced after 2.34)
- Decision point after M1: measure tcp_localhost/seq ratio and decide on M3-M4.

---

## SPEC-18 §3.1-3.4 + §3.6 (Wire Format v2, item 2.23) — IN PROGRESS

**Started:** 2026-04-16
**Test baseline before this work:** 801

### Stage History

- [x] **SPLITTING** (2026-04-16): 5 atomic tasks created in `docs/backlog/`
  following SPEC-18 §6.3 mandated migration order.
  - TASK-0343 — bincode v2 migration (M, ~150 LoC, R1-R4)
  - TASK-0344 — Compact PortRef encoding (S, ~150 LoC, R5-R8) depends on 0343
  - TASK-0345 — Frame header v2 (S, ~150 LoC, R14-R19) depends on 0343
  - TASK-0346 — LZ4 compression pipeline (M, ~180 LoC, R9-R13, R36-R39) depends on 0345
  - TASK-0347 — PROTOCOL_VERSION bump 1→2 (S, ~80 LoC, R28-R32) depends on 0343-0346
- [x] **TESTS** (2026-04-16): 5 TEST-SPEC files created in `docs/tests/`
  specifying acceptance criteria for each task before implementation.
  - TEST-SPEC-0343, 0344, 0345, 0346, 0347 — all enumerate R-numbered
    requirements with concrete `#[test]` blueprints.
  - Cumulative test target: 801 → 819+ across the 5 tasks.
- [~] **DEV** — TDD RED → GREEN → REFACTOR per task, in dependency order.
  - [x] **TASK-0343 (bincode v2 migration) — GREEN** (2026-04-16)
    - Cargo.toml: `bincode = "1"` → `bincode = { version = "2", features = ["serde"] }`
    - Created `src/protocol/bincode_v2.rs` thin wrapper module
      (`encode`, `decode`, `decode_value` over `bincode::config::standard()`).
    - Migrated 13 call sites across `protocol/{frame,types,error}.rs`,
      `net/{core,types}.rs`, `partition/{types,compact}.rs`,
      `merge/types.rs`, `io/binary.rs`.
    - `ProtocolError::Serialize`/`Deserialize` switched from `bincode::Error`
      to `bincode::error::{EncodeError, DecodeError}`.
    - 801 lib + 4 integration tests pass (no regression). Clippy clean,
      fmt clean. Release smoke `compute add 3 5 → 8` works.
  - [x] **TASK-0344 (Compact PortRef encoding) — GREEN** (2026-04-16)
    - Removed `serde::{Serialize,Deserialize}` derive from `PortRef`;
      added manual impls using `serialize_tuple(N)` / `deserialize_tuple(3, …)`
      to bypass bincode v2's enum discriminant overhead.
    - Tag bytes (public): `PORTREF_TAG_DISCONNECTED = 0xFF`,
      `PORTREF_TAG_AGENT_PORT = 0x00`, `PORTREF_TAG_FREE_PORT = 0x01`.
      Wire shapes: DISCONNECTED = 1 B; FreePort(b) = 1 + varint(b);
      AgentPort(id, pid) = 1 + varint(id) + varint(pid).
    - 7 spec-driven tests added in `net::types::tests` (R1-R7):
      round-trip on T2 set; DISCONNECTED = 1 byte; AgentPort(0,0) = 3 bytes;
      small AgentPort ≤ 4 bytes; FreePort(0) = 2 bytes; unknown tag rejected;
      truncated payload rejected.
    - 801 → **808** lib tests + 4 integration tests (no regression).
      Clippy clean, fmt clean. Release smoke `compute add 3 5 → 8` works.
  - [x] **TASK-0345 (Frame header v2) — GREEN** (2026-04-16)
    - `FRAME_HEADER_SIZE` 8 → **9** bytes (added `flags: u8`).
    - New public consts: `FLAG_COMPRESSED = 0b01`, `FLAG_ARCHIVED = 0b10`,
      `FLAG_RESERVED = 0b1111_1100` (mutually exclusive partition of 0xFF).
    - `FrameHeader` gains helpers `is_compressed()`, `is_archived()`,
      `has_unknown_flags()`. `to_bytes()` / `from_bytes()` round-trip the flags.
    - `send_frame` always writes `flags = 0` (compression bit lights up in
      TASK-0346, archive bit deferred to ROADMAP item 2.24).
    - `recv_frame` rejects any frame with `flags & FLAG_RESERVED != 0`
      via new `ProtocolError::UnknownFlags { flags }` BEFORE allocating the
      payload buffer (forward-compat hardening per SPEC-18 R19).
    - 7 spec-driven tests added (R1-R7): size const, flag-const partition,
      flags=0 round-trip, helper truth tables, reserved-bit rejection,
      `UnknownFlags` Display, header-byte regression check.
    - Existing tests updated (not deleted): `test_frame_header_byte_order`
      now asserts the 9-byte LE layout including flags; `test_framing_constants`
      asserts `FRAME_HEADER_SIZE == 9`; all `FrameHeader { … }` literals
      gained `flags: 0`.
    - 808 → **815** lib tests + 4 integration tests (no regression).
      Clippy clean, fmt clean. Release smoke `compute add 3 5 → 8` works.
  - [x] **TASK-0346 (LZ4 compression pipeline) — GREEN** (2026-04-16)
    - Added `lz4_flex = "0.11"` (default-features off, safe-encode +
      safe-decode features) — pure-Rust, no `unsafe`.
    - New module `protocol/compression.rs` (~115 LoC inc. 4 tests):
      `compress_payload(&[u8]) -> Vec<u8>` writes a `[u32 LE size]
      [LZ4 block]` envelope; `decompress_payload(&[u8])` rejects
      truncated buffers and declared sizes >1 GiB before allocating.
    - `protocol/error.rs`: new `DecompressionFailed(String)` variant
      (Display: `"LZ4 decompression failed: <reason>"`).
    - `protocol/config.rs`: `TransportConfig.compression_threshold:
      usize` (default 1024) — `usize::MAX` disables, `0` always
      compresses.
    - `protocol/frame.rs`:
      - New `send_frame_with_threshold(writer, msg, threshold)`. The
        bare `send_frame` now delegates with
        `DEFAULT_COMPRESSION_THRESHOLD = 1024`.
      - `recv_frame` decompresses BEFORE the CRC check so the checksum
        verifies the uncompressed message bytes (R12 invariant).
    - `config.rs`: `--compression-threshold <BYTES>` CLI flag on both
      `coordinator` and `worker` subcommands; threaded through
      `build_transport_config` into `TransportConfig`.
    - 9 spec-driven tests added (R1, R2, R3, R4, R5, R6, R7, R8 ×2, R9):
      LZ4 round-trip, ratio ≥ 2× on redundant data, above/below
      threshold, CRC-on-uncompressed invariant, corrupted payload
      yields `DecompressionFailed | ChecksumMismatch`, default-threshold
      behaviour for both `send_frame` and `TransportConfig::default`,
      and CLI-flag round-trip on coordinator + worker.
    - Existing test count 815 → **830** (+15: 9 spec + 4 new in
      `compression.rs` + 1 error Display + 1 default const).
      Clippy clean, fmt clean. Release smoke `compute add 3 5 → 8` works.
    - **R10 deferral:** `compression_ratio_per_round` field on
      `WorkerRoundStats` (R39 SHOULD) deliberately not added in this
      task — would require coordinator-side aggregation work that the
      atomic wire-break does not need. Recorded as DEFERRED-WORK D-003
      (track for SPEC-11 metrics evolution).
  - [x] **TASK-0347 (PROTOCOL_VERSION bump 1 → 2) — GREEN** (2026-04-16)
    - `protocol/coordinator.rs`: `PROTOCOL_VERSION` bumped 1 → 2 (R28).
      Nack reason on version mismatch now uses the canonical phrase
      `"protocol version mismatch: expected N, received M"` so workers
      can parse it deterministically (R29).
    - `protocol/error.rs`: new `VersionMismatch { expected: u8,
      received: u8 }` variant (R30); Display renders
      `"protocol version mismatch: expected N, received M"` (R4).
    - `protocol/worker.rs`: new private `parse_version_mismatch_nack()`
      helper extracts the received-version `u8` from a nack reason; the
      `RegisterNack` arm in `run_worker_inner` now returns
      `VersionMismatch` for version-shaped nacks and `AuthFailed` for
      everything else (R30, preserves SPEC-10 contract).
    - 7 spec-driven tests added (R1, R2, R3, R3-negative, R3-parser,
      R4, R5×2, R7):
      - `coordinator::tests::protocol_version_is_two` (R1 canary)
      - `coordinator::tests::coordinator_rejects_v1_worker_with_register_nack` (R2)
      - `coordinator::tests::smoke_v2_coordinator_v2_worker_handshake_succeeds` (R7)
      - `worker::tests::worker_terminates_on_version_mismatch_nack` (R3)
      - `worker::tests::worker_returns_auth_failed_for_non_version_nack` (R3 negative)
      - `worker::tests::parse_version_mismatch_nack_recognises_canonical_phrase` (R3 unit)
      - `error::tests::version_mismatch_error_renders` (R4)
      - `frame::tests::v2_pipeline_round_trip_all_message_variants_uncompressed` (R5 — all 7 variants through bincode v2 + 9-byte header + CRC, threshold = MAX)
      - `frame::tests::v2_pipeline_round_trip_all_message_variants_compressed` (R5 — all 7 variants through bincode v2 + LZ4 + 9-byte header + CRC, threshold = 0)
      - Compile-time exhaustive `match` in `sample_all_message_variants` so adding a new `Message` variant breaks the test.
    - Test count 830 → **839** lib tests + 4 integration tests
      (no regression). Clippy clean, fmt clean. Release smoke
      `compute add 3 5 → 8` works.
    - **R6 deferral:** raw-TCP integration test with hand-crafted v1
      bytes (`legacy_v1_register_bytes`) deliberately omitted — the v1
      wire format used bincode v1 + 8-byte header, neither of which
      this build can emit, and the spec explicitly permits gating it
      under `#[ignore]` and relying on R2's ChannelTransport coverage
      for the contract. Recorded as in-spec deferral, no follow-up needed.
- [x] **REVIEW** (2026-04-16) — full-bundle pass over TASK-0343..0347 in
      `docs/reviews/REVIEW-SPEC-18-WireFormat-v2.md`. Verdict: **APPROVE
      WITH MUST-FIX** (2 mechanical edits queued for REFACTOR).
- [x] **QA** (2026-04-16) — 9 adversarial probes from the REVIEW added as
      `qa_probe_*` tests:
      - `frame::tests::qa_probe_1_truncated_portref_body_yields_deserialize_error`
      - `frame::tests::qa_probe_2_compressed_empty_frame_yields_decompression_failed`
      - `frame::tests::qa_probe_3_both_flags_set_archive_bit_currently_ignored`
      - `frame::tests::qa_probe_4_compression_flag_with_corrupted_crc_yields_checksum_mismatch`
      - `coordinator::tests::qa_probe_5_v0_register_rejected_with_canonical_nack`
      - `worker::tests::qa_probe_6_stray_nack_mid_stream_surfaces_unexpected_message`
      - `frame::tests::qa_probe_7_threshold_zero_compresses_minimal_message`
        + `config::tests::qa_probe_7_cli_threshold_zero_threads_through_coordinator_and_worker`
      - `frame::tests::qa_probe_8_threshold_usize_max_skips_compression_on_large_message`
        + `config::tests::qa_probe_8_cli_threshold_usize_max_threads_through_coordinator_and_worker`
      - `coordinator::tests::qa_probe_9_v1_then_v2_workers_v1_nacked_v2_acked`
      Test count 839 → **850** lib + 4 integration. Clippy clean
      (one `needless_range_loop` fixed during the pass), fmt clean,
      release smoke `compute add 3 5 → 8` works. **No new bugs surfaced** —
      every failure mode probed (truncation, decompression bombs, stray
      flag bits, CRC tampering, version edge cases, mid-stream message
      ordering, CLI extremes, concurrent v1/v2 registration) is handled
      gracefully via typed `ProtocolError` variants.
- [x] **REFACTOR** (2026-04-16) — Both REVIEW Must-Fix items applied:
      - **#1 (R29 wire literal):** `coordinator::accept_workers` nack
        format changed from `"… received {M}"` to `"… got {M}"` (matches
        SPEC-18 R29 verbatim). `worker::parse_version_mismatch_nack` now
        keys on `"got "` instead of `"received "`. The
        `ProtocolError::VersionMismatch { received: u8 }` field name and
        the Display impl `"… received N"` are intentionally kept (R35,
        TEST-SPEC-0347 R4 — wire string and Rust field name use different
        terms by design). Test fixtures and assertions in
        `coordinator_rejects_v1_worker_with_register_nack`,
        `qa_probe_5_v0_register_rejected_with_canonical_nack`,
        `worker_terminates_on_version_mismatch_nack`,
        `parse_version_mismatch_nack_recognises_canonical_phrase`, and
        the R5 `sample_all_message_variants` fixture were updated
        together to keep the suite internally consistent.
      - **#2 (`recv_frame` doc-comment):** rewritten to the v2 7-step
        sequence (header → unknown-flag check → length check → payload
        read → decompress → CRC → bincode v2 decode), explicitly noting
        the load-bearing R12 ordering invariant.
      Final gate: 850 lib + 4 integration tests pass, clippy clean, fmt
      clean, release build clean, smoke `compute add 3 5 → 8` works.
      **SPEC-18 §3.1-3.4 + §3.6 (item 2.23) ships green.**

### Deferred (already tracked)

- §3.5 (rkyv zero-copy archive, R20-R27) — DEFERRED-WORK row D-002,
  sequenced after item 2.34 in M1.

---

## SPEC-19 §3.1 (item 2.34) — Coordinator-Free Round (Merge Avoidance) — SHIPPED 2026-04-16

**Started:** 2026-04-16
**Test baseline before this work:** 850 lib + 4 integration (post-SPEC-18 ship)
**Bundle scope:** SPEC-19 §3.1 only — requirements R1, R2, R3, R4, R5, R6, R7.
  Explicitly **OUT of scope** for this bundle: §3.2 BorderGraph (item 2.35),
  §3.3 full Delta-Only Protocol (item 2.26), §3.4-§3.7 (delta wire variants,
  invariant amendments, delta config, delta metrics).
**Estimated size:** ~300 LoC across atomic tasks (<200 LoC each).
**Tier 1 break-even path:** confirmed next after item 2.23 in
  `docs/V2-FEATURE-MATRIX.md` (Implementation order:
  2.22 → 2.23 → **2.34** → 2.24 → 2.25 → 2.35 → 2.26).
**Dependencies:** none beyond what already shipped (strict BSP from v1,
  v1 wire protocol, v2 wire format from SPEC-18).

### Bundle Goal (single sentence)

Detect when **all** workers report `has_border_activity: false` after a round
of local reduction and skip the merge-redistribute cycle for that round; if
they additionally report `local_redexes == 0`, declare Global Normal Form
(R4) and terminate. Stay compatible with both v1 protocol and v2 wire
format (R7) — the `has_border_activity` field is a **new addition to the
worker round result that crosses the wire**, so a bincode v2
message-format change is in scope for this bundle and MUST NOT break any
existing serde round-trip tests (TASK-0343..0347 baseline).

### Stage History

- [x] **SPLITTING** (2026-04-16): complete — 4 atomic tasks delivered to
      `docs/backlog/`, all <200 LoC, ~300 LoC total, every task documents
      v1+v2 protocol compatibility per R7. Bundle index file:
      `docs/backlog/SPEC-19-section-3.1-coordinator-free-round-tasks.md`.
  - TASK-0348 — Add `has_border_activity` field + `compute_border_activity` helper (S, ~80 LoC, R1+R2)
  - TASK-0349 — Populate `has_border_activity` at every WorkerRoundStats build site (S, ~50 LoC, R2) depends on 0348
  - TASK-0350 — Add `coordinator_free_rounds` config flag + metrics counter (S, ~40 LoC, R6+R41 partial+R45 partial)
  - TASK-0351 — Coordinator skip-merge logic + Global Normal Form termination (M, ~130 LoC, R3+R4+R5+R6+R7) depends on 0348+0349+0350
  - DAG: 0348 → 0349 ‖ 0350 → 0351. Cumulative test target 850 → ~865+.
- [x] **TESTS** (2026-04-16): complete — 4 TEST-SPEC files delivered to
      `docs/tests/`, one per task, each enumerating concrete `#[test]`
      blueprints with target file paths, exact inputs/expected outputs,
      and explicit SPEC-19 R-number coverage.
  - `docs/tests/TEST-SPEC-0348.md` — 6 new tests (UT-01..05 cover
    `compute_border_activity` against R1; UT-06 covers
    `WorkerRoundStats` bincode v2 round-trip carrier for R2).
    Adversarial probes A-D referenced as QA candidates (large-N scan,
    `AgentId(u32::MAX)`, full v2 wire pipeline carrier, legacy bytes).
  - `docs/tests/TEST-SPEC-0349.md` — 4 new tests (UT-01/02 positive +
    negative round-build for R2; UT-03 ordering check —
    `compute_border_activity` after `rebuild_free_port_index`;
    IT-04 `run_grid` per-worker activity propagation end-to-end).
    Adversarial probes A-E referenced as QA candidates (oscillation,
    race window flagged non-deterministic, full v2 wire ride, empty
    workers list, single worker).
  - `docs/tests/TEST-SPEC-0350.md` — 2 new tests (UT-01 default flag
    `false` per R43 SHOULD; UT-02 metric counter starts at 0 per R45p).
    Cross-task probes A-C reference TEST-SPEC-0351 for the gating /
    increment behaviour.
  - `docs/tests/TEST-SPEC-0351.md` — 8+ new tests: UT-01..04 exhaustive
    2×2 truth table for `check_global_normal_form` (R3+R4); UT-05
    skip-merge under strict BSP (R3+R6); UT-06 GNF early termination
    (R4); UT-07 default config bit-identical v1 behaviour (R7); UT-08
    lenient mode does not skip (R6 SHOULD interpretation); UT-09
    structural assertion that `protocol/coordinator.rs` FSM is untouched
    by the bundle (R7 wire compat); UT-10 / UT-11 result-equivalence
    on skip-triggered AND skip-NOT-triggered workloads (R5 confluence);
    UT-12 G1 spot check on `church_add(2, 3)` at w=2 strict BSP.
    Adversarial probes A-G referenced as QA candidates (empty / single
    worker, oscillation, lenient + flag combo, `max_rounds = 0`,
    `ep_annihilation_con(100)` per spec T11, async race flagged as
    out-of-bundle for item 2.26).
  - **Cumulative bundle test target:** 850 → **870** lib tests (+20),
    with conservative floor **866+** to absorb implementation
    negotiation. Inside the orchestrator's `+12 to +18` hint.
  - **Hard-to-write-deterministically tests flagged:** UT-0351-09 is
    a source-grep structural assertion (acceptable but flagged for
    future migration to a parsed-AST check); QA-0349-B / QA-0351-G
    (race) are **non-deterministic** and explicitly NOT implemented as
    `#[test]` — left as documented adversarial probes for QA.
  - **No spec ambiguities surfaced** during TEST-SPEC drafting; the R3
    MAY → opt-in flag, R6 SHOULD → strict-BSP gating, and R7 v1 compat
    → wire-FSM-untouched mappings all hold. No spec-critic dispatch
    needed.
- [x] **DEV**: COMPLETE (2026-04-17) — 4 tasks GREEN in sequence on `v2-development`.
  - **TASK-0348 (S, ~80 LoC):** GREEN — added `WorkerRoundStats.has_border_activity: bool` field + `merge::helpers::compute_border_activity(&Partition) -> bool` (O(|free_port_index|), reads `AgentPort(_, 0)`). Test count 850 → 856 (+6: UT-0348-01..05 in `helpers.rs`, UT-06 serde-true round-trip in `types.rs`). All 9 build sites updated to `false` placeholder. Files: `merge/types.rs`, `merge/helpers.rs`, `worker.rs` (FSM stub), `protocol/{worker,types,frame,coordinator}.rs`, `merge/grid.rs`.
  - **TASK-0349 (S, ~50 LoC):** GREEN — wired `compute_border_activity(&partition)` at production sites in `protocol/worker.rs` (after `rebuild_free_port_index`) and `merge/grid.rs` (per-worker reduction loop). R1 ordering preserved (rebuild → compute). Test count 856 → 861 (+5: UT-0349-01..03 in `helpers.rs`, IT-0349-04/05 in `grid.rs`). Files: `merge/{grid,helpers}.rs`, `protocol/worker.rs`.
  - **TASK-0350 (S, ~40 LoC):** GREEN — added `GridConfig.coordinator_free_rounds: bool` (default `false`) + `GridMetrics.coordinator_free_rounds: u32` (Default-derived `0`). Test count 861 → 863 (+2 effective: UT-01 default-disables, UT-02 metric-default-zero is included in existing `test_grid_metrics_default`, UT-03 settable). Fixed 4 explicit `GridConfig` constructors in `config.rs`, `bench/suite.rs`, `bench/benchmarks/church_sum_of_squares.rs` to use `..GridConfig::default()`. Files: `merge/types.rs`, `config.rs`, `bench/suite.rs`, `bench/benchmarks/church_sum_of_squares.rs`.
  - **TASK-0351 (M, ~130 LoC + tests):** GREEN — added `pub(crate) fn check_global_normal_form(&[WorkerRoundStats]) -> (bool, bool)` and the R3/R4 skip-merge / GNF early-exit branches in `run_grid` after stats collection (gated on `coordinator_free_rounds && strict_bsp`). Wire FSM in `protocol/coordinator.rs` untouched (R7). Test count 863 → 874 (+11: UT-0351-01..04 truth table, UT-06 GNF, UT-07 default-v1, UT-08 lenient-no-skip, UT-09 FSM-untouched grep with self-grep mitigation, UT-10/11 R5 equivalences, UT-12 G1 spot check `church_add(2,3) → 5`). Files: `merge/grid.rs` (helper + skip branch + 9 tests), `protocol/coordinator.rs` (1 structural test).
  - **Final gate:** 874 lib + 4 integration = 878 total tests passing (baseline 850 + 24); clippy `--workspace --all-targets -- -D warnings` clean; `cargo fmt --check` clean; release smoke `compute add 3 5 = 8` passes.
- [x] **REVIEW** (2026-04-16) — full-bundle pass over TASK-0348..0351 in
      `docs/reviews/REVIEW-SPEC-19-section-3.1-2026-04-16.md`. Verdict:
      **APPROVE** — 0 MUST-FIX, 2 NICE-TO-HAVE (CLI flag exposure, stub
      unification), R1..R7 all PASS. All four pre-flagged developer smells
      explicitly accepted with rationale (UT-0351-09 self-grep mitigation
      OK; skip-still-merges faithful to §3.1; GNF counter increment correct
      semantics; vacuous-true on empty unreachable in core path). 8 QA
      probes enumerated for Stage 5 (5 documented by developer + 3 added
      by reviewer: empty-partition guard, two-worker round-boundary mix,
      strict-BSP telemetry audit). Architecture clean — `compute_border_activity`
      correctly placed in `merge/helpers.rs`, dependency direction preserved,
      no `unwrap`/`unsafe`/`println!` in production deltas, all GridConfig
      constructors use `..GridConfig::default()` for forward-compat.
- [x] **QA** (2026-04-16) — 9 adversarial probes from §7 of the REVIEW added
      as `qa_probe_*` tests on `merge/{helpers,grid}.rs`:
      - `merge::helpers::tests::qa_probe_a_empty_workers_returns_false_not_vacuous_true`
      - `merge::helpers::tests::qa_probe_f_empty_and_non_principal_border_maps_return_false`
      - `merge::grid::tests::qa_probe_a_run_grid_panics_on_zero_workers`
      - `merge::grid::tests::qa_probe_b_single_worker_always_skips_merge`
      - `merge::grid::tests::qa_probe_c_oscillating_border_activity_no_carryover`
      - `merge::grid::tests::qa_probe_d_lenient_mode_ignores_coordinator_free_flag`
      - `merge::grid::tests::qa_probe_e_race_window_in_local_sim_is_not_applicable` (N/A doc)
      - `merge::grid::tests::qa_probe_g_skip_transition_preserves_correctness`
      - `merge::grid::tests::qa_probe_h_exact_coordinator_free_count`
      Test count 878 → **887** lib (883) + integration (4). All probes PASS
      first try. Clippy `--workspace --all-targets -- -D warnings` clean,
      `cargo fmt --check` clean. **No new bugs surfaced** — the 4 pre-flagged
      developer smells and 3 reviewer-added probes all behave as specified:
      defense-in-depth `assert!(num_workers >= 1)` triggers on zero workers,
      single-worker fast path correctly bypasses skip counter, oscillating
      and skip→non-skip transitions preserve R5 (byte-equal totals + decoded
      results), lenient mode silently ignores the flag (R6 SHOULD), GNF exit
      counts EXACTLY 1 (no double-count of skip + GNF), and helper-direct
      empty / non-principal-only inputs all return false (vacuous-true contract
      pinned at the helper level).
- [x] **REFACTOR** (2026-04-16): no-op — REVIEW returned 0 MUST-FIX, QA found
      0 bugs. The 2 NICE-TO-HAVE items from REVIEW (CLI flag exposure for
      `--coordinator-free-rounds`, deduplicate `build_round_stats` between
      `worker.rs` FSM stub and `protocol/worker.rs` real path) are deferred
      to follow-up backlog items, not blocking. Final gate held at 887 lib +
      4 integration tests, clippy/fmt clean, release smoke green. **Bundle
      shipped.**

### Bundle Shipped — 2026-04-16

SPEC-19 §3.1 (item 2.34) — Coordinator-Free Round (Merge Avoidance) closed
green through all 6 SDD stages. Test count 850 → 887 (+37 across DEV +24
and QA +13 lib counted via 9 probe tests). All R1..R7 PASS. Wire FSM in
`protocol/coordinator.rs` untouched (R7 v1/v2 compat preserved). Next
bundle on Tier 1 break-even path: **item 2.24** (per
`docs/V2-FEATURE-MATRIX.md` order 2.22 → 2.23 → **2.34 ✓** → 2.24 → 2.25
→ 2.35 → 2.26).

### Developer Brief (Stage 3 Dispatch — 2026-04-16)

**Agent:** `developer` (see `.claude/agents/developer.md`)
**Branch:** `v2-development`
**Scope:** 4 tasks in strict sequence, ~300 LoC total, TDD RED→GREEN→REFACTOR per task.

**Inputs per task (read in this order):**
1. `docs/backlog/TASK-03{48,49,50,51}.md` — what to implement + acceptance criteria
2. `docs/tests/TEST-SPEC-03{48,49,50,51}.md` — exact `#[test]` blueprints
3. `specs/SPEC-19-coordinator-free-round.md` — R1..R7 full context
4. `docs/backlog/SPEC-19-section-3.1-coordinator-free-round-tasks.md` — bundle index

**Per-task TDD flow:**
- **TASK-0348 (S):** pragmatic TDD-RED — field + helper + their tests land together
  (trivial additive change; red-phase is a formality for S tasks per user feedback
  `feedback_tdd_red_pragmatic.md`).
- **TASK-0349 (S):** pragmatic TDD-RED; populate 3 build sites (worker.rs,
  protocol/worker.rs if present, merge/grid.rs). Maintain R1 ordering:
  `reduce_all → rebuild_free_port_index → compute_border_activity`.
- **TASK-0350 (S):** pragmatic TDD-RED; additive config field + metric counter.
- **TASK-0351 (M):** REAL TDD-RED — write all tests first with stubs
  (`check_global_normal_form` returning `(false,false)` and no skip branch in
  `run_grid`), confirm reds, then implement.

**Gate after EACH task (order-enforced):**
- `cargo test --workspace` — all green; test count never below 850 lib + 4 integration.
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- `cargo fmt --check` — clean.
- Record test count delta in pipeline-state.md after each task.

**Hard constraints (v2 development rules):**
- No `unwrap()` in production; no `unsafe` without `// SAFETY:`; no `println!`
  (use `tracing`).
- No changes to the wire FSM in `protocol/coordinator.rs` or to any `Message`
  variant in `protocol/mod.rs` beyond the additive
  `has_border_activity: bool` on the `WorkerRoundStats` payload (R7).
- v1 frame format (`protocol/frame.rs`) is untouched; verify by reading the
  file if in doubt — the `has_border_activity` field lives in the bincode v2
  payload of `Message::PartitionResult`, not in the frame header.
- `compute_border_activity` must be O(|free_port_index|) using the existing
  `HashMap<u32, PortRef>` (no net scan).
- For TASK-0351 R5 equivalence test:
  - skip-triggered side: `church_add(2, 3) → 5` at w=2 strict BSP (no border
    activity by construction in the Church encoding).
  - skip-NOT-triggered side: use a multi-partition workload with border
    activity (e.g., `cascade_cross` from benches if exported as a test
    helper; otherwise the smallest existing `run_grid` test workload that
    exhibits non-empty border activity — TASK-0351's TEST-SPEC UT-10/UT-11
    will already name one).
- For TASK-0351 UT-09 (wire-FSM-untouched assertion): read
  `protocol/coordinator.rs` source as a string and assert no new `Message::*`
  variants or new match arms were added in this bundle. Source-grep form is
  acceptable (flagged in TEST-SPEC-0351 for future AST-parse migration).

**Bundle size guardrail:** if any single task exceeds its LoC estimate by >50%,
STOP and report — signal that the spec needs amendment.

**Report-back contract (developer → orchestrator after each task):**
- Task ID + GREEN/RED status
- Test count before → after (+delta)
- Files touched + LoC delta vs estimate
- Any adversarial-probe trip (belong in QA but document if encountered)
- Any design choice that REVIEW should explicitly evaluate

**Orchestrator checkpoint:** after all 4 tasks GREEN, STOP. Do NOT auto-invoke
reviewer — the parent orchestrator checkpoints between stages.

### Hard Gate Carried Forward

Test count must stay **>= 850 lib tests** after the bundle ships
(post-SPEC-18 baseline). Clippy + fmt clean. Release smoke
`compute add 3 5 -> 8` must continue to work.

---

## SPEC-18 §3.5 (item 2.24) — Zero-Copy Archive (rkyv on hot path) — SHIPPED 2026-04-16

**Started:** 2026-04-16
**Test baseline before this work:** 887 lib + 4 integration (post-SPEC-19 §3.1 ship)
**Bundle scope:** SPEC-18 §3.5 only — requirements R20-R27 + tests T11-T14.
  Activates the FLAG_ARCHIVED bit (currently reserved per QA Probe 3 of the
  SPEC-18 §3.1-3.4 ship) under a `#[cfg(feature = "zero-copy")]` gate.
**Explicitly OUT of scope for this bundle:**
  - v1 wire format / v1 protocol path (untouched).
  - SPEC-19 delta protocol code (§3.2/§3.3 are items 2.35/2.26 — separate bundles).
  - Making `zero-copy` a default feature (R20 explicit MUST NOT).
  - `rkyv::access_unchecked` in production code (R24 step 3 explicit MUST NOT).
**Estimated size:** ~600 LoC across atomic tasks (<200 LoC each), 3-5 days,
  Medium-High difficulty (per V2-FEATURE-MATRIX item 2.24 + DEFERRED-WORK D-002).
**Tier 1 break-even path:** confirmed next after item 2.34 in
  `docs/V2-FEATURE-MATRIX.md` (Implementation order:
  2.22 → 2.23 → 2.34 ✓ → **2.24** → 2.25 ✓ → 2.35 → 2.26).
**Dependencies:** SPEC-18 §3.1-3.4 + §3.6 (item 2.23, shipped) + SPEC-19 §3.1
  (item 2.34, shipped). No new external blockers.
**Tracker:** DEFERRED-WORK row D-002 (7-step action plan; this bundle
  fulfils its acceptance signal — close D-002 row when the bundle ships).

### Bundle Goal (single sentence)

When the `zero-copy` cargo feature is enabled, hot-path messages
(`AssignPartition`, `PartitionResult`) MAY be sent as rkyv archives with
`FLAG_ARCHIVED` set in the frame flags byte; receivers decompress (if
`FLAG_COMPRESSED` is also set) → verify CRC32C against the uncompressed
payload (R12 invariant preserved) → call `rkyv::access` (validating API,
NEVER `access_unchecked`) → either use the `ArchivedPartition` reference
directly or `rkyv::deserialize` it; non-hot-path messages with
`FLAG_ARCHIVED` set MUST be rejected with `ProtocolError::ArchiveValidationFailed`;
v2 receivers WITHOUT the `zero-copy` feature MUST cleanly reject
`FLAG_ARCHIVED` frames.

### Stage History

- [x] **SPLITTING** (2026-04-16): complete — 8 atomic tasks delivered
      (TASK-0352..0359, ~600 LoC total). Bundle index at
      `docs/backlog/SPEC-18-section-3.5-zero-copy-tasks.md`. FLAG_RESERVED
      design choice flagged for spec-critic (Option B recommended: route
      `FLAG_ARCHIVED` via `recv_frame` branch; mask `0b1111_1100` already
      excludes bit 1, so zero const changes). Other ambiguities flagged:
      (1) struct vs tuple variant for `ArchiveValidationFailed` (chose
      struct for consistency with `UnknownFlags`/`ChecksumMismatch`),
      (2) try-then-try discrimination strategy for hot-path R22 enforcement
      in `recv_frame`, (3) rkyv send-side error mapping to
      `ArchiveValidationFailed`. SPEC-18 §3.9 R36/R37 confirmed in scope
      (mandates `TransportConfig.use_zero_copy` field + matching CLI flag);
      TASK-0358 covers both. BACKLOG total grew 240 → 248.
- [x] **SPEC-CRITIC** (2026-04-16): complete — 0 spec amendments,
      3 task amendments applied (TASK-0354 tuple variant, TASK-0356
      send-side error prefix, TASK-0357 try-then-try ordering + mandated
      comment). Verdict at
      `docs/spec-reviews/SPEC-18-section-3.5-design-choices-2026-04-16.md`.
      DC-1 (FLAG_RESERVED) already implemented as Option B in
      `frame.rs` — no edit needed. DC-2 mandates tuple form
      `ArchiveValidationFailed(String)` per R35 verbatim — TASK-0354
      flipped struct→tuple, with cascading documentation updates in
      TASK-0356/0357/0358/0359. DC-3 mandates Assign-first try-then-try
      ordering + literal `// SPEC-18 R22 discrimination` source comment
      in `recv_frame` — TASK-0357 acceptance criteria + Key Types code
      block updated. DC-4 mandates conflation of rkyv send-side errors
      into `ArchiveValidationFailed` with `"serialize: "` reason prefix
      — TASK-0356 acceptance criteria + Key Types code block updated
      (placeholder `.map_err(|e| ProtocolError::Serialize(/* see Notes */))`
      replaced with the canonical mapping). TASK-0352, TASK-0353,
      TASK-0355 unaffected (no variant references). Each amended task
      file carries a `## Spec-Critic Amendments (2026-04-16)` section
      recording the change-set and rationale. Bundle index
      `docs/backlog/SPEC-18-section-3.5-zero-copy-tasks.md` updated to
      reflect the resolved design choices. **Stage 2 TESTS unblocked.**
- [x] **TESTS** (2026-04-16): complete — 8 TEST-SPEC files delivered to
      `docs/tests/` (TEST-SPEC-0352 through TEST-SPEC-0359), one per task,
      each enumerating concrete `#[test]` blueprints with target file
      paths, exact fixtures, R-mappings, and feature gates
      (`#[cfg(feature = "zero-copy")]` / unconditional). Spec-critic
      verdicts DC-2/DC-3/DC-4 all pinned by load-bearing tests:
      - **DC-2 (tuple variant)**: UT-0354-04 pattern-matches
        `ProtocolError::ArchiveValidationFailed(s)` — fails to compile if
        struct form is restored.
      - **DC-3 (Assign-first try-then-try + literal source comment)**:
        UT-0357-08 fall-through ordering proof + UT-0357-09 source-grep
        for `// SPEC-18 R22 discrimination` literal in `frame.rs`.
      - **DC-4 (`"serialize: "` prefix mandate)**: UT-0356-07 source-grep
        for the prefix string near `rkyv::to_bytes` in `frame.rs`.
      - **R12 ordering invariant**: UT-0357-04 corrupts CRC bytes of an
        archive+LZ4 frame and asserts `Err(ChecksumMismatch)` — proves
        decompress → CRC → rkyv access ordering.
      - **R22 hot-path-only**: UT-0357-06 rejects bare `u32` archive
        (control-shaped) with `ArchiveValidationFailed`; UT-0359-T14-02
        rejects forged FLAG_ARCHIVED on `Shutdown` variant.
      - **R20 default-OFF**: UT-0352-01 asserts `cfg!(feature = "zero-copy")
        == false` in default build; UT-0352-03 parses `Cargo.toml` via
        `include_str!` to confirm `default = []` does not contain `zero-copy`.
      - **Cumulative bundle test target:** 887 → **904+** lib tests in
        default build (+17), 887 → **939+** lib tests with `--features
        zero-copy` (+52). Conservative floor 887 lib never breached.
      - **Test count exceeds orchestrator hint** (+18 default / +24 feature):
        documented in TEST-SPEC-0359 §"Bundle-wide aggregate test count"
        — variant + CLI tests are unconditional per TASK-0354/0358 design;
        feature-build delta exceeds hint because each per-task TEST-SPEC
        enumerates 5-9 sub-cases covering R-numbers, edge cases, and
        DC-mandates explicitly. Trim path noted: fold UT-0354-04 and
        UT-0357-09 into helper assertions if developer prefers.
      - **Hard-to-write-deterministically tests flagged:**
        UT-0356-07/UT-0357-09 (source-grep self-mitigation, mirrors
        UT-0351-09 pattern); QA-0355-C/D (allocator-misbehavior +
        concurrent-read race, both non-deterministic and NOT implemented
        as `#[test]`); QA-0357-A (premature EOF mid-archive, flagged
        for QA stage with `tokio_test::io::Builder`).
      - **No spec ambiguities surfaced** during TEST-SPEC drafting —
        the 4 DCs already resolved by spec-critic on 2026-04-16 are
        sufficient. No further spec-critic dispatch needed.
- [x] **DEV**: TASK-0352..0359 all GREEN (2026-04-16).
  - **TASK-0352 (S, ~30 LoC):** GREEN (2026-04-16) — `relativist-core/Cargo.toml`
    adds `rkyv = { version = "0.8", optional = true, default-features = false,
    features = ["bytecheck", "alloc", "std"] }` + `zero-copy = ["dep:rkyv"]`.
    Smoke test in `relativist-core/src/lib.rs` confirms u32 round-trip via
    `rkyv::to_bytes`/`access`/`deserialize` under feature. Test count
    883 default / 887 zero-copy.
  - **TASK-0353 (M, ~150 LoC):** GREEN (2026-04-16) — Archive/Serialize/Deserialize
    derives on `Symbol`, `PortRef`, `Agent`, `Net` (`#[rkyv(with = rkyv::with::Skip)]`
    on `freeport_redirects`), `IdRange`, `Partition`, `CompactSubnet`,
    `WorkerRoundStats`. UT-09 cross-build serde-bincode-v2 path test in
    `frame.rs` verifies bincode v2 unaffected by rkyv derives. Test count
    883 default / 893 zero-copy.
  - **TASK-0354 (S, ~30 LoC):** GREEN (2026-04-16) — `protocol/error.rs` adds
    `ArchiveValidationFailed(String)` tuple variant (DC-2) with Display
    `"rkyv archive validation failed: {reason}"`. 5 unconditional tests
    + extended `test_all_variants_debug`. Test count 888 default / 898
    zero-copy.
  - **TASK-0355 (S, ~50 LoC):** GREEN (2026-04-16) — `frame.rs` adds
    `pub(crate) async fn read_aligned_payload<R>(reader, len) -> Result<AlignedVec, _>`
    under `#[cfg(feature = "zero-copy")]` + `#[allow(dead_code)]`
    (wired by TASK-0357 follow-up). 5 zero-copy tests + 1 default
    (cfg-gate symbol absence). R25: AlignedVec base pointer is 16-byte
    aligned for non-empty buffers (debug_assert tolerant of empty case).
    Test count 889 default / 904 zero-copy.
  - **TASK-0356 (M, ~120 LoC):** GREEN (2026-04-16) — `frame.rs` adds
    `ArchiveAssignPayload`, `ArchivePartitionResultPayload` wire wrappers
    (R21 hot-path types), `is_hot_path_message(&Message) -> bool` (R22),
    `pub async fn send_frame_v2<W>(writer, message, use_archive,
    compression_threshold) -> Result<usize, _>` under
    `#[cfg(feature = "zero-copy")]`. R12 CRC over uncompressed archive
    bytes; R23 LZ4 wrap above threshold; non-hot-path messages fall
    through to bincode. DC-4: send-side rkyv errors → `ArchiveValidationFailed(format!("serialize: {}", e))`.
    6 zero-copy tests + 1 default. Test count 890 default / 911 zero-copy.
  - **TASK-0357 (M, ~150 LoC):** GREEN (2026-04-16) — `frame.rs` adds
    `recv_frame` archive branch (`#[cfg(feature = "zero-copy")] if
    header.is_archived() { return decode_archive_payload(&payload, ...); }`)
    after R12 CRC verify, and `decode_archive_payload(payload, total_bytes)
    -> Result<(Message, usize), _>`. DC-3: try-then-try Assign-first
    ordering with mandated `// SPEC-18 R22 discrimination` comments;
    schema mismatch yields R26 error
    `"non-hot-path archive payload (matched neither AssignPartition nor
    PartitionResult)"`. R25 satisfied via `AlignedVec::with_capacity(len)
    + extend_from_slice(payload)` copy at top of decode (recv pipeline
    holds a `Vec<u8>`, allocator-aligned only). QA Probe 3 split
    cfg-gated (default: bincode Deserialize; zero-copy: ArchiveValidationFailed).
    8 zero-copy tests (round-trip Assign+Result, compressed-archive,
    corrupt-archive rejection, R12 CRC-before-rkyv ordering, R26
    non-hot-path rejection, R25 alignment honored, DC-4 prefix
    asymmetry) + 2 default tests (FLAG_ARCHIVED → Deserialize, bincode
    path unaffected). Test count 895 default / 919 zero-copy. Clippy
    + fmt clean both feature configs.
  - **TASK-0358 (S, ~50 LoC):** GREEN (2026-04-16) — `protocol/config.rs`
    adds `TransportConfig.use_zero_copy: bool` (default `false`,
    unconditional field so configs are bit-identical across feature
    builds). `config.rs` adds `--use-zero-copy` flag to both
    `CoordinatorArgs` and `WorkerArgs` (default `false`); threaded
    through `build_transport_config` (now `#[allow(clippy::too_many_arguments)]`
    with 8 params). Test fixtures `make_coordinator_args` /
    `make_worker_args` + 2 `TransportConfig` literals updated. 5
    unconditional tests in `protocol::config::tests` (default false,
    settable, Debug includes field, Clone preserves, NodeConfig chain)
    + 3 CLI tests in `config::tests` (default false on both subcommands,
    flag threads through coordinator, flag threads through worker).
    Test count 903 default / 927 zero-copy.
  - **TASK-0359 (S, ~70 LoC + ~340 LoC test suite):** GREEN (2026-04-16)
    — new module `relativist-core/src/protocol/zero_copy_tests.rs`
    declared in `protocol/mod.rs` under `#[cfg(all(test, feature =
    "zero-copy"))]`. T11..T14 acceptance suite (10 tests):
    - **T11 (round-trip identity):** Assign + Result, both
      uncompressed and compressed; field-by-field equality on
      Partition / WorkerRoundStats (no PartialEq derives).
    - **T12 (corrupt archive rejection):** undersized FLAG_ARCHIVED
      payload yields ArchiveValidationFailed.
    - **T13 (alignment correctness):** R25 round-trip battery at
      sizes [0, 1, 8, 17, 64, 128, 256, 1023, 4096] uncompressed
      + [4, 32, 256, 4096] compressed.
    - **T14 (hot-path-only enforcement):** bare `u32` archive yields
      R26 phrase `"non-hot-path archive payload"`; cold-path
      `Shutdown` with `use_archive=true` falls through to bincode
      (no FLAG_ARCHIVED ever emitted).
    - **T11/T13 cross-cut matrix:** 4 hot-path messages × 2
      compression states = 8 round-trips; variant identity check.
    Test count 903 default / 937 zero-copy. Clippy + fmt clean both
    feature configs.
  - **Final gate:** 903 lib default + 4 integration (+16 from
    baseline 887) / 937 lib zero-copy + 4 integration (+50 from
    baseline). Clippy `--workspace --all-targets -- -D warnings`
    clean both feature configs. `cargo fmt --check` clean. Release
    smoke `compute add 3 5 → 8` works. Bundle ships GREEN through
    Stage 3.
  - [x] **MF-1 RESOLVED (2026-04-16):** Option A (hoist) — `recv_frame`
    now calls `read_aligned_payload` directly on the FLAG_ARCHIVED +
    !FLAG_COMPRESSED fast path, eliminating the second `Vec<u8> →
    AlignedVec` copy that `decode_archive_payload` previously
    performed. The compressed-archive path still copies (decompression
    yields a plain `Vec<u8>`); `decode_archive_payload` is now a pure
    R25-precondition consumer with a `debug_assert!` alignment witness.
    `#[allow(dead_code)]` removed. R12 ordering preserved on both
    paths (decompress → CRC verify on uncompressed payload → rkyv
    access). Doc comments amended to reflect actual behavior. Tests
    now 903 lib default / 937 lib `--features zero-copy` (unchanged
    counts; the 5 helper tests stay valid because the function is
    still called from production code). Clippy + fmt clean both
    feature configs.
- [x] **REVIEW** (2026-04-16): **APPROVE** (MF-1 resolved) — verdict at
      `docs/reviews/REVIEW-SPEC-18-section-3.5-2026-04-16.md`.
      1 MUST-FIX (blocking): **MF-1** resolve `read_aligned_payload`
      dead-code state. The helper at `frame.rs:258-282` is `pub(crate)`,
      fully tested (5 tests), but no caller exists outside tests — the
      `#[allow(dead_code)]` is load-bearing today. Acceptable fixes:
      **(A)** hoist into `recv_frame` uncompressed-archive fast path
      (preferred — saves one copy); **(B)** strip the helper + 5 tests
      and document the deferred hoist in `docs/DEFERRED-WORK.md`.
      "Accept-with-deferred-hoist" is NOT acceptable (no follow-up
      task tracked). 5 NICE-TO-HAVE items deferrable (NTH-1 through
      NTH-5: builder refactor, test-helper dedup, PartialEq audit,
      visibility narrowing × 2). Spec compliance matrix: R20-R27 +
      R35-R37 all PASS. Architecture: dependency direction respected;
      feature gating correct; DC-1..DC-4 spec-critic verdicts honored
      in code. No `unsafe`, no `unwrap()` in production paths, no
      `println!`, no `access_unchecked`. 8 QA probes enumerated for
      Stage 5 (Q1 high-priority pointer-corruption; Q3 cross-feature
      interop; Q6 ARM alignment defer-if-no-ARM-CI). One pipeline
      note (F-9): default-build FLAG_ARCHIVED falls through to bincode
      `Deserialize` error (NOT `ArchiveValidationFailed("zero-copy
      feature disabled")` as the original brief suggested) — behavior
      is consistent with DC-1 Option B and R19 forward-compat; brief
      wording should be amended on next sdd-pipeline pass.
      **Update (2026-04-16):** MF-1 RESOLVED via Option A hoist —
      `read_aligned_payload` is now invoked from `recv_frame` on the
      FLAG_ARCHIVED + !FLAG_COMPRESSED fast path; `#[allow(dead_code)]`
      removed; helper is no longer dead. Verdict transitions to
      **APPROVE** (no longer with MUST-FIX). All gates re-greened on
      both feature configs (test 903/937, clippy clean, fmt clean).
- [ ] **QA**: **READY TO DISPATCH** (qa). MF-1 resolved (2026-04-16);
      gates re-greened: `cargo test --workspace` 903 lib + 4 integration
      default / 937 lib + 4 integration `--features zero-copy`; `cargo
      clippy --workspace --all-targets -- -D warnings` clean both
      feature configs; `cargo fmt --check` clean. Probes Q1-Q8 from
      the review (`docs/reviews/REVIEW-SPEC-18-section-3.5-2026-04-16.md`
      §7) define the QA bug-hunt scope.
- [x] **REFACTOR** (2026-04-16): no-op — Stage 4 REVIEW MF-1 already
      resolved (Option A hoist of `read_aligned_payload` into `recv_frame`);
      Stage 5 QA found 0 bugs across 10 probes. The 5 NICE-TO-HAVE items
      from REVIEW (NTH-1 builder-struct refactor, NTH-2 dedup test helpers,
      NTH-3 PartialEq audit, NTH-4/5 visibility narrowing) are deferred
      as follow-up backlog items. Final gate: 905 lib + 4 integration
      (default), 945 lib + 4 integration (`--features zero-copy`); clippy
      clean both feature configs; fmt clean; release smoke green.
      **Bundle shipped.**

### Bundle Shipped — 2026-04-16

SPEC-18 §3.5 (item 2.24) — Zero-Copy Archive (rkyv on hot path) closed
green through all 6 SDD stages (plus Stage 1.5 spec-critic). Test count
887 → 905 (default, +18) / 887 → 945 (`--features zero-copy`, +58). All
R20-R27 PASS. DEFERRED-WORK D-002 closed and moved to Resolved
Deferrals. Tier 1 break-even path next: **item 2.35** (Delta-Based
Merge with BorderGraph, SPEC-19 §3.2 — item 2.25 already DONE per
commit c360fe5).

### Current Stage

**Stage 4 REVIEW — COMPLETE.** Verdict: **APPROVE** (MF-1 resolved
via Option A hoist on 2026-04-16). Bundle is structurally sound,
spec-compliant, and the previously flagged dead-code seam is now wired
through production. Test counts: default features **903 lib** (+16 from
baseline 887), `--features zero-copy` **937 lib** (+50 from baseline).
Bundle floor 887 lib never breached. Stage 5 QA ready to dispatch.

### Hard Gate Carried Forward

Test count must stay **>= 887 lib tests** after the bundle ships
(post-SPEC-19 §3.1 baseline). Clippy + fmt clean. Release smoke
`compute add 3 5 -> 8` must continue to work. With `--features zero-copy`,
all 887+ tests MUST also pass.

### Task-Splitter Brief (Stage 1 Dispatch — 2026-04-16)

**Agent:** `task-splitter` (see `.claude/agents/task-splitter.md`)
**Branch:** `v2-development`
**Output:** `docs/backlog/SPEC-18-section-3.5-zero-copy-tasks.md` (bundle index)
  + per-task files starting at TASK-0352 (next available; TASK-0351 was the
  last assigned in SPEC-19 §3.1 bundle).

**Pre-flight reading (in this order):**
1. `specs/SPEC-18-wire-format-v2.md` §3.5 (R20-R27), §4.6 (rkyv archive
   types and `rkyv::access` validating API), §7.2 (T11-T14 test list).
2. `docs/DEFERRED-WORK.md` row D-002 — 7-step action plan + files-to-revisit
   list + acceptance signal. **The 7-step plan is a hint, NOT a substitute
   for atomic task decomposition** (each step may map to 1-2 atomic tasks).
3. `relativist-core/Cargo.toml` — current feature flags (none for rkyv yet);
   confirm no rkyv dep already lurks.
4. `relativist-core/src/protocol/frame.rs` — current `FLAG_ARCHIVED` handling
   (the bit IS already defined as a const + reserved-mask partition; QA
   Probe 3 of the SPEC-18 §3.1-3.4 ship pinned that frames with
   `FLAG_ARCHIVED` currently fail bincode v2 deserialization — this bundle
   activates that bit).
5. `relativist-core/src/protocol/types.rs` (Message enum — identify hot-path
   variants `AssignPartition` + `PartitionResult`) and
   `relativist-core/src/partition/types.rs` (Partition, IdRange, free_port_index
   HashMap, CompactSubnet serde adapter — note: rkyv path bypasses
   CompactSubnet per SPEC-18 §4.6).

**Hard scope boundaries (out of scope — task-splitter MUST NOT generate
tasks for these):**
- Modifying v1 wire format or v1 protocol path.
- Touching SPEC-19 delta protocol code (§3.2/§3.3 are items 2.35/2.26).
- Making `zero-copy` a default feature (R20).
- Using `rkyv::access_unchecked` in production code (R24 step 3).

**Suggested task boundaries (task-splitter may refine; ~600 LoC total):**
1. **TASK-0352 — Cargo.toml: rkyv optional dep + `zero-copy` feature gate.**
   ~30 LoC, S. Adds `rkyv = { version = "0.8", optional = true,
   features = ["validation"] }` (or current stable, task-splitter to
   verify) and `zero-copy = ["dep:rkyv"]`. No source changes yet.
2. **TASK-0353 — Derive Archive/rkyv::Serialize/rkyv::Deserialize on the
   8 types listed in R21** (`Net`, `Partition`, `CompactSubnet`, `Agent`,
   `Symbol`, `PortRef`, `IdRange`, `WorkerRoundStats`) under
   `#[cfg_attr(feature = "zero-copy", derive(rkyv::Archive,
   rkyv::Serialize, rkyv::Deserialize))]`. ~150 LoC across multiple
   files (net/types.rs, partition/types.rs, partition/compact.rs,
   merge/types.rs). M. Depends on TASK-0352.
3. **TASK-0354 — `ProtocolError::ArchiveValidationFailed(String)` variant
   + Display impl (R26 + R35)** under `#[cfg(feature = "zero-copy")]` (or
   unconditional — task-splitter to choose). ~30 LoC, S. Depends on
   TASK-0352.
4. **TASK-0355 — Aligned receive buffer using `rkyv::util::AlignedVec`
   (R25)** wired into recv_frame when FLAG_ARCHIVED is set. ~50 LoC, S.
   Depends on TASK-0352, TASK-0353.
5. **TASK-0356 — send_frame archive path: `rkyv::to_bytes`, set
   FLAG_ARCHIVED, optional LZ4 wrap (R23)** restricted to hot-path
   messages only (R22). ~100 LoC, M. Depends on TASK-0353, TASK-0354.
6. **TASK-0357 — recv_frame archive path: detect FLAG_ARCHIVED,
   decompress if combined with FLAG_COMPRESSED, validate CRC against
   uncompressed payload (R12 invariant preserved), call `rkyv::access`
   (validating API), reject non-hot-path messages with
   `ArchiveValidationFailed` (R26).** ~120 LoC, M. Depends on TASK-0354,
   TASK-0355.
7. **TASK-0358 — CLI `--use-zero-copy` flag (R36, R37)** gated on
   `#[cfg(feature = "zero-copy")]`; thread through `TransportTuning` /
   `TransportConfig`. ~50 LoC, S. Depends on TASK-0352. Task-splitter to
   verify whether SPEC-18 §3.9 R36/R37 apply or whether
   `TransportConfig` already has the field.
8. **TASK-0359 — T11-T14 test suite** (round-trip identity, rejection
   of corrupt archives, alignment correctness, hot-path-only enforcement)
   gated under `#[cfg(feature = "zero-copy")]`. ~70 LoC, S — but
   task-splitter to confirm boundary; T11-T14 may yield 4-6 tests each
   (16-24 total).

**R12 ordering invariant (load-bearing — must be preserved):** CRC32C is
ALWAYS computed on the uncompressed payload. The recv pipeline order when
both FLAG_ARCHIVED + FLAG_COMPRESSED are set: **decompress → CRC verify →
rkyv access**. QA Probe 4 from the SPEC-18 §3.1-3.4 ship pinned this; the
rkyv path MUST honor it. Task-splitter to bake this ordering into TASK-0357
acceptance criteria.

**Wire compatibility (additive, not breaking):** v2 wire format is shipped
(item 2.23). FLAG_ARCHIVED bit is currently reserved (QA Probe 3 confirmed
it errors out as bincode Deserialize when no rkyv path exists). Activating
it is purely additive — v2 receivers WITHOUT `zero-copy` feature should
reject FLAG_ARCHIVED frames cleanly. **DESIGN CHOICE FOR SPEC-CRITIC:** the
FLAG_RESERVED check in `recv_frame` currently treats bit 1 as reserved
(via `FLAG_RESERVED = 0b1111_1100` partition); when feature is enabled, bit
1 must NOT be in the reserved mask. Two options:
  - (a) Adjust FLAG_RESERVED conditionally per-feature (
    `#[cfg(feature = "zero-copy")] const FLAG_RESERVED: u8 = 0b1111_1100;`
    vs feature-off);
  - (b) Keep FLAG_RESERVED constant and have the reserved-flag check
    explicitly route FLAG_ARCHIVED through a different code path that
    errors with `ArchiveValidationFailed("zero-copy feature disabled")`
    when the feature is off.
Task-splitter MUST flag this choice for spec-critic review BEFORE the TESTS
stage — the reviewer needs to pick one canonically.

**Test count baseline:** 887 lib + 4 integration. **Target after bundle
ships:** 905+ lib (+18 estimated; T11-T14 each yields 4-6 tests). Hard
floor: never below 887 lib at any point during DEV.

**Per-task acceptance criteria template (task-splitter to expand per task):**
- File paths to create/modify (relativist-core/...).
- R-numbers covered.
- Acceptance bullet: `cargo test --workspace` green; `cargo clippy
  --workspace --all-targets -- -D warnings` clean (with AND without
  `--features zero-copy`); `cargo fmt --check` clean.
- LoC budget; STOP if exceeded by >50%.

**Hard constraints (v2 development rules):**
- No `unwrap()` in production; no `unsafe` without `// SAFETY:`; no
  `println!` (use `tracing`).
- All deliverables in English (CLAUDE.md rule).
- Specs MUST NOT be modified by task-splitter (specialista-em-specs only).
- Source files MUST NOT be modified by task-splitter (developer only).

**Report-back contract (task-splitter → orchestrator):**
- Bundle index file path.
- Per-task: ID, title, complexity (S/M/L), LoC estimate, R-number coverage,
  dependencies.
- DAG diagram (text).
- Cumulative LoC vs ~600 LoC budget.
- Cumulative test target vs 887 → 905+ baseline.
- Any spec ambiguities flagged for spec-critic (esp. FLAG_RESERVED design
  choice).
- Whether D-002 7-step plan was followed verbatim or refined.

**Orchestrator checkpoint:** after task-splitter delivers the bundle, STOP.
Do NOT auto-invoke test-generator — the parent orchestrator checkpoints
between stages.

---

## SPEC-19 §3.2 (item 2.35) — BorderGraph — SHIPPED 2026-04-17

**Started:** 2026-04-17
**Test baseline before this work:** 905 lib default / 945 lib `--features zero-copy`

**Bundle scope:** SPEC-19 §3.2 only — requirements R8, R9, R10, R11, R12,
R15 (part 3 — `add_border_states` primitive), R16, R17, R18 (SHOULD
incremental invariant), R19 (pure-core). Explicitly **OUT of scope**:
R13/R14 coordinator-side `interact_*` dispatch + R15 parts 1-2 coordinator
dispatch + R20-R36 wire-format extensions + `GridConfig.delta_mode` flag
+ `run_grid_delta` BSP loop (all ship under item 2.26 — separate bundle).

### Design-choice verdicts (spec-critic, 2026-04-17)

See `docs/spec-reviews/SPEC-19-section-3.2-design-choices-2026-04-17.md`:

| DC | Verdict | Anchor |
|---|---|---|
| DC-1 | Reuse `crate::net::DISCONNECTED` (no `BorderTarget` enum, no `Option<PortRef>`) | UT-0362-05, UT-0362-06, UT-0362-11 |
| DC-2 | Ship `worker_borders: Vec<Vec<u32>>` now with `#[allow(dead_code)]` + R23 doc comment | field comment `// SPEC-19 §4.1 R23 (item 2.26)` |
| DC-3 | `detect_border_redexes` returns owned `Vec<(u32, BorderState)>` (not borrowed) | UT-0363-03 mutable-borrow-while-iterating test |
| DC-4 | Graph-enforced `is_redex` via `AddBorderEntry` input type (no `is_redex` field on input) | UT-0364-12 invariant test |

Additional spec-critic observations baked in:

- Obs #1: `AgentId` is a `pub type AgentId = u32` alias (not a newtype) —
  fixtures use `PortRef::AgentPort(id, 0)` directly.
- Obs #2: `is_principal_pair` re-exported from
  `merge/helpers.rs` via `use super::helpers::is_principal_pair;` —
  **not redefined** in `border_graph.rs`.

### Stage History

- [x] **SPLITTING** (2026-04-17): 6 atomic tasks created in `docs/backlog/`.
  - TASK-0360 — skeleton: `BorderState` + `BorderGraph` shell + `is_principal_pair` re-export + module wiring
  - TASK-0361 — `from_partition_plan` constructor (R10, C3 validation)
  - TASK-0362 — `apply_deltas` + `BorderDelta` struct (R11, R17, R18)
  - TASK-0363 — `detect_border_redexes` owned return + read-only accessors (R12, DC-3)
  - TASK-0364 — `remove_border` + `add_border_states` + `AddBorderEntry` (R15 part 3, R16, DC-4)
  - TASK-0365 — module `//!` doc + R19 pure-core invariant guard + `Send + Sync` witness
- [x] **SPEC-CRITIC** (2026-04-17): 4 DC verdicts issued
  (`docs/spec-reviews/SPEC-19-section-3.2-design-choices-2026-04-17.md`);
  tasks amended per verdicts.
- [x] **TESTS** (2026-04-17): 6 TEST-SPECs generated in `docs/tests/`.
  - TEST-SPEC-0360 (8 tests, +0 net — `is_principal_pair` tests stay in
    `helpers.rs` per Obs #2)
  - TEST-SPEC-0361 (8 tests, +8 net: constructor + C3 panics)
  - TEST-SPEC-0362 (11 tests, +11 net: `apply_deltas` + DISCONNECTED)
  - TEST-SPEC-0363 (8 tests, +8 net: `detect_border_redexes` + accessors)
  - TEST-SPEC-0364 (12 tests, +12 net: `remove_border` + `add_border_states`)
  - TEST-SPEC-0365 (3 tests, +3 net: R19 source-scan + doc-presence + Send/Sync)
  - Cumulative test target: 905 → 941+ lib default (+36 floor).
- [x] **DEV** (2026-04-17): all 6 tasks bundled into a single new file
  `relativist-core/src/merge/border_graph.rs` (~1.3 kLoC incl. tests; ~380
  LoC of production code) + one-line edit in `relativist-core/src/merge/mod.rs`
  (declares `pub mod border_graph;` and re-exports
  `AddBorderEntry, BorderDelta, BorderGraph, BorderState`).
  - Test count 905 → **955** lib default (+50) / 945 → **995** lib
    `--features zero-copy` (+50) — exceeds +36 floor.
  - All R8, R9, R10, R11, R12, R15-part-3, R16, R17, R18, R19 covered by
    the inline tests (coverage map per TEST-SPEC files).
  - All 4 design-choice verdicts (DC-1..DC-4) honoured in implementation.
  - Quality gates GREEN:
    - `cargo build --workspace` clean.
    - `cargo test --workspace --lib` — 955 pass, 0 fail.
    - `cargo test --workspace --lib --features zero-copy` — 995 pass, 0 fail.
    - `cargo clippy --workspace --all-targets -- -D warnings` clean
      (both default and `--features zero-copy`).
    - `cargo fmt --check` clean.
    - `cargo build --release` clean.
    - Release smoke `target/release/relativist.exe compute add 3 5` prints
      `Result: 8`.
  - R19 pure-core invariant enforced by in-workspace canary
    (`border_graph_source_respects_r19_pure_core_invariant`): no `use tokio`
    / `use async_trait` / `use crate::protocol` lines in `border_graph.rs`.
  - DC-2 dead-code allow: `worker_borders` field carries
    `#[allow(dead_code)]` + inline comment `// SPEC-19 §4.1 R23 (item 2.26)`
    until the worker-dispatch path in item 2.26 consumes it.
- [x] **REVIEW** (2026-04-17) —
      `docs/reviews/REVIEW-SPEC-SPEC-19-section-3.2-2026-04-17.md`.
      Verdict APPROVE, 0 MUST-FIX / 0 SHOULD-FIX / 3 NICE-TO-HAVE. All
      R8-R19 PASS; DC-1..DC-4 faithfully implemented. 10 adversarial
      probe angles enumerated for Stage 5 QA.
- [x] **QA** (2026-04-17) — 13 adversarial probes added to a nested
      `adversarial_probes` module inside
      `relativist-core/src/merge/border_graph.rs#tests`. See "Stage 5
      QA — SPEC-19 §3.2 Summary" section below.
- [x] **REFACTOR** (2026-04-17) — No-op. 0 MUST-FIX from REVIEW and
      0 bugs from QA; no code changes required. 3 NICE-TO-HAVE items
      from REVIEW are cosmetic / constant-factor only and deliberately
      deferred (no dedicated Stage 6 dispatch). Bundle SHIPPED.

### Stage 5 QA — SPEC-19 §3.2 Summary

- **Probes implemented:** 13 (Q1..Q13) in a new submodule
  `adversarial_probes` under `relativist-core/src/merge/border_graph.rs`
  `#[cfg(test)] mod tests`. One-line summary:
  - Q1 — `border_id = u32::MAX` roundtrips without DISCONNECTED
    confusion (init → apply → detect → remove).
  - Q2 — `from_partition_plan` `(side_a, worker_a, side_b, worker_b)`
    byte-stable across 100 runs on identical input.
  - Q3 — C3 panic under 4+ sightings names the TRUE count (`"4"`), not
    a hard-coded `"3"`.
  - Q4 — DC-1 sentinel discipline: `FreePort(u32::MAX - 1)` is NOT
    DISCONNECTED; only exact `PortRef::FreePort(u32::MAX)` triggers R17.
  - Q5 — `apply_deltas(worker_id)` for unknown `worker_id` (including
    `u32::MAX`) is a silent no-op, not a panic or bounds fault.
  - Q6 — Idempotent double-apply of the same delta: no membership
    drift in `active_redexes`; cross-sectional invariant holds.
  - Q7 — DC-4 mixed 6-entry batch (3 redex + 3 non-redex): all states
    land, `active_redexes` matches subset, `worker_borders` updated
    for both sides of every entry.
  - Q8 — `remove_border` on an absent id (9999, `u32::MAX`, 0) returns
    None and preserves every container.
  - Q9 — 10k-border stress: promote-demote-promote cycles leave
    `active_redex_count` deterministically at the end-state value;
    cross-sectional invariant holds at scale.
  - Q10 — Same-worker-on-both-sides degenerate border: contract pinned
    (`updates_a` wins on `worker_a == worker_b` tie).
  - Q11 — `remove_border` on a redex scrubs BOTH `borders` and
    `active_redexes`, and `detect_border_redexes` no longer surfaces it.
  - Q12 — `worker_borders` is populated for both sides after
    `add_border_states`; `apply_deltas` does NOT mutate the reverse
    index (DC-2 landmine guard for item 2.26 consumer).
  - Q13 — Mid-batch panic in `add_border_states` (REVIEW §9 item 6):
    duplicate-id panic leaves the already-inserted prefix consistent
    (invariant `{bid : is_redex} == active_redexes` still holds).
- **Bugs found:** **0.** All 13 probes PASS first try.
- **Final test counts:**
  - `cargo test --workspace --lib` — **968** (955 + 13).
  - `cargo test --workspace --lib --features zero-copy` — **1008**
    (995 + 13).
- **Gate status:**
  - `cargo test --workspace` GREEN (both feature configs, deltas above).
  - `cargo clippy --workspace --all-targets -- -D warnings` GREEN
    (default AND `--features zero-copy`).
  - `cargo fmt --check` GREEN (after one `cargo fmt` pass on the new
    probe block).
  - `cargo build --release` GREEN; smoke
    `target/release/relativist.exe compute add 3 5 → Result: 8`.
- **Stage 6 REFACTOR:** No-op (0 bugs). Ready to ship.

### Deviations / ambiguities

- No spec deviations. All four DC verdicts were applied verbatim.
- DC-2 `worker_borders` is unread production-side until item 2.26 ships;
  the R23 doc-comment + `#[allow(dead_code)]` are the agreed sunset
  convention.
- `from_partition_plan` panic messages include both the offending
  `border_id` and the sighting count (`"SPEC-19 C3 invariant violated:
  border_id {bid} has {count} sightings"`), satisfying both test-specs
  0361-06 (substring `"99"`), 0361-07 (substring `"3"` and `"55"`), and
  0361-08 (substring `"77"`).
- QA Q10 (same-worker both sides) pins the CURRENT tie-break behavior
  (`updates_a` wins). Whether this is the *desirable* semantic is an
  OQ per REVIEW §9 item 9. The probe locks the contract; any future
  change MUST update Q10 accordingly (no silent drift).

### Acceptance criteria — SELF-VERIFIED

1. `cargo test --workspace --lib` — 955 pass, 0 fail (+50 over 905 baseline,
   floor +36 satisfied).
2. `cargo test --workspace --lib --features zero-copy` — 995 pass, 0 fail
   (+50 over 945 baseline).
3. `cargo clippy --workspace --all-targets -- -D warnings` clean
   (default + zero-copy).
4. `cargo fmt --check` clean.
5. `cargo build --release` clean; `compute add 3 5` prints `Result: 8`.
6. R19 in-workspace canary green (source-file scan).
7. `BorderGraph` + `BorderState` + `BorderDelta` + `AddBorderEntry` re-exported
   from `crate::merge::*`.

---

## SPEC-19 §3.4 (item 2.26-A) — Delta-Only Protocol Wire Extensions — DEV COMPLETE 2026-04-17

**Started:** 2026-04-17
**Test baseline before this work:** 968 lib default / 1008 lib `--features zero-copy`.
**Spec anchors:** SPEC-19 §3.4 R33 amendments (2026-04-17) — DC-A1, DC-A2, DC-B3, DC-B5.

**Bundle scope (2.26-A — wire layer only):** 5 new `Message` variants
(`InitialPartition`, `RoundStart`, `RoundResult`, `FinalStateRequest`,
`FinalStateResult` at discriminants 7..=11) + merge-owned wire structs
(`BorderDelta`, `LocalReconnection`, `PendingCommutation`, `MintedAgent`)
serde-derived and re-exported from `crate::protocol::*` for wire callers.
Explicitly **OUT of scope** (ships under 2.26-B and 2.26-C): coordinator
dispatch of delta rounds (`run_grid_delta`), worker-side stateful
reduction handlers, `GridConfig.delta_mode` flag.

### Stage History

- [x] **SPLITTING** (2026-04-17): 6 atomic tasks (TASK-0366..0371) created
  in `docs/backlog/` covering DC-A1 serde+re-export, DC-A2 equality,
  DC-B3 `LocalReconnection`, DC-B5 2-phase AgentId alloc, 5 new Message
  variants, and discriminant stability R37.
- [x] **SPEC-CRITIC** (2026-04-17): R33 amendments ratified inline in
  SPEC-19 §3.4 (`specs/SPEC-19-distribution-protocol.md`). No new DC
  review file for 2.26-A (decisions are embedded in the spec text).
- [x] **TESTS** (2026-04-17): TEST-SPECs for TASK-0366..0371 in
  `docs/tests/`. Floor: +22 net tests.
- [x] **DEV** (2026-04-17): all 6 tasks landed across 4 files.
  - `relativist-core/src/merge/border_graph.rs`: added `serde` derives
    on `BorderDelta`; added 3 new structs `LocalReconnection`,
    `PendingCommutation`, `MintedAgent` (DC-B3, DC-B5) with full
    `Serialize`/`Deserialize`/`Clone`/`Debug`/`PartialEq`/`Eq` derives;
    4 inline bincode round-trip tests.
  - `relativist-core/src/merge/mod.rs`: re-exported the 3 new structs
    from `border_graph::*` alongside existing `BorderDelta`.
  - `relativist-core/src/protocol/types.rs`: imported
    `{BorderDelta, LocalReconnection, MintedAgent, PendingCommutation,
    WorkerRoundStats}` from `crate::merge` and `PortRef` from
    `crate::net`; added 5 new `Message` variants at discriminants
    7..=11 with doc-comments anchoring DC-A1/A2/B3/B5; added helper
    fixtures `make_test_stats_with_activity(bool)` and
    `make_partition_with_n_agents(n)`; appended 17 unit tests
    (per-variant serde round-trips + DC-A2 equality + 12-variant
    discriminant-stability cardinality assertion); extended
    `test_all_variants_serde_roundtrip` with 5 new entries.
  - `relativist-core/src/protocol/mod.rs`: `mod delta_wire_tests;`
    (feature-agnostic integration tests) + `pub use crate::merge::
    {BorderDelta, LocalReconnection, MintedAgent, PendingCommutation};`
    so downstream callers can name all delta-wire structs via
    `crate::protocol::*`.
  - `relativist-core/src/protocol/delta_wire_tests.rs` (new, ~300 LoC):
    7 tokio tests covering framing round-trips for all 5 new variants
    — compression benefit (R35, >=1024 payload via
    `make_large_partition(2000)`), compression-threshold skip
    (`send_frame_with_threshold` with threshold above payload size),
    forced compression when threshold=1, and CRC-tamper rejection
    (accepts `ChecksumMismatch | DecompressionFailed | Serialize |
    Deserialize` families since tampering can fail LZ4 before CRC).
  - `relativist-core/src/protocol/frame.rs`: extended the R5
    `_exhaustive_check` canary with 5 new match arms and appended 5
    new fixture entries to `sample_all_message_variants`.
  - Key invariants honoured:
    - R19 pure-core: `merge/` still has no `use crate::protocol` lines;
      canary green.
    - R37 discriminant stability: variants 0-6 untouched; new variants
      7-11 are append-only; bincode v2 varint still encodes disc 0..=250
      as a single byte.
    - SPEC-18 R22: `FLAG_ARCHIVED` fast path remains restricted to
      `AssignPartition` / `PartitionResult`; all 5 new variants ride
      plain bincode.
  - Quality gates GREEN:
    - `cargo build --workspace` clean.
    - `cargo build --workspace --features zero-copy` clean.
    - `cargo test --workspace --lib` — **995** pass, 0 fail, 1 ignored
      (+27 over 968 baseline; floor +22 satisfied).
    - `cargo test --workspace --lib --features zero-copy` — **1035**
      pass, 0 fail, 1 ignored (+27 over 1008 baseline).
    - `cargo test --workspace` (incl. CLI integration) — all pass.
    - `cargo clippy --workspace --all-targets -- -D warnings` clean
      (default + `--features zero-copy`).
    - `cargo fmt --check` clean (after one `cargo fmt` pass to
      normalise the new `matches!(...)` and tuple-literal formatting).
    - `cargo build --release` clean; release smoke
      `target/release/relativist.exe compute add 3 5 → Result: 8`.
- [ ] **REVIEW** — pending. Awaiting human review before dispatching
  Stage 4 reviewer agent.
- [ ] **QA** — pending (Stage 5).
- [ ] **REFACTOR** — pending (Stage 6).

### Deviations / ambiguities

- **LZ4 benefit fixture size.** `make_large_partition(200)` was initially
  used for the "compression beneficial" tests but produced a payload
  near the LZ4 break-even (bincode 1217 B → compressed 1236 B). The
  fixture was bumped to 2000 agents, which yields a clear
  compression win and makes the R35 assertion deterministic across
  LZ4 implementations.
- **CRC-tamper error family.** Tampering the payload before the CRC
  check can fail in three places depending on which check trips
  first: LZ4 decompression (byte-level corruption of the compressed
  stream), CRC32C verification, or bincode deserialisation. The
  tamper test accepts the union `{ChecksumMismatch,
  DecompressionFailed, Serialize, Deserialize}` with a comment
  documenting why.
- **No commit.** Per the DEV stage contract for this bundle, the work
  is left uncommitted at the end of Stage 3 for human review. The
  working tree carries the full diff on branch `v2-development`.

### Acceptance criteria — SELF-VERIFIED

1. `cargo test --workspace --lib` — **995** pass, 0 fail (+27 over 968).
2. `cargo test --workspace --lib --features zero-copy` — **1035** pass,
   0 fail (+27 over 1008).
3. `cargo test --workspace` (unit + integration + doc) — all pass.
4. `cargo clippy --workspace --all-targets -- -D warnings` clean
   (default + zero-copy).
5. `cargo fmt --check` clean.
6. `cargo build --release` clean; `compute add 3 5` prints `Result: 8`.
7. R19 pure-core canary green (no `use crate::protocol` in `merge/`).
8. R37 discriminant stability: `test_message_discriminant_stability`
   asserts `cases.len() == 12` and byte-0 equals expected disc for
   every variant 0..=11.
9. `BorderDelta`, `LocalReconnection`, `PendingCommutation`,
   `MintedAgent` are reachable via `crate::protocol::*`.

### Stage 4 — REVIEW (2026-04-17)

**Agent:** reviewer (unified code-quality + architecture review)
**Output:** `docs/reviews/REVIEW-SPEC-19-section-3.4-item-2.26-A-2026-04-17.md`
**Verdict:** APPROVE — 0 MUST-FIX, 2 SHOULD-FIX (S1, S2), 4 NICE-TO-HAVE.

**Scope of review**

- Spec conformance per R31-R37 and R48 (coordinator-reserved AgentId range
  `u32::MAX - 10_000 .. u32::MAX`).
- R33 amendment coverage: DC-A1 (`BorderDelta` derives + re-export),
  DC-A2 (`RoundResult.has_border_activity` ↔ `stats.has_border_activity`
  equality invariant), DC-B3 (`LocalReconnection` for interior port
  rewires), DC-B5 (two-phase AgentId allocation via `PendingCommutation`
  request_id + `MintedAgent` echo).
- R19 pure-core canary: source-scan confirms `merge/` does NOT import
  `tokio`, `async_trait`, or `crate::protocol`.
- Test-spec coverage ledger: mapped every assertion in `delta_wire_tests.rs`
  and `types.rs` discriminant-stability test to its TEST-SPEC-0370/0371
  clause.

**SHOULD-FIX items (addressed in Stage 6 REFACTOR)**

- **S1.** `T8 final_state_result_crc_still_valid_no_tamper` was specified
  in TEST-SPEC-0370 but missing from the delivered file. Positive control
  complementary to T7 — proves the CRC path does not systematically
  reject legitimate frames.
- **S2.** Stale docstring on `make_large_partition`: claimed
  `n_agents = 200` exceeds 1024 bytes under bincode v2 varint, but all
  threshold-crossing callers (T1, T2, T7) now pass 2000.

**NICE-TO-HAVE items (declined — premature abstraction)**

- NICE-2 (extract `EXPECTED_VARIANT_COUNT` constant): the single-use
  literal `12` with an in-place comment is clearer than an indirection;
  adding the constant would not lower the cost of a future bump. Declined.
- NICE-3 (document tamper-test error-family rationale in code):
  already documented inline above `assert!(matches!(err, ...))`.

### Stage 5 — QA (2026-04-17)

**Agent:** qa (adversarial bug hunting)
**Probes:** 13 designed (Q1-Q13) / 14 implemented (Q13 split into Q13a
and Q13b, mutually exclusive via `#[cfg(not(feature = "zero-copy"))]`
and `#[cfg(feature = "zero-copy")]`).
**Location:** `relativist-core/src/protocol/delta_wire_tests.rs`,
`mod adversarial_probes`.

**Coverage**

| Probe | Surface | Expected outcome |
|-------|---------|-----------------|
| Q1 | Disc-byte tamper on uncompressed `RoundStart` | `ChecksumMismatch` |
| Q2 | Empty-payload byte-count floor | bincode_len == 7; frame_len == 16 |
| Q3 | Wire semantics preserved: `has_border_activity` vs `stats` | Equal on wire; canonical source = stats |
| Q4 | `PendingCommutation.arity = u8::MAX` | Lossless round-trip |
| Q5 | `LocalReconnection` with `u32::MAX` agent_id, `u8::MAX` port | Lossless round-trip |
| Q6 | `MintedAgent` inside R48-reserved range | Lossless round-trip (R48 is runtime-only; wire MUST NOT enforce) |
| Q7 | `BorderDelta AgentPort(u32::MAX, _)` ≠ `DISCONNECTED` | Distinct wire encoding |
| Q8 | `InitialPartition` below threshold | `FLAG_COMPRESSED` unset |
| Q9 | Threshold boundary: `>= semantics` | Exact threshold compresses; threshold+1 does not |
| Q10 | 10 000-entry `border_deltas` vector | Round-trips; exercises varint length path |
| Q11 | `FinalStateRequest { round: u32::MAX }` | Lossless round-trip (max varint length) |
| Q12 | Truncated frame (8 bytes < 9-byte header) | `ConnectionLost` |
| Q13a | FLAG_ARCHIVED on `RoundStart` (default features) | Falls through to bincode decode (R22 whitelist not enforced when `zero-copy` off) |
| Q13b | FLAG_ARCHIVED on `RoundStart` (zero-copy) | Either `Deserialize` or `ArchiveValidationFailed` — R22 whitelist rejects |

**Results**

- All 14 probes compile and pass on their respective feature-configs.
- Default config: 1008 lib tests (+13 probes over 995). Zero-copy
  config: 1048 lib tests (+13 probes over 1035).
- No probe surfaced a bug. Two probes (Q4, Q5) validated that the
  extreme-value encoding crosses bincode varint boundaries cleanly.

### Stage 6 — REFACTOR (2026-04-17)

**Agent:** developer (fix-in-place after REVIEW + QA)

**Changes**

- **S1.** Added T8 `final_state_result_crc_still_valid_no_tamper` as
  specified in TEST-SPEC-0370 §T8. Positive-control complement to T7.
  Fixture: `make_large_partition(200)` — does not require compression
  engagement.
- **S2.** Rewrote the `make_large_partition` docstring to reflect
  reality: callers that need compression pass 2000; smaller values
  (200, 10) are acceptable for tests that do not need the frame to
  cross the threshold.

**Acceptance criteria — SELF-VERIFIED**

1. `cargo test --workspace --lib` — **1009** pass, 0 fail (+1 T8 over Stage-5 1008).
2. `cargo test --workspace --lib --features zero-copy` — **1049** pass,
   0 fail (+1 T8 over Stage-5 1048).
3. `cargo clippy --workspace --all-targets -- -D warnings` clean (default + zero-copy).
4. `cargo fmt --check` clean.
5. `cargo build --release` clean; `./target/release/relativist.exe compute add 3 5` prints `Result: 8` (0.66 MIPS, 6 interactions, 29 agents, 1 redex).
6. No commit. Full diff on branch `v2-development` awaits human review.

---

## SPEC-19 §3.3 (item 2.26-B) — Coordinator-Side Border-Redex Resolution — DEV COMPLETE

**Bundle scope:** Coordinator's `border_resolver` module: `materialize_agent`, `assert_agent` (DC-B2), `resolve_border_redex` dispatcher + three same-symbol rule bodies (CON-CON, DUP-DUP, ERA-ERA), plus the asymmetric rule bodies (CON-DUP commutation via DC-B5, CON-ERA / DUP-ERA via DC-B6). `package_resolutions` fan-out to per-worker `RoundStartDispatch`, integration tests, and programmatic pure-core grep guard close the bundle. Spec-critic DC-B1..B9 verdicts at `docs/spec-reviews/SPEC-19-section-3.3-2.26B-design-choices-2026-04-17.md`.

**Tasks covered:**
- TASK-0372 — `materialize_agent` helper + module skeleton (+5 tests).
- TASK-0373 — `resolve_border_redex` dispatcher + CON-CON, DUP-DUP, ERA-ERA bodies + DC-B2 `assert_agent` panic (+6 tests).
- TASK-0374 — Asymmetric dispatch: CON-DUP commutation (DC-B5 2-phase flow) + CON-ERA/DUP-ERA erasure (DC-B6 preserve-via-apply_deltas). Adds `BorderIdAllocator`, `resolve_con_dup`, `resolve_non_era_era`, `emit_external_principal`, `emit_erasure_principal`, and `SLOT_MARKER_BASE` / `slot_marker` helpers (+6 tests).
- TASK-0375 — `RoundStartDispatch` + `package_resolutions`: folds a stream of `BorderResolution`s into per-worker dispatch payloads (DC-B3 split / DC-B5 `pending_commutations` fan / DC-B7 triple fan / R23 every-worker-each-round invariant); drops `pending_new_borders` (coordinator-only state) (+5 tests).
- TASK-0376 — End-to-end composition tests (UT-0376-01..07): 6 per-rule integration tests (CON-CON, DUP-DUP, ERA-ERA, CON-DUP, CON-ERA, DUP-ERA) each calling `resolve_border_redex` + `package_resolutions` back-to-back on 2-partition fixtures; plus 1 defensive `catch_unwind` double-resolve test validating the DC-B2 panic-not-silent-default contract (+7 tests).
- TASK-0377 — Programmatic R19 pure-core guard. DC-B8 factoring: shared scanner in `merge/internal/pure_core_guard.rs` (`FORBIDDEN_USE_PREFIXES: &[&str; 5]` + `assert_no_forbidden_imports(src, label)`), with `internal/` module `#[cfg(test)]`-gated since the helper is test-only. DC-B9 cardinality canary: single `#[test]` fn in `border_resolver.rs::tests` asserts the list has exactly 5 entries AND each of the 5 prefixes is present, then hands `include_str!("border_resolver.rs")` to the scanner (+1 test).

**Remaining tasks:** none — bundle 2.26-B DEV complete.

**Current test counts:** **1039** lib default (+30: 5 from TASK-0372 + 6 from TASK-0373 + 6 from TASK-0374 + 5 from TASK-0375 + 7 from TASK-0376 + 1 from TASK-0377) / **1079** lib `--features zero-copy` (+30, same set).

### Stage History (2.26-B)

- [x] **TASK-0372 DEV (2026-04-17)** — Module skeleton (docblock with SPEC cites + DC-B1/B2/B4 rulings), `materialize_agent`, 5 unit tests (principal-port live agent, non-principal slots, FreePort/DISCONNECTED, vacated agent slot, doc-block grep guard). Pure-core invariant preserved.
- [x] **TASK-0373 DEV (2026-04-18)** — `WorkerDeltas` + `BorderResolution` structs (DC-B3 split + DC-B7 resolved-borders triples). `assert_agent` with DC-B2 panic format (grep-able: `"border_resolver: agent missing for border {bid} on side {name}"` + `"cache desync"` + `"DC-B1"`). `resolve_border_redex` dispatcher routes on `(sym_a, sym_b)` and calls `graph.remove_border` post-resolution (R15 part 2). Asymmetric pairs `todo!()`-stubbed for TASK-0374. 3 same-symbol rule bodies: `resolve_con_con` (cross pattern `(a.1↔b.2, a.2↔b.1)`), `resolve_dup_dup` (parallel `(a.1↔b.1, a.2↔b.2)`), `resolve_era_era` (empty `worker_deltas`). 6 unit tests covering UT-0373-01..06 (cross, parallel, void, dispatcher normalization, targeted removal, DC-B2 panic-format substrings). `#[allow(dead_code)]` annotations on pub(crate) items pending TASK-0375 consumption (same precedent as `BorderGraph::worker_borders` R23 reverse index).
- [x] **TASK-0376 DEV (2026-04-18)** — End-to-end integration tests composing `resolve_border_redex` + `package_resolutions` on 2-partition fixtures. Reuses the existing `build_two_partition_same_symbol_fixture`, `build_two_partition_era_era_fixture`, `build_two_partition_asymmetric_fixture`, and `build_con_era_aux_border_fixture` helpers from `mod tests` (avoiding fixture duplication) instead of carving a new inner `mod integration_tests` with parallel builders — the TEST-SPEC-0376 "Resolved ambiguities" section permits either organizational path and the test IDs, names, and assertions are preserved verbatim. 7 new `#[test]` fns: `con_con_border_redex_end_to_end_resolves_and_packages` (UT-0376-01, Anni cross + DC-B3 + DC-B7 fan to both workers; asserts worker 0 keeps both local_reconnections per resolver convention, worker 1 empty), `dup_dup_border_redex_end_to_end_resolves_and_packages` (UT-0376-02, Anni parallel mirror), `era_era_border_redex_end_to_end_resolves_and_packages` (UT-0376-03, Void — empty worker_deltas, both workers receive `resolved_borders == [0]` and all other dispatch fields empty), `con_dup_border_redex_end_to_end_emits_pending_commutations` (UT-0376-04, DC-B5 2-phase: `r.pending_commutations.len() == 2`, each packaged worker gets exactly 1 batch with matching `batch.worker`, `new_borders.is_empty()`, `pending_new_borders.len() <= 4`), `con_era_border_redex_end_to_end_preserves_auxiliary_border` (UT-0376-05, DC-B6: border 0 removed, border 7 preserved — endpoint update threads through either `worker_deltas.border_deltas` OR `pending_new_borders`), `dup_era_border_redex_end_to_end_preserves_auxiliary_border` (UT-0376-06, mirror), `resolve_border_redex_on_absent_border_panics_per_dc_b2` (UT-0376-07, `catch_unwind` + `AssertUnwindSafe` on second resolve; payload string-matches `"border 0 not present"` or `"agent missing for border 0"`). No new fixture helpers added; no new types introduced; grep guard preserved.
- [x] **TASK-0375 DEV (2026-04-18)** — `RoundStartDispatch` struct (DC-B3 `local_reconnections` parallel to `border_deltas`; DC-B5 `new_borders: Vec<(u32, PortRef)>` concrete + `pending_commutations: Vec<CommutationBatch>` fan-out; DC-B7 `resolved_borders: Vec<u32>` collapsed from source triples). Derives `Debug, Clone, Default, PartialEq, Eq` (Eq required by UT-0375-05 full-Vec equality). `CommutationBatch` updated to derive `PartialEq, Eq` (transitive requirement for the dispatch struct's Eq). `package_resolutions(resolutions, num_workers) -> Vec<(WorkerId, RoundStartDispatch)>`: preallocates `num_workers` default dispatches (R23: every worker addressed each round even on empty payload); iterates each resolution folding `worker_deltas → per_worker[wid].{border_deltas, local_reconnections}`, `resolved_borders: (bid, wa, wb) → per_worker[wa] + per_worker[wb]` (self-border guard skips duplicate push when `wa == wb`), `new_borders: AddBorderEntry → per_worker[worker_a].new_borders.push((border_id, side_a)) + worker_b.push((border_id, side_b))` (same self-border guard), `pending_commutations: batch → per_worker[batch.worker]` (never duplicated — QA-0375-C regression). Output sorted `0..num_workers` ascending via `.enumerate()`; input-order preserved within each per-worker bucket. `pending_new_borders` INTENTIONALLY dropped (DC-B5: coordinator-only state pending round N+2). 5 unit tests (UT-0375-01..05) cover: empty input → 3 defaults with ascending keys; DC-B3 fan-out (worker 0 + 1 populated, worker 2 default); DC-B7 two-triple fan `[(0,0,1), (5,1,2)] → worker 1 gets [0,5]`; DC-B5 `pending_commutations` per-worker routing via `batch.worker`; determinism across two invocations with cloned inputs (byte-identical via `assert_eq!` on full `Vec<(WorkerId, RoundStartDispatch)>`). Pure-core invariant preserved.
- [x] **TASK-0374 DEV (2026-04-18)** — Asymmetric rule bodies. **CON-DUP (commutation, DC-B5 2-phase):** `resolve_con_dup` uses a balanced worker assignment — each worker mints 1 Dup + 1 Con into slots `[0, 1]`, producing 2 `CommutationBatch`es with `target_symbols == vec![Dup, Con]`. External principals (p.0↔a1, q.0↔a2, r.0↔b1, s.0↔b2) go through `emit_external_principal`: `AgentPort` targets become `BorderDelta`s on the home worker; `FreePort(bid)` targets become fresh `PendingNewBorder`s. Internal wires (p.1↔r.1, p.2↔s.1, q.1↔r.2, q.2↔s.2) use `SLOT_MARKER_BASE` (`R48` reserved range `u32::MAX - 10_000 .. u32::MAX`) to encode `PendingPortRef::Pending { request_id, agent_slot, port_slot }` sibling references. Two local internal wires (p.1↔r.1, q.2↔s.2) surface as `CommutationBatch.local_wiring`; two cross internal wires (p.2↔s.1, q.1↔r.2) surface as `PendingNewBorder`s with both sides `PendingPortRef::Pending`. `new_borders` is EMPTY at resolver time (round N+2 finalizes after `MintedAgent` echoes arrive). **CON-ERA / DUP-ERA (erasure, DC-B6):** `resolve_non_era_era` emits 1 `CommutationBatch` with `target_symbols == vec![Era, Era]` on the non-ERA worker. `emit_erasure_principal` classifies each auxiliary neighbour of the non-ERA agent: `AgentPort` → `local_wiring` hint; `FreePort(bid)` already in `graph.borders` → `PendingNewBorder` reusing the same `border_id` (DC-B6 preserve-via-apply_deltas); `FreePort(bid)` not in graph → fallback `PendingNewBorder` with side_b Concrete pointing at the ERA worker's DISCONNECTED slot. **New allocator:** `BorderIdAllocator::from_graph(&graph)` scans existing ids to avoid collision; `CommutationIdAllocator::new()` produces monotonic `CommutationId`s. Dispatcher reborrow pattern (`&*graph` inside match, `&mut graph` for `remove_border` after) preserves the pure-core contract. Module docstring extended with DC-B5 + DC-B6 paragraphs; grep guard extended with "DC-B5" and "DC-B6" needles. All 6 existing UT-0373-xx callers updated to declare + pass the two allocators. 6 new unit tests (UT-0374-01..06): CON-DUP pending_commutations shape (balanced Con+Dup counts per worker), PendingNewBorder placeholder-ref structural audit, CON-ERA preserve-border-7-via-apply_deltas, DUP-ERA mirror, back-to-back CON-DUP allocator uniqueness across 4 workers, asymmetric-order dispatcher symmetry across all 6 (sym_a, sym_b) pairs. Clippy `too_many_arguments` waived on 3 helpers (argument list dictated by protocol topology; bundling into struct would just displace signature to new type). Pure-core invariant preserved.
- [x] **TASK-0377 DEV (2026-04-18)** — Programmatic R19 pure-core guard. **Shared helper (DC-B8 option c):** new `merge/internal/` module (gated `#[cfg(test)]` since the guard is test-only — both `merge/mod.rs` and `merge/internal/mod.rs` carry the `#[cfg(test)]` attribute to keep the helper out of the non-test build and side-step `dead_code`). `internal/pure_core_guard.rs` exposes two pub(crate) items: the frozen `FORBIDDEN_USE_PREFIXES: &[&str; 5]` list (DC-B9: `use tokio`, `use async_trait`, `use crate::protocol`, `use crate::coordinator`, `use crate::worker`) and `assert_no_forbidden_imports(src, label)` which scans each line, trims leading whitespace, and on any `use `-prefixed line asserts no entry in the forbidden list is a prefix; failure message tags the offender with `R19 violation: {label} imports {prefix:?}` and cites SPEC-19 §3.2 R19 + DC-B9. **Opt-in site:** `border_resolver.rs::tests::border_resolver_pure_core_no_forbidden_imports` asserts `FORBIDDEN_USE_PREFIXES.len() == 5` (DC-B9 cardinality canary — narrowing without a fresh spec-critic verdict trips this first), asserts each of the 5 expected prefixes is present (drift canary), then hands `include_str!("border_resolver.rs")` to the scanner. **Scanner-in-source hazard:** an initial cardinality message that inlined the 5 prefix spellings across continuation lines was itself matched by the scanner (each continuation line, after `trim_start()`, started with `use crate::coordinator` inside the string literal). Fixed by rewording the message to refer callers to `merge/internal/pure_core_guard.rs` for the canonical list instead of inlining the spellings. Test cardinality +1 (UT-0377-01). Pure-core invariant now enforced programmatically — any future `use tokio|async_trait|crate::protocol|crate::coordinator|crate::worker` in `border_resolver.rs` fails CI loudly with an R19-tagged panic.

### Acceptance (after TASK-0377 — bundle 2.26-B DEV COMPLETE)

1. `cargo test --workspace --lib` — **1039** pass, 0 fail (+1 over TASK-0376 baseline 1038, +30 over bundle-start 1009).
2. `cargo test --workspace --lib --features zero-copy` — **1079** pass, 0 fail (+1 over TASK-0376, +30 over bundle-start 1049).
3. `cargo clippy --workspace --all-targets -- -D warnings` clean (default + zero-copy).
4. `cargo fmt --check` clean.
5. No commit. Bundle 2.26-B DEV complete; proceeding to bundle 2.26-C (coordinator wire-layer — `RoundStartDispatch` → `Message::RoundStart` emission + BSP loop) per user directive "full review at the end, when all features are implemented".

