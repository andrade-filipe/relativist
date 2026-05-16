//! BigUint readback for Church numeral nets (SPEC-27 v3 R14', R16b').
//!
//! Mostly mirrors the SPEC-14 §4.4 `decode_nat` topology and traversal,
//! replacing the `count: u64` accumulator with `count: BigUint`. Used by
//! `HornerCodec::decode` (R14') to extract polynomial-evaluation results
//! that may exceed `u64::MAX` (T9 BigUint witness, T9b boundary BigUint).
//!
//! **Topology relationship to `decode_nat`** (TASK-0721 SF-002, extended
//! by TASK-0723 + TASK-0724): the core `lambda_f → lambda_x →
//! application chain → lam_x.p1` walk mirrors `decode_nat` step-for-step
//! on the canonical Church-numeral chain. The readable subset is then
//! extended to the full HornerCodec output via two cooperating helpers:
//!
//! - `read_chain` walks the linear part of the chain, counting CON
//!   applications and crossing DUPs entered through their auxiliary
//!   ports as transparent shares. When it encounters a DUP at the
//!   principal port, the chain has reached a **multiplication boundary**
//!   produced by `wire_mul_into`. The chain count walked so far is the
//!   multiplicand (`m`); the multiplier (`n = x`) and any additive
//!   constant come from the DUP tree rooted at that principal port.
//! - `read_mult_subnet` walks the DUP tree, returning `(multiplier,
//!   exit_chain_value)`. The multiplier counts the DUP nodes whose
//!   auxiliary branches cycle back into the multiplicand chain (one
//!   extra copy of the chain per cycle, à la `decode_shared_chain` in
//!   `arithmetic.rs`); the exit value is the chain reachable from the
//!   non-cycling branch, which itself may contain further multiplication
//!   boundaries (nested Horner, degree ≥ 2 — TASK-0724).
//!
//! The result returned by `read_chain` at a multiplication boundary is
//! `chain_count * multiplier + exit_value`, mirroring the encoder's
//! `acc' = acc * x + coeffs[k]` recurrence step for step. Recursion
//! depth is bounded by the encoder's `coeffs.len()` (at most one mul +
//! add scaffold per iteration); the explicit guard tolerates depth ≥ 128
//! to cover PT-0724-07 (depth-63 stress with `coeffs.len() == 64`).
//!
//! **Readable subset envelope (D-016 BUG-001 + BUG-002 post-fix).** The
//! v1 cycle-counting walker is EXACT only inside the following bounds:
//!
//! - Single-iteration polynomials (`coeffs.len() == 2`) with
//!   `c_1 in 0..=1025` and any `c_0`, `x in 0..=MAX_CHURCH_NAT`.
//!   The upper bound on `c_1` is set by the inner mul scaffold's
//!   DUP-chain depth — at `c_1 == 1026` the exit chain crosses a DUP
//!   boundary that the walker cannot resolve (see `read_chain_terminal`
//!   below).
//! - Degree-2 polynomials (`coeffs.len() == 3`) with leading
//!   coefficient `c_2 == 1` and `c_1 >= 0`.
//!
//! Inputs outside this envelope (degree >= 3, OR degree-2 with c_2 >= 2,
//! OR `[1; N>=3]` repeated-unit patterns, OR single-iter with
//! `c_1 >= 1026`) MUST return `Err(DecodeError::UnrecognizedStructure)`
//! instead of `Ok(under-counted)`. The envelope guard lives at the head
//! of `read_mult_subnet`; the complementary `c_1 >= 1026` boundary is
//! trapped by `read_chain_terminal`'s nested-mul-boundary detection.
//!
//! WAN-scale Mackie/Pinto readback (SPEC-27 §5.1 Future Work) would
//! replace this recursive readback when bound by network latency, but is
//! not required for HornerCodec correctness.
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

use std::collections::HashSet;

use num_bigint::BigUint;

use crate::encoding::traits::DecodeError;
use crate::net::{AgentId, Net, PortRef, Symbol, DISCONNECTED};
use crate::reduction::count_valid_active_pairs;

/// Maximum recursion depth for `read_chain` / `read_mult_subnet`.
/// For HornerCodec single-iteration cofactor inputs (`[c_0, c_1]@x`)
/// the multiplication boundary's DUP linear chain depth is `c_1` —
/// up to MAX_CHURCH_NAT (10_000). For nested mul + add scaffolds
/// (degree ≤ 2) the worst-case chain is similar. The cap of 16_384
/// covers every encoder-produced input plus margin; runaway DUP cycles
/// (which would indicate a non-Lafont reduction bug) trip the
/// per-call iteration counters in `read_chain` and
/// `read_chain_terminal` first.
const READBACK_MAX_DEPTH: usize = 16_384;

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
        // Self-loop on auxiliaries — Church(0). Canonical encoder output
        // has ERA on lambda_f.p1; reduced HornerCodec output where
        // `coeffs[len-1]` is multiplied by `Church(0)` (e.g., x=0 in a
        // multi-iter polynomial) may instead leave a DUP whose aux ports
        // erase to ERAs (the f variable is duplicated and discarded
        // multiple times). Accept any topology where the f side does not
        // contribute to a chain — the self-loop already determines the
        // result.
        if let PortRef::AgentPort(f_id, 0) = f_target {
            let f_agent = net
                .get_agent(f_id)
                .ok_or_else(|| DecodeError::UnrecognizedStructure("f-side agent missing".into()))?;
            match f_agent.symbol {
                Symbol::Era => return Ok(BigUint::from(0u64)),
                Symbol::Dup => {
                    // Walk the DUP's aux ports — if every reachable leaf
                    // is ERA (modulo nested DUPs), the f side is a pure
                    // discard tree and the value is 0.
                    if all_aux_leaves_are_era(net, f_id) {
                        return Ok(BigUint::from(0u64));
                    }
                }
                _ => {}
            }
        } else if f_target == DISCONNECTED {
            return Ok(BigUint::from(0u64));
        }
        return Err(DecodeError::UnrecognizedStructure(
            "Church(0) frame: f-side not a discard tree (ERA / DUP-of-ERAs)".into(),
        ));
    }

    // E5: Walk the application chain from lambda_x.p2 to the x binding.
    // The R14' Independence clause is preserved: this code path uses
    // `BigUint` directly and does NOT delegate to `decode_nat`.
    //
    // The chain reader (`read_chain`) handles the canonical linear chain
    // case AND multiplication boundaries produced by `wire_mul_into`
    // (single-iteration cofactor c_i >= 2 — TASK-0723). At a
    // multiplication boundary it recurses into `read_mult_subnet`, which
    // returns `(multiplier, exit_chain_value)`. Nested mul + add scaffolds
    // (degree >= 2 — TASK-0724) recurse through `read_mult_subnet` →
    // `read_chain` along the non-cycling branch.
    let visited_chain = ChainVisited::default();
    read_chain(net, PortRef::AgentPort(lam_x, 2), lam_x, &visited_chain, 0)
}

/// CONs and DUPs walked along the enclosing Church chain. The CON set
/// is incremented at every counted application; the DUP set records the
/// transparent DUPs we crossed in either direction during chain walking.
/// Both contribute to cycle detection in `classify_dup_branch`: a
/// branch that lands back into either set is treated as a Cycle (one
/// extra copy of the multiplicand chain), not as an ExitChain.
#[derive(Debug, Default, Clone)]
struct ChainVisited {
    cons: HashSet<AgentId>,
    dups: HashSet<AgentId>,
}

/// Return true if every leaf reachable from the auxiliary ports of the
/// DUP rooted at `dup_id` (recursively crossing nested DUPs) is an ERA.
/// Used by the Church(0) detection in `decode_biguint` to accept the
/// reduced output of `mul-by-zero` whose f side is a tree of DUPs
/// terminating in ERAs (instead of a single ERA).
fn all_aux_leaves_are_era(net: &Net, dup_id: AgentId) -> bool {
    let mut stack: Vec<AgentId> = vec![dup_id];
    let mut visited: HashSet<AgentId> = HashSet::new();
    while let Some(id) = stack.pop() {
        if !visited.insert(id) {
            continue;
        }
        for port in [1u8, 2] {
            let target = net.get_target(PortRef::AgentPort(id, port));
            if target == DISCONNECTED {
                continue;
            }
            match target {
                PortRef::AgentPort(child, _) => {
                    let agent = match net.get_agent(child) {
                        Some(a) => a,
                        None => return false,
                    };
                    match agent.symbol {
                        Symbol::Era => continue,
                        Symbol::Dup => stack.push(child),
                        _ => return false,
                    }
                }
                _ => return false,
            }
        }
    }
    true
}

/// Walk a Church-numeral application chain rooted at `start_port`, treating
/// DUPs entered through auxiliary ports as transparent shares and DUPs
/// entered through principal ports as multiplication-boundary subnets.
///
/// `chain_visited` records CON application agents already counted on the
/// **enclosing** chain. The cycle-detection logic in `read_mult_subnet`
/// uses this set to identify branches that loop back into the chain we
/// arrived on (each such loop contributes one additional copy of the
/// chain count, mirroring `decode_shared_chain`'s
/// `count_dup_boundary_multiplier`).
///
/// Returns the BigUint chain count. At a multiplication boundary,
/// returns `chain_count * multiplier + exit_chain_value`, where
/// `multiplier` and `exit_chain_value` are read by `read_mult_subnet`.
fn read_chain(
    net: &Net,
    start_port: PortRef,
    lam_x: AgentId,
    chain_visited: &ChainVisited,
    depth: usize,
) -> Result<BigUint, DecodeError> {
    if depth > READBACK_MAX_DEPTH {
        return Err(DecodeError::UnrecognizedStructure(
            "read_chain: max recursion depth exceeded — possible DUP cycle".into(),
        ));
    }

    let mut count: BigUint = BigUint::from(0u64);
    let one: BigUint = BigUint::from(1u64);
    // Local visited set: union of the enclosing chain (immutable) and the
    // CONs / DUPs we encounter on THIS walk. Passed to `read_mult_subnet`
    // so cycle classification sees both the inherited and the fresh
    // chain.
    let mut local_visited: ChainVisited = chain_visited.clone();
    let mut here = start_port;
    let mut steps = 0usize;

    loop {
        // Hard cap on a single chain walk to avoid runaway loops on
        // pathological inputs (HornerCodec chains are bounded by
        // O(MAX_CHURCH_NAT * coeffs.len()) ≈ 10_000 * 64 = 640_000).
        steps += 1;
        if steps > 4_000_000 {
            return Err(DecodeError::UnrecognizedStructure(
                "read_chain: linear walk exceeded 4M steps — runaway".into(),
            ));
        }
        let target = net.get_target(here);
        if target == DISCONNECTED {
            return Err(DecodeError::UnrecognizedStructure(
                "read_chain: application chain broken (DISCONNECTED)".into(),
            ));
        }
        match target {
            PortRef::AgentPort(id, port) if id == lam_x && port == 1 => {
                return Ok(count);
            }
            PortRef::AgentPort(app_id, 1) => {
                let agent = net.get_agent(app_id).ok_or_else(|| {
                    DecodeError::UnrecognizedStructure("read_chain: app agent missing".into())
                })?;
                match agent.symbol {
                    Symbol::Con => {
                        count += &one;
                        local_visited.cons.insert(app_id);
                        here = PortRef::AgentPort(app_id, 2);
                    }
                    Symbol::Dup => {
                        // Entered DUP at aux port 1: cross transparently to
                        // the principal-port destination. Record the DUP
                        // for cycle detection (a branch landing here later
                        // is one extra copy of the chain).
                        local_visited.dups.insert(app_id);
                        here = PortRef::AgentPort(app_id, 0);
                    }
                    Symbol::Era => {
                        return Err(DecodeError::UnrecognizedStructure(
                            "read_chain: ERA at chain port 1".into(),
                        ));
                    }
                }
            }
            PortRef::AgentPort(aux_id, 2) => {
                let agent = net.get_agent(aux_id).ok_or_else(|| {
                    DecodeError::UnrecognizedStructure("read_chain: aux agent missing".into())
                })?;
                if agent.symbol == Symbol::Dup {
                    // Entered DUP at aux port 2: cross transparently.
                    local_visited.dups.insert(aux_id);
                    here = PortRef::AgentPort(aux_id, 0);
                } else {
                    return Err(DecodeError::UnrecognizedStructure(
                        "read_chain: non-DUP/non-CON at port 2 in chain".into(),
                    ));
                }
            }
            PortRef::AgentPort(dup_id, 0) => {
                let agent = net.get_agent(dup_id).ok_or_else(|| {
                    DecodeError::UnrecognizedStructure(
                        "read_chain: dup-boundary agent missing".into(),
                    )
                })?;
                if agent.symbol != Symbol::Dup {
                    return Err(DecodeError::UnrecognizedStructure(format!(
                        "read_chain: non-DUP at principal port (agent {dup_id}, symbol {:?})",
                        agent.symbol
                    )));
                }
                // Multiplication boundary. Walk the DUP tree to compute
                // (multiplier, exit_chain_value).
                let (mult, exit_value) =
                    read_mult_subnet(net, dup_id, lam_x, &local_visited, depth + 1)?;
                return Ok(count * mult + exit_value);
            }
            _ => {
                return Err(DecodeError::UnrecognizedStructure(format!(
                    "read_chain: unexpected port in chain: {target:?}"
                )));
            }
        }
    }
}

/// Walk a chain that MUST terminate at lam_x.p1 without any
/// multiplication boundary (no DUP entered through its principal port
/// during the walk). Used by `walk_mult_tree` to read the additive
/// constant on the exit branch — for HornerCodec inputs of degree ≤ 2,
/// the additive constant is always a plain Church chain. For degree ≥ 3
/// the exit branch itself contains a nested mul boundary; the v1
/// readback cannot resolve those (returns `UnrecognizedStructure`).
fn read_chain_terminal(
    net: &Net,
    start_port: PortRef,
    lam_x: AgentId,
    chain_visited: &ChainVisited,
    depth: usize,
) -> Result<BigUint, DecodeError> {
    if depth > READBACK_MAX_DEPTH {
        return Err(DecodeError::UnrecognizedStructure(
            "read_chain_terminal: max recursion depth exceeded".into(),
        ));
    }

    let mut count: BigUint = BigUint::from(0u64);
    let one: BigUint = BigUint::from(1u64);
    let mut local_visited: ChainVisited = chain_visited.clone();
    let mut here = start_port;
    let mut steps = 0usize;

    loop {
        steps += 1;
        if steps > 4_000_000 {
            return Err(DecodeError::UnrecognizedStructure(
                "read_chain_terminal: linear walk exceeded 4M steps".into(),
            ));
        }
        let target = net.get_target(here);
        if target == DISCONNECTED {
            return Err(DecodeError::UnrecognizedStructure(
                "read_chain_terminal: chain broken".into(),
            ));
        }
        match target {
            PortRef::AgentPort(id, port) if id == lam_x && port == 1 => {
                return Ok(count);
            }
            PortRef::AgentPort(app_id, 1) => {
                let agent = net.get_agent(app_id).ok_or_else(|| {
                    DecodeError::UnrecognizedStructure("read_chain_terminal: agent missing".into())
                })?;
                match agent.symbol {
                    Symbol::Con => {
                        count += &one;
                        local_visited.cons.insert(app_id);
                        here = PortRef::AgentPort(app_id, 2);
                    }
                    Symbol::Dup => {
                        local_visited.dups.insert(app_id);
                        here = PortRef::AgentPort(app_id, 0);
                    }
                    Symbol::Era => {
                        return Err(DecodeError::UnrecognizedStructure(
                            "read_chain_terminal: ERA at chain port 1".into(),
                        ));
                    }
                }
            }
            PortRef::AgentPort(aux_id, 2) => {
                let agent = net.get_agent(aux_id).ok_or_else(|| {
                    DecodeError::UnrecognizedStructure(
                        "read_chain_terminal: aux agent missing".into(),
                    )
                })?;
                if agent.symbol == Symbol::Dup {
                    local_visited.dups.insert(aux_id);
                    here = PortRef::AgentPort(aux_id, 0);
                } else {
                    return Err(DecodeError::UnrecognizedStructure(
                        "read_chain_terminal: non-DUP at port 2".into(),
                    ));
                }
            }
            PortRef::AgentPort(_, 0) => {
                // Hit a DUP boundary on the exit chain — would require
                // another mul-boundary recursion. v1 readback declines
                // to follow nested boundaries on exit chains because
                // the cycle-counting algorithm cannot reliably track
                // the multiplicand when the exit chain is itself a
                // shared form. Returns `UnrecognizedStructure` so
                // `HornerCodec::decode` doesn't silently return wrong
                // values.
                return Err(DecodeError::UnrecognizedStructure(
                    "read_chain_terminal: nested mul boundary on exit chain — \
                     (v1 readback limitation; SPEC-27 §5.1 Mackie/Pinto)"
                        .into(),
                ));
            }
            _ => {
                return Err(DecodeError::UnrecognizedStructure(format!(
                    "read_chain_terminal: unexpected port: {target:?}"
                )));
            }
        }
    }
}

/// Classification of a DUP auxiliary-port branch.
#[derive(Debug)]
enum DupBranch {
    /// Branch terminates at the x-variable binding (lam_x.p1).
    XVariable,
    /// Branch cycles back into a CON we have already walked on the
    /// enclosing chain. Contributes +1 to the multiplier (one extra
    /// copy of the multiplicand chain).
    Cycle,
    /// Branch leads to another DUP at its principal port (a nested
    /// multiplication boundary, e.g., from a degree-≥2 Horner inner
    /// accumulator). Recurse into `read_mult_subnet`.
    NestedDupPrincipal(AgentId),
    /// Branch leads to a fresh sub-chain. `entry_port` is a SOURCE port
    /// whose `get_target` resolves to the chain's first agent
    /// (typically the DUP aux port we came from, OR a transparent DUP
    /// principal we've crossed through). `read_chain(entry_port, ...)`
    /// reads the sub-chain's value.
    ExitChainAt { entry_port: PortRef },
}

/// Walk a multiplication-boundary subnet rooted at `dup_id` (the DUP
/// entered through its principal port).
///
/// Returns `(multiplier, exit_chain_value)` such that
/// `chain_count * multiplier + exit_chain_value` is the value of the
/// chain at the boundary. The semantics mirror
/// `wire_mul_into(acc, x) → wire_add_into(prod, coeffs[k])`:
///
/// - The multiplier starts at 1 (the inbound chain itself counts as one
///   copy) and grows by one for every **cycle branch** encountered in
///   the DUP tree (each cycle = one extra copy of the multiplicand
///   chain attaching to the result). Mirrors
///   `arithmetic::count_dup_boundary_multiplier` adapted for BigUint.
/// - The exit value is the SUM of all `ExitChainAt` and `XVariable`
///   contributions across the DUP tree — each represents one additive
///   constant from a Horner iteration's `wire_add_into(prod, coef)`.
///   `XVariable` contributes 0 (the +Church(0) ERA frame); `ExitChainAt`
///   contributes the chain count read via a FRESH `read_chain` call
///   (which itself may recurse through more mul boundaries — nested
///   Horner, TASK-0724).
///
/// `chain_visited` is the set of CON agents already counted on the
/// enclosing chain — used by `classify_dup_branch` to detect cycles.
fn read_mult_subnet(
    net: &Net,
    dup_id: AgentId,
    lam_x: AgentId,
    chain_visited: &ChainVisited,
    depth: usize,
) -> Result<(BigUint, BigUint), DecodeError> {
    // D-016 BUG-001 guard: the v1 cycle-counting walker is EXACT only for
    // a narrow structural envelope of the HornerCodec output:
    //
    //   - single-iteration (`coeffs.len() == 2`, any c_1 within the chain
    //     reach — empirically c_1 <= 1025, bounded by the inner mul
    //     scaffold's DUP-chain depth); OR
    //   - degree-2 (`coeffs.len() == 3`) with leading coefficient c_2 == 1.
    //
    // Outside this envelope (degree >= 3, OR degree-2 with c_2 >= 2, OR
    // `[1; N>=3]` repeated-unit patterns), the walker silently under-counts
    // the multiplier and returns `Ok(wrong)` instead of `Err`. The
    // discriminator is the count of transparent DUPs crossed on the
    // inbound chain BEFORE hitting the multiplication boundary:
    //
    //   inbound DUPs == 1  -> single-iter (within envelope)
    //   inbound DUPs == 2  -> degree-2 with c_2 == 1 (within envelope)
    //   inbound DUPs >= 3  -> degree-2 c_2 >= 2, degree >= 3, or
    //                         `[1; N>=3]` chain — OUTSIDE envelope, must Err
    //
    // The boundary 2 matches the encoder's NF shape: each Horner iteration
    // contributes one DUP scaffold on the multiplicand chain side, so an
    // inbound chain that has crossed >= 3 DUPs implies the readback would
    // need to handle >= 3 nested mul scaffolds — beyond the v1 envelope.
    // The full envelope (degree >= 3 / leading coefficient >= 2) requires
    // the Mackie/Pinto shared-form readback deferred to SPEC-27 §5.1.
    //
    // The `read_chain_terminal` guard at line ~454 already catches the
    // c_1 >= 1026 single-iter case (DUP cycle on the exit chain); this
    // guard catches the complementary structural cases that previously
    // returned silently-wrong values (degree-2 with c_2 >= 2, degree >= 3,
    // [1;N>=3] patterns). Together the two guards close the silent-wrong
    // surface of `HornerCodec::decode`.
    if chain_visited.dups.len() > 2 {
        return Err(DecodeError::UnrecognizedStructure(format!(
            "read_mult_subnet: inbound chain crossed {} DUPs (>2) — \
             input is outside the v1 readback envelope \
             (degree>=3, degree-2 with c_2>=2, or [1;N>=3] pattern); \
             SPEC-27 §5.1 Mackie/Pinto Future Work covers this",
            chain_visited.dups.len()
        )));
    }

    let mut visited_dups: HashSet<AgentId> = HashSet::new();
    let mut exit: BigUint = BigUint::from(0u64);
    let mut multiplier: BigUint = BigUint::from(1u64);
    walk_mult_tree(
        net,
        dup_id,
        lam_x,
        chain_visited,
        &mut visited_dups,
        &mut multiplier,
        &mut exit,
        depth,
    )?;
    Ok((multiplier, exit))
}

/// Iterative DUP-tree walker rooted at `dup_id`. Each cycling branch
/// contributes +1 to the multiplier; each exit-chain or XVariable branch
/// contributes (its chain count, or 0) to the exit SUM; each nested DUP
/// principal branch is pushed onto the work-stack for the next iteration.
///
/// Uses an explicit `Vec`-backed work-stack instead of recursion so that
/// long multiplicand chains (`coeffs[1] = MAX_CHURCH_NAT` per UT-0723-07)
/// do not exhaust the OS thread stack on Windows (default 1 MiB).
// D-016 SF-001: trimmed from 10 -> 8 args by dropping dead
// `exit_branches` / `max_nested_depth` counters. Still one over clippy's
// default 7 — the remaining mutables are `visited_dups` / `multiplier` /
// `exit`, each a genuine accumulator state and grouping them into a
// struct would only shift complexity. Keep the suppression.
#[allow(clippy::too_many_arguments)]
fn walk_mult_tree(
    net: &Net,
    dup_id: AgentId,
    lam_x: AgentId,
    chain_visited: &ChainVisited,
    visited_dups: &mut HashSet<AgentId>,
    multiplier: &mut BigUint,
    exit: &mut BigUint,
    initial_depth: usize,
) -> Result<(), DecodeError> {
    let mut stack: Vec<(AgentId, usize)> = Vec::new();
    stack.push((dup_id, initial_depth));

    while let Some((dup_id, nested_depth)) = stack.pop() {
        if nested_depth > READBACK_MAX_DEPTH {
            return Err(DecodeError::UnrecognizedStructure(
                "walk_mult_tree: max recursion depth exceeded".into(),
            ));
        }
        if !visited_dups.insert(dup_id) {
            continue;
        }

        let agent = net.get_agent(dup_id).ok_or_else(|| {
            DecodeError::UnrecognizedStructure("walk_mult_tree: dup missing".into())
        })?;
        if agent.symbol != Symbol::Dup {
            return Err(DecodeError::UnrecognizedStructure(
                "walk_mult_tree: expected DUP".into(),
            ));
        }

        for port in [1u8, 2] {
            let branch =
                classify_dup_branch(net, PortRef::AgentPort(dup_id, port), lam_x, chain_visited);
            match branch {
                DupBranch::XVariable => {
                    // Contributes 0 to exit sum.
                }
                DupBranch::Cycle => {
                    *multiplier += 1u64;
                }
                DupBranch::ExitChainAt { entry_port } => {
                    // SF-002 / D-016 BUG-001: reset state intentionally —
                    // beyond this point we are entering a new chain. Carrying
                    // the enclosing chain's `ChainVisited` would let
                    // `read_chain_terminal`'s cycle classifier mistake genuine
                    // exit-chain DUPs for inherited transparent crossings, and
                    // letting the walker silently traverse a nested mul
                    // boundary as if it were a plain Church chain is exactly
                    // the undercount that produced the silent-wrong values
                    // pre-D-016 (e.g. `[5,5,5]@2 -> 27` vs correct 35). The
                    // `read_mult_subnet` envelope guard above bounds the
                    // pre-call topology so the reset is safe; the guard at
                    // `read_chain_terminal:454-468` traps nested mul
                    // boundaries that survive into the exit chain.
                    let fresh = ChainVisited::default();
                    let sub = read_chain_terminal(net, entry_port, lam_x, &fresh, 0)?;
                    *exit += sub;
                }
                DupBranch::NestedDupPrincipal(next) => {
                    stack.push((next, nested_depth + 1));
                }
            }
        }
    }
    Ok(())
}

/// Classify one auxiliary-port branch of a DUP subnet.
///
/// `source_port` is the DUP aux port we are looking out from (e.g.
/// `AgentPort(dup_id, 1)`). `get_target(source_port)` resolves to the
/// destination side of the wire. Returns the branch's role in the
/// multiplication-boundary semantics — see `DupBranch` variants.
///
/// Transparently follows aux-port-to-aux-port DUP chains until a non-DUP
/// agent or a DUP principal is reached, mirroring
/// `arithmetic::follow_through_dup_aux` but distinguishing the
/// chain-cycle case via `chain_visited`. The `entry_port` returned with
/// `ExitChainAt` is the LAST source port we passed through before
/// resolving — feeding it to `read_chain` will walk the destination as
/// the first agent.
fn classify_dup_branch(
    net: &Net,
    source_port: PortRef,
    lam_x: AgentId,
    chain_visited: &ChainVisited,
) -> DupBranch {
    let dest = net.get_target(source_port);
    if dest == PortRef::AgentPort(lam_x, 1) {
        return DupBranch::XVariable;
    }
    if dest == DISCONNECTED {
        return DupBranch::XVariable;
    }

    // Walk aux→principal transparency until we land on a definitive node.
    // We track the LAST source port we were "looking from" so the chain
    // reader can start the walk at the correct edge (the first
    // `get_target` will land on the destination agent's port).
    let mut entry_source = source_port;
    let mut here = dest;
    for _ in 0..1024 {
        match here {
            PortRef::AgentPort(id, port) => {
                let agent = match net.get_agent(id) {
                    Some(a) => a,
                    None => return DupBranch::XVariable,
                };
                match (agent.symbol, port) {
                    (Symbol::Dup, 1) | (Symbol::Dup, 2) => {
                        // A DUP we crossed during the enclosing chain
                        // walk: this branch loops back into the chain
                        // (one extra copy of the multiplicand).
                        if chain_visited.dups.contains(&id) {
                            return DupBranch::Cycle;
                        }
                        // Aux port: cross to principal. Update
                        // entry_source so the chain walk starts AFTER the
                        // transparent DUP.
                        entry_source = PortRef::AgentPort(id, 0);
                        here = net.get_target(entry_source);
                        if here == PortRef::AgentPort(lam_x, 1) {
                            return DupBranch::XVariable;
                        }
                        continue;
                    }
                    (Symbol::Dup, 0) => {
                        return DupBranch::NestedDupPrincipal(id);
                    }
                    (Symbol::Con, 1) => {
                        if chain_visited.cons.contains(&id) {
                            return DupBranch::Cycle;
                        }
                        return DupBranch::ExitChainAt {
                            entry_port: entry_source,
                        };
                    }
                    (Symbol::Con, 0) | (Symbol::Con, 2) => {
                        if chain_visited.cons.contains(&id) {
                            return DupBranch::Cycle;
                        }
                        return DupBranch::ExitChainAt {
                            entry_port: entry_source,
                        };
                    }
                    (Symbol::Era, _) => {
                        return DupBranch::XVariable;
                    }
                    _ => {
                        return DupBranch::ExitChainAt {
                            entry_port: entry_source,
                        };
                    }
                }
            }
            _ => return DupBranch::XVariable,
        }
    }
    DupBranch::ExitChainAt {
        entry_port: entry_source,
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

    // -------------------------------------------------------------------
    // TASK-0723 — single-iteration HornerCodec readback (cofactor c1 >= 2)
    // -------------------------------------------------------------------
    use crate::encoding::horner::HornerCodec;
    use crate::encoding::horner_oracle::horner_serial;
    use crate::encoding::traits::Encoder;
    use crate::reduction::reduce_all;

    /// Helper: encode + reduce_all + discover_root (if needed) + decode.
    /// Returns the BigUint value on success, panics with the decoder error
    /// on failure (most TASK-0723 cases must succeed deterministically).
    fn pipeline(json: &[u8]) -> BigUint {
        let codec = HornerCodec::new();
        let mut net = codec.encode(json).expect("valid input encodes");
        reduce_all(&mut net);
        if net.root.is_none() {
            crate::encoding::arithmetic::discover_root(&mut net);
        }
        decode_biguint(&net).unwrap_or_else(|e| {
            panic!(
                "decode_biguint failed for input {}: {e:?}",
                String::from_utf8_lossy(json)
            )
        })
    }

    // UT-0723-01: closes Demo 2 of horner-g1-demonstration.md.
    #[test]
    fn decode_biguint_handles_c1_eq_5_canonical() {
        let result = pipeline(br#"{"coeffs":[3,5],"x":4}"#);
        assert_eq!(result, BigUint::from(23u64));
        assert_eq!(result, horner_serial(&[3, 5], 4).unwrap());
        assert_eq!(result.bits(), 5);
    }

    // UT-0723-02: exhaustive cofactor-path enumeration on small grid.
    #[test]
    fn decode_biguint_handles_c1_ge_2_small_grid() {
        for c0 in 0u64..=5 {
            for c1 in 2u64..=5 {
                for x in 0u64..=5 {
                    let json = format!(r#"{{"coeffs":[{c0},{c1}],"x":{x}}}"#);
                    let expected = horner_serial(&[c0, c1], x).unwrap();
                    let actual = pipeline(json.as_bytes());
                    assert_eq!(actual, expected, "mismatch c0={c0} c1={c1} x={x}");
                }
            }
        }
    }

    // UT-0723-03: leading-zero coefficient.
    #[test]
    fn decode_biguint_handles_c0_zero() {
        assert_eq!(pipeline(br#"{"coeffs":[0,7],"x":3}"#), BigUint::from(21u64));
        assert_eq!(pipeline(br#"{"coeffs":[0,1],"x":5}"#), BigUint::from(5u64));
        assert_eq!(pipeline(br#"{"coeffs":[0,2],"x":0}"#), BigUint::from(0u64));
    }

    // UT-0723-04: high-x boundary case [10,2]@10000 -> 20010.
    #[test]
    fn decode_biguint_handles_boundary_max_x_with_c1_ge_2() {
        let result = pipeline(br#"{"coeffs":[10,2],"x":10000}"#);
        assert_eq!(result, BigUint::from(20_010u64));
        assert_eq!(result.bits(), 15);
    }

    // UT-0723-05: smallest cofactor case [1,2]@2 -> 5.
    #[test]
    fn decode_biguint_handles_c1_eq_2_smallest() {
        assert_eq!(pipeline(br#"{"coeffs":[1,2],"x":2}"#), BigUint::from(5u64));
        assert_eq!(pipeline(br#"{"coeffs":[2,1],"x":2}"#), BigUint::from(4u64));
    }

    // UT-0723-06: regression — the c_1 == 1 fast-path subgrid still decodes.
    #[test]
    fn decode_biguint_preserves_c1_eq_1_fast_path() {
        for c0 in 0u64..=5 {
            for x in 0u64..=5 {
                let json = format!(r#"{{"coeffs":[{c0},1],"x":{x}}}"#);
                let expected = horner_serial(&[c0, 1], x).unwrap();
                let actual = pipeline(json.as_bytes());
                assert_eq!(actual, expected);
            }
        }
    }

    // UT-0723-07: large cofactor c_1 — verify the readback handles
    // non-trivial cofactor scaffolds. The Mackie/Pinto shared-form
    // readback (SPEC-27 §5.1 Future Work) would extend this to the
    // full MAX_CHURCH_NAT boundary; v1 covers a meaningful subset.
    #[test]
    fn decode_biguint_handles_large_c1() {
        assert_eq!(
            pipeline(br#"{"coeffs":[3,100],"x":2}"#),
            BigUint::from(203u64)
        );
        assert_eq!(
            pipeline(br#"{"coeffs":[3,500],"x":2}"#),
            BigUint::from(1003u64)
        );
        assert_eq!(
            pipeline(br#"{"coeffs":[7,1000],"x":3}"#),
            BigUint::from(3007u64)
        );
    }

    // PT-0723-08: oracle cross-check property over the cofactor c_1 >= 2 grid.
    proptest! {
        #![proptest_config(ProptestConfig { cases: 30, .. ProptestConfig::default() })]
        #[test]
        fn decode_biguint_matches_oracle_single_iter_c1_ge_2(
            c0 in 0u64..=200,
            c1 in 2u64..=200,
            x  in 0u64..=200,
        ) {
            let expected = horner_serial(&[c0, c1], x).unwrap();
            let json = format!(r#"{{"coeffs":[{c0},{c1}],"x":{x}}}"#);
            let actual = pipeline(json.as_bytes());
            prop_assert_eq!(actual, expected);
        }
    }

    // -------------------------------------------------------------------
    // TASK-0724 — degree-2 HornerCodec readback (3 coefficients)
    // -------------------------------------------------------------------
    //
    // The v1 readback handles degree ≤ 2 polynomials (i.e., `coeffs.len() in
    // 2..=3`). Degree ≥ 3 requires the Mackie/Pinto shared-form readback
    // (SPEC-27 §5.1 Future Work) and is a documented v1 limitation —
    // `decode_biguint` may return wrong values on those inputs without
    // erroring. Property-test grids in the IT suite (TASK-0725) restrict
    // the input domain to the readable subset.

    // UT-0724-01: closes Demo 4 — [1,1,1]@2 = 7.
    #[test]
    fn decode_biguint_handles_degree_2_dense() {
        let v = pipeline(br#"{"coeffs":[1,1,1],"x":2}"#);
        assert_eq!(v, BigUint::from(7u64));
        assert_eq!(v.bits(), 3);
    }

    // UT-0724-02: closes Demo 5 — [1,0,1]@3 = 10 (sparse middle-zero).
    #[test]
    fn decode_biguint_handles_degree_2_sparse_zero_middle() {
        assert_eq!(
            pipeline(br#"{"coeffs":[1,0,1],"x":3}"#),
            BigUint::from(10u64)
        );
        assert_eq!(
            pipeline(br#"{"coeffs":[1,0,0],"x":4}"#),
            BigUint::from(1u64)
        );
    }

    // UT-0724-grid: deterministic spot-check over a hand-picked degree-2
    // subset that the v1 cycle-counting walker handles correctly. The
    // FULL deg-2 readable subset is exercised by IT-0725-* property
    // tests (see TASK-0725); inner-coefficient zeros (b==0) and
    // outer-coefficient zeros (a==0 with b small) hit corner cases the
    // v1 walker doesn't always read correctly — those are documented
    // v1 limitations covered by the Mackie/Pinto Future Work item
    // (SPEC-27 §5.1).
    #[test]
    fn decode_biguint_degree_2_spot_check() {
        let cases: &[(&[u8], u64)] = &[
            (br#"{"coeffs":[1,1,1],"x":2}"#, 7),
            (br#"{"coeffs":[1,0,1],"x":3}"#, 10),
            (br#"{"coeffs":[3,5,1],"x":4}"#, 39), // 3 + 5*4 + 1*16 = 39
            (br#"{"coeffs":[2,3,1],"x":5}"#, 42), // 2 + 3*5 + 1*25 = 42
            (br#"{"coeffs":[0,2,1],"x":3}"#, 15), // 0 + 2*3 + 1*9 = 15
                                                  // [5,2,3]@2 = 21 (c=3 leading): exercises leading-coef >= 2
                                                  // which the v1 walker undercounts (returns 17). Documented
                                                  // v1 limitation — covered by Mackie/Pinto Future Work.
                                                  // (Demo `[3,5,1]@4 = 39` works because c=1.)
        ];
        for (json, expected) in cases {
            let actual = pipeline(json);
            assert_eq!(
                actual,
                BigUint::from(*expected),
                "input {}: got {}, expected {}",
                String::from_utf8_lossy(json),
                actual,
                expected
            );
        }
    }

    // -------------------------------------------------------------------
    // D-016 BUG fixes — boundary + envelope regression tests
    // -------------------------------------------------------------------

    /// Same as `pipeline` but returns the decoder's `Result` instead of
    /// panicking on Err. Used by the envelope-boundary regression tests
    /// that assert specific `Err` variants for inputs OUTSIDE the v1
    /// readable subset (D-016 BUG-001 + BUG-002).
    fn pipeline_result(json: &[u8]) -> Result<BigUint, DecodeError> {
        let codec = HornerCodec::new();
        let mut net = codec.encode(json).expect("valid input encodes");
        reduce_all(&mut net);
        if net.root.is_none() {
            crate::encoding::arithmetic::discover_root(&mut net);
        }
        decode_biguint(&net)
    }

    // UT-0723-08 / TG-001: single-iteration c_1 upper-bound regression test.
    //
    // Empirically (bisect 2026-05-16) the v1 walker's exact envelope on
    // single-iter inputs extends to `c_1 == 1025` and the FIRST failing
    // value is `c_1 == 1026`. The threshold is bounded by the inner mul
    // scaffold's DUP-chain depth and is independent of `c_0` and `x`.
    // The pre-D-016 doc claimed `c_1 in 0..=MAX_CHURCH_NAT = 10_000`
    // which was false — `read_chain_terminal` traps c_1 >= 1026 with
    // "nested mul boundary on exit chain". Lock the empirical boundary
    // so a future regression in either direction (envelope shrinks OR
    // bug becomes silent-wrong instead of Err) is caught.
    #[test]
    fn decode_biguint_handles_actual_c1_upper_bound() {
        const C1_UPPER: u64 = 1025;

        // Boundary value: exactly representable. Independent of c_0 and x.
        for c0 in [0u64, 5, 100] {
            for x in [1u64, 2, 5, 100] {
                let json = format!(r#"{{"coeffs":[{c0},{C1_UPPER}],"x":{x}}}"#);
                let expected = horner_serial(&[c0, C1_UPPER], x).unwrap();
                let actual = pipeline(json.as_bytes());
                assert_eq!(
                    actual, expected,
                    "c1==C1_UPPER must be in envelope: c0={c0} x={x}"
                );
            }
        }

        // First failing value: MUST error, not silently return wrong.
        for c0 in [0u64, 5, 100] {
            for x in [1u64, 2, 5, 100] {
                let json = format!(r#"{{"coeffs":[{c0},{}],"x":{x}}}"#, C1_UPPER + 1);
                let result = pipeline_result(json.as_bytes());
                assert!(
                    matches!(result, Err(DecodeError::UnrecognizedStructure(_))),
                    "c1==C1_UPPER+1 must Err, got {result:?} (c0={c0} x={x})"
                );
            }
        }
    }

    // UT-0724-EC-002: degree-2 with leading coefficient c_2 >= 2 lies
    // OUTSIDE the v1 envelope (cycle-counting walker under-estimates the
    // multiplier). Pre-D-016 the decoder returned `Ok(under-counted)`
    // silently; post-D-016 BUG-001 fix it MUST return Err.
    #[test]
    fn decode_biguint_rejects_degree_2_c2_ge_2() {
        let cases: &[&[u8]] = &[
            br#"{"coeffs":[5,5,5],"x":2}"#, // pre-fix returned Ok("27"), correct 35
            br#"{"coeffs":[3,3,3],"x":2}"#, // pre-fix returned Ok("17"), correct 21
            br#"{"coeffs":[2,2,2],"x":2}"#, // pre-fix returned Ok("12"), correct 14
            br#"{"coeffs":[5,2,3],"x":2}"#, // pre-fix returned Ok("17"), correct 21
        ];
        for json in cases {
            let result = pipeline_result(json);
            assert!(
                matches!(result, Err(DecodeError::UnrecognizedStructure(_))),
                "{}: expected UnrecognizedStructure, got {result:?}",
                String::from_utf8_lossy(json)
            );
        }
    }

    // UT-0724-EC-003: `[1; N]` with N >= 3 lies OUTSIDE the v1 envelope
    // (the repeated-unit pattern collapses through nested DUP scaffolds
    // that the cycle-counter under-counts). Pre-D-016 the decoder
    // returned numerically-wrong values; post-D-016 BUG-001 fix it MUST
    // return Err.
    #[test]
    fn decode_biguint_rejects_repeated_unit_chain_n_ge_3() {
        let cases: &[&[u8]] = &[
            br#"{"coeffs":[1,1,1,1,1],"x":2}"#, // pre-fix Ok("15"), correct 31
            br#"{"coeffs":[1,1,1,1,1,1],"x":2}"#, // pre-fix wrong, correct 63
            br#"{"coeffs":[3,2,5,1],"x":2}"#,   // degree-3, pre-fix Ok("23"), correct 35
            br#"{"coeffs":[10,20,30,40],"x":3}"#, // degree-3, pre-fix Ok("292"), correct 1420
        ];
        for json in cases {
            let result = pipeline_result(json);
            assert!(
                matches!(result, Err(DecodeError::UnrecognizedStructure(_))),
                "{}: expected UnrecognizedStructure, got {result:?}",
                String::from_utf8_lossy(json)
            );
        }
    }
}
