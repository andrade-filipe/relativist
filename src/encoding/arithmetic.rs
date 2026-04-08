//! Arithmetic operation combinators for Church numerals (SPEC-14 R15-R18).
//!
//! Each function builds an IC net that, when reduced via `reduce_all`,
//! yields a Church numeral encoding the arithmetic result.
//!
//! - `build_add(a, b)`: addition via `add = lambda m. lambda n. lambda f. lambda x. m f (n f x)`
//! - `build_mul(a, b)`: multiplication via `mul = lambda m. lambda n. lambda f. m (n f)`
//! - `build_exp(base, exp)`: exponentiation via `exp = lambda m. lambda n. n m`
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
/// Scans for a CON agent whose principal port (port 0) is DISCONNECTED in the
/// port array — this is the outer lambda of the Church numeral result.
/// Returns true if a root was found and set.
pub fn discover_root(net: &mut Net) -> bool {
    let mut candidate: Option<AgentId> = None;
    for agent in net.live_agents() {
        if agent.symbol == Symbol::Con {
            let target = net.get_target(PortRef::AgentPort(agent.id, 0));
            if target == DISCONNECTED {
                if candidate.is_some() {
                    // Multiple candidates — ambiguous, don't set
                    return false;
                }
                candidate = Some(agent.id);
            }
        }
    }
    if let Some(id) = candidate {
        net.root = Some(PortRef::AgentPort(id, 0));
        true
    } else {
        false
    }
}

/// Build an IC net for addition: `a + b` (SPEC-14 R15).
///
/// Constructs the full add combinator `lambda m. lambda n. lambda f. lambda x. m f (n f x)`
/// and applies it to `church(a)` and `church(b)`. The resulting net contains redexes
/// that, when reduced via `reduce_all`, produce `church(a + b)`.
///
/// After reduction, call `discover_root` to set the root for decoding.
pub fn build_add(a: u64, b: u64) -> Net {
    let mut net = Net::new();

    // Step 1: Encode Church numerals as sub-nets
    let m_root = encode_church_into(&mut net, a);
    let n_root = encode_church_into(&mut net, b);

    // Step 2: Build the add combinator
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
    net.connect(
        PortRef::AgentPort(lam_m, 2),
        PortRef::AgentPort(lam_n, 0),
    );
    net.connect(
        PortRef::AgentPort(lam_n, 2),
        PortRef::AgentPort(lam_f, 0),
    );
    net.connect(
        PortRef::AgentPort(lam_f, 2),
        PortRef::AgentPort(lam_x, 0),
    );

    // Variable bindings (each lambda's p1 connects to where the variable is used):
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
        PortRef::AgentPort(app_nfx, 1), // x -> argument of ((nf) x)
    );

    // DUP distributes f to both applications
    net.connect(
        PortRef::AgentPort(dup_f, 1),
        PortRef::AgentPort(app_mf, 1), // f copy 1 -> argument of (m f)
    );
    net.connect(
        PortRef::AgentPort(dup_f, 2),
        PortRef::AgentPort(app_nf, 1), // f copy 2 -> argument of (n f)
    );

    // Application chain wiring:
    net.connect(
        PortRef::AgentPort(app_nf, 2),
        PortRef::AgentPort(app_nfx, 0), // result of (nf) -> function of ((nf) x)
    );
    net.connect(
        PortRef::AgentPort(app_nfx, 2),
        PortRef::AgentPort(app_all, 1), // result of ((nf) x) -> argument of final app
    );
    net.connect(
        PortRef::AgentPort(app_mf, 2),
        PortRef::AgentPort(app_all, 0), // result of (mf) -> function of final app
    );
    net.connect(
        PortRef::AgentPort(app_all, 2),
        PortRef::AgentPort(lam_x, 2), // result -> body of lambda x
    );

    // Step 3: Apply add to church(a) and church(b)
    // app_1 applies add to church(a): forms redex with lam_m
    let app_1 = net.create_agent(Symbol::Con);
    net.connect(
        PortRef::AgentPort(app_1, 0),
        PortRef::AgentPort(lam_m, 0),
    );
    net.connect(
        PortRef::AgentPort(app_1, 1),
        PortRef::AgentPort(m_root, 0),
    );

    // app_2 applies result to church(b): awaits app_1's resolution
    let app_2 = net.create_agent(Symbol::Con);
    net.connect(
        PortRef::AgentPort(app_1, 2),
        PortRef::AgentPort(app_2, 0),
    );
    net.connect(
        PortRef::AgentPort(app_2, 1),
        PortRef::AgentPort(n_root, 0),
    );

    // Connect result to a FreePort sentinel so the output is tracked
    // through reduction. After reduce_all, discover_root will find it.
    net.connect(
        PortRef::AgentPort(app_2, 2),
        PortRef::FreePort(0),
    );

    // Root not set — the Church numeral root emerges after reduction
    net.root = None;

    net
}

/// Build an IC net for multiplication: `a * b` (SPEC-14 R16).
///
/// Constructs `mul = lambda m. lambda n. lambda f. m (n f)` and applies
/// it to `church(a)` and `church(b)`.
pub fn build_mul(a: u64, b: u64) -> Net {
    let mut net = Net::new();

    let m_root = encode_church_into(&mut net, a);
    let n_root = encode_church_into(&mut net, b);

    // Build mul combinator: lambda m. lambda n. lambda f. m (n f)
    let lam_m = net.create_agent(Symbol::Con);
    let lam_n = net.create_agent(Symbol::Con);
    let lam_f = net.create_agent(Symbol::Con);

    let app_nf = net.create_agent(Symbol::Con); // (n f)
    let app_m = net.create_agent(Symbol::Con); // (m (nf))

    // Lambda chain
    net.connect(
        PortRef::AgentPort(lam_m, 2),
        PortRef::AgentPort(lam_n, 0),
    );
    net.connect(
        PortRef::AgentPort(lam_n, 2),
        PortRef::AgentPort(lam_f, 0),
    );

    // Variable bindings
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
        PortRef::AgentPort(app_nf, 1), // f -> argument of (n f)
    );

    // Application chain
    net.connect(
        PortRef::AgentPort(app_nf, 2),
        PortRef::AgentPort(app_m, 1), // result of (nf) -> argument of m
    );
    net.connect(
        PortRef::AgentPort(app_m, 2),
        PortRef::AgentPort(lam_f, 2), // result of m(nf) -> body of lambda f
    );

    // Apply mul to church(a) and church(b)
    let app_1 = net.create_agent(Symbol::Con);
    net.connect(
        PortRef::AgentPort(app_1, 0),
        PortRef::AgentPort(lam_m, 0),
    );
    net.connect(
        PortRef::AgentPort(app_1, 1),
        PortRef::AgentPort(m_root, 0),
    );

    let app_2 = net.create_agent(Symbol::Con);
    net.connect(
        PortRef::AgentPort(app_1, 2),
        PortRef::AgentPort(app_2, 0),
    );
    net.connect(
        PortRef::AgentPort(app_2, 1),
        PortRef::AgentPort(n_root, 0),
    );

    net.connect(
        PortRef::AgentPort(app_2, 2),
        PortRef::FreePort(0),
    );

    net.root = None;
    net
}

/// Build an IC net for exponentiation: `base ^ exp` (SPEC-14 R17).
///
/// Constructs `exp_comb = lambda m. lambda n. n m` and applies it to
/// `church(base)` and `church(exp)`.
///
/// WARNING: Reduction requires O(base^exp) interactions. Use small values.
pub fn build_exp(base: u64, exp: u64) -> Net {
    let mut net = Net::new();

    let m_root = encode_church_into(&mut net, base);
    let n_root = encode_church_into(&mut net, exp);

    // Build exp combinator: lambda m. lambda n. n m
    let lam_m = net.create_agent(Symbol::Con);
    let lam_n = net.create_agent(Symbol::Con);
    let app_nm = net.create_agent(Symbol::Con); // (n m)

    // Lambda chain
    net.connect(
        PortRef::AgentPort(lam_m, 2),
        PortRef::AgentPort(lam_n, 0),
    );

    // Variable bindings
    net.connect(
        PortRef::AgentPort(lam_n, 1),
        PortRef::AgentPort(app_nm, 0), // n -> function of (n m)
    );
    net.connect(
        PortRef::AgentPort(lam_m, 1),
        PortRef::AgentPort(app_nm, 1), // m -> argument of (n m)
    );

    // Body result
    net.connect(
        PortRef::AgentPort(app_nm, 2),
        PortRef::AgentPort(lam_n, 2), // result of (nm) -> body of lambda n
    );

    // Apply exp to church(base) and church(exp)
    let app_1 = net.create_agent(Symbol::Con);
    net.connect(
        PortRef::AgentPort(app_1, 0),
        PortRef::AgentPort(lam_m, 0),
    );
    net.connect(
        PortRef::AgentPort(app_1, 1),
        PortRef::AgentPort(m_root, 0),
    );

    let app_2 = net.create_agent(Symbol::Con);
    net.connect(
        PortRef::AgentPort(app_1, 2),
        PortRef::AgentPort(app_2, 0),
    );
    net.connect(
        PortRef::AgentPort(app_2, 1),
        PortRef::AgentPort(n_root, 0),
    );

    net.connect(
        PortRef::AgentPort(app_2, 2),
        PortRef::FreePort(0),
    );

    net.root = None;
    net
}

/// Build an arithmetic net, reduce it, and decode the result.
///
/// This is the core pipeline: build -> reduce -> discover_root -> decode.
/// Used by both tests and the `compute` CLI subcommand.
pub fn compute_arithmetic(
    build_fn: impl FnOnce() -> Net,
) -> (Net, Option<u64>) {
    use crate::reduction::reduce_all;

    let mut net = build_fn();
    reduce_all(&mut net);
    discover_root(&mut net);
    let result = super::decode_nat(&net);
    (net, result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::decode_nat;
    use crate::reduction::reduce_all;

    /// Helper: build, reduce, discover root, decode
    fn reduce_and_decode(mut net: Net) -> Option<u64> {
        reduce_all(&mut net);
        discover_root(&mut net);
        decode_nat(&net)
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

    // --- ET-8: Exponentiation correctness ---
    #[test]
    fn test_exp_correctness() {
        let cases: [(u64, u32); 5] = [(2, 0), (2, 1), (2, 3), (2, 8), (3, 3)];
        for (a, b) in cases {
            let result = reduce_and_decode(build_exp(a, b as u64));
            assert_eq!(
                result,
                Some(a.pow(b)),
                "build_exp({a}, {b}): expected {}, got {:?}",
                a.pow(b),
                result
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
}
