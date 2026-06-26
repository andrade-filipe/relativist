---
name: pocock-git-guardrails
description: Set up Claude Code PreToolUse hooks that intercept and block dangerous git operations before they execute. Use when: user wants to prevent Claude from running destructive git commands (force push, reset --hard, branch deletion, clean, etc.); operator wants to install project-scoped or global git safety hooks; user wants to customize the blocked-command list; user says "add git guardrails", "prevent git push", "block dangerous git ops", or "protect my repo from Claude". Guides scope decision (project vs global), copies the bundled hook script, wires it into the appropriate settings.json, optionally customizes the blocked patterns, and verifies the hook fires correctly. Does not apply to non-git shell commands — only git operations in the blocked list.
license: MIT
source_author: Matt Pocock
source_url: https://github.com/mattpocock/skills/tree/main/skills/misc/git-guardrails-claude-code
---

> **Attribution:** Matt Pocock, MIT license. Original at https://github.com/mattpocock/skills/tree/main/skills/misc/git-guardrails-claude-code.

# Pocock Git Guardrails

Sets up a `PreToolUse` hook that intercepts and blocks dangerous git commands before Claude executes them.

## What Gets Blocked

- `git push` (all variants including `--force`)
- `git reset --hard`
- `git clean -f` / `git clean -fd`
- `git branch -D`
- `git checkout .` / `git restore .`

When blocked, Claude sees a message telling it that it does not have authority to access these commands.

## Steps

### 1. Ask scope

Ask the user: install for **this project only** (`.claude/settings.json`) or **all projects** (`~/.claude/settings.json`)?

### 2. Copy the hook script

The bundled script is at: [scripts/block-dangerous-git.sh](scripts/block-dangerous-git.sh)

Copy it to the target location based on scope:

- **Project**: `.claude/hooks/block-dangerous-git.sh`
- **Global**: `~/.claude/hooks/block-dangerous-git.sh`

Make it executable with `chmod +x`.

### 3. Add hook to settings

Add to the appropriate settings file:

**Project** (`.claude/settings.json`):

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/block-dangerous-git.sh"
          }
        ]
      }
    ]
  }
}
```

**Global** (`~/.claude/settings.json`):

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "~/.claude/hooks/block-dangerous-git.sh"
          }
        ]
      }
    ]
  }
}
```

If the settings file already exists, merge the hook into existing `hooks.PreToolUse` array — don't overwrite other settings.

### 4. Ask about customization

Ask if user wants to add or remove any patterns from the blocked list. Edit the copied script accordingly.

### 5. Verify

Run a quick test:

```bash
echo '{"tool_input":{"command":"git push origin main"}}' | <path-to-script>
```

Should exit with code 2 and print a BLOCKED message to stderr.

## Limitations

- **Install-time only.** This Skill configures a hook; it does not audit existing settings.json for conflicting hooks. If a `PreToolUse` hook with a different `matcher` or overlapping `command` already exists, manual merge review is advisable.
- **Platform-scoped.** The bundled `block-dangerous-git.sh` is a bash script; it assumes a Unix-like shell. Windows environments using PowerShell as the Bash tool substrate may need a PowerShell-variant hook.
- **Pattern list is not exhaustive.** The default blocked list covers the most destructive operations per Pocock's original design. It does NOT block all potentially irreversible operations (e.g., `git tag -d`, rebase in some modes). Users should customize via Step 4 for their risk profile.
- **Verification step is manual.** Step 5 requires the user to run a test payload — it is not auto-executed by the Skill. The exit-code 2 contract is what the Claude Code harness uses to block the tool call; if the script exits with 0 or 1, hooks may not fire as expected.

---

> Provenance + framework classification: see `composition.yaml` (sidecar).
> Compliance badges: see `badges-draft.yaml` (architect sign-off pending).
