# CV Triage — v1_local_baseline

- **Threshold:** CV > 0.15
- **Baseline dir:** `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\codigo\relativist\results\locked\v1_local_baseline`
- **Flagged datapoints:** 63

Default dispositions are automatic and conservative. Review each row manually; change `keep` to `exclude` to drop from the article plots, or to `rerun` to regenerate before signing off the snapshot.

## Phase 1 (lenient)

| Benchmark | Size | Mode | Workers | Reps | Correct | Mean (s) | CV | Disposition | Reason |
|---|---|---|---|---|---|---|---|---|---|
| tree_sum_balanced | 10 | sequential | 0 | 10 | true | 0.000000 | 1.2268 | keep | tiny wall-clock (0.00 ms); CV 1.227 is timer noise, not variance |
| tree_sum | 10 | sequential | 0 | 10 | true | 0.000000 | 1.1487 | keep | tiny wall-clock (0.00 ms); CV 1.149 is timer noise, not variance |
| church_add | 10 | sequential | 0 | 10 | true | 0.000000 | 1.0403 | keep | tiny wall-clock (0.00 ms); CV 1.040 is timer noise, not variance |
| erasure_propagation | 100 | sequential | 0 | 10 | true | 0.000005 | 0.8805 | keep | tiny wall-clock (0.01 ms); CV 0.880 is timer noise, not variance |
| church_mul | 4 | sequential | 0 | 10 | true | 0.000001 | 0.7553 | keep | tiny wall-clock (0.00 ms); CV 0.755 is timer noise, not variance |
| church_mul | 100 | sequential | 0 | 10 | true | 0.000002 | 0.7290 | keep | tiny wall-clock (0.00 ms); CV 0.729 is timer noise, not variance |
| tree_sum | 100 | local | 4 | 10 | true | 0.000065 | 0.7092 | keep | tiny wall-clock (0.06 ms); CV 0.709 is timer noise, not variance |
| ep_annihilation | 100 | sequential | 0 | 10 | true | 0.000001 | 0.6877 | keep | tiny wall-clock (0.00 ms); CV 0.688 is timer noise, not variance |
| tree_sum_balanced | 500 | sequential | 0 | 10 | true | 0.000001 | 0.6578 | keep | tiny wall-clock (0.00 ms); CV 0.658 is timer noise, not variance |
| church_add | 500 | sequential | 0 | 10 | true | 0.000001 | 0.6161 | keep | tiny wall-clock (0.00 ms); CV 0.616 is timer noise, not variance |
| condup_expansion | 100 | sequential | 0 | 10 | true | 0.000009 | 0.5706 | keep | tiny wall-clock (0.01 ms); CV 0.571 is timer noise, not variance |
| tree_sum_balanced | 100 | local | 1 | 10 | true | 0.000001 | 0.5449 | keep | tiny wall-clock (0.00 ms); CV 0.545 is timer noise, not variance |
| church_add | 1000 | sequential | 0 | 10 | true | 0.000001 | 0.5031 | keep | tiny wall-clock (0.00 ms); CV 0.503 is timer noise, not variance |
| tree_sum | 500 | sequential | 0 | 10 | true | 0.000000 | 0.4919 | keep | tiny wall-clock (0.00 ms); CV 0.492 is timer noise, not variance |
| cascade_cross | 10 | sequential | 0 | 10 | true | 0.000001 | 0.4859 | keep | tiny wall-clock (0.00 ms); CV 0.486 is timer noise, not variance |
| ep_annihilation_con | 500 | local | 8 | 10 | true | 0.000260 | 0.4362 | keep | tiny wall-clock (0.26 ms); CV 0.436 is timer noise, not variance |
| tree_sum | 1000 | sequential | 0 | 10 | true | 0.000001 | 0.4295 | keep | tiny wall-clock (0.00 ms); CV 0.429 is timer noise, not variance |
| tree_sum_balanced | 1000 | sequential | 0 | 10 | true | 0.000001 | 0.4178 | keep | tiny wall-clock (0.00 ms); CV 0.418 is timer noise, not variance |
| dual_tree | 4 | sequential | 0 | 10 | true | 0.000002 | 0.3778 | keep | tiny wall-clock (0.00 ms); CV 0.378 is timer noise, not variance |
| church_mul | 16 | sequential | 0 | 10 | true | 0.000001 | 0.3377 | keep | tiny wall-clock (0.00 ms); CV 0.338 is timer noise, not variance |
| ep_annihilation | 1000 | local | 4 | 10 | true | 0.000190 | 0.3079 | keep | tiny wall-clock (0.19 ms); CV 0.308 is timer noise, not variance |
| ep_annihilation_con | 10000 | sequential | 0 | 10 | true | 0.001429 | 0.3058 | keep | tiny wall-clock (1.43 ms); CV 0.306 is timer noise, not variance |
| tree_sum | 100 | local | 2 | 10 | true | 0.000043 | 0.3012 | keep | tiny wall-clock (0.04 ms); CV 0.301 is timer noise, not variance |
| tree_sum | 1000 | local | 8 | 10 | true | 0.000647 | 0.2890 | keep | tiny wall-clock (0.65 ms); CV 0.289 is timer noise, not variance |
| church_mul | 9 | sequential | 0 | 10 | true | 0.000001 | 0.2762 | keep | tiny wall-clock (0.00 ms); CV 0.276 is timer noise, not variance |
| tree_sum_balanced | 500 | local | 1 | 10 | true | 0.000002 | 0.2597 | keep | tiny wall-clock (0.00 ms); CV 0.260 is timer noise, not variance |
| tree_sum_balanced | 1000 | local | 2 | 10 | true | 0.000510 | 0.2566 | keep | tiny wall-clock (0.51 ms); CV 0.257 is timer noise, not variance |
| mixed_net | 100 | local | 1 | 10 | true | 0.000050 | 0.2549 | keep | tiny wall-clock (0.05 ms); CV 0.255 is timer noise, not variance |
| tree_sum | 1000 | local | 1 | 10 | true | 0.000003 | 0.2518 | keep | tiny wall-clock (0.00 ms); CV 0.252 is timer noise, not variance |
| church_mul | 25 | sequential | 0 | 10 | true | 0.000001 | 0.2501 | keep | tiny wall-clock (0.00 ms); CV 0.250 is timer noise, not variance |
| ep_annihilation | 10000 | sequential | 0 | 10 | true | 0.000072 | 0.2427 | keep | tiny wall-clock (0.07 ms); CV 0.243 is timer noise, not variance |
| erasure_propagation | 100 | local | 1 | 10 | true | 0.000003 | 0.2426 | keep | tiny wall-clock (0.00 ms); CV 0.243 is timer noise, not variance |
| tree_sum | 10 | local | 8 | 10 | true | 0.000016 | 0.2274 | keep | tiny wall-clock (0.02 ms); CV 0.227 is timer noise, not variance |
| dual_tree | 4 | local | 1 | 10 | true | 0.000002 | 0.2179 | keep | tiny wall-clock (0.00 ms); CV 0.218 is timer noise, not variance |
| ep_annihilation_con | 5000 | local | 1 | 10 | true | 0.000716 | 0.2145 | keep | tiny wall-clock (0.72 ms); CV 0.214 is timer noise, not variance |
| ep_annihilation_con | 5000 | local | 4 | 10 | true | 0.001503 | 0.2110 | keep | tiny wall-clock (1.50 ms); CV 0.211 is timer noise, not variance |
| tree_sum_balanced | 100 | sequential | 0 | 10 | true | 0.000000 | 0.2108 | keep | tiny wall-clock (0.00 ms); CV 0.211 is timer noise, not variance |
| tree_sum_balanced | 1000 | local | 1 | 10 | true | 0.000002 | 0.2075 | keep | tiny wall-clock (0.00 ms); CV 0.207 is timer noise, not variance |
| condup_expansion | 500 | local | 1 | 10 | true | 0.000071 | 0.2063 | keep | tiny wall-clock (0.07 ms); CV 0.206 is timer noise, not variance |
| tree_sum | 50 | sequential | 0 | 10 | true | 0.000000 | 0.1986 | keep | tiny wall-clock (0.00 ms); CV 0.199 is timer noise, not variance |
| ep_annihilation | 100000 | local | 8 | 10 | true | 0.034687 | 0.1931 | keep | CV 0.193 above 0.15 threshold but < 0.30; keep with footnote in the article |
| tree_sum | 100 | sequential | 0 | 10 | true | 0.000000 | 0.1917 | keep | tiny wall-clock (0.00 ms); CV 0.192 is timer noise, not variance |
| tree_sum_balanced | 50 | sequential | 0 | 10 | true | 0.000000 | 0.1917 | keep | tiny wall-clock (0.00 ms); CV 0.192 is timer noise, not variance |
| ep_annihilation_con | 100 | sequential | 0 | 10 | true | 0.000010 | 0.1914 | keep | tiny wall-clock (0.01 ms); CV 0.191 is timer noise, not variance |
| ep_annihilation_dup | 100000 | local | 1 | 10 | true | 0.015081 | 0.1890 | keep | CV 0.189 above 0.15 threshold but < 0.30; keep with footnote in the article |
| mixed_net | 500 | local | 1 | 10 | true | 0.000246 | 0.1866 | keep | tiny wall-clock (0.25 ms); CV 0.187 is timer noise, not variance |
| mixed_net | 10000 | local | 1 | 10 | true | 0.005664 | 0.1862 | keep | tiny wall-clock (5.66 ms); CV 0.186 is timer noise, not variance |
| ep_annihilation_con | 10000 | local | 1 | 10 | true | 0.001508 | 0.1859 | keep | tiny wall-clock (1.51 ms); CV 0.186 is timer noise, not variance |
| church_add | 100 | sequential | 0 | 10 | true | 0.000000 | 0.1831 | keep | tiny wall-clock (0.00 ms); CV 0.183 is timer noise, not variance |
| church_add | 1000 | local | 1 | 10 | true | 0.000003 | 0.1712 | keep | tiny wall-clock (0.00 ms); CV 0.171 is timer noise, not variance |
| ep_annihilation_con | 50000 | local | 2 | 10 | true | 0.015765 | 0.1670 | keep | CV 0.167 above 0.15 threshold but < 0.30; keep with footnote in the article |
| ep_annihilation_dup | 100 | sequential | 0 | 10 | true | 0.000009 | 0.1661 | keep | tiny wall-clock (0.01 ms); CV 0.166 is timer noise, not variance |
| tree_sum | 500 | local | 1 | 10 | true | 0.000002 | 0.1661 | keep | tiny wall-clock (0.00 ms); CV 0.166 is timer noise, not variance |
| erasure_propagation | 5000 | sequential | 0 | 10 | true | 0.000245 | 0.1614 | keep | tiny wall-clock (0.24 ms); CV 0.161 is timer noise, not variance |
| tree_sum_balanced | 500 | local | 2 | 10 | true | 0.000211 | 0.1592 | keep | tiny wall-clock (0.21 ms); CV 0.159 is timer noise, not variance |
| tree_sum_balanced | 500 | local | 8 | 10 | true | 0.000303 | 0.1585 | keep | tiny wall-clock (0.30 ms); CV 0.159 is timer noise, not variance |
| ep_annihilation_con | 5000 | sequential | 0 | 10 | true | 0.000682 | 0.1562 | keep | tiny wall-clock (0.68 ms); CV 0.156 is timer noise, not variance |
| ep_annihilation_dup | 100000 | local | 2 | 10 | true | 0.034381 | 0.1547 | keep | CV 0.155 above 0.15 threshold but < 0.30; keep with footnote in the article |
| tree_sum | 100 | local | 8 | 10 | true | 0.000065 | 0.1520 | keep | tiny wall-clock (0.06 ms); CV 0.152 is timer noise, not variance |
| ep_annihilation | 1000 | local | 1 | 10 | true | 0.000012 | 0.1514 | keep | tiny wall-clock (0.01 ms); CV 0.151 is timer noise, not variance |

## Phase 1 (strict)

| Benchmark | Size | Mode | Workers | Reps | Correct | Mean (s) | CV | Disposition | Reason |
|---|---|---|---|---|---|---|---|---|---|
| cascade_cross | 10 | sequential | 0 | 10 | true | 0.000001 | 0.4534 | keep | tiny wall-clock (0.00 ms); CV 0.453 is timer noise, not variance |
| dual_tree | 6 | sequential | 0 | 10 | true | 0.000006 | 0.2526 | keep | tiny wall-clock (0.01 ms); CV 0.253 is timer noise, not variance |

## Phase 2 (Docker)

| Benchmark | Size | Mode | Workers | Reps | Correct | Mean (s) | CV | Disposition | Reason |
|---|---|---|---|---|---|---|---|---|---|
| condup_expansion | 1000 | tcp_localhost | 1 | 10 | true | 0.001991 | 0.1718 | keep | tiny wall-clock (1.99 ms); CV 0.172 is timer noise, not variance |

---

Excluded rows (if any) should be listed in the article as a 'datapoints descartados por variancia' footnote; rerun rows block the snapshot sign-off until regenerated.
