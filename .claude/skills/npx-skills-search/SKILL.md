---
name: npx-skills-search
description: Use whenever any agent (main or subagent) needs to sample, discover, or manage community Skills via the `npx skills` CLI (skills.sh marketplace) — including any of these triggers. Search/find triggers — user types "search skills", "find a skill", "browse skills.sh", "what skills exist for X", or mentions skills.sh directly; agent needs to run `npx skills find <term>` to enumerate community SKILL.md candidates. Authoring triggers — agent is drafting or reviewing a `.claude/skills/<name>/SKILL.md` file and would benefit from sampling community SKILL.md shapes (description style, trigger density, frontmatter, marketplace_metadata) before designing in a vacuum; fires at the moment a Skill artifact is being shaped, not after it is written. Sampling triggers — designing a new Technique-tier Skill artifact (per the framework's tier→artifact mapping: `technique → .claude/skills/<technique>/SKILL.md`) and wants live community examples (`owner/repo@skill` plus install counts) before committing to a shape; fires alongside `superpowers:writing-skills` but BEFORE it, to feed it sampled examples. Discovery triggers — "is there a skill for X?", "what's available for <domain>?", or any request to discover/explore the community catalog without committing to install; the CLI's non-interactive `find` is the cheapest discovery channel. Install-management triggers — user explicitly asks to install / remove / update / list skills via `npx skills add | remove | update | list | init | experimental_sync | experimental_install`; ALWAYS confirm with user before running `add`, `remove`, `update`, or `experimental_*` because they touch global or project state. Anti-triggers — REF-NNNN catalog lookups → use `ref-index`; framework tier classification meta-questions (constant vs technique vs workflow) → use `framework-glossary`; authoring a new repo-internal Skill from scratch with NO community sampling needed → go directly to `superpowers:writing-skills`; discovering an MCP server (not a Skill) → use `smithery-ai-cli`. This skill is the SAMPLING channel: it tells the agent what shapes the community uses; framework glossaries and the architect's write-boundary decide what to DO with the sample.
---

# npx-skills-search

Inject the `npx skills` CLI as a sampling and discovery channel for the `skills.sh` community marketplace. Read-mostly: this skill enumerates community Skills and surfaces their SKILL.md shapes; it never edits files itself, and any actual install or write goes through the architect.

## Commands

| Command | Purpose |
|---|---|
| `npx skills find <term>` | Non-interactive search of skills.sh; returns `owner/repo@skill`, install count, URL. The primary sampling call. |
| `npx skills list [-g] [--json]` | Enumerate installed skills (project by default; `-g` for global; `--json` for machine-readable). |
| `npx skills add <owner/repo[@skill]>` | Install a skill. Flags: `-g` (global), `-a <agent>` (target agent), `-s <skill>` (target slot), `--all`, `--copy`. State-touching — confirm with user first. |
| `npx skills remove [skills]` | Uninstall one or more skills. State-touching — confirm with user first. |
| `npx skills update [skills...]` | Update one or more installed skills. State-touching — confirm with user first. |
| `npx skills init [name]` | Scaffold a new local skill. |
| `npx skills experimental_sync` | Experimental: sync local manifest with marketplace. State-touching — confirm. |
| `npx skills experimental_install` | Experimental: bulk install pathway. State-touching — confirm. |

## When to fire

Three concrete scenarios that map to the trigger categories in the description:

1. **Sampling before authoring (Authoring + Sampling triggers).** Architect is about to write `.claude/skills/<new-name>/SKILL.md`. Run `npx skills find <closest-keyword>` first; pull 3-5 community SKILL.md examples to calibrate description density and trigger-list shape; then hand off to `superpowers:writing-skills` (or author directly under architect write-boundary) with the samples in context.
2. **Discovery for a vague ask (Search/find + Discovery triggers).** User asks "is there a skill for postgres migrations?" or "what's on skills.sh for code review?". Run `npx skills find <term>` non-interactively; surface the top hits with install counts; let the user decide whether to install or just inspect.
3. **Explicit install management (Install-management triggers).** User says "install the X skill globally" or "uninstall Y". Echo the exact `npx skills add|remove|update` command, confirm scope (`-g` vs project), confirm with user, then run it. Never silently install.

## Discipline / guardrails

- **Community quality varies — sampling, not authority.** A high-install-count skill on skills.sh is evidence of community fit, not framework fit. The framework's own glossaries (`framework-glossary`, `techniques-glossary`, `workflows-glossary`) and tier system override any community pattern that conflicts.
- **Do NOT auto-install.** Always confirm with the user before running `npx skills add`, `remove`, `update`, `experimental_sync`, or `experimental_install`. These touch global or project state; silent state mutation is out of scope for this skill.
- **Sampling, not copying.** Extract the shape or idea from a community SKILL.md (description density, trigger categories, body-section layout), then author the artifact per this framework's conventions (tier-aware frontmatter, REF citations, anti-triggers, write-boundary discipline). Do not lift a community SKILL.md verbatim.
- **Write-boundary preserved.** This skill does NOT bypass the architect-only `.claude/skills/*` write boundary declared in `CLAUDE.md`. After sampling, the actual write to `.claude/skills/<name>/SKILL.md` goes through the `architect` meta-agent under a `goal-execution` envelope — not through this skill, and not through `general-purpose`.

## References

- **skills.sh marketplace** — https://skills.sh (the community directory the CLI queries).
- **Full flag matrix** — `npx skills --help` (and `npx skills <subcommand> --help` for per-subcommand flags).
- **Auto-memory pointer** — `[[reference-npx-skills-catalog]]` (project auto-memory entry on the CLI surface and routing).
- **Write-boundary pointer** — `[[feedback-dispatch-via-repo-subagents]]` (the architect-only `.claude/skills/*` write boundary; never use `general-purpose` to write a Skill).
- **Sibling skills** — `superpowers:writing-skills` (the actual author-a-skill loop, fed by samples from here); `framework-glossary` (tier → artifact_type=skill mapping); `ref-index` (for REF citations in the new Skill's evidence section if applicable).
