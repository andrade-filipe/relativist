---
title: Horner G1 demonstration
summary: Horner's method evaluated across distributed workers yields the same result as sequential reduction — a concrete witness of G1.
keywords: [horner, G1, confluence, compute, codec, encode, decode, distributed, determinism, church numerals]
modules: [encoding]
specs: [SPEC-14, SPEC-27]
audience: [user, llm]
status: guide
updated: 2026-06-26
---

# Horner G1 demonstration

A walkthrough that uses **Horner's method** as a concrete probe to show that an
IC net reduces to the **same numerical result** regardless of strategy —
sequential (`reduce_all`, in-process) or distributed (BSP over W parallel
workers). That invariance is a direct witness of property **G1 (Fundamental
Property)** from [SPEC-01](../specs/SPEC-01-invariantes.md), itself derived
from Lafont's strong confluence (1997).

For the encoder/decoder flags see the [CLI reference](../reference/cli.md#compute);
for IC theory see [interaction-combinators](../theory/interaction-combinators.md).

## why Horner is a strong demo

Horner evaluates a polynomial

```
p(x) = c0 + c1*x + c2*x^2 + ... + cn*x^n
```

in a re-associated form that minimizes multiplications:

```
p(x) = c0 + x*(c1 + x*(c2 + ... + x*cn))
```

That formulation is **intrinsically sequential as text**: to start computing the
re-associated `c2*x^2 + c1*x + c0` you first need the innermost parenthesis.
Pure functional languages typically parallelize `map` but struggle to
parallelize Horner without rewriting the algorithm.

In IC the re-association disappears — the net encodes the structure with no
implicit order, and any reduction strategy converges to the same normal form
(G1). That contrast is exactly what makes Horner a sharp demonstration: a
sequential algorithm executed correctly across distributed workers.

## cli pipeline

The Horner encoder/decoder is exposed through the registry:

```bash
relativist compute --codec horner --input '{"coeffs":[c0,c1,c2,...],"x":value}'
```

where `coeffs[i]` is the coefficient of `x^i`. Distribution is optional via
`--workers N`. Confirm the codec is registered:

```bash
relativist encoders list
```

```
Available encoders:
  church_add             Church numeral addition (a + b)
  church_exp             Church numeral exponentiation (a ^ b)
  church_mul             Church numeral multiplication (a * b)
  church_sum_of_squares  Sum of squares (1^2 + 2^2 + ... + n^2)
  horner                 Polynomial evaluation via Horner's method
```

## demo 1 constant polynomial

`p(x) = 42`. Degenerate case: the net encodes the Church numeral 42 with no
redex, and the decoder reads the numeral directly (x = 99 ignored).

```bash
relativist compute --codec horner --input '{"coeffs":[42],"x":99}'
```

```
=== Relativist Compute (encoder: horner) ===
Encoding:    85 agents, 0 redexes
Reduction:   0 interactions in 0.00s (0.00 MIPS)
Result:      {
  "bit_length": 6,
  "value": "42"
}
```

## demo 2 linear sequential

`p(x) = 1 + x` at `x = 5`, reduced sequentially in-process.

```bash
relativist compute --codec horner --input '{"coeffs":[1,1],"x":5}'
```

```
=== Relativist Compute (encoder: horner) ===
Encoding:    35 agents, 2 redexes
Reduction:   11 interactions in 0.00s (1.90 MIPS)
Result:      {
  "bit_length": 3,
  "value": "6"
}
```

Expected `1 + 5 = 6`; obtained **6**.

## demo 3 same equation distributed W=4

```bash
relativist compute --codec horner --input '{"coeffs":[1,1],"x":5}' --workers 4
```

```
=== Relativist Compute (encoder: horner) ===
Encoding:    35 agents, 2 redexes
Reduction:   11 interactions in 0.00s (2.00 MIPS)
Result:      {
  "bit_length": 3,
  "value": "6"
}
```

**Identical to demo 2**: same interaction count (11), same `bit_length`, same
`value`. The reducer partitioned the net into 4 sub-nets, distributed them to
workers, and the merge converged to the same normal form. **This is G1.**

## demo 4 larger scale sequential

`p(x) = 100 + x` at `x = 50`.

```bash
relativist compute --codec horner --input '{"coeffs":[100,1],"x":50}'
```

```
=== Relativist Compute (encoder: horner) ===
Encoding:    323 agents, 2 redexes
Reduction:   11 interactions in 0.00s (2.00 MIPS)
Result:      {
  "bit_length": 8,
  "value": "150"
}
```

Encoding agent count grows with `x` (323 ~ 2*(50+100) + offset), but reduction
stays at **11 interactions** — evaluating the polynomial is cheap relative to the
Church-numeral representation size.

## demo 5 same polynomial distributed W=8

```bash
relativist compute --codec horner --input '{"coeffs":[100,1],"x":50}' --workers 8
```

```
=== Relativist Compute (encoder: horner) ===
Encoding:    323 agents, 2 redexes
Reduction:   11 interactions in 0.00s (2.04 MIPS)
Result:      {
  "bit_length": 8,
  "value": "150"
}
```

**Identical to demo 4** with 8 parallel workers. The numerical result does not
change even when the net is split into 8 independent fragments and merged at the
end. G1 holds at scale.

## demo 6 supported scale limit

`p(x) = 42 + x` at `x = 10000`, distributed over 4 workers.

```bash
relativist compute --codec horner --input '{"coeffs":[42,1],"x":10000}' --workers 4
```

```
=== Relativist Compute (encoder: horner) ===
Encoding:    20107 agents, 2 redexes
Reduction:   11 interactions in 0.00s (1.53 MIPS)
Result:      {
  "bit_length": 14,
  "value": "10042"
}
```

Encoding with ~20k agents (linear in `x`); reduction still **11 interactions**
(constant).

## demo 7 input validation

`MAX_CHURCH_NAT = 10000` caps the largest accepted Church numeral, preventing a
malicious JSON from constructing a net with billions of agents.

```bash
relativist compute --codec horner --input '{"coeffs":[1,1],"x":99999}'
```

```
=== Relativist Compute (encoder: horner) ===
error: encoding error: invalid input: x = 99999 exceeds cap (max 10000)
```

Reported cleanly — no panic, no corruption.

## what these demos witness

| Property | Evidence |
|---|---|
| **G1 (Fundamental Property)** — same observable across strategies | demo 2 == demo 3 (W=4); demo 4 == demo 5 (W=8). Same value, sequential vs distributed. |
| **Determinism** — repeated runs converge | Each demo is deterministic in interaction count (0, 11, 11, 11, 11). |
| **Strong confluence (Lafont 1997)** — unique normal form | Same `bit_length` and `value` across runs with different partitioning. |
| **Input robustness** — encoder bounds-checks | demo 7 (`MAX_CHURCH_NAT` enforcement, no panic). |

## decode readback envelope

Decoding the reduced net to an integer is supported for a defined subset:

- **Single-iteration polynomials** (`coeffs.len() == 2`) with cofactor
  `c1 in 0..=1025` and any `c0`, `x in 0..=MAX_CHURCH_NAT`.
- **Degree-2 polynomials** (`coeffs.len() == 3`) with leading coefficient
  `c2 == 1` and middle coefficient `c1 >= 0`.

Cases inside the envelope decode correctly:

```bash
relativist compute --codec horner --input '{"coeffs":[3,5],"x":4}'    # -> 23
relativist compute --codec horner --input '{"coeffs":[1,1,1],"x":2}'  # -> 7
relativist compute --codec horner --input '{"coeffs":[1,0,1],"x":3}'  # -> 10
```

Inputs **outside** the envelope (degree >= 3, or degree-2 with `c2 >= 2`, or
`c1 > 1025`) return a structured `Err(UnrecognizedStructure: ...)` rather than an
`Ok` with a numerically wrong value. The reduction is still correct and the
`horner_serial` oracle still produces the right number — only the readback is
refused. Closing this gap (arbitrary degree, `c1 > 1025`) is future work via
Mackie/Pinto shared-form readback ([SPEC-27](../specs/SPEC-27-encoder-decoder-api.md) §5.1).

## decode a saved net

A reduced net written by `coordinator`/`local`/`compute -o` can be decoded later:

```bash
relativist compute --codec horner --input '{"coeffs":[3,5],"x":4}' -o reduced.bin
relativist decode --codec horner -i reduced.bin            # prints JSON
relativist decode --codec horner -i reduced.bin -o out.json
```

## reproducibility

```bash
cd codigo/relativist
cargo build --release --bin relativist
target/release/relativist compute --codec horner --input '<JSON>' [--workers N]
```

## cross-references

- **Property:** [SPEC-01](../specs/SPEC-01-invariantes.md) (G1, Fundamental Property)
- **Codec API:** [SPEC-27](../specs/SPEC-27-encoder-decoder-api.md) (HornerCodec)
- **Argument:** `discussoes/argumentos/ARG-001-confluencia-preserva-determinismo.md` (P1-P6)
- **Church arithmetic guide:** [church-arithmetic](../guides/church-arithmetic.md)
- **CLI reference:** [compute](../reference/cli.md#compute), [decode](../reference/cli.md#decode), [encoders](../reference/cli.md#encoders)
