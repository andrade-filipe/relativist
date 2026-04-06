//! Rule dispatch: enum, dispatch table, and pair normalization.
//!
//! Maps each pair of symbols to the correct interaction rule in O(1).
//! Implements SPEC-03 Sections 4.3, 4.3.1, and 4.4.

use crate::net::{AgentId, Net, Symbol};

// ---------------------------------------------------------------------------
// Rule (4-variant dispatch category)
// ---------------------------------------------------------------------------

/// Rule constants for dispatch (SPEC-03 Section 4.3).
///
/// Groups the 6 Lafont IC interaction rules into 4 categories that
/// determine which `interact_*` function to call. The distinction
/// between same-symbol annihilations (CON-CON vs DUP-DUP) is not
/// needed at the dispatch level since both use `interact_anni`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Rule {
    /// Annihilation: same symbol, arity 2 (CON-CON or DUP-DUP).
    Anni = 0,
    /// Commutation: CON-DUP or DUP-CON (different symbols, both arity 2).
    Comm = 1,
    /// Erasure: ERA with CON or DUP (erasure propagation).
    Eras = 2,
    /// Void: ERA-ERA (erasers annihilate).
    Void = 3,
}

// ---------------------------------------------------------------------------
// SpecificRule (6-variant per-rule discriminant)
// ---------------------------------------------------------------------------

/// Identifies each of the 6 Lafont IC interaction rules individually.
///
/// Used for per-rule interaction counting in `ReductionStats` (SPEC-03 R17).
/// The array index assignment is canonical and used by:
/// - `ReductionStats.interactions_by_rule: [u64; 6]` (SPEC-03 Section 4.6.2)
/// - `WorkerRoundStats.interactions_by_rule` (SPEC-05 R37)
/// - Prometheus `interactions_by_rule_total` metric (SPEC-11 R12)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SpecificRule {
    ConCon = 0,
    ConDup = 1,
    ConEra = 2,
    DupDup = 3,
    DupEra = 4,
    EraEra = 5,
}

// ---------------------------------------------------------------------------
// Dispatch tables
// ---------------------------------------------------------------------------

/// 3x3 dispatch table. Indexed by `(Symbol as usize, Symbol as usize)`.
/// Symmetric: `TABLE[a][b] == TABLE[b][a]`.
///
/// ```text
///          Con    Dup    Era
/// Con  [  ANNI   COMM   ERAS ]
/// Dup  [  COMM   ANNI   ERAS ]
/// Era  [  ERAS   ERAS   VOID ]
/// ```
pub const DISPATCH_TABLE: [[Rule; 3]; 3] = [
    // Con row
    [Rule::Anni, Rule::Comm, Rule::Eras],
    // Dup row
    [Rule::Comm, Rule::Anni, Rule::Eras],
    // Era row
    [Rule::Eras, Rule::Eras, Rule::Void],
];

/// 3x3 table mapping `(Symbol, Symbol)` to `SpecificRule`.
/// Symmetric: `TABLE[a][b] == TABLE[b][a]`.
///
/// ```text
///          Con        Dup        Era
/// Con  [ CON_CON    CON_DUP    CON_ERA ]
/// Dup  [ CON_DUP    DUP_DUP    DUP_ERA ]
/// Era  [ CON_ERA    DUP_ERA    ERA_ERA ]
/// ```
pub const SPECIFIC_RULE_TABLE: [[SpecificRule; 3]; 3] = [
    // Con row
    [
        SpecificRule::ConCon,
        SpecificRule::ConDup,
        SpecificRule::ConEra,
    ],
    // Dup row
    [
        SpecificRule::ConDup,
        SpecificRule::DupDup,
        SpecificRule::DupEra,
    ],
    // Era row
    [
        SpecificRule::ConEra,
        SpecificRule::DupEra,
        SpecificRule::EraEra,
    ],
];

// ---------------------------------------------------------------------------
// Dispatch functions
// ---------------------------------------------------------------------------

/// Determines the interaction rule category for a pair of symbols.
///
/// Complexity: O(1) (table index lookup).
/// Satisfies SPEC-03 R8: maps each `(Symbol, Symbol)` to exactly one `Rule`.
#[inline]
pub const fn get_rule(a: Symbol, b: Symbol) -> Rule {
    DISPATCH_TABLE[a as usize][b as usize]
}

/// Determines the specific (6-variant) rule for a pair of symbols.
///
/// Complexity: O(1) (table index lookup).
/// Used for per-rule interaction tracking (SPEC-03 R17).
#[inline]
pub const fn get_specific_rule(a: Symbol, b: Symbol) -> SpecificRule {
    SPECIFIC_RULE_TABLE[a as usize][b as usize]
}

// ---------------------------------------------------------------------------
// Pair normalization
// ---------------------------------------------------------------------------

/// Normalizes the agent pair for dispatch (SPEC-03 Section 4.4, R9).
///
/// Guarantees `sym_a <= sym_b` (by Symbol ordering: Con=0, Dup=1, Era=2).
/// Swaps arguments if necessary.
///
/// This allows:
/// - `interact_anni` to always receive (Con, Con) or (Dup, Dup), never inverted
/// - `interact_comm` to always receive (Con, Dup), never (Dup, Con)
/// - `interact_eras` to always receive (X, Era) where X is Con or Dup, never (Era, X)
/// - `interact_void` to receive (Era, Era)
#[inline]
pub fn normalize_pair(a: AgentId, b: AgentId, net: &Net) -> (AgentId, AgentId) {
    let sym_a = net.agents[a as usize].unwrap().symbol;
    let sym_b = net.agents[b as usize].unwrap().symbol;
    if (sym_a as u8) <= (sym_b as u8) {
        (a, b)
    } else {
        (b, a)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::Symbol;

    // -----------------------------------------------------------------------
    // Rule enum tests
    // -----------------------------------------------------------------------

    // T1: Rule has exactly 4 variants (exhaustive match compiles).
    #[test]
    fn test_rule_exhaustive_match() {
        fn name(r: Rule) -> &'static str {
            match r {
                Rule::Anni => "annihilation",
                Rule::Comm => "commutation",
                Rule::Eras => "erasure",
                Rule::Void => "void",
            }
        }
        assert_eq!(name(Rule::Anni), "annihilation");
        assert_eq!(name(Rule::Comm), "commutation");
        assert_eq!(name(Rule::Eras), "erasure");
        assert_eq!(name(Rule::Void), "void");
    }

    // T2: Rule repr values match spec.
    #[test]
    fn test_rule_repr_values() {
        assert_eq!(Rule::Anni as u8, 0);
        assert_eq!(Rule::Comm as u8, 1);
        assert_eq!(Rule::Eras as u8, 2);
        assert_eq!(Rule::Void as u8, 3);
    }

    // T3: Rule derives Debug, Clone, Copy, PartialEq, Eq.
    #[test]
    fn test_rule_derives() {
        let r = Rule::Anni;
        let r2 = r; // Copy
        let r3 = r.clone(); // Clone
        assert_eq!(r, r2); // PartialEq
        assert_eq!(r, r3);
        assert_ne!(Rule::Anni, Rule::Comm);
    }

    // T4: Rule Debug formatting.
    #[test]
    fn test_rule_debug() {
        assert_eq!(format!("{:?}", Rule::Anni), "Anni");
        assert_eq!(format!("{:?}", Rule::Comm), "Comm");
        assert_eq!(format!("{:?}", Rule::Eras), "Eras");
        assert_eq!(format!("{:?}", Rule::Void), "Void");
    }

    // -----------------------------------------------------------------------
    // SpecificRule enum tests
    // -----------------------------------------------------------------------

    // T5: SpecificRule has exactly 6 variants (exhaustive match compiles).
    #[test]
    fn test_specific_rule_exhaustive_match() {
        fn name(r: SpecificRule) -> &'static str {
            match r {
                SpecificRule::ConCon => "con-con",
                SpecificRule::ConDup => "con-dup",
                SpecificRule::ConEra => "con-era",
                SpecificRule::DupDup => "dup-dup",
                SpecificRule::DupEra => "dup-era",
                SpecificRule::EraEra => "era-era",
            }
        }
        assert_eq!(name(SpecificRule::ConCon), "con-con");
        assert_eq!(name(SpecificRule::ConDup), "con-dup");
        assert_eq!(name(SpecificRule::ConEra), "con-era");
        assert_eq!(name(SpecificRule::DupDup), "dup-dup");
        assert_eq!(name(SpecificRule::DupEra), "dup-era");
        assert_eq!(name(SpecificRule::EraEra), "era-era");
    }

    // T6: SpecificRule repr values.
    #[test]
    fn test_specific_rule_repr_values() {
        assert_eq!(SpecificRule::ConCon as u8, 0);
        assert_eq!(SpecificRule::ConDup as u8, 1);
        assert_eq!(SpecificRule::ConEra as u8, 2);
        assert_eq!(SpecificRule::DupDup as u8, 3);
        assert_eq!(SpecificRule::DupEra as u8, 4);
        assert_eq!(SpecificRule::EraEra as u8, 5);
    }

    // T7: SpecificRule derives Debug, Clone, Copy, PartialEq, Eq.
    #[test]
    fn test_specific_rule_derives() {
        let r = SpecificRule::ConDup;
        let r2 = r; // Copy
        let r3 = r.clone(); // Clone
        assert_eq!(r, r2);
        assert_eq!(r, r3);
        assert_ne!(SpecificRule::ConCon, SpecificRule::DupDup);
    }

    // -----------------------------------------------------------------------
    // DISPATCH_TABLE / get_rule tests
    // -----------------------------------------------------------------------

    // T8: All 9 combinations return expected Rule.
    #[test]
    fn test_get_rule_all_9_combinations() {
        // Diagonal (same symbol):
        assert_eq!(get_rule(Symbol::Con, Symbol::Con), Rule::Anni);
        assert_eq!(get_rule(Symbol::Dup, Symbol::Dup), Rule::Anni);
        assert_eq!(get_rule(Symbol::Era, Symbol::Era), Rule::Void);

        // Off-diagonal (different symbol):
        assert_eq!(get_rule(Symbol::Con, Symbol::Dup), Rule::Comm);
        assert_eq!(get_rule(Symbol::Dup, Symbol::Con), Rule::Comm);
        assert_eq!(get_rule(Symbol::Con, Symbol::Era), Rule::Eras);
        assert_eq!(get_rule(Symbol::Era, Symbol::Con), Rule::Eras);
        assert_eq!(get_rule(Symbol::Dup, Symbol::Era), Rule::Eras);
        assert_eq!(get_rule(Symbol::Era, Symbol::Dup), Rule::Eras);
    }

    // T9: Symmetry -- get_rule(a, b) == get_rule(b, a) for all (a, b).
    #[test]
    fn test_get_rule_symmetry() {
        let symbols = [Symbol::Con, Symbol::Dup, Symbol::Era];
        for &a in &symbols {
            for &b in &symbols {
                assert_eq!(
                    get_rule(a, b),
                    get_rule(b, a),
                    "Symmetry violated for ({:?}, {:?})",
                    a,
                    b
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // SPECIFIC_RULE_TABLE / get_specific_rule tests
    // -----------------------------------------------------------------------

    // T10: All 9 combinations return expected SpecificRule.
    #[test]
    fn test_get_specific_rule_all_9_combinations() {
        assert_eq!(
            get_specific_rule(Symbol::Con, Symbol::Con),
            SpecificRule::ConCon
        );
        assert_eq!(
            get_specific_rule(Symbol::Con, Symbol::Dup),
            SpecificRule::ConDup
        );
        assert_eq!(
            get_specific_rule(Symbol::Con, Symbol::Era),
            SpecificRule::ConEra
        );
        assert_eq!(
            get_specific_rule(Symbol::Dup, Symbol::Con),
            SpecificRule::ConDup
        );
        assert_eq!(
            get_specific_rule(Symbol::Dup, Symbol::Dup),
            SpecificRule::DupDup
        );
        assert_eq!(
            get_specific_rule(Symbol::Dup, Symbol::Era),
            SpecificRule::DupEra
        );
        assert_eq!(
            get_specific_rule(Symbol::Era, Symbol::Con),
            SpecificRule::ConEra
        );
        assert_eq!(
            get_specific_rule(Symbol::Era, Symbol::Dup),
            SpecificRule::DupEra
        );
        assert_eq!(
            get_specific_rule(Symbol::Era, Symbol::Era),
            SpecificRule::EraEra
        );
    }

    // T11: Symmetry -- get_specific_rule(a, b) == get_specific_rule(b, a).
    #[test]
    fn test_get_specific_rule_symmetry() {
        let symbols = [Symbol::Con, Symbol::Dup, Symbol::Era];
        for &a in &symbols {
            for &b in &symbols {
                assert_eq!(
                    get_specific_rule(a, b),
                    get_specific_rule(b, a),
                    "SpecificRule symmetry violated for ({:?}, {:?})",
                    a,
                    b
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // normalize_pair tests
    // -----------------------------------------------------------------------

    /// Helper: creates a minimal net with two agents of given symbols.
    /// Returns (net, id_a, id_b).
    fn make_pair(sym_a: Symbol, sym_b: Symbol) -> (Net, AgentId, AgentId) {
        let mut net = Net::new();
        let id_a = net.create_agent(sym_a);
        let id_b = net.create_agent(sym_b);
        (net, id_a, id_b)
    }

    // T12: Already-normalized pairs are unchanged (sym_a <= sym_b).
    #[test]
    fn test_normalize_pair_already_ordered() {
        let (net, a, b) = make_pair(Symbol::Con, Symbol::Dup);
        let (na, nb) = normalize_pair(a, b, &net);
        assert_eq!(na, a);
        assert_eq!(nb, b);

        let (net, a, b) = make_pair(Symbol::Con, Symbol::Era);
        let (na, nb) = normalize_pair(a, b, &net);
        assert_eq!(na, a);
        assert_eq!(nb, b);

        let (net, a, b) = make_pair(Symbol::Dup, Symbol::Era);
        let (na, nb) = normalize_pair(a, b, &net);
        assert_eq!(na, a);
        assert_eq!(nb, b);
    }

    // T13: Reversed pairs are swapped (sym_a > sym_b).
    #[test]
    fn test_normalize_pair_reversed() {
        let (net, a, b) = make_pair(Symbol::Dup, Symbol::Con);
        let (na, nb) = normalize_pair(a, b, &net);
        // After normalization: Con (b) should be first, Dup (a) second.
        assert_eq!(na, b);
        assert_eq!(nb, a);

        let (net, a, b) = make_pair(Symbol::Era, Symbol::Con);
        let (na, nb) = normalize_pair(a, b, &net);
        assert_eq!(na, b);
        assert_eq!(nb, a);

        let (net, a, b) = make_pair(Symbol::Era, Symbol::Dup);
        let (na, nb) = normalize_pair(a, b, &net);
        assert_eq!(na, b);
        assert_eq!(nb, a);
    }

    // T14: Equal-symbol pairs are unchanged.
    #[test]
    fn test_normalize_pair_equal_symbols() {
        let (net, a, b) = make_pair(Symbol::Con, Symbol::Con);
        let (na, nb) = normalize_pair(a, b, &net);
        assert_eq!(na, a);
        assert_eq!(nb, b);

        let (net, a, b) = make_pair(Symbol::Dup, Symbol::Dup);
        let (na, nb) = normalize_pair(a, b, &net);
        assert_eq!(na, a);
        assert_eq!(nb, b);

        let (net, a, b) = make_pair(Symbol::Era, Symbol::Era);
        let (na, nb) = normalize_pair(a, b, &net);
        assert_eq!(na, a);
        assert_eq!(nb, b);
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    // E1: Rule and SpecificRule size is 1 byte each (repr(u8)).
    #[test]
    fn test_enum_sizes() {
        assert_eq!(std::mem::size_of::<Rule>(), 1);
        assert_eq!(std::mem::size_of::<SpecificRule>(), 1);
    }

    // E2: get_rule is const fn (usable in const context).
    #[test]
    fn test_get_rule_const() {
        const RESULT: Rule = get_rule(Symbol::Con, Symbol::Dup);
        assert_eq!(RESULT, Rule::Comm);
    }

    // E3: get_specific_rule is const fn (usable in const context).
    #[test]
    fn test_get_specific_rule_const() {
        const RESULT: SpecificRule = get_specific_rule(Symbol::Era, Symbol::Era);
        assert_eq!(RESULT, SpecificRule::EraEra);
    }
}
