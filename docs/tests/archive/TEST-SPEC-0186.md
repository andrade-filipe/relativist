# TEST-SPEC-0186: Statistical functions

**Task:** TASK-0186
**Spec:** SPEC-09
**Requirements verified:** R32, R33, R34

---

## Tests

### T1: mean of known values
**Input:** `&[1.0, 2.0, 3.0, 4.0, 5.0]`
**Expected:** `3.0`

### T2: std_dev of known values
**Input:** `&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]`
**Expected:** `~2.138` (within 0.001)

### T3: median odd length
**Input:** `&[1.0, 3.0, 5.0]`
**Expected:** `3.0`

### T4: median even length
**Input:** `&[1.0, 2.0, 3.0, 4.0]`
**Expected:** `2.5`

### T5: mean of empty slice
**Input:** `&[]`
**Expected:** `0.0`

### T6: std_dev of single element
**Input:** `&[42.0]`
**Expected:** `0.0`

### T7: coeff_of_variation no variance
**Input:** `&[10.0, 10.0, 10.0]`
**Expected:** `0.0`

### T8: coeff_of_variation empty
**Input:** `&[]`
**Expected:** `0.0`

### T9: min_f64 and max_f64
**Input:** `&[3.0, 1.0, 4.0, 1.5, 9.0, 2.6]`
**Expected:** min=`1.0`, max=`9.0`

### T10: min_f64 and max_f64 empty
**Input:** `&[]`
**Expected:** min=`f64::INFINITY`, max=`f64::NEG_INFINITY`

## Edge Cases

1. **Unsorted input for median:** `&[5.0, 1.0, 3.0]` should still return `3.0`.
2. **All same values:** `std_dev(&[7.0, 7.0, 7.0])` == `0.0`.
