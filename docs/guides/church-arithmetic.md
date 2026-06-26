---
title: Church arithmetic
summary: Encode naturals as Church numerals, reduce as an IC net, and decode the integer back via the compute subcommand.
keywords: [church numerals, arithmetic, compute, add, mul, exp, encode, decode, readback, G1, encoding]
modules: [encoding]
specs: [SPEC-14, SPEC-27]
audience: [user, llm]
status: guide
updated: 2026-06-26
---

# Church arithmetic

The `compute` subcommand encodes natural numbers as **Church numerals** in IC,
reduces the expression (locally or across a grid), and decodes the normal form
back to an integer. It is the most direct way to watch an IC reduction produce a
real number end-to-end.

```bash
relativist compute <OPERATION> <A> <B> [--workers N]
```

Prerequisite: you know `relativist local` ([local-grid](local-grid.md)), because
`compute --workers N` runs the in-process grid internally. For the formal
encoding see [SPEC-14](../../specs/SPEC-14-encoding.md); for IC theory see
[interaction-combinators](../theory/interaction-combinators.md); for every flag
see the [CLI reference](../reference/cli.md#compute).

## operations

| Operation | Formula | Example | Status |
|-----------|---------|--------------------------|----------|
| `add` | `a + b` | `compute add 3 5` -> 8 | stable |
| `mul` | `a * b` | `compute mul 3 4` -> 12 | stable |
| `exp` | `a ^ b` | `compute exp 2 3` -> (no decode) | limited |

## encode reduce decode pipeline

`compute add a b` runs four stages internally:

1. Build `encode_nat(a)` and `encode_nat(b)` as Church numerals in IC.
2. Wire them through the `add` operator (itself an IC sub-net).
3. Reduce: `reduce_all` when `--workers` is omitted, else `run_grid`.
4. Decode the reduced net back to an integer (`decode_nat_or_shared`).

Because the result is independent of the reduction strategy, sequential and
distributed runs yield the same number — a witness of **G1** (see the
[Horner G1 demonstration](../demos/horner-g1-demonstration.md)).

## sequential addition

```bash
relativist compute add 3 5
```

```
=== Relativist Compute ===
Expression:  add(3, 5)
Encoding:    29 agents, 1 redexes
Reduction:   6 interactions in 0.00s (0.88 MIPS)
Result:      8
```

## sequential multiplication

```bash
relativist compute mul 3 4
```

```
=== Relativist Compute ===
Expression:  mul(3, 4)
Encoding:    23 agents, 1 redexes
Reduction:   9 interactions in 0.00s
Result:      12
```

`mul` exercises the full rule set (gamma-gamma, gamma-delta, delta-delta) and
requires functional `delta` (DUP) sharing — there is no way to reduce it in IC
without duplication.

## distributed addition

The same expression across two in-process workers returns the identical value:

```bash
relativist compute add 10 20 --workers 2
```

```
=== Relativist Compute ===
Expression:  add(10, 20)
Encoding:    73 agents, 1 redexes
Reduction:   6 interactions in 0.00s
Workers:     2
Rounds:      1
Result:      30
```

## saving the net and metrics

```bash
relativist compute mul 5 6 --workers 4 -o result.bin -m metrics.json
```

`result.bin` is the reduced net (bincode); `metrics.json` matches the `local`
metrics format. Decode a saved net later with `decode`:

```bash
relativist decode --codec church_mul -i result.bin
```

## exp readback limitation (L5)

`exp` reduces correctly — the net reaches normal form — but the result uses a
**cyclic shared form** (DUP sharing from optimal reduction) that the current
reader (`decode_shared_chain`) cannot walk.

```bash
relativist compute exp 2 3
```

```
=== Relativist Compute ===
Expression:  exp(2, 3)
Encoding:    17 agents, 1 redexes
Reduction:   15 interactions in 0.00s
Result:      (non-decodable normal form)
  Final agents: 7
```

The normal form is correct; the decoder simply cannot extract the integer `8`.
This is a known optimal-reduction readback gap, tracked as item **L5** in
[limitations](../benchmarks/limitations.md). The reduction itself is sound — only
the readback is blocked.

## registry codecs

`add`/`mul`/`exp` are the legacy positional form. The same encoders are also
registered in the codec registry, alongside `church_sum_of_squares` and
`horner`:

```bash
relativist encoders list      # or: relativist codecs list
```

```
Available encoders:
  church_add             Church numeral addition (a + b)
  church_exp             Church numeral exponentiation (a ^ b)
  church_mul             Church numeral multiplication (a * b)
  church_sum_of_squares  Sum of squares (1^2 + 2^2 + ... + n^2)
  horner                 Polynomial evaluation via Horner's method
```

Invoke a registry codec with JSON input:

```bash
relativist compute --codec church_sum_of_squares --input '{"n":5}'
```

## why Church numerals

Church numerals are the most demanding test of the IC reduction stack:

- **They use all 6 rules** — `mul` combines gamma-gamma, gamma-delta, delta-delta.
- **They require sharing** — `delta` (DUP) is mandatory for `mul`.
- **They validate the decoder** — if the decoded result matches the closed form,
  the entire `encode -> reduce -> decode` stack is correct.

---

**Next ->** [v2-features](v2-features.md): delta protocol, zero-copy, elastic
grid, streaming, and arena management.
