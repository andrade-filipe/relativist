# TEST-SPEC-0027: Define StepResult and implement reduce_step

**Task:** TASK-0027
**Spec:** SPEC-03 Sections 4.6.1, R8, R9, R12
**Generated:** 2026-04-06

---

## Unit Tests — StepResult enum

### T1: StepResult has exactly 2 variants (NormalForm, Reduced(Rule, SpecificRule))
### T2: StepResult derives Debug, Clone, Copy, PartialEq, Eq

## Unit Tests — reduce_step

### T3: Empty net returns NormalForm
### T4: ERA-ERA pair returns Reduced(Void, EraEra)
### T5: CON-CON pair returns Reduced(Anni, ConCon)
### T6: DUP-DUP pair returns Reduced(Anni, DupDup)
### T7: CON-DUP pair returns Reduced(Comm, ConDup)
### T8: CON-ERA pair returns Reduced(Eras, ConEra)
### T9: DUP-ERA pair returns Reduced(Eras, DupEra)
### T10: Stale redex is silently discarded, next valid redex processed
### T11: All stale redexes return NormalForm when queue empties
### T12: reduce_step applies the correct rule (net state mutated)

## Edge Cases

### E1: Multiple redexes -- reduce_step processes exactly one
### E2: reduce_step with reversed pair (DUP-CON) still dispatches correctly via normalize_pair
