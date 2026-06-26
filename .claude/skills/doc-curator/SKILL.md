---
name: doc-curator
description: >
  The standard for writing and maintaining Relativist's LLM-native documentation. Use whenever you
  add, move, rewrite, or review a doc under docs/ — and in the "update the living docs" step of the
  RPI loop. It defines the YAML frontmatter schema (the cataloging technique that makes docs
  keyword-retrievable), the compactness rules (compact, precise, no redundancy), the source-of-truth
  order (code > specs > archive), and the rule that the catalog (docs/README.md) and the module map
  (docs/architecture/modules.md) must stay in sync. Triggers: "write/update the docs", "document
  this module/feature", "the docs are stale", adding a new doc, moving/renaming a doc, or finishing
  an RPI cycle that changed behavior. Anti-triggers: writing the formal specs in docs/specs/ (those
  keep their own format — only update docs/specs/README.md when one is added); editing docs/_archive/
  (frozen, read-only); generating code.
license: MIT
---

# doc-curator — Relativist documentation standard

Relativist's docs exist to give an LLM (or a human) **exactly the right context, fast**. Every doc
is optimized for retrieval, not prose volume. Follow this standard for any change under `docs/`.

## Source-of-truth order

1. **Code** (`relativist-core`, `relativist-cli`) — the living truth. If a doc disagrees with the
   code, the code wins; fix the doc.
2. **Specs** (`docs/specs/`) — define intent and the invariants (SPEC-01: T/D/I/G). Reference, not
   a gate.
3. **Archive** (`docs/_archive/`) — frozen history. Read-only. Never cite it as current behavior.

Verify claims against the code before writing them. Prefer pointing to the canonical doc over
restating it (no redundancy).

## Frontmatter schema (required on every live narrative doc)

Every doc under `docs/` EXCEPT `docs/specs/*` and `docs/_archive/*` starts with:

```yaml
---
title: <short human title>
summary: <one sentence, <=140 chars — what this doc is>
keywords: [<8-15 lowercase terms a searcher would grep for>]
modules: [<code modules this doc explains, e.g. net, merge; [] if none>]
specs: [<related SPEC-NN ids; [] if none>]
audience: [<subset of: user, contributor, llm, researcher>]
status: <reference | guide | draft>
updated: <YYYY-MM-DD>
---
```

`keywords` is the load-bearing field: it is how the catalog and `grep` find this doc. Include the
terms, synonyms, flag names, type names, and SPEC ids a reader would search for. `summary` must be
specific (name the phase/feature/topic), not generic.

## Compactness rules

- **Compact:** say it once, in the fewest words that stay precise. Cut filler and restated context.
- **No redundancy:** one canonical home per topic. If two docs cover the same thing, merge or make
  one point to the other.
- **Keyword-first:** lead sections with the term a reader greps for; put it in a heading.
- **Stable anchors:** predictable kebab-case `##`/`###` headings and filenames, so deep links and
  keyword scoping are reliable. Don't rename headings casually.
- **English**, present tense, active voice. Tables for enumerable facts (flags, modules, rules).
- **Link, don't duplicate:** theory → `docs/theory/`, architecture → `docs/architecture/`, formal
  detail → `docs/specs/`. Cross-link instead of copying.

## When you add, move, or rename a doc

1. Give it correct frontmatter (above).
2. Add/Update its row in the catalog **`docs/README.md`** (category table + the by-module index).
3. If it explains a code module or subcommand, update **`docs/architecture/modules.md`**.
4. If it's a formal spec, update **`docs/specs/README.md`** instead of adding frontmatter.
5. Fix inbound links. Leave `docs/_archive/**` untouched (its links intentionally reflect old
   layouts).

## Reviewing a doc (checklist)

- [ ] Frontmatter present and accurate (summary specific, keywords rich).
- [ ] Claims verified against code/specs; no stale flags, paths, or counts.
- [ ] No redundancy with another live doc; cross-links instead of copies.
- [ ] Headings are stable kebab-case; the doc is in the catalog and (if code-related) the module map.
- [ ] English, compact, present tense.

## Validation

Run the [`doc-catalog`](../../agents/doc-catalog.md) agent to check that every live doc has
frontmatter, appears in `docs/README.md`, and has no broken intra-repo links.
