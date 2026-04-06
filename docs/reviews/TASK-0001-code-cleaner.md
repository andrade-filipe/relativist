# Code Cleaner Review — TASK-0001

**Task:** Convert net module to directory structure
**Reviewer:** Code Cleaner (Stage 4)
**Date:** 2026-04-06

---

## Summary

This task is purely structural (file reorganization). No logic, no functions, no types. The review scope is limited to module organization and naming.

## Issues Found

### Should-Fix

**SF-1: `#[allow(unused_imports)]` will become permanent noise**

- **Location:** `src/net/mod.rs:10-13`
- **Problem:** The `#[allow(unused_imports)]` annotations suppress warnings for empty modules. As types are added in TASK-0002+, these should be removed.
- **Recommendation:** Remove the allow attributes as soon as TASK-0002 (Symbol) and TASK-0008 (Net) are implemented and the re-exports have actual items.
- **Severity:** Should-Fix (remove in TASK-0002/TASK-0008, not a separate task)

### Nice-to-Have

None.

## Must-Fix

None.

## Verdict

**PASS** — No must-fix issues. One should-fix tracked for cleanup in subsequent tasks.
