//! Church numeral encoding and decoding (SPEC-14 R4-R14).
//!
//! Church numeral n encodes the lambda term `lambda f. lambda x. f^n(x)`.
//! - n = 0: `lambda f. lambda x. x` (f erased, x identity)
//! - n = 1: `lambda f. lambda x. f x` (single application, no DUP)
//! - n >= 2: `lambda f. lambda x. f^n(x)` (n applications, n-1 DUPs to share f)
//!
//! The encoding uses only CON, DUP, ERA — the three IC symbols (Lafont 1997, Section 4).

use crate::net::{AgentId, Net, PortRef, Symbol, DISCONNECTED};

/// Maximum supported Church numeral value (SPEC-14 R4).
const MAX_CHURCH: u64 = 10_000;

/// Encode a natural number as a Church numeral IC net (SPEC-14 R4).
///
/// The resulting net is already in Normal Form (zero redexes).
/// To perform computation, compose with arithmetic combinators
/// (`build_add`, `build_mul`, `build_exp`) which introduce redexes.
///
/// # Panics
/// Panics if `n > 10_000`.
pub fn encode_nat(n: u64) -> Net {
    assert!(
        n <= MAX_CHURCH,
        "encode_nat: n = {n} exceeds maximum supported value {MAX_CHURCH}"
    );
    let mut net = Net::new();
    let lam_f = encode_church_into(&mut net, n);
    net.root = Some(PortRef::AgentPort(lam_f, 0));
    debug_assert!(
        net.redex_queue.is_empty(),
        "Church numeral must be in Normal Form"
    );
    net
}

/// Encode a natural number as a Church numeral inside an existing net (SPEC-14 R4b).
///
/// Returns the AgentId of the outer lambda agent (`lambda f`).
/// Does NOT set `net.root` — the caller is responsible for wiring
/// the returned agent's principal port into the surrounding net.
///
/// # Panics
/// Panics if `n > 10_000`.
pub fn encode_church_into(net: &mut Net, n: u64) -> AgentId {
    assert!(
        n <= MAX_CHURCH,
        "encode_church_into: n = {n} exceeds maximum supported value {MAX_CHURCH}"
    );

    // Step 1: Create the two lambda abstractions (always present)
    let lam_f = net.create_agent(Symbol::Con); // outer lambda (lambda f)
    let lam_x = net.create_agent(Symbol::Con); // inner lambda (lambda x)

    // Connect lambda_f body (p2) to lambda_x principal (p0)
    net.connect(
        PortRef::AgentPort(lam_f, 2),
        PortRef::AgentPort(lam_x, 0),
    );

    match n {
        0 => {
            // lambda f. lambda x. x — f is erased, x is identity (self-loop)
            // ERA agent erases the unused f variable
            let era = net.create_agent(Symbol::Era);
            net.connect(
                PortRef::AgentPort(lam_f, 1),
                PortRef::AgentPort(era, 0),
            );
            // Self-loop on lambda_x auxiliaries: p1 <-> p2 (identity on x)
            // This is correct per SPEC-14 R5 and satisfies T1/I1 from SPEC-01.
            net.connect(
                PortRef::AgentPort(lam_x, 1),
                PortRef::AgentPort(lam_x, 2),
            );
        }
        1 => {
            // lambda f. lambda x. f x — single application, no DUP needed
            let app = net.create_agent(Symbol::Con); // application node
            net.connect(
                PortRef::AgentPort(lam_f, 1), // f binding
                PortRef::AgentPort(app, 0),   // -> app function port (principal)
            );
            net.connect(
                PortRef::AgentPort(lam_x, 1), // x binding
                PortRef::AgentPort(app, 1),   // -> app argument
            );
            net.connect(
                PortRef::AgentPort(lam_x, 2), // body result
                PortRef::AgentPort(app, 2),   // -> app result
            );
        }
        n => {
            // lambda f. lambda x. f^n(x) — n applications, (n-1) DUPs for sharing f
            let n = n as usize;

            // Create n application agents (CON)
            let apps: Vec<AgentId> = (0..n).map(|_| net.create_agent(Symbol::Con)).collect();

            // Create (n-1) DUP agents for sharing the f variable
            let dups: Vec<AgentId> = (0..n - 1)
                .map(|_| net.create_agent(Symbol::Dup))
                .collect();

            // Wire f variable to DUP chain head
            net.connect(
                PortRef::AgentPort(lam_f, 1),
                PortRef::AgentPort(dups[0], 0),
            );

            // Wire DUP chain: each DUP's left output (p1) feeds one app,
            // right output (p2) feeds the next DUP (or last app)
            for i in 0..dups.len() {
                // Left output -> application i's function port (principal)
                net.connect(
                    PortRef::AgentPort(dups[i], 1),
                    PortRef::AgentPort(apps[i], 0),
                );
                if i + 1 < dups.len() {
                    // Right output -> next DUP's principal
                    net.connect(
                        PortRef::AgentPort(dups[i], 2),
                        PortRef::AgentPort(dups[i + 1], 0),
                    );
                } else {
                    // Last DUP: right output -> last application's function port
                    net.connect(
                        PortRef::AgentPort(dups[i], 2),
                        PortRef::AgentPort(apps[n - 1], 0),
                    );
                }
            }

            // Wire x variable to innermost application's argument
            net.connect(
                PortRef::AgentPort(lam_x, 1),
                PortRef::AgentPort(apps[n - 1], 1),
            );

            // Chain application results: app[i].p2 -> app[i-1].p1
            // (each app's result feeds the previous app's argument)
            for i in (1..n).rev() {
                net.connect(
                    PortRef::AgentPort(apps[i], 2),
                    PortRef::AgentPort(apps[i - 1], 1),
                );
            }

            // Outermost application result -> body result of lambda_x
            net.connect(
                PortRef::AgentPort(apps[0], 2),
                PortRef::AgentPort(lam_x, 2),
            );
        }
    }

    lam_f
}

/// Decode a Church numeral IC net in Normal Form to a natural number (SPEC-14 R11-R14).
///
/// Returns `Some(n)` if the net has the structure of Church numeral n.
/// Returns `None` if the net is not a recognizable Church numeral
/// (e.g., not in Normal Form, or has an unexpected topology).
///
/// This function does NOT modify the input net (takes `&Net`).
pub fn decode_nat(net: &Net) -> Option<u64> {
    // Must be in Normal Form (SPEC-14 R14)
    if !net.redex_queue.is_empty() {
        return None;
    }

    // Step 1: Find outer lambda (lambda f) from root
    let root = net.root?;
    let lam_f = match root {
        PortRef::AgentPort(id, 0) => id,
        _ => return None,
    };
    let lam_f_agent = net.get_agent(lam_f)?;
    if lam_f_agent.symbol != Symbol::Con {
        return None;
    }

    // Step 2: Find inner lambda (lambda x) from lambda_f.p2
    let lam_f_p2_target = net.get_target(PortRef::AgentPort(lam_f, 2));
    if lam_f_p2_target == DISCONNECTED {
        return None;
    }
    let lam_x = match lam_f_p2_target {
        PortRef::AgentPort(id, 0) => id,
        _ => return None,
    };
    let lam_x_agent = net.get_agent(lam_x)?;
    if lam_x_agent.symbol != Symbol::Con {
        return None;
    }

    // Step 3: Check for n = 0 case (self-loop on lambda_x auxiliaries + ERA on f)
    let f_target = net.get_target(PortRef::AgentPort(lam_f, 1));
    let x_bind = net.get_target(PortRef::AgentPort(lam_x, 1));
    let x_body = net.get_target(PortRef::AgentPort(lam_x, 2));

    if f_target == DISCONNECTED || x_bind == DISCONNECTED || x_body == DISCONNECTED {
        return None;
    }

    // Check self-loop: x_bind == lambda_x.p2 and x_body == lambda_x.p1
    if x_bind == PortRef::AgentPort(lam_x, 2) && x_body == PortRef::AgentPort(lam_x, 1) {
        // Verify ERA on f
        if let PortRef::AgentPort(era_id, 0) = f_target {
            let era_agent = net.get_agent(era_id)?;
            if era_agent.symbol == Symbol::Era {
                return Some(0);
            }
        }
        return None;
    }

    // Step 4: Walk application chain from lambda_x.p2 (body result)
    // In Normal Form, lambda_x.p2 connects to the outermost application's p2.
    let mut count: u64 = 0;
    let mut current = PortRef::AgentPort(lam_x, 2);

    loop {
        let target = net.get_target(current);
        if target == DISCONNECTED {
            return None;
        }
        match target {
            PortRef::AgentPort(app_id, 2) => {
                // This is an application agent (CON used as @)
                let agent = net.get_agent(app_id)?;
                if agent.symbol != Symbol::Con {
                    return None;
                }
                count += 1;
                // Follow to the application's argument port (p1),
                // which connects to the next application's result (p2)
                // or to lambda_x.p1 for the innermost application
                current = PortRef::AgentPort(app_id, 1);
            }
            PortRef::AgentPort(id, 1) if id == lam_x => {
                // Reached the x variable binding — end of chain
                break;
            }
            _ => return None,
        }
    }

    Some(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{PortRef, Symbol};

    // --- ET-1: Structure test for Church(0) ---
    #[test]
    fn test_encode_nat_0_structure() {
        let net = encode_nat(0);
        assert_eq!(net.count_live_agents(), 3); // 2 CON + 1 ERA

        // Verify agent symbols
        let agents: Vec<_> = net.live_agents().collect();
        let con_count = agents.iter().filter(|a| a.symbol == Symbol::Con).count();
        let era_count = agents.iter().filter(|a| a.symbol == Symbol::Era).count();
        assert_eq!(con_count, 2);
        assert_eq!(era_count, 1);

        // Verify zero redexes (Normal Form)
        assert!(net.redex_queue.is_empty());

        // Verify root is set to outer lambda's principal port
        assert!(net.root.is_some());
        if let Some(PortRef::AgentPort(id, 0)) = net.root {
            let agent = net.get_agent(id).unwrap();
            assert_eq!(agent.symbol, Symbol::Con);
        } else {
            panic!("root must be AgentPort(_, 0)");
        }
    }

    // --- ET-2: Structure test for Church(1) ---
    #[test]
    fn test_encode_nat_1_structure() {
        let net = encode_nat(1);
        assert_eq!(net.count_live_agents(), 3); // 3 CON, 0 DUP, 0 ERA

        let agents: Vec<_> = net.live_agents().collect();
        let con_count = agents.iter().filter(|a| a.symbol == Symbol::Con).count();
        let dup_count = agents.iter().filter(|a| a.symbol == Symbol::Dup).count();
        let era_count = agents.iter().filter(|a| a.symbol == Symbol::Era).count();
        assert_eq!(con_count, 3);
        assert_eq!(dup_count, 0);
        assert_eq!(era_count, 0);

        assert!(net.redex_queue.is_empty());
    }

    // --- ET-3: Structure test for Church(2) ---
    #[test]
    fn test_encode_nat_2_structure() {
        let net = encode_nat(2);
        // (n+2) CON + (n-1) DUP = 4 CON + 1 DUP = 5 agents
        assert_eq!(net.count_live_agents(), 5);

        let agents: Vec<_> = net.live_agents().collect();
        let con_count = agents.iter().filter(|a| a.symbol == Symbol::Con).count();
        let dup_count = agents.iter().filter(|a| a.symbol == Symbol::Dup).count();
        assert_eq!(con_count, 4);
        assert_eq!(dup_count, 1);

        assert!(net.redex_queue.is_empty());
    }

    // --- ET-4: Normal Form test ---
    #[test]
    fn test_encode_nat_normal_form() {
        for n in [0, 1, 2, 5, 10, 100] {
            let net = encode_nat(n);
            assert!(
                net.redex_queue.is_empty(),
                "Church({n}) must have zero redexes"
            );
        }
    }

    // --- ET-5: Roundtrip test (encode then decode) ---
    #[test]
    fn test_encode_decode_roundtrip() {
        for n in [0, 1, 2, 3, 5, 10, 50, 100] {
            let net = encode_nat(n);
            let decoded = decode_nat(&net);
            assert_eq!(
                decoded,
                Some(n),
                "decode_nat(encode_nat({n})) should be Some({n}), got {:?}",
                decoded
            );
        }
    }

    // --- ET-9: Invariant preservation (agent count formula) ---
    #[test]
    fn test_encode_nat_agent_count_formula() {
        // n=0: 3 agents; n=1: 3 agents; n>=2: 2n+1 agents
        assert_eq!(encode_nat(0).count_live_agents(), 3);
        assert_eq!(encode_nat(1).count_live_agents(), 3);
        for n in [2u64, 3, 5, 10, 20, 50] {
            let net = encode_nat(n);
            let expected = 2 * n as usize + 1;
            assert_eq!(
                net.count_live_agents(),
                expected,
                "Church({n}) should have {expected} agents, got {}",
                net.count_live_agents()
            );
        }
    }

    // --- ET-12: Decode rejection tests ---
    #[test]
    fn test_decode_nat_rejects_non_church() {
        // Empty net
        let net = Net::new();
        assert_eq!(decode_nat(&net), None);

        // Net with redexes (not Normal Form)
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        assert_eq!(decode_nat(&net), None);

        // ep_annihilation net (ERA-ERA pairs, not a Church numeral)
        let mut net = Net::new();
        let e1 = net.create_agent(Symbol::Era);
        let e2 = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(e1, 0), PortRef::AgentPort(e2, 0));
        net.root = Some(PortRef::AgentPort(e1, 0));
        // Has a redex
        assert_eq!(decode_nat(&net), None);
    }

    #[test]
    #[should_panic(expected = "exceeds maximum")]
    fn test_encode_nat_exceeds_max() {
        encode_nat(10_001);
    }

    // Verify port connections for Church(0) match SPEC-14 Section 4.2
    #[test]
    fn test_church_0_port_connections() {
        let net = encode_nat(0);
        let lam_f = match net.root {
            Some(PortRef::AgentPort(id, 0)) => id,
            _ => panic!("bad root"),
        };

        // lam_f.p1 -> ERA.p0
        let f_p1_target = net.get_target(PortRef::AgentPort(lam_f, 1));
        match f_p1_target {
            PortRef::AgentPort(era_id, 0) => {
                assert_eq!(net.get_agent(era_id).unwrap().symbol, Symbol::Era);
            }
            _ => panic!("lam_f.p1 should connect to ERA.p0"),
        }

        // lam_f.p2 -> lam_x.p0
        let f_p2_target = net.get_target(PortRef::AgentPort(lam_f, 2));
        let lam_x = match f_p2_target {
            PortRef::AgentPort(id, 0) => id,
            _ => panic!("lam_f.p2 should connect to lam_x.p0"),
        };

        // Self-loop: lam_x.p1 <-> lam_x.p2
        assert_eq!(
            net.get_target(PortRef::AgentPort(lam_x, 1)),
            PortRef::AgentPort(lam_x, 2)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(lam_x, 2)),
            PortRef::AgentPort(lam_x, 1)
        );
    }

    // Verify port connections for Church(1) match SPEC-14 Section 4.2
    #[test]
    fn test_church_1_port_connections() {
        let net = encode_nat(1);
        let lam_f = match net.root {
            Some(PortRef::AgentPort(id, 0)) => id,
            _ => panic!("bad root"),
        };

        // lam_f.p1 -> app.p0
        let f_p1_target = net.get_target(PortRef::AgentPort(lam_f, 1));
        let app = match f_p1_target {
            PortRef::AgentPort(id, 0) => id,
            _ => panic!("lam_f.p1 should connect to app.p0"),
        };
        assert_eq!(net.get_agent(app).unwrap().symbol, Symbol::Con);

        // lam_f.p2 -> lam_x.p0
        let lam_x = match net.get_target(PortRef::AgentPort(lam_f, 2)) {
            PortRef::AgentPort(id, 0) => id,
            _ => panic!("lam_f.p2 should connect to lam_x.p0"),
        };

        // lam_x.p1 -> app.p1
        assert_eq!(
            net.get_target(PortRef::AgentPort(lam_x, 1)),
            PortRef::AgentPort(app, 1)
        );
        // lam_x.p2 -> app.p2
        assert_eq!(
            net.get_target(PortRef::AgentPort(lam_x, 2)),
            PortRef::AgentPort(app, 2)
        );
    }
}
