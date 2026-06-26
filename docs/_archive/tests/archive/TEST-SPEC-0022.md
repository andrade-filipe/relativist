# TEST-SPEC-0022: Implement normalize_pair function

**Task:** TASK-0022
**Spec:** SPEC-03 R9
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Con-Dup pair returned as-is
Create CON(a) and DUP(b). `normalize_pair(a, b, &net)` returns `(a, b)` (Con < Dup).

### T2: Dup-Con pair is swapped
Create DUP(a) and CON(b). `normalize_pair(a, b, &net)` returns `(b, a)`.

### T3: Equal symbols not swapped (Con-Con)
Create CON(a) and CON(b). `normalize_pair(a, b, &net)` returns `(a, b)`.

### T4: Era-Con pair is swapped
Create ERA(a) and CON(b). `normalize_pair(a, b, &net)` returns `(b, a)`.

### T5: Era-Era pair returned as-is
Create ERA(a) and ERA(b). `normalize_pair(a, b, &net)` returns `(a, b)`.

### T6: Dup-Era pair returned as-is
Create DUP(a) and ERA(b). `normalize_pair(a, b, &net)` returns `(a, b)` (Dup < Era).

### T7: Era-Dup pair is swapped
Create ERA(a) and DUP(b). `normalize_pair(a, b, &net)` returns `(b, a)`.

## Edge Cases

### E1: All 9 symbol combinations covered
Test all 9 (a_sym, b_sym) pairs to verify normalization produces sym_a <= sym_b.

### E2: Stable sort (equal symbols, original order preserved)
Two CON agents a and b where a < b. `normalize_pair(a, b, ...)` returns `(a, b)`, not `(b, a)`.
