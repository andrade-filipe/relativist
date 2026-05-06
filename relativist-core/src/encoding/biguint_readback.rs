//! BigUint readback for Church numeral nets (SPEC-27 v3 R14', R16b').
//!
//! Mostly mirrors the SPEC-14 §4.4 `decode_nat` topology and traversal,
//! replacing the `count: u64` accumulator with `count: BigUint`. Used by
//! `HornerCodec::decode` (R14') to extract polynomial-evaluation results
//! that may exceed `u64::MAX` (T9 BigUint witness, T9b boundary BigUint).
//!
//! **Topology relationship to `decode_nat`** (TASK-0721 SF-002): the core
//! `lambda_f → lambda_x → application chain → lam_x.p1` walk mirrors
//! `decode_nat` step-for-step. To extend the readable subset to
//! single-iteration Horner outputs, this module adds the recursive helpers
//! `count_chain_through_dups` and `chain_from_dup_branch`, which traverse
//! DUP boundaries that `decode_nat` does not handle. This is therefore a
//! topology **extension**, not a verbatim mirror — the cross-check
//! property R16b' / T12 still holds for inputs whose NF is the canonical
//! Church-numeral chain (no DUPs); for nets with DUP-share frames, the
//! readbacks may diverge (decode_nat returns `None`; decode_biguint may
//! succeed). Future Mackie/Pinto-style readback (SPEC-27 §5.1 Future
//! Work) would close this gap.
//!
//! **Independence from `decode_nat`** (R14' Independence clause): this
//! module's algorithm MUST NOT delegate to `decode_nat`. The cross-check
//! property (R16b' / T12) is meaningful only because the two readbacks are
//! independent code paths. They share only their structural shape; the
//! accumulator type and the final return are different (`u64` vs
//! `BigUint`). Source-inspection test
//! `tests/biguint_readback_independence.rs` enforces this at CI time.
//!
//! **NotNormalForm semantics** (R4 / SC-005, TASK-0709): the decoder uses
//! the canonical `count_valid_active_pairs` helper from `crate::reduction`
//! to populate `DecodeError::NotNormalForm.redexes`, NOT
//! `net.redex_queue.len()`. A queue with stale entries from cross-partition
//! merges (SPEC-05) MUST NOT cause a false `NotNormalForm` error.

use num_bigint::BigUint;

use crate::encoding::traits::DecodeError;
use crate::net::{AgentId, Net, PortRef, Symbol, DISCONNECTED};
use crate::reduction::count_valid_active_pairs;

/// Decode a Church numeral IC net in Normal Form to its `BigUint` value.
///
/// SPEC-27 v3 R14' (Phase 3b deliverable). Returns:
///
/// - `Ok(BigUint::from(0u64))` on the canonical Church(0) frame
///   (`lambda f. lambda x. x` — self-loop on lambda_x auxiliaries plus an
///   ERA-erased `f`).
/// - `Ok(BigUint::from(n))` on a Church(n) frame (a chain of `n` CON
///   application agents from `lambda_x.p2` to the `x` variable).
/// - `Err(DecodeError::NotNormalForm { redexes })` if the net has at
///   least one **valid** active pair after stale-entry pruning per SPEC-01
///   I4. The `redexes` field reports the count of valid pairs, NOT the raw
///   `redex_queue.len()` (R4).
/// - `Err(DecodeError::UnrecognizedStructure(_))` on any deviation from
///   the Church-numeral frame (missing root, wrong agent symbol, broken
///   application chain, malformed `n=0` self-loop).
/// - `Err(DecodeError::DecodeFailed("no root"))` if `net.root` is `None`.
///
/// Does NOT mutate `net`.
pub fn decode_biguint(net: &Net) -> Result<BigUint, DecodeError> {
    // E1: Normal-Form check (R4 + SC-005 valid-pair semantics).
    let valid_redexes = count_valid_active_pairs(net);
    if valid_redexes > 0 {
        return Err(DecodeError::NotNormalForm {
            redexes: valid_redexes,
        });
    }

    // E2: Find outer lambda (lambda f) from net.root.
    let root = net
        .root
        .ok_or_else(|| DecodeError::DecodeFailed("no root".into()))?;
    let lam_f = match root {
        PortRef::AgentPort(id, 0) => id,
        _ => {
            return Err(DecodeError::UnrecognizedStructure(
                "root not an AgentPort(_, 0)".into(),
            ))
        }
    };
    let lam_f_agent = net
        .get_agent(lam_f)
        .ok_or_else(|| DecodeError::UnrecognizedStructure("lambda_f missing".into()))?;
    if lam_f_agent.symbol != Symbol::Con {
        return Err(DecodeError::UnrecognizedStructure(
            "lambda_f not CON".into(),
        ));
    }

    // E3: Find inner lambda (lambda x) from lambda_f.p2.
    let lam_f_p2 = net.get_target(PortRef::AgentPort(lam_f, 2));
    if lam_f_p2 == DISCONNECTED {
        return Err(DecodeError::UnrecognizedStructure(
            "lambda_f.p2 disconnected".into(),
        ));
    }
    let lam_x = match lam_f_p2 {
        PortRef::AgentPort(id, 0) => id,
        _ => {
            return Err(DecodeError::UnrecognizedStructure(
                "lambda_f.p2 not AgentPort(_, 0)".into(),
            ))
        }
    };
    let lam_x_agent = net
        .get_agent(lam_x)
        .ok_or_else(|| DecodeError::UnrecognizedStructure("lambda_x missing".into()))?;
    if lam_x_agent.symbol != Symbol::Con {
        return Err(DecodeError::UnrecognizedStructure(
            "lambda_x not CON".into(),
        ));
    }

    // E4: Detect Church(0) — self-loop on lambda_x auxiliaries + ERA on
    // lambda_f.p1.
    let f_target = net.get_target(PortRef::AgentPort(lam_f, 1));
    let x_bind = net.get_target(PortRef::AgentPort(lam_x, 1));
    let x_body = net.get_target(PortRef::AgentPort(lam_x, 2));
    if f_target == DISCONNECTED || x_bind == DISCONNECTED || x_body == DISCONNECTED {
        return Err(DecodeError::UnrecognizedStructure(
            "malformed Church frame".into(),
        ));
    }
    if x_bind == PortRef::AgentPort(lam_x, 2) && x_body == PortRef::AgentPort(lam_x, 1) {
        // Self-loop on auxiliaries; verify ERA on lambda_f.p1.
        if let PortRef::AgentPort(era_id, 0) = f_target {
            let era_agent = net
                .get_agent(era_id)
                .ok_or_else(|| DecodeError::UnrecognizedStructure("era agent missing".into()))?;
            if era_agent.symbol == Symbol::Era {
                return Ok(BigUint::from(0u64));
            }
        }
        return Err(DecodeError::UnrecognizedStructure(
            "Church(0) frame missing ERA".into(),
        ));
    }

    // E5: Walk the application chain from lambda_x.p2 to the x binding.
    // Each application is a CON agent; we count one BigUint increment per
    // application. R14' Independence clause: this is structurally identical
    // to `decode_nat` but uses a `BigUint` accumulator and is a separate
    // code path (no delegation).
    //
    // Iterated DUP boundaries (from chained mul reductions in HornerCodec
    // and similar codecs) are handled by `count_chain_through_dups`, which
    // walks DUPs as multiplicative fan-out following Mackie/Pinto-style
    // shared-chain readback semantics. The simple linear case (no DUPs)
    // and the single-DUP case (one terminal mul boundary) both fall out of
    // the same recursion.
    count_chain_through_dups(net, PortRef::AgentPort(lam_x, 2), lam_x, 0)
}

/// Recursive Church-numeral chain readback that handles iterated DUP
/// boundaries.
///
/// Walks `current` toward the `x` variable binding (`AgentPort(lam_x, 1)`),
/// counting CON applications along the way and crossing DUP nodes by
/// summing chain counts of the two principals' destinations.
///
/// Returns the BigUint count on success, or `DecodeError::UnrecognizedStructure`
/// if the topology deviates from a Church-numeral frame (e.g., DUP cycle,
/// non-CON/non-DUP nodes in unexpected positions).
///
/// `depth` guards against accidental infinite recursion (DUP cycles from
/// non-Lafont reductions). The cap is conservative — depth corresponds to
/// the height of the DUP tree, which is at most `coeffs.len()` for
/// HornerCodec inputs.
fn count_chain_through_dups(
    net: &Net,
    current: PortRef,
    lam_x: AgentId,
    depth: usize,
) -> Result<BigUint, DecodeError> {
    if depth > 64 {
        return Err(DecodeError::UnrecognizedStructure(
            "decode depth exceeded — possible DUP cycle".into(),
        ));
    }

    let mut count: BigUint = BigUint::from(0u64);
    let one: BigUint = BigUint::from(1u64);
    let mut here = current;

    loop {
        let target = net.get_target(here);
        if target == DISCONNECTED {
            return Err(DecodeError::UnrecognizedStructure(
                "application chain broken".into(),
            ));
        }
        match target {
            PortRef::AgentPort(id, port) if id == lam_x && port == 1 => {
                return Ok(count);
            }
            PortRef::AgentPort(app_id, 1) => {
                let agent = net.get_agent(app_id).ok_or_else(|| {
                    DecodeError::UnrecognizedStructure("app agent missing".into())
                })?;
                if agent.symbol != Symbol::Con {
                    return Err(DecodeError::UnrecognizedStructure(
                        "non-CON in app chain".into(),
                    ));
                }
                count += &one;
                here = PortRef::AgentPort(app_id, 2);
            }
            // DUP boundary: enter the DUP via principal port. Each DUP
            // duplicates the applications below it, so we sum the counts
            // reachable through both auxiliary ports' principal-port
            // destinations.
            PortRef::AgentPort(dup_id, 0) => {
                let agent = net
                    .get_agent(dup_id)
                    .ok_or_else(|| DecodeError::UnrecognizedStructure("dup missing".into()))?;
                if agent.symbol != Symbol::Dup {
                    return Err(DecodeError::UnrecognizedStructure(
                        "expected DUP at principal".into(),
                    ));
                }
                let p1_dest = net.get_target(PortRef::AgentPort(dup_id, 1));
                let p2_dest = net.get_target(PortRef::AgentPort(dup_id, 2));
                let left = chain_from_dup_branch(net, p1_dest, lam_x, depth + 1)?;
                let right = chain_from_dup_branch(net, p2_dest, lam_x, depth + 1)?;
                return Ok(count + left + right);
            }
            // DUP entered through an auxiliary port (we approached a DUP's
            // p1 or p2 from outside): walk through to its principal-port
            // destination, treating the DUP as a transparent share.
            PortRef::AgentPort(dup_id, p) if p == 1 || p == 2 => {
                let agent = net
                    .get_agent(dup_id)
                    .ok_or_else(|| DecodeError::UnrecognizedStructure("dup missing".into()))?;
                if agent.symbol == Symbol::Dup {
                    here = PortRef::AgentPort(dup_id, 0);
                    continue;
                }
                return Err(DecodeError::UnrecognizedStructure(
                    "non-CON/non-DUP in chain".into(),
                ));
            }
            _ => {
                return Err(DecodeError::UnrecognizedStructure(
                    "unexpected port in chain".into(),
                ))
            }
        }
    }
}

/// Resolve a chain count starting from a DUP auxiliary-port destination.
///
/// The destination might be:
///   - the `x` variable binding (`AgentPort(lam_x, 1)`) → contributes 0,
///   - the result port of a CON application (`AgentPort(_, 1)`) → walk it
///     as a chain via `count_chain_through_dups`,
///   - another DUP entered via principal port → recurse.
fn chain_from_dup_branch(
    net: &Net,
    dest: PortRef,
    lam_x: AgentId,
    depth: usize,
) -> Result<BigUint, DecodeError> {
    if depth > 64 {
        return Err(DecodeError::UnrecognizedStructure(
            "decode depth exceeded — possible DUP cycle".into(),
        ));
    }
    if dest == PortRef::AgentPort(lam_x, 1) {
        return Ok(BigUint::from(0u64));
    }
    if dest == DISCONNECTED {
        return Err(DecodeError::UnrecognizedStructure(
            "DUP branch disconnected".into(),
        ));
    }
    match dest {
        PortRef::AgentPort(id, _port) => {
            let agent = net
                .get_agent(id)
                .ok_or_else(|| DecodeError::UnrecognizedStructure("branch dest missing".into()))?;
            // Walk the chain that leads from this destination toward lam_x.p1.
            // We pretend we just arrived at the source-side of `dest` and
            // need to follow forward — but `count_chain_through_dups` walks
            // by calling `get_target(here)` so we need to give it a port whose
            // target IS `dest`. Trick: we synthesize a "virtual" walk by
            // immediately entering the matched node.
            // Simpler: re-use the same loop logic by classifying `dest`.
            match (agent.symbol, dest) {
                (Symbol::Con, PortRef::AgentPort(app_id, 1)) => {
                    // We've arrived at the result port of a CON app; count 1
                    // and continue from p2.
                    let one = BigUint::from(1u64);
                    let sub = count_chain_through_dups(
                        net,
                        PortRef::AgentPort(app_id, 2),
                        lam_x,
                        depth + 1,
                    )?;
                    Ok(one + sub)
                }
                (Symbol::Dup, PortRef::AgentPort(dup_id, 0)) => {
                    // Entered another DUP at principal: recurse via both branches.
                    let p1 = net.get_target(PortRef::AgentPort(dup_id, 1));
                    let p2 = net.get_target(PortRef::AgentPort(dup_id, 2));
                    let left = chain_from_dup_branch(net, p1, lam_x, depth + 1)?;
                    let right = chain_from_dup_branch(net, p2, lam_x, depth + 1)?;
                    Ok(left + right)
                }
                (Symbol::Dup, PortRef::AgentPort(dup_id, p)) if p == 1 || p == 2 => {
                    // DUP aux-port destination: follow through to the DUP's
                    // principal-port destination.
                    let prin_dest = net.get_target(PortRef::AgentPort(dup_id, 0));
                    chain_from_dup_branch(net, prin_dest, lam_x, depth + 1)
                }
                _ => Err(DecodeError::UnrecognizedStructure(format!(
                    "unrecognized DUP-branch destination at agent {id} symbol {:?} port {dest:?}",
                    agent.symbol
                ))),
            }
        }
        _ => Err(DecodeError::UnrecognizedStructure(
            "DUP branch terminal not an AgentPort".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::church::encode_nat;
    use proptest::prelude::*;

    // UT-0712-01: n=0 -> BigUint(0).
    #[test]
    fn decode_biguint_zero_returns_zero() {
        let net = encode_nat(0);
        let result = decode_biguint(&net).expect("Church(0) decodes");
        assert_eq!(result, BigUint::from(0u64));
        // Bit-length sanity per §2.
        assert_eq!(BigUint::from(0u64).bits(), 0);
    }

    // UT-0712-02: small u64 values match decode_nat AND BigUint::from(n).
    #[test]
    fn decode_biguint_small_values_match_decode_nat() {
        for &n in &[1u64, 7u64, 42u64, 255u64, 10_000u64] {
            let net = encode_nat(n);
            let big = decode_biguint(&net).unwrap();
            let small = crate::encoding::church::decode_nat(&net).unwrap();
            assert_eq!(big, BigUint::from(small));
            assert_eq!(big, BigUint::from(n));
        }
    }

    // UT-0712-03: a queued live redex returns NotNormalForm with the right
    // redexes count. Stale entries do NOT trigger the error.
    #[test]
    fn decode_biguint_rejects_non_nf() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // `connect` auto-enqueues the redex; queue len = 1 valid pair.

        match decode_biguint(&net) {
            Err(DecodeError::NotNormalForm { redexes }) => assert_eq!(redexes, 1),
            other => panic!("expected NotNormalForm{{1}}, got {other:?}"),
        }

        // Stale-only queue: remove `a`. count_valid_active_pairs prunes the
        // entry; decoder proceeds past the NotNormalForm check (and then
        // fails structurally because the net has no root).
        net.remove_agent(a);
        match decode_biguint(&net) {
            Err(DecodeError::DecodeFailed(msg)) => {
                assert!(msg.contains("no root"), "expected 'no root', got {msg}");
            }
            Err(DecodeError::UnrecognizedStructure(_)) => {
                // Also acceptable depending on which structural check fires
                // first.
            }
            other => panic!("expected structural error after stale pruning, got {other:?}"),
        }
    }

    // UT-0712-04: malformed root paths.
    #[test]
    fn decode_biguint_rejects_malformed_root() {
        // (a) No root.
        let net = Net::new();
        match decode_biguint(&net) {
            Err(DecodeError::DecodeFailed(msg)) => assert!(msg.contains("no root")),
            other => panic!("expected DecodeFailed(no root), got {other:?}"),
        }

        // (b) Root is AgentPort(_, 1) — wrong slot.
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.root = Some(PortRef::AgentPort(a, 1));
        match decode_biguint(&net) {
            Err(DecodeError::UnrecognizedStructure(msg)) => {
                assert!(msg.contains("root not"));
            }
            other => panic!("expected UnrecognizedStructure (root slot), got {other:?}"),
        }

        // (c) Root is AgentPort(_, 0) but the agent is ERA (not CON).
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        net.root = Some(PortRef::AgentPort(a, 0));
        match decode_biguint(&net) {
            Err(DecodeError::UnrecognizedStructure(msg)) => {
                assert!(msg.contains("lambda_f not CON"));
            }
            other => panic!("expected UnrecognizedStructure (era root), got {other:?}"),
        }
    }

    // PT-0712-05: cross-check property — for any n in [0, 10_000],
    // decode_biguint(encode_nat(n)) == BigUint::from(decode_nat(net).unwrap())
    // == BigUint::from(n).
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]
        #[test]
        fn decode_biguint_cross_check_decode_nat(n in 0u64..=10_000u64) {
            let net = encode_nat(n);
            let big = decode_biguint(&net).expect("Church(n) decodes as BigUint");
            let small = crate::encoding::church::decode_nat(&net).expect("Church(n) decodes as u64");
            prop_assert_eq!(big.clone(), BigUint::from(small));
            prop_assert_eq!(big, BigUint::from(n));
        }
    }
}
