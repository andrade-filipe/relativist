# D-014 Stress Curve Campaign — MANIFEST

- git rev: 502ae692db897e1b49855e36b4b0ab7c08d58b93
- rustc:   rustc 1.94.1 (e408947bf 2026-03-25)
- cargo:   cargo 1.94.1 (29ea6fb6a 2026-03-24)
- mode:    full
- output:  results/locked/v2_stress_curve_2026-05-15_optC
- run at:  2026-05-16T01:07:26Z

## Environment

```
uname: MINGW64_NT-10.0-26200 Lombardo 3.6.5-22c95533.x86_64 2025-10-10 12:02 UTC x86_64 Msys
rustc: rustc 1.94.1 (e408947bf 2026-03-25)
cargo: cargo 1.94.1 (29ea6fb6a 2026-03-24)
git rev: 502ae692db897e1b49855e36b4b0ab7c08d58b93
meminfo: MemTotal:       33203076 kB
cpuinfo: model name	: 13th Gen Intel(R) Core(TM) i7-1365U
```

## Files
./.lock
./MANIFEST.md
./aggregated.csv
./checksums.sha256
./raw/env.txt
./raw/ep_annihilation_1_10000000_1.stderr
./raw/ep_annihilation_1_10000000_2.stderr
./raw/ep_annihilation_1_10000000_3.stderr
./raw/ep_annihilation_1_10000000_4.stderr
./raw/ep_annihilation_1_10000000_5.stderr
./raw/ep_annihilation_1_1000000_1.stderr
./raw/ep_annihilation_1_1000000_2.stderr
./raw/ep_annihilation_1_1000000_3.stderr
./raw/ep_annihilation_1_1000000_4.stderr
./raw/ep_annihilation_1_1000000_5.stderr
./raw/ep_annihilation_1_100000_1.stderr
./raw/ep_annihilation_1_100000_2.stderr
./raw/ep_annihilation_1_100000_3.stderr
./raw/ep_annihilation_1_100000_4.stderr
./raw/ep_annihilation_1_100000_5.stderr
./raw/ep_annihilation_1_10000_1.stderr
./raw/ep_annihilation_1_10000_2.stderr
./raw/ep_annihilation_1_10000_3.stderr
./raw/ep_annihilation_1_10000_4.stderr
./raw/ep_annihilation_1_10000_5.stderr
./raw/ep_annihilation_1_3162278_1.stderr
./raw/ep_annihilation_1_3162278_2.stderr
./raw/ep_annihilation_1_3162278_3.stderr
./raw/ep_annihilation_1_3162278_4.stderr
./raw/ep_annihilation_1_3162278_5.stderr
./raw/ep_annihilation_1_316228_1.stderr
./raw/ep_annihilation_1_316228_2.stderr
./raw/ep_annihilation_1_316228_3.stderr
./raw/ep_annihilation_1_316228_4.stderr
./raw/ep_annihilation_1_316228_5.stderr
./raw/ep_annihilation_1_31623_1.stderr
./raw/ep_annihilation_1_31623_2.stderr
./raw/ep_annihilation_1_31623_3.stderr
./raw/ep_annihilation_1_31623_4.stderr
./raw/ep_annihilation_1_31623_5.stderr
./raw/ep_annihilation_2_10000000_1.stderr
./raw/ep_annihilation_2_10000000_2.stderr
./raw/ep_annihilation_2_10000000_3.stderr
./raw/ep_annihilation_2_10000000_4.stderr
./raw/ep_annihilation_2_10000000_5.stderr
./raw/ep_annihilation_2_1000000_1.stderr
./raw/ep_annihilation_2_1000000_2.stderr
./raw/ep_annihilation_2_1000000_3.stderr
./raw/ep_annihilation_2_1000000_4.stderr
./raw/ep_annihilation_2_1000000_5.stderr
./raw/ep_annihilation_2_100000_1.stderr
./raw/ep_annihilation_2_100000_2.stderr
./raw/ep_annihilation_2_100000_3.stderr
./raw/ep_annihilation_2_100000_4.stderr
./raw/ep_annihilation_2_100000_5.stderr
./raw/ep_annihilation_2_10000_1.stderr
./raw/ep_annihilation_2_10000_2.stderr
./raw/ep_annihilation_2_10000_3.stderr
./raw/ep_annihilation_2_10000_4.stderr
./raw/ep_annihilation_2_10000_5.stderr
./raw/ep_annihilation_2_3162278_1.stderr
./raw/ep_annihilation_2_3162278_2.stderr
./raw/ep_annihilation_2_3162278_3.stderr
./raw/ep_annihilation_2_3162278_4.stderr
./raw/ep_annihilation_2_3162278_5.stderr
./raw/ep_annihilation_2_316228_1.stderr
./raw/ep_annihilation_2_316228_2.stderr
./raw/ep_annihilation_2_316228_3.stderr
./raw/ep_annihilation_2_316228_4.stderr
./raw/ep_annihilation_2_316228_5.stderr
./raw/ep_annihilation_2_31623_1.stderr
./raw/ep_annihilation_2_31623_2.stderr
./raw/ep_annihilation_2_31623_3.stderr
./raw/ep_annihilation_2_31623_4.stderr
./raw/ep_annihilation_2_31623_5.stderr
./raw/ep_annihilation_4_10000000_1.stderr
./raw/ep_annihilation_4_10000000_2.stderr
./raw/ep_annihilation_4_10000000_3.stderr
./raw/ep_annihilation_4_10000000_4.stderr
./raw/ep_annihilation_4_10000000_5.stderr
./raw/ep_annihilation_4_1000000_1.stderr
./raw/ep_annihilation_4_1000000_2.stderr
./raw/ep_annihilation_4_1000000_3.stderr
./raw/ep_annihilation_4_1000000_4.stderr
./raw/ep_annihilation_4_1000000_5.stderr
./raw/ep_annihilation_4_100000_1.stderr
./raw/ep_annihilation_4_100000_2.stderr
./raw/ep_annihilation_4_100000_3.stderr
./raw/ep_annihilation_4_100000_4.stderr
./raw/ep_annihilation_4_100000_5.stderr
./raw/ep_annihilation_4_10000_1.stderr
./raw/ep_annihilation_4_10000_2.stderr
./raw/ep_annihilation_4_10000_3.stderr
./raw/ep_annihilation_4_10000_4.stderr
./raw/ep_annihilation_4_10000_5.stderr
./raw/ep_annihilation_4_3162278_1.stderr
./raw/ep_annihilation_4_3162278_2.stderr
./raw/ep_annihilation_4_3162278_3.stderr
./raw/ep_annihilation_4_3162278_4.stderr
./raw/ep_annihilation_4_3162278_5.stderr
./raw/ep_annihilation_4_316228_1.stderr
./raw/ep_annihilation_4_316228_2.stderr
./raw/ep_annihilation_4_316228_3.stderr
./raw/ep_annihilation_4_316228_4.stderr
./raw/ep_annihilation_4_316228_5.stderr
./raw/ep_annihilation_4_31623_1.stderr
./raw/ep_annihilation_4_31623_2.stderr
./raw/ep_annihilation_4_31623_3.stderr
./raw/ep_annihilation_4_31623_4.stderr
./raw/ep_annihilation_4_31623_5.stderr
./raw/ep_annihilation_8_10000000_1.stderr
./raw/ep_annihilation_8_10000000_2.stderr
./raw/ep_annihilation_8_10000000_3.stderr
./raw/ep_annihilation_8_10000000_4.stderr
./raw/ep_annihilation_8_10000000_5.stderr
./raw/ep_annihilation_8_1000000_1.stderr
./raw/ep_annihilation_8_1000000_2.stderr
./raw/ep_annihilation_8_1000000_3.stderr
./raw/ep_annihilation_8_1000000_4.stderr
./raw/ep_annihilation_8_1000000_5.stderr
./raw/ep_annihilation_8_100000_1.stderr
./raw/ep_annihilation_8_100000_2.stderr
./raw/ep_annihilation_8_100000_3.stderr
./raw/ep_annihilation_8_100000_4.stderr
./raw/ep_annihilation_8_100000_5.stderr
./raw/ep_annihilation_8_10000_1.stderr
./raw/ep_annihilation_8_10000_2.stderr
./raw/ep_annihilation_8_10000_3.stderr
./raw/ep_annihilation_8_10000_4.stderr
./raw/ep_annihilation_8_10000_5.stderr
./raw/ep_annihilation_8_3162278_1.stderr
./raw/ep_annihilation_8_3162278_2.stderr
./raw/ep_annihilation_8_3162278_3.stderr
./raw/ep_annihilation_8_3162278_4.stderr
./raw/ep_annihilation_8_3162278_5.stderr
./raw/ep_annihilation_8_316228_1.stderr
./raw/ep_annihilation_8_316228_2.stderr
./raw/ep_annihilation_8_316228_3.stderr
./raw/ep_annihilation_8_316228_4.stderr
./raw/ep_annihilation_8_316228_5.stderr
./raw/ep_annihilation_8_31623_1.stderr
./raw/ep_annihilation_8_31623_2.stderr
./raw/ep_annihilation_8_31623_3.stderr
./raw/ep_annihilation_8_31623_4.stderr
./raw/ep_annihilation_8_31623_5.stderr
./raw/in_process.csv
