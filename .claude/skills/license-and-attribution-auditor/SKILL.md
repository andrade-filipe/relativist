---
name: license-and-attribution-auditor
description: >
  Audits a software repository's license and attribution posture. Use when: auditing
  whether a repository is legally ready for open-source publication; choosing between
  permissive (MIT/BSD/Apache 2.0) and copyleft (GPL/LGPL/AGPL) licenses using the
  Fogel reciprocity-intent rubric; checking whether per-file SPDX copyright headers
  are present and correct; checking dependency-license compatibility, especially
  GPL/AGPL copyleft against permissive bundled code; deciding between DCO
  (Signed-off-by) and CLA as the contributor-provenance mechanism; detecting the
  "public repo is not the same as licensed" gap; or auditing git history for
  copyright holders before a proposed relicense. Produces a structured seven-criterion
  gap report — one pass/fail/uncertain verdict per criterion, evidence, severity, and
  prioritized remediation. All file-mutation remediation requires explicit human
  approval. Model-agnostic, provider-agnostic, IDE-agnostic. Scoped to Dimension 2
  (License and Attribution) of open-source readiness; does not audit secrets or git
  history hygiene (Dimension 3) or governance and CI (Dimension 4).
tier: technique
task_class: decision-coherent
targets_scenarios:
  - prd-completeness
skill_contract: technique-evaluable
emitted_by: marketplace-generator-sonnet
emitted_at: 2026-06-25
---

# License and Attribution Auditor

> Source provenance: [REF-0201](../../research/articles/catalog/fact-sheets/REF-0201_2017-open-source-fogel-producing-open-source-software.md) (Fogel, *Producing Open Source Software*, CC BY-SA) and [REF-0202](../../research/articles/catalog/fact-sheets/REF-0202_opensource-guide-github-open-source-readiness.md) (GitHub `opensource.guide`, CC BY), consolidated in [`research/concepts/open-source-readiness.md`](../../research/concepts/open-source-readiness.md) § Dimension 2 — License & Attribution.
> Target scenario: [`framework/scenarios/prd-completeness/`](../../framework/scenarios/prd-completeness/scenario.md) (closest available decision-coherent completeness-audit scenario; see `badges-draft.yaml` stage5_observations for imperfect-fit note and scenario-seeding recommendation).

Applies the Fogel / opensource.guide license-and-attribution audit discipline to a software repository. Runs a deterministic seven-criterion checklist. Produces a structured gap report: pass/fail/uncertain per criterion, evidence, severity, and prioritized remediation actions. No file is mutated without explicit human approval.

## When to invoke

- Preparing a repository for its first public release.
- Adding a dependency and needing to verify its license is compatible with the project's outbound license.
- Selecting a contributor-agreement model (DCO vs CLA vs do-nothing).
- Considering a license change (triggers the pre-relicense copyright-holder history audit at Criterion 7).
- Building or reviewing a CI license-compliance gate.

Do NOT use for: secrets and git-history hygiene (use the secrets-and-history-hygiene reviewer), governance-document drafting (use the security-ci-governance reviewer), README and CONTRIBUTING completeness (use the docs-completeness reviewer).

## Audit criteria

The checklist runs in order. Criterion 1 is a hard blocker: if it fails, remaining
criteria are noted as blocked and the report stops at remediation for Criterion 1.

---

### Criterion 1 — Public Does Not Equal Licensed (hard blocker)

**Check:** A top-level `LICENSE` (or `COPYING`) file exists in the repository root.

**Pass:** File present and non-empty.
**FAIL — BLOCKER:** File absent or empty. Making a repository public on GitHub or
any platform grants no downstream use rights. Only an explicit LICENSE file creates
open-source permissions for downstream users (REF-0202 `public-repo-licensed-corrective`;
REF-0201 Ch. 9). No other criterion matters until this is resolved.

**Remediation (HUMAN APPROVAL REQUIRED before execution):** Add a verbatim
OSI-approved license (see Criterion 2 for selection). Never draft custom terms.

---

### Criterion 2 — License Selection Defensibility

**Check:** The license in `LICENSE` / `COPYING` is (a) OSI-approved, (b) chosen by
an explicit reciprocity-intent decision, and (c) not a bespoke or custom instrument.

**Fogel license-selection rubric** (REF-0201 `license-choice`; REF-0202 `license-selection-by-intent`):

| Reciprocity intent | Recommended license | Key properties |
|---|---|---|
| Maximize adoption; allow proprietary derivatives; no patent concerns | MIT or BSD-2-Clause / BSD-3-Clause | Minimal permissive; short |
| Maximize adoption; explicit patent grant needed | Apache 2.0 | Patent grant + NOTICE obligation |
| Keep derivatives free; distributed as binaries | GPL-2.0-or-later | Strong copyleft on distribution |
| Keep derivatives free; allow linking from proprietary code | LGPL-2.1-or-later | Weak copyleft; library-safe |
| Close the SaaS and network-delivery loophole | AGPL-3.0 | Copyleft on network use |

Always verify: (1) OSI-approved or DFSG-compliant, (2) GPL-compatible with all
runtime dependencies (see Criterion 5), (3) verbatim standard text — never bespoke
(REF-0201 `never-write-your-own-license`; anti-pattern `bespoke-license`).

**Pass:** Recognized OSI SPDX identifier; selection matches one intent path above;
license text is verbatim standard.
**Fail:** Custom or bespoke license text; unrecognized SPDX identifier; stated intent
does not match chosen license class.
**Uncertain (escape hatch):** Genuinely ambiguous case — emit
`UNCERTAIN — consult legal counsel` rather than a forced pass or fail verdict.

**Remediation (HUMAN APPROVAL REQUIRED):** Replace bespoke text with verbatim standard
license. Resolve intent interactively with the repository owner using the rubric above.

---

### Criterion 3 — Canonical Application (SPDX headers and NOTICE)

**Check** (REF-0201 `apply-license-canonically`):
- Full license text in `LICENSE` / `COPYING`, verbatim not paraphrased.
- Each non-vendored, non-auto-generated source file carries: copyright holder, year(s),
  and `SPDX-License-Identifier: <id>`.
- Apache 2.0 projects: a `NOTICE` file exists for required third-party attributions.
- GPL projects: file header includes "or any later version" when GPL-2.0-or-later was
  chosen.

**Pass (machine-auditable signals):**
- `LICENSE` / `COPYING` present and verbatim
- SPDX header coverage at or above 95% of source files (excluding vendored and
  auto-generated code)
- `NOTICE` present when Apache 2.0 is the license and bundled third-party code
  requires attribution

**Fail:** Headers missing on more than 5% of source files; `NOTICE` absent for
Apache 2.0; license text paraphrased instead of verbatim.

**Remediation (HUMAN APPROVAL REQUIRED):** Add `SPDX-License-Identifier: <id>` and
`Copyright <year> <holder>` to each source file header. Generate `NOTICE` from
third-party attribution requirements for Apache 2.0.

---

### Criterion 4 — Contributor Provenance Mechanism

**Check:** A contributor-agreement model is chosen and documented in `CONTRIBUTING.md`
or equivalent.

Three accepted models (REF-0201 `contributor-provenance-tracking` and
`contributor-agreement-choice`; REF-0202 DCO vs CLA):

| Model | Contributor friction | When appropriate |
|---|---|---|
| Do-Nothing (implicit; contributions accepted under project license) | None | Small, low-risk project; no relicensing path anticipated; document this choice explicitly |
| DCO — Developer Certificate of Origin (`Signed-off-by:` in each commit) | Minimal | Default recommendation; sufficient for most projects |
| CLA — Contributor License Agreement (signed legal instrument) | High | Only when relicensing rights or centralized copyright are concretely required; document the reason |

**Two-source consensus (REF-0201 + REF-0202): prefer DCO; accept CLA friction only
with a documented concrete relicensing or foundation-stewardship reason.**

**Pass:** One model declared in `CONTRIBUTING.md`; if CLA, rationale documented.
**Fail:** No provenance model declared; CLA used without documented rationale.

**Remediation:** Add `Signed-off-by` requirement to `CONTRIBUTING.md` (DCO default).
Escalate CLA decision to legal or maintainer before adopting.

---

### Criterion 5 — Dependency License Compatibility

**Check:** Every runtime dependency's license is compatible with the project's
declared outbound license per the GPL compatibility matrix and OSI norms.

Common critical incompatibilities:
- A permissive project (MIT or Apache 2.0) bundling a strong copyleft dependency
  (GPL) may restrict distribution of the combined work under permissive terms.
- An AGPL project depending on GPL-2.0-only code may face version-incompatibility
  linking restrictions.
- A dependency carrying a non-OSI-standard clause (non-commercial, no-AI-training,
  Commons Clause) renders license compatibility uncertain.

**Pass:** All runtime dependencies carry OSI-approved licenses compatible with the
project's outbound license.
**Fail:** Confirmed incompatible dependency bundled in the distribution.
**Uncertain (escape hatch):** Dual-licensed dependency, unusual or non-OSI license,
or a legal edge case — emit `UNCERTAIN — consult legal counsel before bundling`
for that specific dependency rather than a forced pass or fail.

**Remediation (HUMAN APPROVAL REQUIRED for dependency changes):** Replace the
incompatible dependency with a permissive-licensed alternative; or restructure
the integration (optional plugin, subprocess boundary, or dynamic loading) to
break the copyleft propagation chain.

---

### Criterion 6 — Public-but-Unlicensed Period Detection

**Check:** If the repository was ever public with no LICENSE file, determine whether
the unlicensed period is understood and addressed.

**Pass:** Criterion 1 passes and the repository was never public without a license;
or a relicensing notice retroactively covers the unlicensed period.
**Flag (low severity):** Repository was public before a LICENSE file was added.
Historical consumers who forked or used the code during the unlicensed window
received no explicit use rights under the "public does not equal licensed" principle.

**Remediation:** Document the date the license was added in a commit or in the
project history. Consider a retroactive statement in CHANGELOG or the repository
description clarifying whether the copyright holder condones historical usage during
the unlicensed window. This is a low-severity flag; the critical blocker is
Criterion 1 (license presence going forward).

---

### Criterion 7 — Pre-Relicense Copyright-Holder History Audit

**Activation:** Triggered only when a license change is under consideration. For
all other audits: mark as NOT-TRIGGERED and move on.

**Check** (REF-0202 `copyright-holder-audit-for-relicensing`; REF-0201 Ch. 9):
The git history is audited to enumerate every contributor; explicit consent is
obtained from each copyright holder; employer-held contributions are identified
(employee code may carry employer copyright by default regardless of project origin).

**Pass:** All contributors enumerated (`git log --format='%aN <%aE>' | sort -u`);
written consent documented for each before the license change proceeds.
**Fail:** License change proposed without history audit; consent not documented.

**Remediation:** Run the contributor enumeration command. Contact each contributor
for written consent before changing the license. Reference: Mozilla's multi-year
Firefox and Thunderbird relicensing as a complexity calibration — relicensing is
near-impossible without contributor consent (REF-0201 Ch. 9).

---

## Output format

Emit the audit as a structured report in this shape:

```
## License and Attribution Audit Report

Repository: <name or path>
Audit date: <ISO 8601 date>
Declared license: <SPDX identifier, or "none">

### Summary

| # | Criterion | Verdict | Severity |
|---|---|---|---|
| 1 | Public does not equal Licensed | PASS / FAIL | BLOCKER |
| 2 | License Selection Defensibility | PASS / FAIL / UNCERTAIN | HIGH |
| 3 | Canonical Application (SPDX + NOTICE) | PASS / FAIL | HIGH |
| 4 | Contributor Provenance Mechanism | PASS / FAIL | MEDIUM |
| 5 | Dependency License Compatibility | PASS / FAIL / UNCERTAIN | HIGH |
| 6 | Public-but-Unlicensed Period | PASS / FLAG | LOW |
| 7 | Pre-Relicense History Audit | PASS / NOT-TRIGGERED / FAIL | HIGH if triggered |

### Findings (non-passing entries only)

For each FAIL, UNCERTAIN, or FLAG criterion:

**Criterion <N> — <name>**
Verdict: FAIL / UNCERTAIN / FLAG
Evidence: <what was observed — file present/absent, header coverage %, dependency
  name and license, etc.>
Remediation: HUMAN APPROVAL REQUIRED — <specific action required>

### Next actions (priority order)

1. <highest-severity remediation first>
2. ...
```

All remediation actions that write or modify files (`LICENSE`, source-file SPDX
headers, `NOTICE`, `CONTRIBUTING.md`) are prefixed `HUMAN APPROVAL REQUIRED` and
are not executed without explicit operator confirmation.

---

## Anti-patterns

| Anti-pattern | Why it fails | Correct practice |
|---|---|---|
| `bespoke-license` | Custom terms create incompatibility, legal ambiguity, and contributor distrust | Reuse verbatim OSI-approved license (REF-0201 `never-write-your-own-license`) |
| `hidden-or-missing-license` | Public repository grants no use rights without explicit LICENSE | Treat missing LICENSE as Criterion 1 BLOCKER before publication |
| `public-repo-as-implied-open-source` | GitHub public does not equal licensed | Only an explicit LICENSE file creates open-source permissions |
| `cla-by-default` | CLA friction suppresses casual contributors without justification | Prefer DCO; adopt CLA only with a documented relicensing or foundation-stewardship reason |
| `relicense-without-history-audit` | Contributors hold copyright; consent is required for any license change | Enumerate contributors via git log and obtain written consent first |
| `ignore-dependency-licenses` | GPL-incompatible dependency in a permissive project creates a distribution conflict | Audit all runtime dependency licenses before bundling (Criterion 5) |
| `emit-pass-when-uncertain` | False confidence on a legal question is worse than an explicit UNCERTAIN verdict | Use the escape hatch (`UNCERTAIN — consult legal counsel`) for genuinely ambiguous cases |

---

## Source provenance

- **REF-0201** — Karl Fogel, *Producing Open Source Software*, 2nd ed. 2017 (CC BY-SA). Key techniques cited: `license-selection-by-reciprocity-intent`, `apply-license-canonically`, `never-write-your-own-license`, `contributor-provenance-tracking`, `license-choice` rubric, `contributor-agreement-choice` rubric. Primary chapters: Ch. 2 "Getting Started", Ch. 9 "Legal Matters".
- **REF-0202** — GitHub `opensource.guide`, 2016–present (CC BY). Key techniques cited: `public-repo-licensed-corrective`, `license-selection-by-intent`, `copyright-holder-audit-for-relicensing`. Primary guide: "The Legal Side of Open Source".
- **Concept card** — [`research/concepts/open-source-readiness.md`](../../research/concepts/open-source-readiness.md) § Dimension 2 — License & Attribution. Two-source consensus: both REF-0201 and REF-0202 corroborate all seven load-bearing claims in this audit checklist.
