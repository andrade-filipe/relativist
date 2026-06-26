# Governance

Relativist is an open-source research project. This document is honest about how
decisions are made today, and how that can grow.

## Current model: single maintainer (BDFL)

The project is maintained by its author, **Filipe Andrade Nascimento**, as the
practical artifact of a Computer Science thesis (TCC) at Universidade Tiradentes
(UNIT), advised by Yuri Faro Dantas de Sant'Anna. As of the open-source launch
(2026), Filipe is the sole maintainer and final decision-maker on:

- what gets merged,
- the roadmap and priorities (`docs/ROADMAP.md`),
- releases and versioning,
- the formal model and its invariants (the parts that make the research claim).

This "benevolent dictator" model is appropriate for a young, single-author
project. It keeps the research narrative coherent. It is not meant to be
permanent.

## How decisions are made

- **Small changes** (bug fixes, docs, tests, non-architectural improvements):
  open a PR against `develop`. If it is correct, green on CI, and follows
  `CODING_STANDARDS.md`, it gets merged.
- **Architectural / model-affecting changes** (anything touching the six
  interaction rules, the partition/merge protocol, the SPEC-01 invariants, or
  the `reduce_all ≅ run_grid` contract): open an issue first to discuss. These
  are weighed against the research integrity of the project, not just code
  quality. Cite the relevant spec.
- **Disagreements** are resolved by discussion in the issue/PR; if consensus
  isn't reached, the maintainer decides and records the rationale.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the practical workflow (RPI:
Research → Plan → Implement) and [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) for
community expectations. Contributions are accepted under the project's license
(Apache-2.0); by contributing you agree your work is licensed under those terms.

## Evolving the governance

If the project attracts sustained contribution, governance is expected to grow
toward a small group of maintainers with explicit areas of ownership (e.g. core
reducer, distribution/protocol, tooling/CI). The triggers and the path:

1. **Earning commit/triage rights** — a contributor with a track record of
   high-quality, sustained contributions can be invited to help triage issues
   and review PRs.
2. **Maintainer status** — granted by the current maintainer(s) to people who
   have demonstrated good judgment on architectural calls, not just volume.
3. **This document is updated** to name maintainers and their areas, and to
   describe how ties are broken (e.g. lazy consensus with a maintainer vote as
   fallback).

Until then, the single-maintainer model above is the rule, stated plainly so
contributors know what to expect.
