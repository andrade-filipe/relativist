---
tier: workflow
task_class: mixed
targets_scenarios: []
pack_name: open-source-readiness
version: "1.0.0"
status: draft
orchestration_substrate: runtime
primary_task_class: mixed
context_isolation_substrate: separate-context-windows
reviewer_count: 4
aggregator_protocol: structured-triage
license: MIT
attribution_string: "Context Engineering Framework, MIT license. Grounded in REF-0201 (Fogel, Producing Open Source Software, 2nd ed. 2017) + REF-0202 (GitHub opensource.guide) + research/concepts/open-source-readiness.md."
---

# Open-Source Readiness Pack

The Open-Source Readiness Pack audits a repository across four independently-reviewed dimensions — Essential Docs, License & Attribution, Secrets & Git Hygiene, Security & CI/Governance — then optionally drives a human-gated remediation phase that generates or repairs the missing files.

The pack specializes the `adversarial-review-pipeline-pack` workflow (framework/workflows/adversarial-review-pipeline-pack.md) for the open-source launch gate, using strict context isolation between dimension reviewers to prevent cross-contamination of findings, then aggregating into a single prioritized gap report before handing off to a human-approval-gated remediator.

## Pack Details

- **Orchestration substrate**: runtime (parallel subagents per dimension, phase-serialized)
- **Primary task-class**: mixed — reviewer dispatch is research-breadth-coherent (four isolated contexts over the same repo); aggregation is decision-coherent (single triage); remediation is decision-coherent (file-by-file with human gates)
- **Reviewer count**: 4 (one per open-source readiness dimension)
- **Context isolation substrate**: separate-context-windows — each dimension reviewer spawned in its own subagent process; no reviewer sees another's findings
- **Aggregator protocol**: structured-triage — severity-categorized gap report with cross-reviewer corroboration counts and reviewer attribution per finding
- **Provenance**: REF-0201 (Fogel), REF-0202 (GitHub opensource.guide), research/concepts/open-source-readiness.md

## Member Agents (pack-internal)

All agents live under `generation/packs/open-source-readiness/agents/`.

| Agent | Dimension | Role |
|---|---|---|
| `readiness-orchestrator` | Cross-cutting | Dispatches the 4 dimension reviewers in strict context isolation; enforces phase serialization |
| `docs-readiness-reviewer` | Dim 1 — Essential Docs | Audits README / CONTRIBUTING / CODE_OF_CONDUCT / GOVERNANCE / docs/ gaps |
| `license-readiness-reviewer` | Dim 2 — License & Attribution | Wraps the `license-and-attribution-auditor` Skill |
| `secrets-git-reviewer` | Dim 3 — Secrets & Git Hygiene | Secret/credential scan of working tree AND git history; .gitignore / .env; pre-commit |
| `security-ci-reviewer` | Dim 4 — Security & CI/Governance | SECURITY.md + private reporting; dependency vulns; issue/PR templates; branch protection; minimal CI; company compliance |
| `readiness-aggregator` | Aggregation | Structured-triage of 4 reviewers' findings into one severity-ranked gap report |
| `readiness-remediator` | Remediation | Consumes gap report; generates/repairs missing files under HUMAN-APPROVAL-GATED operations |

## Member Skills (referenced from generation/skills/)

| Skill | Dimension | Purpose |
|---|---|---|
| `license-and-attribution-auditor` | Dim 2 | License selection, SPDX headers, DCO/CLA, relicense audit |
| `addyosmani-documentation-adrs` | Dim 1 | ADR / decision-record documentation patterns |
| `awesome-copilot-documentation-writer` | Dim 1 | Structured README / CONTRIBUTING authoring |
| `pocock-git-guardrails` | Dim 3 | Git hygiene guardrails, branch protection, history safety |
| `pocock-setup-pre-commit` | Dim 3 | Pre-commit hook installation; secret-scan hook; .gitignore baseline |
| `auth-hardening-best-practices` | Dim 4 | Auth / access-control hardening (feeds security-ci review) |
| `stride-threat-modeling` | Dim 4 | STRIDE threat surface enumeration (partial — informs SECURITY.md scope) |

## Workflow Overview

```
Input: target repository path
          |
          v
  readiness-orchestrator
    |    |    |    |
    v    v    v    v
 dim1  dim2  dim3  dim4     (parallel, strict context isolation)
    \    |    |    /
     v   v   v   v
    readiness-aggregator    (decision-coherent phase; severity-ranked gap report)
          |
          v
    [HUMAN REVIEW — operator reads gap report; decides to remediate]
          |
          v
    readiness-remediator    (human-approval-gated per irreversible file mutation)
          |
          v
Output: remediated repository
```

## Compliance

Honors the following framework constants:

- `context-curation-discipline` — each dimension reviewer receives only the repo path + its own archetype prompt; no peer outputs injected (Isolate pillar of the four-pillar discipline)
- `human-approval-on-irreversible-actions` — the remediator gates every file write, git history rewrite, and secret rotation on explicit human approval via `request-human-approval-pattern`
- `observable-agent-traces` — per-round reviewer outputs and triage reports persisted to `review-state/round-N/`; replay protocol named
- `deterministic-outer-loop` — orchestrator runs a fixed four-reviewer fan-out with a finite step budget; loop termination is harness-controlled, not model-decided
- `bounded-tool-surface` — each reviewer has a declared and capped tool set (read_file, grep_repo, list_directory only)
- `minimal-sufficient-context` — reviewer context capped to (repo path + dimension-specific checklist + archetype prompt); no full-conversation history injected

## Anti-patterns (inherited from parent workflow)

- `shared-context-between-reviewers` — reviewers must never see peer findings; violates context isolation and collapses finding diversity
- `aggregator-as-free-form-prose` — the aggregator must emit structured triage conforming to `glue/triage-schema.yaml`; narrative summary fails
- `remediator-without-approval-gate` — any file write or git history mutation performed without human approval is a security violation per `human-approval-on-irreversible-actions`
- `single-reviewer-as-adversarial-review` — using fewer than 4 dimension reviewers loses coverage of a readiness dimension entirely

## Cross-references

- `framework/workflows/adversarial-review-pipeline-pack.md` — parent workflow pattern this pack specializes
- `research/concepts/open-source-readiness.md` — four-dimension readiness model (Dim 1–4 definitions, auditable surfaces, readiness criteria, remediation actions)
- `generation/skills/license-and-attribution-auditor/SKILL.md` — Dimension 2 Skill
- `framework/constants/context-curation-discipline.md` — Isolate pillar enforcement
- `framework/constants/human-approval-on-irreversible-actions.md` — remediator approval gate
- REF-0201 — Fogel, *Producing Open Source Software*, 2nd ed. 2017
- REF-0202 — GitHub `opensource.guide`
