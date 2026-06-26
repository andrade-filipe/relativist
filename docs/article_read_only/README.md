---
title: The article
summary: The published thesis (TCC) that introduced Relativist — PT-BR original + AI English translation, read-only.
keywords: [article, paper, thesis, TCC, publication, pdf, portuguese, english, citation, research]
modules: []
specs: []
audience: [researcher, user]
status: reference
updated: 2026-06-26
---

# The article

This folder holds the academic article that introduced Relativist — a Computer Science
thesis (TCC) at Universidade Tiradentes (UNIT), 2026, by **Filipe Andrade Nascimento**,
advised by **Yuri Faro Dantas de Sant'Anna**. It is **read-only**: a frozen snapshot of the
publication. The code in this repository is its living continuation.

| File | What it is |
|------|------------|
| [`article_pt_br.pdf`](article_pt_br.pdf) | The **original**, in Portuguese (the language of submission and defense). |
| [`article_en_us.pdf`](article_en_us.pdf) | An **English translation made with AI** — convenience for international readers; the PT-BR PDF is authoritative. |

## What it argues

The article investigates whether **Lafont's Interaction Combinators** (1997) can serve as a
formal model for **deterministic distributed reduction in Grid Computing**: can a graph-
rewriting computation be split across many machines and reduced in any order, yet yield the
exact same result as a single machine — for terminating nets?

The answer it defends is **yes**, resting on **strong confluence** (the result is independent
of reduction order) plus a structure-preserving partition/merge protocol. Relativist (this
repo) is the empirical validation: across thousands of executions, distributed reduction is
bit-for-bit equivalent to sequential reduction (the property **G1**, `reduce_all ≅ run_grid`).

## Relationship to the code

- The **theory → specs → code** flow lives in [`../specs/`](../specs/README.md) (the formal
  invariants the article relies on) and [`../theory/`](../theory/invariants.md).
- The article's **empirical claims** (correctness across the benchmark campaigns, the
  break-even analysis) are reproducible from frozen, checksummed data under
  [`../../reproduce_article/`](../../reproduce_article/README.md).
- Where the software goes **next** (the path the article's future-work section sketches):
  [`../reference/next-steps.md`](../reference/next-steps.md).

## Citing

If you use Relativist or build on this work, please cite the thesis. A machine-readable
`CITATION.cff` will be added at the repository root once the final citation / DOI is assigned.
