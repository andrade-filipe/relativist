---
title: Maintainer GitHub Setup Guide
summary: Web-UI/gh-CLI steps to finish before announcing the open-source repo — About sidebar, social preview, license detection, branch protection.
keywords: [maintainer, github, open source, gh cli, branch protection, social preview, license detection, apache-2.0, topics, repository settings, launch, announce]
modules: []
specs: []
audience: [contributor]
status: guide
updated: 2026-06-26
---

# Maintainer guide — GitHub setup before announcing

Everything in this repo (license, governance, docs, CI, reproducibility) is ready.
This file lists the steps that **must be done in the GitHub web UI or with `gh`** —
they can't be committed from the codebase. Do these before you post on social media.

Repository: `github.com/andrade-filipe/relativist`. Most settings are under
**Settings** (gear) unless noted. Commands assume the [`gh` CLI](https://cli.github.com/)
is authenticated (`gh auth login`).

---

## 0. Land this work first

This branch is `chore/open-source-launch-readiness`. Review it, then merge it the
way you normally integrate (`v2-development` → `main`). Nothing here was pushed for
you.

```bash
git push -u origin chore/open-source-launch-readiness
gh pr create --base v2-development --fill   # or merge locally, your call
```

> The Apache-2.0 relicense and the `git mv` of ~1,900 files are in the history of
> this branch. No history was rewritten, so existing clones/forks stay valid.

## 1. Repository "About" sidebar

**Main repo page → ⚙ next to "About":**

- **Description:** e.g. *"Distributed Interaction Combinator reducer for Grid
  Computing — Rust, formally specified, deterministic via strong confluence."*
- **Website:** link to the published article / your page, if any.
- **Topics:** `rust`, `interaction-combinators`, `interaction-nets`,
  `distributed-systems`, `grid-computing`, `graph-rewriting`, `lambda-calculus`,
  `confluence`, `bsp`, `research`.
- Tick **Releases** and **Packages** visibility as you prefer.

```bash
gh repo edit --description "Distributed Interaction Combinator reducer for Grid Computing (Rust)" \
  --add-topic rust --add-topic interaction-combinators --add-topic distributed-systems \
  --add-topic grid-computing --add-topic graph-rewriting --add-topic research
```

## 2. Social preview image

**Settings → General → Social preview.** Upload a 1280×640 PNG (the architecture
diagram or a title card). This is what renders when the repo link is shared on
X/LinkedIn — worth doing before you announce.

## 3. License detection

GitHub auto-detects `LICENSE`. Confirm the repo sidebar shows **"Apache-2.0"**
(not "View license"). If it still says MIT, it's caching the old `LICENCE` file —
it updates after the merge to the default branch.

## 4. Branch protection (protect `main`)

**Settings → Branches → Add branch ruleset** (or classic "Branch protection
rules") for `main` (and optionally `v2-development`):

- ✅ Require a pull request before merging.
- ✅ Require status checks to pass — select the CI checks once they've run at least
  once (see §5): the `cargo test` / `clippy` / `fmt` jobs.
- ✅ Require branches to be up to date before merging.
- ✅ Require conversation resolution.
- ✅ Include administrators (optional but recommended for a clean history).
- ❌ Leave force-push and deletion disabled.

```bash
# Example (classic protection) — adjust the check names to your workflow's job names:
gh api -X PUT repos/andrade-filipe/relativist/branches/main/protection \
  -F required_pull_request_reviews.required_approving_review_count=0 \
  -F enforce_admins=true \
  -F required_status_checks.strict=true \
  -F 'required_status_checks.contexts[]=build' \
  -F restrictions=
```

## 5. Confirm CI runs

The workflows already exist in `.github/workflows/` (`ci.yml`, `docker.yml`,
`docker-smoke.yml`, `bench-smoke.yml`, `release.yml`). After the merge:

- **Actions tab** → confirm `ci.yml` runs green on `main`.
- Then go back to §4 and add those check names as required status checks.
- Consider adding **`cargo audit`** (RustSec advisory scan) and **Dependabot** —
  see §6.

## 6. Supply-chain / dependency security

- **Dependabot:** Settings → **Code security and analysis** → enable *Dependabot
  alerts* and *Dependabot security updates*. Optionally add a
  `.github/dependabot.yml` for `cargo` to get version-bump PRs.
- **`cargo audit` in CI:** add a job that runs `cargo install cargo-audit && cargo
  audit` so known-vulnerable crates fail CI.
- **Secret scanning + push protection:** same panel — enable both (free for public
  repos). Nothing secret is in the tree today (`.env` is gitignored, only
  `.env.example` is tracked), and history was not rewritten, so this is forward
  protection.

## 7. Private vulnerability reporting

**Settings → Code security and analysis → Private vulnerability reporting →
Enable.** This activates the **"Report a vulnerability"** button that `SECURITY.md`
points contributors to.

## 8. Issues, templates, Discussions

- **Issues** are on by default; the bug/feature templates already exist in
  `.github/ISSUE_TEMPLATE/`. Confirm they render on **New issue**.
- Consider enabling **Discussions** (Settings → General → Features) for Q&A and
  "how do I…" threads, so issues stay for actionable work.
- The PR template (`.github/PULL_REQUEST_TEMPLATE.md`) is already in place.

## 9. Releases

`release.yml` builds artifacts on tag. Per the project's versioning floor, the
next tag should be **≥ `v0.20.0`** (continuing from `v0.20.0-pre`); don't regress
to a `v0.1x` tag.

```bash
# When you're ready to cut the first public release:
git tag v0.20.0 && git push origin v0.20.0
gh release create v0.20.0 --generate-notes
```

Mark it as the **Latest release**; not a pre-release if it's stable.

## 10. Community standards checklist

**Insights → Community Standards** should show all green after the merge: README,
LICENSE, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, issue+PR templates. If any item
is missing, it'll point to what's absent.

## 11. Citation (nice-to-have)

Add a `CITATION.cff` at the repo root so GitHub shows a **"Cite this repository"**
button — useful since this is academic work. (Not added automatically; create it
when the article's final citation/DOI is known.)

---

### Quick pre-announce checklist

- [ ] Branch merged; sidebar shows **Apache-2.0**
- [ ] About: description + topics + (optional) website
- [ ] Social preview image uploaded
- [ ] `main` protected; CI green and required
- [ ] Dependabot + secret scanning + push protection on
- [ ] Private vulnerability reporting on
- [ ] Discussions enabled (optional)
- [ ] First release tagged (≥ v0.20.0) — optional but nice for an announcement
- [ ] Community Standards all green
