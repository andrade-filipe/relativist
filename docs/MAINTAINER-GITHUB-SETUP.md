---
title: Maintainer GitHub Setup Guide
summary: Web-UI/gh-CLI steps to finish the open-source repo — enforce GitFlow-lite (branch protection on main + develop), About sidebar, security, releases.
keywords: [maintainer, github, gitflow-lite, branch protection, ruleset, required status checks, codeowners, branch-policy, default branch, gh cli, social preview, license, apache-2.0, dependabot, announce]
modules: []
specs: []
audience: [contributor]
status: guide
updated: 2026-06-26
---

# Maintainer guide — GitHub setup & enforcing GitFlow-lite

Everything in this repo (license, governance, docs, CI, reproducibility, branch
model) is in place. This file lists the steps that **must be done in the GitHub web
UI or with `gh`** — they can't be committed from the codebase. The headline is
**§4: how to *enforce* the GitFlow-lite workflow** so future contributors can't
bypass it.

Repository: `github.com/andrade-filipe/relativist`. Most settings are under
**Settings** (gear) unless noted. Commands assume the [`gh` CLI](https://cli.github.com/)
is authenticated (`gh auth login`).

---

## 0. Current state

Already done and pushed: the Apache-2.0 relicense, the docs regeneration, and the
**GitFlow-lite branch model** —

- **`main`** — production/release (default branch).
- **`develop`** — active integration branch (was `v2-development`).
- **`v1-feature-complete`** — frozen v1 archive (tag `v0.10.0-bench`).

All other branches were deleted (local + remote). The enforcement tooling
(`.github/workflows/branch-policy.yml`, `.github/CODEOWNERS`, the PR template) is on
`develop` via `chore/enforce-gitflow` → open a PR into `develop` to land it, then
let it reach `main` at the next integration. The remaining steps below are
GitHub-side settings.

## 1. Repository "About" sidebar

**Main repo page → ⚙ next to "About":**

- **Description:** e.g. *"Distributed Interaction Combinator reducer for Grid
  Computing — Rust, formally specified, deterministic via strong confluence."*
- **Website:** link to the published article / your page, if any.
- **Topics:** `rust`, `interaction-combinators`, `interaction-nets`,
  `distributed-systems`, `grid-computing`, `graph-rewriting`, `lambda-calculus`,
  `confluence`, `bsp`, `research`.

```bash
gh repo edit --description "Distributed Interaction Combinator reducer for Grid Computing (Rust)" \
  --add-topic rust --add-topic interaction-combinators --add-topic distributed-systems \
  --add-topic grid-computing --add-topic graph-rewriting --add-topic research
```

## 2. Social preview image

**Settings → General → Social preview.** Upload a 1280×640 PNG (the architecture
diagram or a title card) — what renders when the repo link is shared.

## 3. License detection

Confirm the repo sidebar shows **"Apache-2.0"** (GitHub auto-detects `LICENSE`).

## 4. Enforce GitFlow-lite (the important part)

Documentation *guides* the workflow; **branch protection + required checks are what
actually block a contributor from bypassing it.** Three layers, all server-side:

### 4.1 What the model is

```
feature/*, fix/*, …  ──PR──▶  develop  ──PR (release/*)──▶  main  ──tag──▶ release
hotfix/*  ───────────────────PR──────────────────────────▶  main
```

- Contributors branch from `develop`, PR **into `develop`**.
- Only `develop`, `release/*`, or `hotfix/*` may PR **into `main`**.
- `v1-feature-complete` is frozen — no PRs.

### 4.2 The automated gate (already in the repo)

`.github/workflows/branch-policy.yml` runs on every PR and **fails** when:
- the head branch isn't named `<type>/<desc>` (feature/, fix/, chore/, docs/,
  refactor/, test/, perf/, ci/, build/, release/, hotfix/, or dependabot/);
- a PR targets `main` from anything other than `develop` / `release/*` / `hotfix/*`;
- a PR targets `v1-feature-complete`;
- the PR title isn't a Conventional Commit.

Because a PR's workflow is read from its **base** branch, `branch-policy.yml` must
live on **both `main` and `develop`** (it does, once this work lands on both). Making
the check **required** (next step) is what turns "fails the check" into "cannot
merge."

### 4.3 Protect `main` AND `develop` (rulesets)

**Settings → Rules → Rulesets → New branch ruleset** (modern; or **Settings →
Branches** classic). Create one targeting `main` and one targeting `develop` with:

- ✅ **Require a pull request before merging** (no direct pushes).
  - ✅ Require **1 approval**; ✅ **Require review from Code Owners** (uses
    `.github/CODEOWNERS`); ✅ Dismiss stale approvals on new commits.
- ✅ **Require status checks to pass** + **Require branches up to date**. Select, once
  they've run at least once (§5): the CI jobs (`ci`, docker/bench smoke as desired)
  **and `branch-policy`** ← this is the GitFlow gate.
- ✅ **Require conversation resolution.**
- ✅ **Block force pushes**; ✅ **Restrict deletions.**
- ✅ (recommended) **Require linear history** — matches the repo's FF style.
- ✅ **Include administrators** / "Enforce for admins" — so the rules bind you too
  (you can still merge via PR).

`gh` equivalent (classic protection API; run for `main` and `develop`, fill the
real check names from §5):

```bash
for BR in main develop; do
  gh api -X PUT "repos/andrade-filipe/relativist/branches/$BR/protection" \
    -F required_pull_request_reviews.required_approving_review_count=1 \
    -F required_pull_request_reviews.require_code_owner_reviews=true \
    -F enforce_admins=true \
    -F required_status_checks.strict=true \
    -F 'required_status_checks.contexts[]=branch-policy / validate' \
    -F 'required_status_checks.contexts[]=ci' \
    -F required_linear_history=true \
    -F allow_force_pushes=false -F allow_deletions=false \
    -F restrictions=
done
```

> The exact check context name (`branch-policy / validate` vs `validate`) is whatever
> appears in the PR's checks list after the workflow runs once — select it from the
> UI list rather than guessing.

### 4.4 Default branch

Keep **`main`** as the default (it's the public face: README, releases, social
preview). Contributors who accidentally open a PR into `main` get an automatic,
explained rejection from `branch-policy` telling them to retarget `develop`. (If you
prefer zero friction over a clean public landing, you *may* set `develop` as default
so PRs default to it — but then the repo homepage renders `develop`. `main` default
is recommended here.)

### 4.5 Belt-and-suspenders (optional, client-side)

Server-side rules are the enforcement. For a nicer local experience you can ship a
`pre-push` hook / the pre-commit framework that warns on bad branch names before the
push even reaches GitHub — see the `pocock-setup-pre-commit` skill. Note these are
**advisory** (a contributor can skip them); never rely on them as the gate.

## 5. Confirm CI runs

Four workflows (see [`docs/TESTING.md`](TESTING.md) for the test tiers):

| Workflow | Trigger | Required gate? |
|---|---|---|
| `ci.yml` | push/PR to `main`,`develop` | ✅ **yes** — fmt + clippy + `cargo test --lib` + build (~3 min) |
| `branch-policy.yml` | PR | ✅ **yes** — GitFlow gate |
| `release.yml` | tag `v*` | n/a — builds binaries/.deb + Release |
| `docker.yml` | tag `v*` | n/a — publishes GHCR image |
| `extended-tests.yml` | weekly + manual | ❌ **no — do NOT mark required** (slow optional integration suite) |

- Open a throwaway PR into `develop` → confirm `ci` and `branch-policy` run green.
- Add **only** `branch-policy` + `ci` as **required** checks in the §4.3 rulesets.
  Do NOT require `extended-tests` (it's the optional tier and runs weekly/manual).
- Consider adding **`cargo audit`** (RustSec) and **Dependabot** — see §6.

## 6. Supply-chain / dependency security

- **Dependabot:** Settings → **Code security and analysis** → enable *Dependabot
  alerts* + *security updates*. Optionally add `.github/dependabot.yml` for `cargo`.
  (Dependabot's `dependabot/*` branches are whitelisted by `branch-policy`.)
- **`cargo audit` in CI:** a job running `cargo install cargo-audit && cargo audit`.
- **Secret scanning + push protection:** enable both (free for public repos).

## 7. Private vulnerability reporting

**Settings → Code security and analysis → Private vulnerability reporting → Enable**
— activates the "Report a vulnerability" button `SECURITY.md` points to.

## 8. Issues, templates, Discussions

- **Issues** on by default; bug/feature templates exist in `.github/ISSUE_TEMPLATE/`.
- The **PR template** (`.github/PULL_REQUEST_TEMPLATE.md`) states the GitFlow-lite
  target-branch rule and the RPI/gates checklist.
- Consider enabling **Discussions** for Q&A.

## 9. Releases (the develop → main → tag flow)

Cutting a release follows the model: open a `release/x.y.z` PR from `develop` into
`main`, merge it, then tag `main`.

```bash
git switch develop && git switch -c release/0.23.0
gh pr create --base main --head release/0.23.0 --title "release: v0.23.0" --fill
# after merge:
git switch main && git pull
git tag v0.23.0 && git push origin v0.23.0
gh release create v0.23.0 --generate-notes
```

Version floor: tags must be **≥ v0.20.0** (v0.20/0.21/0.22 already exist); don't
regress to `v0.1x`. `release.yml` builds artifacts on tag.

## 10. Community standards checklist

**Insights → Community Standards** should be all green: README, LICENSE,
CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, issue+PR templates, CODEOWNERS.

## 11. Citation (nice-to-have)

Add a `CITATION.cff` at the repo root for a "Cite this repository" button — once the
article's final citation/DOI is known.

---

### Quick checklist

- [ ] `chore/enforce-gitflow` merged into `develop` (then to `main`) — `branch-policy` on both
- [ ] Rulesets protect **`main`** and **`develop`**: PR required, CODEOWNERS review, `branch-policy` + `ci` required, no force-push/delete, admins included
- [ ] Default branch = `main`; sidebar shows **Apache-2.0**
- [ ] About: description + topics + (optional) website; social preview uploaded
- [ ] Dependabot + secret scanning + push protection on; private vuln reporting on
- [ ] Discussions enabled (optional)
- [ ] Community Standards all green
