// IT-0719-04: SPEC-27 v3 R25 — HornerCodec falls back to centralized
// partition (T23). Encode a Horner input, reduce sequentially, then
// reduce via `run_grid` with W=2; assert the decoded values are equal.
//
// HornerCodec is NOT a `RecipeEncoder` (Q4 in §8): the coordinator
// generates the full net centrally and partitions it via SPEC-04 R25
// fallback. This test witnesses the fallback path empirically.
//
// **v1 readback gating** (cf. TASK-0714 / TASK-0715): multi-iteration
// Horner Normal Forms have nested DUPs that the v1 Church readbacks
// cannot fully traverse. We therefore use a constant-polynomial input
// (`coeffs.len() == 1`), where the encoded net is in Normal Form (no
// Horner loop runs) and decode_biguint succeeds. The `run_grid` path
// still partitions + merges the net, exercising the R25 fallback —
// the absence of redex work just means W=2 trivially produces the
// same NF as W=1 (sequential).
//
// For multi-iteration Horner inputs, the structural-isomorphism test
// in `horner_distributed_g1.rs` covers G1 directly without depending
// on a successful readback.

use relativist_core::encoding::{Decoder, Encoder, HornerCodec};
use relativist_core::merge::{run_grid, GridConfig};
use relativist_core::partition::ContiguousIdStrategy;
use relativist_core::reduction::reduce_all;

#[test]
fn horner_codec_centralized_fallback_matches_sequential() {
    let codec = HornerCodec::new();
    // Constant polynomial Horner input — readable post-reduce.
    let json = br#"{"coeffs":[42],"x":7}"#;

    // Sequential baseline.
    let mut net_seq = codec.encode(json).expect("encode");
    reduce_all(&mut net_seq);
    if net_seq.root.is_none() {
        relativist_core::encoding::discover_root(&mut net_seq);
    }
    let seq_value = codec.decode(&net_seq).expect("seq decode");

    // Centralized fallback via run_grid (W=2). HornerCodec does NOT
    // implement RecipeEncoder, so SPEC-04 R25 fallback applies: the
    // coordinator generates the full net (here, just the encoded one)
    // and partitions via the round-robin strategy.
    let net_dist = codec.encode(json).expect("encode");
    let config = GridConfig {
        num_workers: 2,
        max_rounds: None,
        ..GridConfig::default()
    };
    let (mut merged, _metrics) = run_grid(net_dist, &config, &ContiguousIdStrategy);
    if merged.root.is_none() {
        relativist_core::encoding::discover_root(&mut merged);
    }
    let dist_value = codec.decode(&merged).expect("dist decode");

    assert_eq!(
        seq_value, dist_value,
        "T23: HornerCodec centralized fallback (W=2) must produce the same decoded value as sequential"
    );
}
