# TEST-SPEC-0021: Rule enum and dispatch table

**Task:** TASK-0021
**Spec:** SPEC-03 R8, R9 (Sections 4.3, 4.3.1, 4.4)
**Generated:** 2026-04-06

---

## Unit Tests — Rule enum

### T1: Rule has exactly 4 variants (exhaustive match compiles)
### T2: Rule repr values: Anni=0, Comm=1, Eras=2, Void=3
### T3: Rule derives Debug, Clone, Copy, PartialEq, Eq
### T4: Rule Debug formatting matches variant names

## Unit Tests — SpecificRule enum

### T5: SpecificRule has exactly 6 variants (exhaustive match compiles)
### T6: SpecificRule repr values: ConCon=0, ConDup=1, ConEra=2, DupDup=3, DupEra=4, EraEra=5
### T7: SpecificRule derives Debug, Clone, Copy, PartialEq, Eq

## Unit Tests — DISPATCH_TABLE (get_rule)

### T8: All 9 combinations return expected Rule
- (Con, Con) -> Anni
- (Con, Dup) -> Comm
- (Con, Era) -> Eras
- (Dup, Con) -> Comm
- (Dup, Dup) -> Anni
- (Dup, Era) -> Eras
- (Era, Con) -> Eras
- (Era, Dup) -> Eras
- (Era, Era) -> Void

### T9: Symmetry — get_rule(a, b) == get_rule(b, a) for all (a, b)

## Unit Tests — SPECIFIC_RULE_TABLE (get_specific_rule)

### T10: All 9 combinations return expected SpecificRule
- (Con, Con) -> ConCon
- (Con, Dup) -> ConDup
- (Con, Era) -> ConEra
- (Dup, Con) -> ConDup
- (Dup, Dup) -> DupDup
- (Dup, Era) -> DupEra
- (Era, Con) -> ConEra
- (Era, Dup) -> DupEra
- (Era, Era) -> EraEra

### T11: Symmetry — get_specific_rule(a, b) == get_specific_rule(b, a) for all (a, b)

## Unit Tests — normalize_pair

### T12: Already-normalized pairs are unchanged (sym_a <= sym_b)
### T13: Reversed pairs are swapped (sym_a > sym_b)
### T14: Equal-symbol pairs are unchanged

## Edge Cases

### E1: Rule and SpecificRule size is 1 byte each (repr(u8))
### E2: get_rule is const fn (usable in const context)
### E3: get_specific_rule is const fn (usable in const context)
