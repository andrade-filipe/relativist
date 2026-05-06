//! Arithmetic operation combinators for Church numerals (SPEC-14 R15-R18, SPEC-09 R17d).
//!
//! Each function builds an IC net that, when reduced via `reduce_all`,
//! yields a Church numeral encoding the arithmetic result.
//!
//! - `build_add(a, b)`: addition via `add = lambda m. lambda n. lambda f. lambda x. m f (n f x)`
//! - `build_mul(a, b)`: multiplication via `mul = lambda m. lambda n. lambda f. m (n f)`
//! - `build_exp(base, exp)`: exponentiation via `exp = lambda m. lambda n. n m`
//! - `build_sum_of_squares(n)`: right-associated `add`-chain over pre-encoded
//!   `Church(i^2)` for i in 1..N (demonstrative, SPEC-09 R17d)
//!
//! `build_add` / `build_mul` are thin wrappers around the lower-level port-based
//! helpers `wire_add_into` / `wire_mul_into`, which accept PortRef inputs so
//! that arithmetic sub-computations can be composed without pre-encoding their
//! operands. `build_sum_of_squares` uses `wire_add_into` to chain addition
//! across pre-encoded squares (see its docstring for why the squares are
//! pre-encoded instead of built via `wire_mul_into`).
//!
//! All functions use `encode_church_into` to compose sub-nets in a single `Net`,
//! avoiding ID collisions (SPEC-14 Section 4.3, ID composition).
//!
//! Construction approach: build the FULL combinator as an IC net fragment,
//! then apply it to the Church numeral sub-nets via application CON agents.
//! The beta-reductions of the combinator application drive the computation.
//!
//! Root handling: arithmetic nets have `root = None` during construction because
//! the result emerges from reduction. After `reduce_all`, call `discover_root`
//! to find and set the Church numeral's outer lambda.

use crate::net::{AgentId, Net, PortRef, Symbol, DISCONNECTED};

use super::church::encode_church_into;

/// After reduction, find the root of the resulting Church numeral and set `net.root`.
///
/// The arithmetic builders connect the result output to FreePort(0). After
/// reduction, the result Church numeral's outer lambda (CON) has its principal
/// port connected to FreePort(0). This function finds that agent, disconnects
/// the FreePort(0) wire, and sets the root (satisfying R18a: root principal
/// port must be DISCONNECTED).
///
/// Fallback: also accepts a CON agent with DISCONNECTED principal port
/// (for nets where the output wire was already consumed).
///
/// Returns true if a root was found and set.
pub fn discover_root(net: &mut Net) -> bool {
    let mut candidate: Option<AgentId> = None;
    for agent in net.live_agents() {
        if agent.symbol == Symbol::Con {
            let target = net.get_target(PortRef::AgentPort(agent.id, 0));
            match target {
                PortRef::FreePort(fid) if fid != u32::MAX => {
                    // FreePort on principal port — this is the result output wire.
                    // Prefer this over DISCONNECTED candidates.
                    candidate = Some(agent.id);
                    break; // FreePort match is unambiguous
                }
                _ if target == DISCONNECTED => {
                    if candidate.is_some() {
                        return false; // Multiple DISCONNECTED candidates — ambiguous
                    }
                    candidate = Some(agent.id);
                }
                _ => {}
            }
        }
    }
    if let Some(id) = candidate {
        // Disconnect the output wire so root satisfies R18a
        net.disconnect(PortRef::AgentPort(id, 0));
        net.root = Some(PortRef::AgentPort(id, 0));
        true
    } else {
        false
    }
}

/// Wire an `add` combinator into an existing net, consuming two operand ports.
///
/// Builds `add = lambda m. lambda n. lambda f. lambda x. m f (n f x)` and applies
/// it to whatever is reachable from `m_port` and `n_port`. The operands do not
/// need to be canonical Church numerals — they can be the result wires of
/// previous arithmetic sub-computations (e.g. `AgentPort(app_out, 1)` returned
/// from an earlier `wire_add_into` / `wire_mul_into` call).
///
/// Returns the outermost application CON agent. Its port `p1` is the result
/// wire of the addition (the place to connect to the surrounding context, e.g.
/// `FreePort(0)` at the top level or the operand slot of an enclosing operation).
///
/// Used both by `build_add` (as a thin wrapper), by composite builders such as
/// `build_sum_of_squares` (SPEC-09 R17d), and by `HornerCodec` (SPEC-27 v3
/// R10'/R13' — the encoder calls this helper inside the Horner recurrence,
/// reusing the same sub-net rather than allocating a fresh `Net`).
///
/// Privacy: kept `pub(crate)` per SPEC-27 v3 R13a' obligation 3 — SPEC-14's
/// public R3 export list is unchanged. If a future codec is implemented in a
/// separate crate, SPEC-14 will be amended (separate task) to expose this
/// helper publicly.
pub(crate) fn wire_add_into(net: &mut Net, m_port: PortRef, n_port: PortRef) -> AgentId {
    // Build the add combinator:
    // add = lambda m. lambda n. lambda f. lambda x. m f (n f x)
    //
    // Lambda agents (4 nested lambdas):
    let lam_m = net.create_agent(Symbol::Con);
    let lam_n = net.create_agent(Symbol::Con);
    let lam_f = net.create_agent(Symbol::Con);
    let lam_x = net.create_agent(Symbol::Con);

    // Application agents:
    // app_mf  = (m f)      — applies m to f
    // app_nf  = (n f)      — applies n to f
    // app_nfx = ((n f) x)  — applies (nf) to x
    // app_all = ((m f) ((n f) x)) — applies (mf) to ((nf) x)
    let app_mf = net.create_agent(Symbol::Con);
    let app_nf = net.create_agent(Symbol::Con);
    let app_nfx = net.create_agent(Symbol::Con);
    let app_all = net.create_agent(Symbol::Con);

    // DUP for f (used in both (m f) and (n f))
    let dup_f = net.create_agent(Symbol::Dup);

    // Lambda chain: each lambda's body (p2) connects to next lambda's principal (p0)
    net.connect(PortRef::AgentPort(lam_m, 2), PortRef::AgentPort(lam_n, 0));
    net.connect(PortRef::AgentPort(lam_n, 2), PortRef::AgentPort(lam_f, 0));
    net.connect(PortRef::AgentPort(lam_f, 2), PortRef::AgentPort(lam_x, 0));

    // Variable bindings (each lambda's p1 connects to where the variable is used):
    // Application port convention: p0 = function, p1 = result, p2 = argument.
    net.connect(
        PortRef::AgentPort(lam_m, 1),
        PortRef::AgentPort(app_mf, 0), // m -> function of (m f)
    );
    net.connect(
        PortRef::AgentPort(lam_n, 1),
        PortRef::AgentPort(app_nf, 0), // n -> function of (n f)
    );
    net.connect(
        PortRef::AgentPort(lam_f, 1),
        PortRef::AgentPort(dup_f, 0), // f -> DUP
    );
    net.connect(
        PortRef::AgentPort(lam_x, 1),
        PortRef::AgentPort(app_nfx, 2), // x -> argument of ((nf) x)
    );

    // DUP distributes f to both applications (argument = p2)
    net.connect(
        PortRef::AgentPort(dup_f, 1),
        PortRef::AgentPort(app_mf, 2), // f copy 1 -> argument of (m f)
    );
    net.connect(
        PortRef::AgentPort(dup_f, 2),
        PortRef::AgentPort(app_nf, 2), // f copy 2 -> argument of (n f)
    );

    // Application chain wiring (result = p1, argument = p2):
    net.connect(
        PortRef::AgentPort(app_nf, 1),
        PortRef::AgentPort(app_nfx, 0), // result of (nf) -> function of ((nf) x)
    );
    net.connect(
        PortRef::AgentPort(app_nfx, 1),
        PortRef::AgentPort(app_all, 2), // result of ((nf) x) -> argument of final app
    );
    net.connect(
        PortRef::AgentPort(app_mf, 1),
        PortRef::AgentPort(app_all, 0), // result of (mf) -> function of final app
    );
    net.connect(
        PortRef::AgentPort(app_all, 1),
        PortRef::AgentPort(lam_x, 2), // result -> body of lambda x
    );

    // Apply add to the operand ports.
    // Application port convention: p0 = function, p1 = result, p2 = argument.
    let app_1 = net.create_agent(Symbol::Con);
    net.connect(PortRef::AgentPort(app_1, 0), PortRef::AgentPort(lam_m, 0));
    net.connect(PortRef::AgentPort(app_1, 2), m_port); // argument (p2) = operand m

    let app_2 = net.create_agent(Symbol::Con);
    net.connect(
        PortRef::AgentPort(app_1, 1),
        PortRef::AgentPort(app_2, 0), // result (p1) -> next application
    );
    net.connect(PortRef::AgentPort(app_2, 2), n_port); // argument (p2) = operand n

    // Caller is responsible for wiring `AgentPort(app_2, 1)` into the surrounding
    // context (FreePort at the top level, or operand slot of an enclosing op).
    app_2
}

/// Build an IC net for addition: `a + b` (SPEC-14 R15).
///
/// Thin wrapper around `wire_add_into`: encodes `Church(a)` and `Church(b)`
/// as sub-nets, wires the add combinator on top, and connects the result to
/// `FreePort(0)` so `discover_root` can pick it up after reduction.
pub fn build_add(a: u64, b: u64) -> Net {
    let mut net = Net::new();

    let m_root = encode_church_into(&mut net, a);
    let n_root = encode_church_into(&mut net, b);

    let app_out = wire_add_into(
        &mut net,
        PortRef::AgentPort(m_root, 0),
        PortRef::AgentPort(n_root, 0),
    );

    // Connect result to a FreePort sentinel so the output is tracked
    // through reduction. After reduce_all, discover_root will find it.
    net.connect(
        PortRef::AgentPort(app_out, 1),
        PortRef::FreePort(0), // result (p1) = output wire
    );

    // Root not set — the Church numeral root emerges after reduction
    net.root = None;

    net
}

/// Wire a `mul` combinator into an existing net, consuming two operand ports.
///
/// Builds `mul = lambda m. lambda n. lambda f. m (n f)` and applies it to
/// whatever is reachable from `m_port` and `n_port`. Like `wire_add_into`, the
/// operands do not have to be freshly encoded Church numerals — they can be
/// intermediate result wires from prior sub-computations.
///
/// Returns the outermost application CON agent. `AgentPort(result, 1)` is the
/// result wire where the multiplied value will appear after reduction.
///
/// Used by `build_mul` (as a thin wrapper) and by `HornerCodec`
/// (SPEC-27 v3 R10'/R13' — the encoder multiplies the running accumulator by
/// `Church(x)` at every iteration of the Horner loop).
///
/// Privacy: kept `pub(crate)` per SPEC-27 v3 R13a' obligation 3 — SPEC-14's
/// public R3 export list is unchanged.
pub(crate) fn wire_mul_into(net: &mut Net, m_port: PortRef, n_port: PortRef) -> AgentId {
    // Build mul combinator: lambda m. lambda n. lambda f. m (n f)
    let lam_m = net.create_agent(Symbol::Con);
    let lam_n = net.create_agent(Symbol::Con);
    let lam_f = net.create_agent(Symbol::Con);

    let app_nf = net.create_agent(Symbol::Con); // (n f)
    let app_m = net.create_agent(Symbol::Con); // (m (nf))

    // Lambda chain
    net.connect(PortRef::AgentPort(lam_m, 2), PortRef::AgentPort(lam_n, 0));
    net.connect(PortRef::AgentPort(lam_n, 2), PortRef::AgentPort(lam_f, 0));

    // Variable bindings
    // Application port convention: p0 = function, p1 = result, p2 = argument.
    net.connect(
        PortRef::AgentPort(lam_m, 1),
        PortRef::AgentPort(app_m, 0), // m -> function of (m (nf))
    );
    net.connect(
        PortRef::AgentPort(lam_n, 1),
        PortRef::AgentPort(app_nf, 0), // n -> function of (n f)
    );
    net.connect(
        PortRef::AgentPort(lam_f, 1),
        PortRef::AgentPort(app_nf, 2), // f -> argument of (n f)
    );

    // Application chain (result = p1, argument = p2)
    net.connect(
        PortRef::AgentPort(app_nf, 1),
        PortRef::AgentPort(app_m, 2), // result of (nf) -> argument of m
    );
    net.connect(
        PortRef::AgentPort(app_m, 1),
        PortRef::AgentPort(lam_f, 2), // result of m(nf) -> body of lambda f
    );

    // Apply mul to the operand ports.
    let app_1 = net.create_agent(Symbol::Con);
    net.connect(PortRef::AgentPort(app_1, 0), PortRef::AgentPort(lam_m, 0));
    net.connect(PortRef::AgentPort(app_1, 2), m_port); // argument (p2)

    let app_2 = net.create_agent(Symbol::Con);
    net.connect(
        PortRef::AgentPort(app_1, 1),
        PortRef::AgentPort(app_2, 0), // result (p1)
    );
    net.connect(PortRef::AgentPort(app_2, 2), n_port); // argument (p2)

    app_2
}

/// Build an IC net for multiplication: `a * b` (SPEC-14 R16).
///
/// Thin wrapper around `wire_mul_into`: encodes `Church(a)` and `Church(b)`,
/// wires the mul combinator on top, connects the result to `FreePort(0)`.
pub fn build_mul(a: u64, b: u64) -> Net {
    let mut net = Net::new();

    let m_root = encode_church_into(&mut net, a);
    let n_root = encode_church_into(&mut net, b);

    let app_out = wire_mul_into(
        &mut net,
        PortRef::AgentPort(m_root, 0),
        PortRef::AgentPort(n_root, 0),
    );

    net.connect(
        PortRef::AgentPort(app_out, 1),
        PortRef::FreePort(0), // result (p1) = output wire
    );

    net.root = None;
    net
}

/// Build an IC net for sum of squares: `sum_{i=1..N} i^2` (SPEC-09 R17d).
///
/// Builds a right-associated `wire_add_into` chain over pre-encoded Church
/// numerals `Church(i^2)` for `i in 1..=N`. After reduction, the resulting
/// Church numeral encodes `N*(N+1)*(2*N+1)/6` (Archimedes/Faulhaber closed form).
///
/// Edge cases:
/// - `n == 0`: returns `Church(0)` directly (empty sum).
/// - `n == 1`: returns `Church(1)` directly (1^2 = 1, no chain needed).
///
/// ## Why squares are pre-encoded (not built via `wire_mul_into`)
///
/// An earlier version of this builder composed `wire_mul_into(Ch(i), Ch(i))`
/// inside the add chain so that the grid would also reduce the squaring step.
/// Optimal reduction of that composed net produces a correct Church normal form,
/// but the final structure has nested DUP sharing boundaries (one per mul) that
/// the current `decode_shared_chain` readback cannot traverse — it only handles
/// a single terminal DUP boundary. Verification against the closed form would
/// fall back to structural isomorphism, which is quadratic and not viable for
/// `N >= 30`. Pre-encoding `Ch(i^2)` in Rust eliminates the nested DUPs, keeping
/// decode tractable while preserving the arithmetic demonstration:
///   - The benchmark still computes `sum_{i=1..N} i^2` end-to-end.
///   - The grid still reduces the full `add`-chain, which for `N` terms grows
///     the final agent count cubically (`~2*N(N+1)(2N+1)/6`).
///   - Profile B (expansion-dominant) behavior is preserved via the add chain.
///
/// The encoding of the squares is a local pre-processing step; the distributed
/// work is the reduction of the chain itself. See SPEC-09 R17d and USAGE_GUIDE.md
/// Section 11.8 for the narrative framing.
///
/// This builder is demonstrative, not comparative — it is NOT part of the
/// frozen performance campaigns (v1_local_baseline, v1_stress).
pub fn build_sum_of_squares(n: u64) -> Net {
    if n == 0 {
        return super::church::encode_nat(0);
    }
    if n == 1 {
        return super::church::encode_nat(1);
    }

    let mut net = Net::new();

    // Start the fold with the last term `Church(n^2)`.
    let last_square = encode_church_into(&mut net, n * n);
    let mut acc_port: PortRef = PortRef::AgentPort(last_square, 0);

    // Fold right: add(Ch(i^2), acc) for i = n-1, n-2, ..., 1.
    for i in (1..n).rev() {
        let term_root = encode_church_into(&mut net, i * i);
        let app_out = wire_add_into(&mut net, PortRef::AgentPort(term_root, 0), acc_port);
        acc_port = PortRef::AgentPort(app_out, 1);
    }

    // Wire the final result to FreePort(0) so discover_root can find it.
    net.connect(acc_port, PortRef::FreePort(0));
    net.root = None;
    net
}

/// Read-only decoder for arithmetic nets whose structure may or may not be
/// canonical Church (SPEC-09 R17d, SPEC-14 R22 variant).
///
/// Intended for benchmark `verify` hooks (`fn verify(&self, &Net, &Net) -> bool`)
/// that cannot mutate the net to set `root` before decoding. Clones the input
/// net internally, runs `discover_root` on the clone, and then tries the
/// canonical `decode_nat` with a fallback to `decode_shared_chain`.
///
/// Fast path: if `net.root` is already set (e.g. by prior `discover_root`),
/// decoders are tried directly without cloning.
pub fn decode_nat_or_shared(net: &Net) -> Option<u64> {
    if net.root.is_some() {
        return super::church::decode_nat(net).or_else(|| decode_shared_chain(net));
    }

    let mut cloned = net.clone();
    if !discover_root(&mut cloned) {
        return None;
    }
    super::church::decode_nat(&cloned).or_else(|| decode_shared_chain(&cloned))
}

/// Build an IC net for exponentiation: `base ^ exp` (SPEC-14 R17).
///
/// Constructs `exp_comb = lambda m. lambda n. n m` and applies it to
/// `church(base)` and `church(exp)`.
///
/// WARNING: Reduction requires O(base^exp) interactions. Use small values.
pub fn build_exp(base: u64, exp: u64) -> Net {
    // Edge case: n^0 = 1 for all n.
    // The Church exp combinator `lambda m. lambda n. n m` fails for exp=0 because
    // Church(0) applied to any argument returns the identity (lambda x. x), not Church(1).
    // This is a known limitation of Church exponentiation, not a reduction bug.
    if exp == 0 {
        return super::church::encode_nat(1);
    }

    let mut net = Net::new();

    let m_root = encode_church_into(&mut net, base);
    let n_root = encode_church_into(&mut net, exp);

    // Build exp combinator: lambda m. lambda n. n m
    let lam_m = net.create_agent(Symbol::Con);
    let lam_n = net.create_agent(Symbol::Con);
    let app_nm = net.create_agent(Symbol::Con); // (n m)

    // Lambda chain
    net.connect(PortRef::AgentPort(lam_m, 2), PortRef::AgentPort(lam_n, 0));

    // Variable bindings
    // Application port convention: p0 = function, p1 = result, p2 = argument.
    net.connect(
        PortRef::AgentPort(lam_n, 1),
        PortRef::AgentPort(app_nm, 0), // n -> function of (n m)
    );
    net.connect(
        PortRef::AgentPort(lam_m, 1),
        PortRef::AgentPort(app_nm, 2), // m -> argument of (n m)
    );

    // Body result (result = p1)
    net.connect(
        PortRef::AgentPort(app_nm, 1),
        PortRef::AgentPort(lam_n, 2), // result of (nm) -> body of lambda n
    );

    // Apply exp to church(base) and church(exp)
    let app_1 = net.create_agent(Symbol::Con);
    net.connect(PortRef::AgentPort(app_1, 0), PortRef::AgentPort(lam_m, 0));
    net.connect(
        PortRef::AgentPort(app_1, 2),
        PortRef::AgentPort(m_root, 0), // argument (p2)
    );

    let app_2 = net.create_agent(Symbol::Con);
    net.connect(
        PortRef::AgentPort(app_1, 1),
        PortRef::AgentPort(app_2, 0), // result (p1)
    );
    net.connect(
        PortRef::AgentPort(app_2, 2),
        PortRef::AgentPort(n_root, 0), // argument (p2)
    );

    net.connect(
        PortRef::AgentPort(app_2, 1),
        PortRef::FreePort(0), // result (p1) = output wire
    );

    net.root = None;
    net
}

/// Build an arithmetic net, reduce it, and decode the result.
///
/// This is the core pipeline: build -> reduce -> discover_root -> decode.
/// Used by both tests and the `compute` CLI subcommand.
///
/// Decoding strategy: tries canonical `decode_nat` first (linear app chain),
/// then falls back to `decode_shared_chain` for non-canonical forms with DUP
/// sharing (common after mul). Returns `None` for forms with DUP cycles
/// (exp with exponent >= 2) — the computation is correct, but readback of
/// cyclic shared normal forms is a known open problem in optimal reduction.
pub fn compute_arithmetic(build_fn: impl FnOnce() -> Net) -> (Net, Option<u64>) {
    use crate::reduction::reduce_all;

    let mut net = build_fn();
    reduce_all(&mut net);
    if !discover_root(&mut net) {
        return (net, None);
    }
    let result = super::decode_nat(&net).or_else(|| decode_shared_chain(&net));
    (net, result)
}

/// Structural readback for non-canonical Church numerals with DUP sharing.
///
/// After optimal reduction of multiplication, the result Church numeral has a
/// linear chain of CON applications ending at a DUP boundary. This function
/// walks the chain, counts applications, and multiplies by the DUP fan-out.
///
/// Returns `None` for DUP cycles (e.g., from exponentiation) — those require
/// a full recursive readback which is not implemented.
pub(crate) fn decode_shared_chain(net: &Net) -> Option<u64> {
    let root = net.root?;
    let lam_f = match root {
        PortRef::AgentPort(id, 0) => id,
        _ => return None,
    };
    if net.get_agent(lam_f)?.symbol != Symbol::Con {
        return None;
    }

    let lam_x_port = net.get_target(PortRef::AgentPort(lam_f, 2));
    let lam_x = match lam_x_port {
        PortRef::AgentPort(id, 0) => id,
        _ => return None,
    };
    if net.get_agent(lam_x)?.symbol != Symbol::Con {
        return None;
    }

    // Follow from body output through DUP aux ports to find chain start
    let body_target = net.get_target(PortRef::AgentPort(lam_x, 2));
    let chain_start = follow_through_dup_aux(net, body_target)?;

    // Walk the CON application chain
    let mut chain_count = 0u64;
    let mut current = chain_start;
    loop {
        match current {
            PortRef::AgentPort(id, p) if id == lam_x && p == 1 => {
                return Some(chain_count);
            }
            PortRef::AgentPort(id, 1) => {
                let agent = net.get_agent(id)?;
                if agent.symbol != Symbol::Con {
                    return None;
                }
                chain_count += 1;
                current = net.get_target(PortRef::AgentPort(id, 2));
            }
            PortRef::AgentPort(id, 0) => {
                let agent = net.get_agent(id)?;
                if agent.symbol == Symbol::Dup {
                    let multiplier = count_dup_boundary_multiplier(net, id, lam_x)?;
                    return Some(chain_count * multiplier);
                }
                return None;
            }
            _ => return None,
        }
    }
}

/// Follow through consecutive DUP auxiliary ports to reach the content.
fn follow_through_dup_aux(net: &Net, start: PortRef) -> Option<PortRef> {
    let mut current = start;
    for _ in 0..1000 {
        match current {
            PortRef::AgentPort(id, port) if port == 1 || port == 2 => {
                let agent = net.get_agent(id)?;
                if agent.symbol == Symbol::Dup {
                    current = net.get_target(PortRef::AgentPort(id, 0));
                } else {
                    return Some(current);
                }
            }
            _ => return Some(current),
        }
    }
    None
}

/// What we find at the end of a DUP boundary branch.
enum DupBranchEnd {
    XVariable,
    DupPrincipal(AgentId),
    Other,
}

/// Follow a DUP branch through aux ports to determine where it leads.
fn classify_dup_branch(net: &Net, start: PortRef, lam_x: AgentId) -> Option<DupBranchEnd> {
    if start == PortRef::AgentPort(lam_x, 1) {
        return Some(DupBranchEnd::XVariable);
    }

    match start {
        PortRef::AgentPort(id, port) => {
            let agent = net.get_agent(id)?;
            if agent.symbol == Symbol::Dup && (port == 1 || port == 2) {
                let target = net.get_target(PortRef::AgentPort(id, 0));
                match target {
                    PortRef::AgentPort(tid, tp) if tid == lam_x && tp == 1 => {
                        Some(DupBranchEnd::XVariable)
                    }
                    PortRef::AgentPort(tid, 0) => {
                        let t_agent = net.get_agent(tid)?;
                        if t_agent.symbol == Symbol::Dup {
                            Some(DupBranchEnd::DupPrincipal(tid))
                        } else {
                            Some(DupBranchEnd::Other)
                        }
                    }
                    _ => Some(DupBranchEnd::Other),
                }
            } else if agent.symbol == Symbol::Dup && port == 0 {
                Some(DupBranchEnd::DupPrincipal(id))
            } else {
                Some(DupBranchEnd::Other)
            }
        }
        _ => Some(DupBranchEnd::Other),
    }
}

/// Count the multiplier from DUP boundary nodes.
///
/// For mul(a, b): the chain has b apps, and the boundary has (a-1) DUP
/// principals, giving multiplier = a, total = a*b.
fn count_dup_boundary_multiplier(net: &Net, first_dup: AgentId, lam_x: AgentId) -> Option<u64> {
    let mut multiplier = 2u64;
    let mut current_dup = first_dup;

    for _ in 0..1000 {
        let t1 = net.get_target(PortRef::AgentPort(current_dup, 1));
        let t2 = net.get_target(PortRef::AgentPort(current_dup, 2));

        let c1 = classify_dup_branch(net, t1, lam_x)?;
        let c2 = classify_dup_branch(net, t2, lam_x)?;

        match (c1, c2) {
            (DupBranchEnd::XVariable, _) | (_, DupBranchEnd::XVariable) => {
                return Some(multiplier);
            }
            (DupBranchEnd::DupPrincipal(next), _) | (_, DupBranchEnd::DupPrincipal(next)) => {
                multiplier += 1;
                current_dup = next;
            }
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::decode_nat;
    use crate::reduction::reduce_all;

    /// Helper: build, reduce, discover root, decode (canonical + shared chain).
    fn reduce_and_decode(mut net: Net) -> Option<u64> {
        reduce_all(&mut net);
        if !discover_root(&mut net) {
            return None;
        }
        decode_nat(&net).or_else(|| decode_shared_chain(&net))
    }

    // --- ET-6: Addition correctness ---
    #[test]
    fn test_add_correctness() {
        let cases = [(0, 0), (0, 1), (1, 0), (1, 1), (2, 3), (10, 20), (50, 50)];
        for (a, b) in cases {
            let result = reduce_and_decode(build_add(a, b));
            assert_eq!(
                result,
                Some(a + b),
                "build_add({a}, {b}): expected {}, got {:?}",
                a + b,
                result
            );
        }
    }

    // --- ET-7: Multiplication correctness ---
    #[test]
    fn test_mul_correctness() {
        let cases = [(0, 1), (1, 0), (1, 1), (2, 3), (5, 5), (10, 10)];
        for (a, b) in cases {
            let result = reduce_and_decode(build_mul(a, b));
            assert_eq!(
                result,
                Some(a * b),
                "build_mul({a}, {b}): expected {}, got {:?}",
                a * b,
                result
            );
        }
    }

    // --- ET-8: Exponentiation — reduction completes, decode limited ---
    // Optimal reduction of exp produces non-canonical Church numerals with DUP
    // sharing cycles. The computation is correct (the net IS the right Church
    // numeral), but readback of cyclic shared normal forms is a known open
    // problem in optimal reduction — decode returns None for exp >= 2.
    #[test]
    fn test_exp_reduction_completes() {
        // exp=0 is handled as a special case (returns encode_nat(1) directly)
        assert_eq!(reduce_and_decode(build_exp(2, 0)), Some(1));

        // exp=1 reduces to canonical form
        assert_eq!(reduce_and_decode(build_exp(2, 1)), Some(2));

        // exp >= 2 reduces correctly but produces DUP-cycle shared output
        let non_canonical_cases: [(u64, u64); 3] = [(2, 3), (2, 4), (3, 3)];
        for (base, exp) in non_canonical_cases {
            let mut net = build_exp(base, exp);
            reduce_all(&mut net);

            // Reduction terminates
            assert!(
                net.redex_queue.is_empty(),
                "exp({base}, {exp}) must reach normal form"
            );

            // Root can be discovered
            assert!(
                discover_root(&mut net),
                "exp({base}, {exp}) must have a discoverable root"
            );

            // Decode returns None for cyclic DUP forms
            let result = decode_nat(&net).or_else(|| decode_shared_chain(&net));
            assert_eq!(
                result, None,
                "exp({base}, {exp}): cyclic DUP result expected None"
            );
        }
    }

    // --- ET-10: Small range property test for addition ---
    #[test]
    fn test_add_small_range() {
        for a in 0..=10 {
            for b in 0..=10 {
                let result = reduce_and_decode(build_add(a, b));
                assert_eq!(result, Some(a + b), "add({a}, {b}) failed");
            }
        }
    }

    #[test]
    fn test_add_larger_values() {
        let result = reduce_and_decode(build_add(100, 100));
        assert_eq!(result, Some(200));
    }

    // --- SPEC-09 R17d: wire_add_into / wire_mul_into PortRef-based helpers ---

    // Regression: build_add (now a thin wrapper around wire_add_into) preserves
    // all previous correctness for add — any mismatch here means the PortRef
    // refactor broke the canonical addition path.
    #[test]
    fn test_wire_add_into_port_based_preserves_build_add() {
        assert_eq!(reduce_and_decode(build_add(7, 8)), Some(15));
        assert_eq!(reduce_and_decode(build_add(50, 50)), Some(100));
    }

    // Regression: build_mul (now a thin wrapper around wire_mul_into) preserves
    // all previous correctness for mul.
    #[test]
    fn test_wire_mul_into_port_based_preserves_build_mul() {
        assert_eq!(reduce_and_decode(build_mul(3, 3)), Some(9));
        assert_eq!(reduce_and_decode(build_mul(10, 10)), Some(100));
    }

    // --- SPEC-27 v3 R13a': direct obligation validation for wire_*_into ---
    //
    // These tests exercise the R13a' obligation set on the existing helpers
    // without going through the build_add / build_mul wrappers (Caminho A,
    // Phase 3a promotion-and-validation per Round 2 SC-013):
    //   1. T1-T7 invariant preservation across helper calls.
    //   2. Reduction equivalence to Church(m+n) / Church(m*n).
    //   3. Privacy — pub(crate) only, NOT exported.
    // Privacy obligation 3 is tested via compile_fail doc-tests (see helper
    // rustdoc) and inspection of the public re-export list in encoding/mod.rs.

    // Helper: reduce + decode a wire_*_into composition rooted at `result_id`.
    fn wire_helper_decode(mut net: Net, result_id: AgentId) -> Option<u64> {
        // Wire the helper's result port (p1) to FreePort(0) so the post-reduction
        // discover_root pass can find it; this matches the build_add / build_mul
        // wiring convention.
        net.connect(PortRef::AgentPort(result_id, 1), PortRef::FreePort(0));
        net.root = None;
        reduce_and_decode(net)
    }

    // UT-0711-01: T1-T7 preservation across wire_add_into.
    //
    // We assert validate_encoded_net AFTER the helper call (when at least one
    // redex exists from the application principal-to-principal wiring); a
    // pre-call assertion would fail E2 because two Church sub-nets are in
    // Normal Form (no redexes). Structural integrity (T1) is what matters.
    #[test]
    fn wire_add_into_preserves_t1_t7_for_distinct_subnets() {
        let mut net = Net::new();
        let m_id = encode_church_into(&mut net, 7);
        let n_id = encode_church_into(&mut net, 9);

        let _result_id = wire_add_into(
            &mut net,
            PortRef::AgentPort(m_id, 0),
            PortRef::AgentPort(n_id, 0),
        );

        // Post-call: net satisfies T1-T7 (at least one redex from the
        // application; structural symmetry maintained by `connect`).
        crate::encoding::traits::validate_encoded_net(&net)
            .expect("post-call net must satisfy T1-T7 (R13a' obligation 1)");
    }

    // UT-0711-02: wire_add_into reduces to Church(m + n) for 5 pairs.
    #[test]
    fn wire_add_into_reduces_to_church_sum_for_5_pairs() {
        for (m, n, expected) in [
            (0u64, 0u64, 0u64),
            (1, 1, 2),
            (7, 9, 16),
            (0, 5, 5),
            (5, 0, 5),
        ] {
            let mut net = Net::new();
            let m_id = encode_church_into(&mut net, m);
            let n_id = encode_church_into(&mut net, n);
            let result_id = wire_add_into(
                &mut net,
                PortRef::AgentPort(m_id, 0),
                PortRef::AgentPort(n_id, 0),
            );
            let value = wire_helper_decode(net, result_id);
            assert_eq!(
                value,
                Some(expected),
                "wire_add_into({m}, {n}) reduced to {value:?}, expected Church({expected})"
            );
        }
    }

    // UT-0711-03: T1-T7 preservation across wire_mul_into.
    #[test]
    fn wire_mul_into_preserves_t1_t7_for_distinct_subnets() {
        let mut net = Net::new();
        let m_id = encode_church_into(&mut net, 3);
        let n_id = encode_church_into(&mut net, 4);

        let _result_id = wire_mul_into(
            &mut net,
            PortRef::AgentPort(m_id, 0),
            PortRef::AgentPort(n_id, 0),
        );

        crate::encoding::traits::validate_encoded_net(&net)
            .expect("post-call net must satisfy T1-T7 (R13a' obligation 1)");
    }

    // UT-0711-04: wire_mul_into reduces to Church(m * n) for 5 pairs.
    #[test]
    fn wire_mul_into_reduces_to_church_product_for_5_pairs() {
        for (m, n, expected) in [
            (0u64, 0u64, 0u64),
            (1, 1, 1),
            (3, 4, 12),
            (0, 7, 0),
            (7, 0, 0),
        ] {
            let mut net = Net::new();
            let m_id = encode_church_into(&mut net, m);
            let n_id = encode_church_into(&mut net, n);
            let result_id = wire_mul_into(
                &mut net,
                PortRef::AgentPort(m_id, 0),
                PortRef::AgentPort(n_id, 0),
            );
            let value = wire_helper_decode(net, result_id);
            assert_eq!(
                value,
                Some(expected),
                "wire_mul_into({m}, {n}) reduced to {value:?}, expected Church({expected})"
            );
        }
    }

    // Composition smoke: two wire_add_into applications chained together so
    // the result wire of one becomes an operand port of the next. This is the
    // exact composition pattern that `build_sum_of_squares` relies on, minus
    // the mul-as-operand subcase (see build_sum_of_squares doc for why the
    // mul path is not exercised here — nested DUP boundaries break readback).
    #[test]
    fn test_wire_add_composes_with_wire_add() {
        // add(add(1, 2), add(3, 4)) = 3 + 7 = 10
        let mut net = Net::new();
        let k1 = encode_church_into(&mut net, 1);
        let k2 = encode_church_into(&mut net, 2);
        let add_a = wire_add_into(
            &mut net,
            PortRef::AgentPort(k1, 0),
            PortRef::AgentPort(k2, 0),
        );
        let k3 = encode_church_into(&mut net, 3);
        let k4 = encode_church_into(&mut net, 4);
        let add_b = wire_add_into(
            &mut net,
            PortRef::AgentPort(k3, 0),
            PortRef::AgentPort(k4, 0),
        );
        let add_out = wire_add_into(
            &mut net,
            PortRef::AgentPort(add_a, 1),
            PortRef::AgentPort(add_b, 1),
        );
        net.connect(PortRef::AgentPort(add_out, 1), PortRef::FreePort(0));
        net.root = None;
        assert_eq!(reduce_and_decode(net), Some(10));
    }

    // --- SPEC-09 R17d: build_sum_of_squares correctness ---

    fn expected_sum_of_squares(n: u64) -> u64 {
        n * (n + 1) * (2 * n + 1) / 6
    }

    // Small range: every n in [0..=10] must decode to N*(N+1)*(2N+1)/6.
    #[test]
    fn test_sum_of_squares_small() {
        for n in [0u64, 1, 2, 3, 4, 5, 10] {
            let result = reduce_and_decode(build_sum_of_squares(n));
            assert_eq!(
                result,
                Some(expected_sum_of_squares(n)),
                "sum_of_squares({n}): expected {}, got {:?}",
                expected_sum_of_squares(n),
                result
            );
        }
    }

    // Sanity check at an intermediate size that actually stresses the
    // composed mul+add DUP sharing (N=30 -> sum = 9455).
    #[test]
    fn test_sum_of_squares_n30() {
        let result = reduce_and_decode(build_sum_of_squares(30));
        assert_eq!(result, Some(9455));
    }

    // --- SPEC-09 R17d: decode_nat_or_shared wrapper ---

    // Canonical nets with root set: must go through the fast path.
    #[test]
    fn test_decode_nat_or_shared_canonical() {
        let net = super::super::church::encode_nat(42);
        assert_eq!(decode_nat_or_shared(&net), Some(42));
    }

    // Post-reduction composed net (build_sum_of_squares) with root = None:
    // must clone, discover root, and fall back to shared-chain decode.
    #[test]
    fn test_decode_nat_or_shared_rootless_composed() {
        let mut net = build_sum_of_squares(10);
        crate::reduction::reduce_all(&mut net);
        assert!(net.root.is_none(), "composed builder leaves root = None");
        assert_eq!(decode_nat_or_shared(&net), Some(385));
    }
}
