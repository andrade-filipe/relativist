# QA Review: D-016 — HornerCodec decoder extension

**Date:** 2026-05-16
**Reviewer:** qa agent (Stage 5)
**Bundle commits reviewed:**

- `29c371d` docs(d-016): land Stage 1+2 task-splitter + test-generator
- `a9ab3d0` feat(encoding): TASK-0723/0724 biguint_readback handles cofactor c1>=2 and degree-2 nested Horner
- `9fae64d` test(encoding): TASK-0725 horner pipeline property tests + PT-0715-06 final tightening
- `6a098be` docs(demos): TASK-0726 rewrite Limitacoes section + add scripts/horner_demo.sh

**Files reviewed:**

- `relativist-core/src/encoding/biguint_readback.rs` (1007 LoC)
- `relativist-core/src/encoding/horner.rs` (lines 490-706 — UT-0715-02 + PT-0715-06)
- `relativist-core/tests/horner_pipeline_property.rs` (110 LoC)
- `docs/demos/horner-g1-demonstration.md` (309 LoC, post-rewrite)
- `scripts/horner_demo.sh` (97 LoC)

**Verdict:** **not-safe-to-merge** — P0 silent-correctness regression + P1 doc/spec mismatch.

**Bug verdict:** BUGS FOUND (1 P0, 1 P1, 1 P2)
**Test coverage:** GAPS FOUND (3 — see Test Coverage Gaps section)

---

## Bugs Found

### BUG-001 — Decoder silently returns numerically-wrong values on out-of-scope inputs (P0)

**Severity:** CRITICAL (P0)
**File:** `relativist-core/src/encoding/biguint_readback.rs:522-548` (`read_mult_subnet`) +
`relativist-core/src/encoding/horner.rs` (decode entry point — no guard)
**Category:** Logic Error / Silent-Wrong-Answer / Trust Boundary

**Description.** The decoder, when fed a HornerCodec input that lies in the
documented "out-of-scope" envelope (degree ≥ 3, OR degree-2 with leading
coefficient ≥ 2, OR `[1,1,...,1]` with arbitrary length), returns
`Ok(BigUint)` with a **numerically incorrect value** instead of `Err(...)`.
The value is always *less* than the correct value (under-counting per the
cycle-counting algorithm's limitation), but the caller has **no way to
distinguish** a correct decode from a corrupted one without running the
serial oracle.

This is explicitly the case the user flagged in the QA brief: "O usuário
consegue distinguir esse caso silenciosamente errado de um caso correto
sem rodar oracle?" — **No, they cannot.** The CLI prints the same
`Result: { "value": "...", "bit_length": ... }` block in both cases.

**Reproduction (all run against `target/release/relativist.exe` from
commit `6a098be`, with `cargo build --release --bin relativist`):**

```
$ target/release/relativist compute --codec horner --input '{"coeffs":[5,5,5],"x":2}'
Result:      { "bit_length": 5, "value": "27" }
# Correct: 5 + 5*2 + 5*4 = 35 — under-counted by 8

$ target/release/relativist compute --codec horner --input '{"coeffs":[3,3,3],"x":2}'
Result:      { "bit_length": 5, "value": "17" }
# Correct: 3 + 3*2 + 3*4 = 21 — under-counted by 4

$ target/release/relativist compute --codec horner --input '{"coeffs":[2,2,2],"x":2}'
Result:      { "bit_length": 4, "value": "12" }
# Correct: 2 + 2*2 + 2*4 = 14 — under-counted by 2

$ target/release/relativist compute --codec horner --input '{"coeffs":[5,2,3],"x":2}'
Result:      { "bit_length": 5, "value": "17" }
# Correct: 5 + 2*2 + 3*4 = 21 — under-counted by 4

$ target/release/relativist compute --codec horner --input '{"coeffs":[3,2,5,1],"x":2}'
Result:      { "bit_length": 5, "value": "23" }
# Correct: 3 + 2*2 + 5*4 + 1*8 = 35 — under-counted by 12

$ target/release/relativist compute --codec horner --input '{"coeffs":[1,1,1,1,1],"x":2}'
Result:      { "bit_length": 4, "value": "15" }
# Correct: 1+2+4+8+16 = 31 — under-counted by 16

$ target/release/relativist compute --codec horner --input \
    "{\"coeffs\":[$(printf '1,%.0s' {1..63})1],\"x\":2}"  # [1; 64] @ 2
Result:      { "bit_length": 8, "value": "251" }
# Correct: 2^64 - 1 ≈ 1.8e19. Magnitude error ~17 orders of decimal.

$ target/release/relativist compute --codec horner --input '{"coeffs":[10,20,30,40],"x":3}'
Result:      { "bit_length": 9, "value": "292" }
# Correct: 10 + 60 + 270 + 1080 = 1420 — under-counted by 1128
```

**Determinism + G1 confirmation** (the wrong value is itself
deterministic across workers — so G1 holds on the wrong value, which is
even more dangerous because the user might validate by running across
W and seeing "they all agree, must be right"):

```
$ for w in 1 2 4 8; do
>   target/release/relativist compute --codec horner --input '{"coeffs":[5,5,5],"x":2}' --workers $w \
>     | grep value
> done
  "value": "27"
  "value": "27"
  "value": "27"
  "value": "27"
```

**Expected behavior.** When the decoder hits the structural pattern its
v1 walker cannot resolve exactly, it MUST return
`Err(DecodeError::UnrecognizedStructure(...))` with a message naming the
limitation — never `Ok(wrong_value)`.

**Actual behavior.** `Ok(under-counted-value)` is returned silently. The
`read_chain_terminal` branch already does the right thing
(`biguint_readback.rs:454-468` returns `Err` on nested boundaries); the
bug is that `read_mult_subnet` / `walk_mult_tree` accepts a DUP tree
whose structure exceeds the cycle-counting algorithm's exact envelope
**without flagging it**. The walker happily counts cycles, gets a
smaller multiplier than the true one, and returns `Ok`.

**Root cause sketch.** `walk_mult_tree` (`biguint_readback.rs:557-615`)
unions `XVariable` / `Cycle` / `ExitChainAt` / `NestedDupPrincipal`
branches but does NOT detect that the walked subnet topology does not
correspond to a single multiplication boundary. For a degree-2 input
with leading coef ≥ 2, the DUP tree shape is more complex than the
walker assumes, so cycle classifications and exit-branch sums no longer
add up to `chain_count * x + addend`. The walker yields a number — just
not the right one.

**Fix suggestion (defensive — must ship before merge).** Add a
structural arity / shape check at the entry of `read_mult_subnet` and
either (a) verify the walked subnet matches the c2==1 / single-iter
template and return `Err(UnrecognizedStructure("multiplier-tree shape
exceeds v1 cycle-counting envelope; SPEC-27 §5.1 Mackie/Pinto"))`
otherwise, or (b) cross-check the readback value against a structural
upper bound (e.g., agent count vs expected for the claimed multiplier)
and reject if the bound is violated.

Minimal first-cut fix in `walk_mult_tree`: count the number of
**distinct** nested-DUP-principal branches walked; if it exceeds 1
(degree ≥ 3) OR the inbound chain visited more than one DUP principal
on its way in (leading coef ≥ 2), refuse and return
`UnrecognizedStructure`. This converts every BUG-001 reproduction above
to a clean `Err` — exactly the same outcome as the c1 ≥ ~1024 cases
that already error out via `read_chain_terminal` (BUG-002 below
reverses on that one, but for different reasons).

Until this guard ships, the doc claim in
`docs/demos/horner-g1-demonstration.md` lines 252-268 ("os seguintes
casos retornam valores incorretos via decoder mas a redução em si está
correta") is a **CVE-class trust statement**: it documents that the
decoder will lie to you, which is incompatible with shipping the
HornerCodec as a user-facing CLI.

---

### BUG-002 — Doc/spec advertises envelope larger than what the decoder accepts (P1)

**Severity:** HIGH (P1)
**File:** `docs/demos/horner-g1-demonstration.md:214-223` (Envelope leitor section);
docstring `relativist-core/src/encoding/biguint_readback.rs:33-34` (read_chain doc);
test `relativist-core/src/encoding/biguint_readback.rs:910-924` (UT-0723-07 stops at c1=1000)
**Category:** Documentation / Spec mismatch / False positive of advertised support

**Description.** The post-D-016 demo doc claims:

> "Single-iteration polinômios (`coeffs.len() == 2`) com qualquer
> cofactor `c₁ ≥ 1` em `0..=MAX_CHURCH_NAT` (TASK-0723)."

(`docs/demos/horner-g1-demonstration.md:219-220`)

This is **not what the decoder actually accepts**. Empirically the
threshold lies between `c1 == 1024` and `c1 == 2000`; above it the
decoder uniformly returns `Err`. Independent of `x`. Independent of
`c0`. The `MAX_CHURCH_NAT = 10000` boundary is **never reachable**
for single-iter inputs with c1 > ~1024.

**Reproduction.**

```
$ for c1 in 1024 2000 5000 10000; do
>   for x in 1 2 3 5 10 10000; do
>     out=$(target/release/relativist compute --codec horner \
>           --input "{\"coeffs\":[5,$c1],\"x\":$x}" 2>&1 | grep -E 'value|error' | head -1)
>     echo "[5,$c1]@$x: $out"
>   done
> done
[5,1024]@1:        "value": "1029"
[5,1024]@2:        "value": "2053"
[5,1024]@3:        "value": "3077"
[5,1024]@5:        "value": "5125"
[5,1024]@10:       "value": "10245"
[5,1024]@10000:    "value": "10240005"
[5,2000]@1:  error: unrecognized net structure: read_chain_terminal: nested mul boundary on exit chain
[5,2000]@2:  error: unrecognized net structure: read_chain_terminal: nested mul boundary on exit chain
[5,2000]@3:  error: unrecognized net structure: read_chain_terminal: nested mul boundary on exit chain
[5,2000]@5:  error: unrecognized net structure: read_chain_terminal: nested mul boundary on exit chain
[5,2000]@10: error: unrecognized net structure: read_chain_terminal: nested mul boundary on exit chain
[5,2000]@10000: error: unrecognized net structure: read_chain_terminal: nested mul boundary on exit chain
[5,5000]@*:  error: ...
[5,10000]@*: error: ...
```

The boundary is independent of `x` (the chain length on the multiplicand
side is what reduces) and roughly corresponds to the Church-numeral
encoding of c1 reducing into a DUP tree whose depth exceeds the v1
cycle-counter's structural template.

Same failure surface for `c0 == 0`: `[0, 2000] @ 2` fails;
`[0, 1020] @ 2` works (returns "2040" correctly). Note that
`[0, 10000] @ 2` was implicitly claimed in scope (matches Demo 8 shape
`[10, 2] @ 10000` per UT-0723-04) but actually errors.

**Expected behavior.** Either (a) docs/spec rewrite so the
"readable subset" claim matches reality — explicitly say `c1` capped at
~1024, OR (b) implementation patched to actually handle the advertised
`0..=MAX_CHURCH_NAT` envelope.

**Actual behavior.** Doc / spec say one thing, decoder accepts another.
Mid-spec false advertising is a P1 because it will cause downstream
agents (and the TCC reviewer) to plan around a non-existent capability.

**Fix suggestion.**

1. Empirically locate the true threshold (binary search). UT-0723-07
   should be extended with the actual upper bound as a regression test:
   ```rust
   #[test]
   fn decode_biguint_handles_actual_c1_upper_bound() {
       // Determined empirically; the v1 walker fails above this.
       const C1_UPPER: u64 = 1024; // or whatever bisection finds
       assert!(pipeline(format!(r#"{{"coeffs":[5,{C1_UPPER}],"x":2}}"#).as_bytes()).is_ok());
       assert!(matches!(
           pipeline_result(format!(r#"{{"coeffs":[5,{}],"x":2}}"#, C1_UPPER+1).as_bytes()),
           Err(DecodeError::UnrecognizedStructure(_))
       ));
   }
   ```
2. Rewrite the "Envelope leitor (pós D-016)" section in
   `docs/demos/horner-g1-demonstration.md` to state the true single-iter
   bound, not the encoder bound. Same for `biguint_readback.rs:32-34`
   docstring ("tolerates depth ≥ 128 to cover PT-0724-07").

UT-0723-07's hand-picked test grid stops at `c1 = 1000`, which is below
the actual breaking point — that is why CI did not catch this.

---

### BUG-003 — Test gate `UT-0715-02` accepts ANY decimal answer for the deg-5 sparse case (P2 → upgrade to P1 if not also fixed alongside BUG-001)

**Severity:** MEDIUM (P2)
**File:** `relativist-core/src/encoding/horner.rs:500-537`
**Category:** Test Coverage / Regression Gate Weakening

**Description.** The UT-0715-02 test was downgraded in commit `9fae64d`
to accept `Err(UnrecognizedStructure)` OR `Ok(v)` where `v` is any
decimal string:

```rust
match pipeline(br#"{"coeffs":[1,0,0,0,0,1],"x":10}"#) {
    Err(DecodeError::UnrecognizedStructure(_)) => {}
    Ok((v, _)) => {
        assert!(
            v == "100001" || v.parse::<u64>().is_ok(),
            "expected 100001 (future readback) or any decimal value (v1 walker), got {v}"
        );
    }
    other => panic!("unexpected pipeline result: {other:?}"),
}
```

The accepted condition `v.parse::<u64>().is_ok()` is true for **any**
non-empty decimal string up to `u64::MAX`. The test therefore passes
trivially for any wrong-but-decimal answer the decoder might emit. This
is the regression gate that should have caught BUG-001, and instead it
silently rubber-stamps the bug.

**Reproduction.** Test is already green on the broken decoder:

```
$ cargo test --release horner_decode_sparse_coefficients_match_oracle
test encoding::horner::tests::horner_decode_sparse_coefficients_match_oracle ... ok
```

**Expected behavior.** A regression test on a known-wrong path should
either (a) assert the correct value explicitly (and remain `#[ignore]`
until the Mackie/Pinto readback ships), or (b) assert that the result
is `Err(...)` so that the moment BUG-001's defensive guard ships, the
test naturally becomes pass-by-construction without further edits.

**Fix suggestion.**

```rust
// Multi-iteration sparse degree-5: v1 readback cannot resolve.
// MUST return Err — accepting Ok(any_decimal) is a silent-pass gate.
match pipeline(br#"{"coeffs":[1,0,0,0,0,1],"x":10}"#) {
    Err(DecodeError::UnrecognizedStructure(_)) => {}
    Ok((v, _)) if v == "100001" => {} // future Mackie/Pinto readback OK
    other => panic!(
        "BUG-001 regression: expected Err or Ok(\"100001\"), got {other:?}"
    ),
}
```

The companion `pt_0715_06_skip_rate_is_bounded` (commit `9fae64d`
lowered the threshold from 95% to 5% skip rate) is also affected
indirectly — its assertion `skips <= total / 20` would still pass even
if all responses were wrong-but-Ok, because the loop counts
`codec.decode(&net).is_err()`, not `decode == expected`. A real
correctness regression in the readable subset would slip past this gate
silently. Add an oracle cross-check inside the loop.

---

## Edge Cases Not Covered

### EC-001 — `[0, c1] @ x` with `c1` between threshold and MAX (advertised in scope, actually fails)

**Scenario:** Single-iteration polynomial with leading zero and large
cofactor — advertised in `docs/demos/horner-g1-demonstration.md` line
220 as supported up to MAX_CHURCH_NAT.

**Input:** `{"coeffs":[0, 2000], "x": 2}` — should be `4000`.
**Current behavior:** Errors with `read_chain_terminal: nested mul
boundary on exit chain`.
**Suggested test:** Add to UT-0723 grid; binary-search the exact
threshold; document it.

### EC-002 — Degree-2 with `c2 >= 2` does not error explicitly

**Scenario:** Doc claims degree-2 readable subset has `c2 == 1`. Inputs
with `c2 >= 2` exit the subset.

**Input:** `{"coeffs":[5,5,5], "x":2}` — should be 35, decoder returns
27.
**Current behavior:** Silent Ok with wrong value.
**Suggested test:** After BUG-001 fix, add explicit:
```rust
#[test]
fn decode_biguint_rejects_degree_2_c2_ge_2() {
    // Out of v1 envelope; MUST Err, not silently return wrong.
    let r = pipeline_result(br#"{"coeffs":[5,5,5],"x":2}"#);
    assert!(matches!(r, Err(DecodeError::UnrecognizedStructure(_))));
}
```

### EC-003 — `[1; N] @ 2` for N ≥ 3

**Scenario:** Repeated unit coefficients — the developer's known
pendency. Returns wrong-but-Ok.

**Input:** `{"coeffs":[1,1,1,1,1],"x":2}` — expected 31, returns 15.
**Current behavior:** Silent Ok(15). Tighter pattern: `4*N - 5`
(N=5→15, N=64→251, N=128→507). Looks like the walker is collapsing
the nested DUP tree to a linear count.
**Suggested test:** Add to UT-0724 as known-Err.

---

## Test Coverage Gaps

### TG-001 — No regression test for the single-iter c1 upper bound

UT-0723-07 stops at `c1 = 1000`. The actual envelope ends near `c1 ≈
1024`. The first input that fails (`[5, 1025+] @ *`) has no regression
test — silent regressions in either direction (envelope shrinking or
the bug becoming silent-wrong instead of Err) will not be caught.

### TG-002 — PT-0715-06 skip-rate guard does not cross-check values

`pt_0715_06_skip_rate_is_bounded` (`horner.rs:670`) counts `Err`
returns but does NOT compare `Ok` results against the oracle. A
regression that turns correct `Ok` answers into wrong `Ok` answers
would pass this gate. Add `let expected = horner_serial(&[a, b], x);`
and `if Ok(d) = decode { assert_eq(d, expected) }` inside the loop.

### TG-003 — `horner_distributed_g1.rs` IT does not exercise the BUG-001 inputs

A G1 IT that runs the same input across W=1, 2, 4, 8 and confirms
identical OUTPUT is necessary but not sufficient: identical *wrong*
output is also indistinguishable from identical *right* output without
an oracle cross-check inside the IT. Suggest adding `horner_serial` as
the ground truth.

---

## Stress Scenarios

### SS-001 — A2 stack safety verdict: PASS

Tested `[1; 64] @ 2` and `[1; 128] @ 2` — both ran to completion in
release build without panic or stack overflow. The iterative
`walk_mult_tree` work-stack design (commit `a9ab3d0`, lines 557-615)
appears to deliver on its PT-0724-07 promise — Windows 1MB default
stack is not exhausted at depth 127. Note the values returned are
**wrong** (BUG-001) but the runtime behavior is safe.

Debug build was not tested (release binary already exists; rebuilding
debug for stack-safety only is low-priority given release passes).

### SS-002 — A5 concurrency: NO new shared state introduced

`read_chain`, `read_chain_terminal`, `read_mult_subnet`,
`walk_mult_tree` are all pure functions operating on `&Net` and
local mutable accumulators (`HashSet<AgentId>`, `BigUint`,
`Vec<(AgentId, usize)>` work-stack). No `Arc`, no `Mutex`, no
inter-thread state. The cap-hit empty-Net interaction (F4) the user
asked about is not directly affected because readback runs after merge
and the decoder simply sees a possibly-empty net (in which case
`net.root.is_none()` triggers the `DecodeFailed("no root")` branch —
already covered).

### SS-003 — A6 demo script verdict: PASS

`bash scripts/horner_demo.sh` runs all 10 demos in ~3s with `10
passed, 0 failed` and exit 0. The script's structure (timeout 30 per
demo, grep on `"value": "<expected>"`, exit non-zero on any failure) is
sound. One minor nit: the JSON grep is sensitive to exact whitespace
formatting (matches `"value": "<x>"` literally including the single
space after colon) — if the CLI ever changes its JSON pretty-printer
the script will start false-failing. Not a blocker.

---

## Recommendation

**BLOCK Stage 6 REFACTOR until BUG-001 is fixed.** The combination of
(a) silent-wrong returns on out-of-scope inputs, (b) the test
infrastructure (UT-0715-02 + PT-0715-06 skip-rate) blessing those
wrong returns, and (c) the demo doc promising more than the decoder
delivers (BUG-002), is a P0 trust regression for the encoder CLI
surface — the very feature the TCC demo section is built around.

Concrete remediation order:

1. **BUG-001 P0** — add a defensive guard in `read_mult_subnet` /
   `walk_mult_tree` that returns `Err(UnrecognizedStructure)` for any
   topology outside the exact v1 cycle-counting envelope (degree ≥ 3
   OR degree-2 with c2 ≥ 2 OR `[1;N≥3]`-style nested unit chains).
   The downstream behavior — a clean `Err` instead of `Ok(wrong)` — is
   already what `read_chain_terminal` does at the c1 ≥ ~1024 boundary
   (BUG-002 regime), so the user experience is consistent.

2. **BUG-003 P2 (upgrade to P1 alongside BUG-001 fix)** — tighten
   `UT-0715-02` to assert `Err` OR `Ok("100001")` specifically, not
   `Ok(any_decimal)`. Add oracle cross-check inside
   `pt_0715_06_skip_rate_is_bounded`.

3. **BUG-002 P1** — bisect the c1 upper bound, encode it as a
   constant in `biguint_readback.rs`, write a regression test
   (TG-001), and rewrite the "Envelope leitor (pós D-016)" section in
   `docs/demos/horner-g1-demonstration.md` + the docstring in
   `biguint_readback.rs:33` to reflect the true envelope (single-iter
   `c1 in 0..=~1024`, degree-2 `c2 == 1 ∧ c1 ≥ 1`).

After (1)+(2)+(3): re-run `bash scripts/horner_demo.sh` (all 10 must
still pass — they all live well within the true envelope) and
`cargo test --release` (the test floor should rise by the new
regression tests rather than fall).

**Once BUG-001 ships,** the decoder's behavior on out-of-scope input
becomes "fail loud with structured `Err`", which is exactly the
contract the user asked for in the QA brief: "se sim, sugira
`Result::Err` em vez de Ok-com-valor-errado." Confirmed: **sim, é
necessário**.

---

## Cross-references

- Source under attack: `relativist-core/src/encoding/biguint_readback.rs:522-615`
- Weak test gate: `relativist-core/src/encoding/horner.rs:500-537`
- Doc overpromise: `docs/demos/horner-g1-demonstration.md:214-268`
- Demo script: `scripts/horner_demo.sh` (PASSES — no fixes needed there)
- Future-work plan: `specs/SPEC-27-encoder-decoder-api.md` §5.1 (Mackie/Pinto)
