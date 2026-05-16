//! RecipeEncoder trait: optional extension of `Encoder` for codecs that can
//! produce K independent "recipes" — small descriptions a worker can use to
//! generate its own partition locally, instead of receiving a chunk of an
//! already-built net.
//!
//! **Phase 6 mínimo (2026-04-16):** This module ships only R24 (trait
//! definition) and R25 (non-coupling with the `Codec` registry) from SPEC-27.
//! Integration with the coordinator/worker wire protocol (R26, R27, R28)
//! is deferred until SPEC-25 (Recipe-Based Distributed Generation,
//! ROADMAP item 2.29, milestone M7) is implemented. See
//! `docs/DEFERRED-WORK.md` row D-001 for the unblock checklist.
//!
//! **Design rationale.** `RecipeEncoder` is **not** part of the `Codec`
//! supertrait (R25). Codecs that do not implement `RecipeEncoder` fall back
//! to centralized generation — the coordinator builds the full net and ships
//! partitions. The registry stores `Box<dyn Codec>`; `RecipeEncoder` is
//! checked separately (via downcast or a parallel registry in a future
//! SPEC-25 integration). This keeps the registry storage object-safe while
//! still allowing decomposable codecs to opt in.

use serde::{de::DeserializeOwned, Serialize};

use crate::encoding::traits::{EncodeError, Encoder};
use crate::partition::Partition;

/// Optional extension of [`Encoder`] for codecs that can decompose a problem
/// into K independent recipes, one per worker.
///
/// A recipe is a small (bytes-to-kilobytes), self-contained, deterministic
/// description of a partition. The coordinator sends the recipe over the
/// wire; each worker materializes its partition locally via
/// [`Self::generate_partition`]. This sidesteps the "ship a huge net over
/// the wire" problem for encodings that can be generated in parallel.
///
/// **SPEC-27 R24, R25.**
///
/// Implementers must satisfy:
/// - [`Self::Recipe`] is `Serialize + DeserializeOwned + Send + Sync` (wire-ready).
/// - [`Self::make_recipes`] is deterministic: same `(input, num_workers)` → same recipes.
/// - [`Self::generate_partition`] is deterministic: same recipe → same partition.
/// - The union of partitions from all K recipes is observationally equivalent
///   (under IC reduction) to the net that `Encoder::encode(input)` would have
///   produced. This is a proof obligation, not a runtime check.
pub trait RecipeEncoder: Encoder {
    /// Recipe payload sent over the wire to each worker.
    /// Must be small, self-contained, and deterministic.
    type Recipe: Serialize + DeserializeOwned + Send + Sync;

    /// Whether this encoder supports recipe-based distributed generation.
    /// `false` means callers should fall back to centralized generation via
    /// [`Encoder::encode`] + [`crate::partition::split`].
    fn is_decomposable(&self) -> bool;

    /// Produce K recipes from the problem description and worker count.
    ///
    /// On success, `recipes.len() == num_workers as usize`.
    /// Returns [`EncodeError::InvalidInput`] if `num_workers == 0` or the
    /// input cannot be decomposed.
    fn make_recipes(
        &self,
        input: &[u8],
        num_workers: u32,
    ) -> Result<Vec<Self::Recipe>, EncodeError>;

    /// Generate a local partition from a single recipe.
    ///
    /// Workers call this after receiving an `AssignRecipe` message (future
    /// SPEC-27 R27 integration, deferred to M7).
    fn generate_partition(&self, recipe: &Self::Recipe) -> Result<Partition, EncodeError>;
}

// ---------------------------------------------------------------------------
// MinimalRecipeEncoder — demo implementation exercising the trait.
// ---------------------------------------------------------------------------

/// Demo recipe: one pair of (Con, Era) agents per slot.
///
/// Production recipes (e.g., for `ep_annihilation`) will carry an ID range,
/// a seed, and a symbol-distribution descriptor. This demo exists only to
/// prove the trait compiles, serializes, and can be implemented end-to-end.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MinimalRecipe {
    /// Worker index (0-based).
    pub worker_id: u32,
    /// Number of (Con, Era) pairs this worker should materialize.
    pub pairs: u32,
}

/// Minimal demo `RecipeEncoder` wired into Phase 6 tests only.
///
/// Accepts JSON `{"size": N}` on `encode`/`make_recipes`. Distributes
/// `N` (Con, Era) pairs evenly across workers; remainders go to the first
/// workers. Not registered in [`default_registry`](crate::encoding::default_registry)
/// because it has no domain value beyond trait exercise.
#[derive(Debug, Default, Clone, Copy)]
pub struct MinimalRecipeEncoder;

impl MinimalRecipeEncoder {
    pub fn new() -> Self {
        MinimalRecipeEncoder
    }

    fn parse_size(input: &[u8]) -> Result<u32, EncodeError> {
        #[derive(serde::Deserialize)]
        struct In {
            size: u32,
        }
        let parsed: In = serde_json::from_slice(input)
            .map_err(|e| EncodeError::InvalidInput(format!("expected {{\"size\":N}}: {}", e)))?;
        Ok(parsed.size)
    }
}

impl Encoder for MinimalRecipeEncoder {
    fn name(&self) -> &str {
        "minimal_recipe"
    }

    fn encode(&self, input: &[u8]) -> Result<crate::net::Net, EncodeError> {
        // Centralized fallback: build the full net (equivalent to concatenating
        // all recipes). Used when `is_decomposable()` is ignored.
        let size = Self::parse_size(input)?;
        let recipe = MinimalRecipe {
            worker_id: 0,
            pairs: size,
        };
        let p = self.generate_partition(&recipe)?;
        Ok(p.subnet)
    }
}

impl RecipeEncoder for MinimalRecipeEncoder {
    type Recipe = MinimalRecipe;

    fn is_decomposable(&self) -> bool {
        true
    }

    fn make_recipes(
        &self,
        input: &[u8],
        num_workers: u32,
    ) -> Result<Vec<Self::Recipe>, EncodeError> {
        if num_workers == 0 {
            return Err(EncodeError::InvalidInput("num_workers must be >= 1".into()));
        }
        let size = Self::parse_size(input)?;
        let base = size / num_workers;
        let remainder = size % num_workers;
        let mut out = Vec::with_capacity(num_workers as usize);
        for w in 0..num_workers {
            let pairs = base + if w < remainder { 1 } else { 0 };
            out.push(MinimalRecipe {
                worker_id: w,
                pairs,
            });
        }
        Ok(out)
    }

    fn generate_partition(&self, recipe: &Self::Recipe) -> Result<Partition, EncodeError> {
        use crate::net::{Net, PortRef, Symbol};
        use crate::partition::{IdRange, Partition};
        use std::collections::HashMap;

        let mut net = Net::new();
        // At least one (Con, Era) pair so validate_encoded_net's E2 passes
        // for codecs that want to reuse the net. For empty recipes we still
        // emit a single pair so the resulting Partition is non-empty.
        let pairs = recipe.pairs.max(1);
        for _ in 0..pairs {
            let c = net.create_agent(Symbol::Con);
            let e = net.create_agent(Symbol::Era);
            net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(e, 0));
            net.redex_queue.push_back((c, e));
        }
        Ok(Partition {
            subnet: net,
            worker_id: recipe.worker_id,
            free_port_index: HashMap::new(),
            id_range: IdRange {
                start: 0,
                end: u32::MAX,
            },
            border_id_start: 0,
            border_id_end: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // R2: demo is decomposable.
    #[test]
    fn minimal_encoder_is_decomposable() {
        let enc = MinimalRecipeEncoder::new();
        assert!(enc.is_decomposable());
    }

    // R3: make_recipes(input, K) returns exactly K recipes for multiple K values.
    #[test]
    fn make_recipes_returns_k_for_various_k() {
        let enc = MinimalRecipeEncoder::new();
        let input = br#"{"size":12}"#;
        for k in [1u32, 2, 4, 8] {
            let recipes = enc.make_recipes(input, k).expect("make_recipes");
            assert_eq!(recipes.len() as u32, k, "expected {} recipes", k);
            // Total pairs across recipes equals size (invariant).
            let total: u32 = recipes.iter().map(|r| r.pairs).sum();
            assert_eq!(total, 12);
        }
    }

    // R4: deterministic — same (input, K) yields identical recipes.
    #[test]
    fn make_recipes_is_deterministic() {
        let enc = MinimalRecipeEncoder::new();
        let input = br#"{"size":12}"#;
        let a = enc.make_recipes(input, 4).unwrap();
        let b = enc.make_recipes(input, 4).unwrap();
        assert_eq!(a, b);
    }

    // R5: K=0 is rejected.
    #[test]
    fn make_recipes_rejects_zero_workers() {
        let enc = MinimalRecipeEncoder::new();
        let res = enc.make_recipes(br#"{"size":12}"#, 0);
        assert!(matches!(res, Err(EncodeError::InvalidInput(_))));
    }

    // R6: generate_partition produces a non-empty Partition.
    #[test]
    fn generate_partition_is_non_empty() {
        let enc = MinimalRecipeEncoder::new();
        let recipes = enc.make_recipes(br#"{"size":6}"#, 3).unwrap();
        for r in &recipes {
            let p = enc.generate_partition(r).expect("generate_partition");
            assert!(
                p.subnet.count_live_agents() > 0,
                "partition for worker {} must have live agents",
                r.worker_id
            );
        }
    }

    // R7: recipes round-trip through serde (wire-readiness).
    #[test]
    fn recipes_roundtrip_through_serde() {
        let enc = MinimalRecipeEncoder::new();
        let recipes = enc.make_recipes(br#"{"size":10}"#, 2).unwrap();
        let bytes = serde_json::to_vec(&recipes[0]).unwrap();
        let back: MinimalRecipe = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back, recipes[0]);
    }

    // R8 / R25: RecipeEncoder is NOT a supertrait of Codec. The registry stores
    // `Box<dyn Codec>` — this line compiles only because Codec does not require
    // RecipeEncoder. If the trait hierarchy ever changes, this test fails to
    // compile, which is the intended signal.
    #[test]
    fn codec_does_not_require_recipe_encoder() {
        let c: Box<dyn crate::encoding::traits::Codec> =
            Box::new(crate::encoding::codec_church::ChurchArithmeticCodec::add());
        assert_eq!(c.name(), "church_add");
    }

    // Sanity: invalid JSON input is rejected with InvalidInput.
    #[test]
    fn make_recipes_rejects_invalid_json() {
        let enc = MinimalRecipeEncoder::new();
        let res = enc.make_recipes(b"not json", 2);
        assert!(matches!(res, Err(EncodeError::InvalidInput(_))));
    }

    // -----------------------------------------------------------------------
    // QA Stage 5 — adversarial cases.
    // -----------------------------------------------------------------------

    // QA1: num_workers > size → first `size` workers get 1 pair, rest get 0.
    // Must still return exactly K recipes (no early exit) and assign each
    // worker a unique worker_id.
    #[test]
    fn qa_more_workers_than_size() {
        let enc = MinimalRecipeEncoder::new();
        let recipes = enc.make_recipes(br#"{"size":3}"#, 5).unwrap();
        assert_eq!(recipes.len(), 5);
        let pairs: Vec<u32> = recipes.iter().map(|r| r.pairs).collect();
        assert_eq!(pairs, vec![1, 1, 1, 0, 0]);
        let ids: Vec<u32> = recipes.iter().map(|r| r.worker_id).collect();
        assert_eq!(ids, vec![0, 1, 2, 3, 4]);
    }

    // QA2: a recipe with pairs == 0 still yields a non-empty partition.
    // generate_partition floors to 1 pair so workers always have something to
    // reduce (avoids E2 invariant violation downstream).
    #[test]
    fn qa_zero_pair_recipe_still_non_empty() {
        let enc = MinimalRecipeEncoder::new();
        let recipe = MinimalRecipe {
            worker_id: 99,
            pairs: 0,
        };
        let p = enc.generate_partition(&recipe).unwrap();
        assert_eq!(p.worker_id, 99);
        assert_eq!(
            p.subnet.count_live_agents(),
            2,
            "floor-to-1 pair = 1 Con + 1 Era = 2 live agents"
        );
    }

    // QA3: encoder is Send + Sync (the trait bound says so; this is a
    // compile-time check that the struct actually satisfies it). Required
    // for future SPEC-25 use across worker threads.
    #[test]
    fn qa_encoder_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MinimalRecipeEncoder>();
        assert_send_sync::<MinimalRecipe>();
    }

    // QA4: encode (centralized fallback) produces a net that passes
    // validate_encoded_net. Proves the demo can be used both as a
    // recipe encoder AND as a regular Encoder transparently.
    #[test]
    fn qa_centralized_encode_passes_validation() {
        let enc = MinimalRecipeEncoder::new();
        let net = enc.encode(br#"{"size":4}"#).expect("encode");
        crate::encoding::traits::validate_encoded_net(&net).expect("validate");
    }

    // --- TASK-0719 / SPEC-27 v3 R24-R28 audit ---

    // UT-0719-01 (R24 audit): RecipeEncoder trait signature matches
    // SPEC-27 v3 §3.7. Compile-time check: a function bounded on the
    // trait must be able to accept `MinimalRecipeEncoder`, exercising
    // every required associated type and method shape.
    //
    // Per recipe.rs module-level rustdoc, R26/R27/R28 (refactor SPEC-25
    // generators, AssignRecipe encoder_name field, worker registry
    // dispatch) are deferred to the SPEC-25 (M7) milestone — they are
    // explicitly out-of-scope for SPEC-27 v3 v1 per "Phase 6 mínimo".
    // This task therefore covers R24 (trait shape) and R25 (fallback
    // path; exercised by the integration test below).
    #[test]
    fn recipe_encoder_trait_signature_audit() {
        fn requires_full_trait<E>(_e: &E)
        where
            E: RecipeEncoder + Send + Sync,
            E::Recipe: Serialize + DeserializeOwned + Send + Sync,
        {
        }
        let enc = MinimalRecipeEncoder::new();
        requires_full_trait(&enc);
    }

    // UT-0719-02 (R25 audit) / TASK-0721 MF-001 fix: HornerCodec does NOT
    // implement RecipeEncoder. Replaces the previous string-based probe
    // (which would have silently passed even after `impl RecipeEncoder for
    // HornerCodec` was added) with a compile-time negative trait bound via
    // `static_assertions::assert_not_impl_all!`. If anybody adds the impl
    // in the future, this module fails to compile, surfacing the change
    // loudly rather than missing it.
    //
    // Q4 in SPEC-27 v3 §8: Horner is sequential by construction;
    // recipe-based decomposition is future work, §5.2 PolynomialMultiEvalCodec.
    static_assertions::assert_not_impl_all!(
        crate::encoding::HornerCodec: RecipeEncoder
    );

    #[test]
    fn horner_codec_is_not_recipe_encoder() {
        // Runtime sentinel companion to the compile-time
        // `static_assertions::assert_not_impl_all!` above. The structural
        // witness is the compile-time bound; this test exists for
        // discoverability when grepping for `RecipeEncoder for HornerCodec`.
        // If the static_assertion is ever loosened, this test still serves
        // as a documentation breadcrumb.
        fn requires_recipe_encoder<T: RecipeEncoder>(_: T) {}
        // Intentionally NOT called — the compile-time `assert_not_impl_all`
        // above is what enforces the negative bound. If we tried to call
        // `requires_recipe_encoder(HornerCodec::new())` here, the file
        // would fail to compile when (and only when) `impl RecipeEncoder
        // for HornerCodec` exists. The static_assertions macro flips that
        // contract: compilation fails when the impl exists, succeeds otherwise.
        let _phantom: fn(MinimalRecipeEncoder) = requires_recipe_encoder;
    }
}
