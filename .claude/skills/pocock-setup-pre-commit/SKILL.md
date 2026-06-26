---
name: pocock-setup-pre-commit
description: Set up Husky pre-commit hooks with lint-staged (Prettier), type checking, and tests in the current repo. Use when: user wants to add pre-commit hooks; user wants to set up Husky; user wants to configure lint-staged; user wants commit-time formatting, type-checking, or testing enforcement; user says "add pre-commit hooks", "set up Husky", "configure lint-staged", or "add Prettier on commit". Detects the project's package manager (npm/pnpm/yarn/bun), installs Husky + lint-staged + Prettier as devDependencies, initializes Husky, writes the pre-commit hook, creates .lintstagedrc and .prettierrc (if missing), and smoke-tests by committing through the new hooks. Adapts the hook script to the detected package manager and skips typecheck/test invocations if those scripts are absent from package.json. Composes naturally with pocock-git-guardrails — install both to protect the repo from both bad commits and destructive git commands.
license: MIT
source_author: Matt Pocock
source_url: https://github.com/mattpocock/skills/tree/main/skills/misc/setup-pre-commit
---

> **Attribution:** Matt Pocock, MIT license. Original at https://github.com/mattpocock/skills/tree/main/skills/misc/setup-pre-commit.

# Pocock Setup Pre-Commit

Sets up Husky pre-commit hooks with lint-staged (Prettier), type checking, and tests in the current repo.

## What This Sets Up

- **Husky** pre-commit hook
- **lint-staged** running Prettier on all staged files
- **Prettier** config (if missing)
- **typecheck** and **test** scripts in the pre-commit hook

## Steps

### 1. Detect package manager

Check for `package-lock.json` (npm), `pnpm-lock.yaml` (pnpm), `yarn.lock` (yarn), `bun.lockb` (bun). Use whichever is present. Default to npm if unclear.

### 2. Install dependencies

Install as devDependencies:

```
husky lint-staged prettier
```

### 3. Initialize Husky

```bash
npx husky init
```

This creates `.husky/` dir and adds `prepare: "husky"` to package.json.

### 4. Create `.husky/pre-commit`

Write this file (no shebang needed for Husky v9+):

```
npx lint-staged
npm run typecheck
npm run test
```

**Adapt**: Replace `npm` with detected package manager. If repo has no `typecheck` or `test` script in package.json, omit those lines and tell the user.

### 5. Create `.lintstagedrc`

```json
{
  "*": "prettier --ignore-unknown --write"
}
```

### 6. Create `.prettierrc` (if missing)

Only create if no Prettier config exists. Use these defaults:

```json
{
  "useTabs": false,
  "tabWidth": 2,
  "printWidth": 80,
  "singleQuote": false,
  "trailingComma": "es5",
  "semi": true,
  "arrowParens": "always"
}
```

### 7. Verify

- [ ] `.husky/pre-commit` exists and is executable
- [ ] `.lintstagedrc` exists
- [ ] `prepare` script in package.json is `"husky"`
- [ ] prettier config exists
- [ ] Run `npx lint-staged` to verify it works

### 8. Commit

Stage all changed/created files and commit with message: `Add pre-commit hooks (husky + lint-staged + prettier)`

This will run through the new pre-commit hooks — a good smoke test that everything works.

## Notes

- Husky v9+ doesn't need shebangs in hook files
- `prettier --ignore-unknown` skips files Prettier can't parse (images, etc.)
- The pre-commit runs lint-staged first (fast, staged-only), then full typecheck and tests

## Limitations

- **Node.js projects only.** This Skill requires `package.json` to be present. It is not applicable to non-Node.js repositories (Rust, Python, Go, etc.) — those ecosystems have different pre-commit tooling (e.g., pre-commit.com, lefthook).
- **Husky v9+ assumed.** The `.husky/pre-commit` hook is written without a shebang per Husky v9+ conventions. If the project pins Husky v8 or earlier, a shebang line (`#!/usr/bin/env sh`) and a `. "$(dirname "$0")/_/husky.sh"` source line are required.
- **typecheck / test script availability.** The Skill omits those hook lines and notifies the user if the relevant scripts are absent from `package.json`. It does not scaffold them — that is outside this Skill's scope.
- **Prettier conflict detection.** The Skill creates `.prettierrc` only if no Prettier config exists. It does not detect config in `package.json` `prettier` key or in `.prettierrc.js` / `.prettierrc.cjs` / `prettier.config.js`. Operators should verify Prettier resolution order if they use a non-standard config location.
- **Smoke-test commit side-effect.** Step 8 creates a real commit. Operators who want a clean pre-Husky history should squash or drop this commit post-install.

---

> Provenance + framework classification: see `composition.yaml` (sidecar).
> Compliance badges: see `badges-draft.yaml` (architect sign-off pending).
