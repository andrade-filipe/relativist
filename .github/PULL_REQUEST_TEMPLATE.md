<!--
GitFlow-lite: open this PR against `develop` (not `main`).
Only `develop`, `release/*`, or `hotfix/*` may target `main`.
The `branch-policy` check enforces this — a PR into `main` from a feature branch
will be rejected automatically.
-->

## Summary

What does this change and why? (One or two sentences.)

## Target branch

- [ ] This PR targets **`develop`** (feature/fix work), **or** it is a `release/*` / `hotfix/*` PR into `main`.

## Workflow (RPI + TDD)

- [ ] **TDD**: behavioral changes arrived test-first (a test that would have failed before this
      change). Core/engine behavior is covered by a **library unit test** (the `cargo test --lib`
      gate). N/A for docs/CI-only changes.
- [ ] Followed Research → Plan → Implement (or it's a trivial change).
- [ ] If this touches the model (the six interaction rules, partition/merge, the
      SPEC-01 invariants, or the `reduce_all ≅ run_grid` contract), an issue was
      opened to discuss first (see `GOVERNANCE.md`).

## Gates (CI enforces these)

- [ ] `cargo test` passes — no regression in the test count.
- [ ] `cargo clippy --all-features -- -D warnings` is clean.
- [ ] `cargo fmt --check` passes.

## Docs

- [ ] Living docs updated for any behavior/flag/module change, per the
      [`doc-curator`](../.claude/skills/doc-curator/SKILL.md) standard (frontmatter +
      the catalog `docs/README.md` stay in sync). N/A if no behavior change.

## Notes

Anything reviewers should know (trade-offs, follow-ups, screenshots, …).
