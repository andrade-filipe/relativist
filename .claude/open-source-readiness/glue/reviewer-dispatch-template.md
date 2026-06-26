# Reviewer Dispatch Template

This file is the orchestrator's reference for constructing the context envelope dispatched
to each dimension reviewer. The template enforces the context-isolation invariant:
**a reviewer receives only its own archetype prompt and the shared inputs listed below.**

## Shared Inputs (ALL reviewers receive)

```
REPO_PATH: <absolute or repo-relative path to target repository root>
OUTPUT_PATH: review-state/round-<N>/reviewer-outputs/<dimension>.yaml
ROUND: <N>
```

## Per-Reviewer Context Envelope

Each reviewer's context window contains EXACTLY these elements — nothing more:

```
SYSTEM:
  You are [reviewer_name] for the Open-Source Readiness Pack.
  Your dimension: [dimension_label]
  [Full agent file body for this reviewer]

USER:
  Repository path: <REPO_PATH>
  Write your structured findings to: <OUTPUT_PATH>
  Round: <ROUND>
```

## Isolation Invariants

The orchestrator MUST enforce all of the following:

1. **No peer findings** — Reviewer A's output file is NOT included in Reviewer B's context envelope. This is verified by checking that no other reviewer's output path appears in the dispatch prompt.

2. **No aggregator schema** — The `glue/triage-schema.yaml` is NOT injected into reviewer context. Reviewers produce their own per-dimension YAML schema; the aggregator handles the triage-schema conversion.

3. **No prior round state** — If this is Round > 1, prior round outputs are NOT included in reviewer context. Reviewer must re-inspect the repository fresh each round.

4. **No cross-reviewer communication** — Reviewers are not told what other reviewers are looking for, which reviewers are running in parallel, or what the overall pack structure is.

5. **Capped context** — The reviewer context envelope must not exceed the declared `minimal-sufficient-context` ceiling: (system prompt for this reviewer) + (repo path) + (output path). No file contents from the repository are pre-loaded into context — the reviewer reads them using its tool surface.

## Anti-Pattern: Context Cross-Contamination

The `shared-context-between-reviewers` anti-pattern (from the parent adversarial-review-pipeline-pack.md) is the primary failure mode. Symptoms:
- All four reviewers produce similar severity distributions
- Later-dispatched reviewers reference findings that could only have come from earlier reviewers
- The aggregated triage shows zero corroboration_count > 1 findings (because reviewers converged to the same framing rather than independently discovering the same gap)

Fix: audit the dispatch log in `review-state/round-<N>/orchestrator-trace.jsonl` and confirm that each reviewer's `context_sent` field contains only this template's allowed elements.

## Dispatch Sequence

The orchestrator dispatches all four reviewers in parallel (simultaneous subagent spawn). It does NOT dispatch them sequentially — sequential dispatch would allow the orchestrator to observe early results and inadvertently include them in later dispatches.

Parallel dispatch also ensures the Review phase completes in O(1) wall-clock rounds rather than O(4).

## State Directory Layout

```
review-state/
  round-<N>/
    orchestrator-trace.jsonl        # orchestrator step log
    reviewer-outputs/
      dim1-docs.yaml                # docs-readiness-reviewer output
      dim2-license.yaml             # license-readiness-reviewer output
      dim3-secrets.yaml             # secrets-git-reviewer output
      dim4-security-ci.yaml         # security-ci-reviewer output
    triage-report.yaml              # readiness-aggregator output
    remediation-summary.yaml        # readiness-remediator output (after human approval)
```
