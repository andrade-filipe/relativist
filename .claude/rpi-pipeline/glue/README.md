# RPI Orchestration Glue

The RPI (Research-Plan-Implement) pipeline uses **filesystem-based handoffs** as its primary orchestration substrate.

## Phase Transitions

Transitions between phases are triggered by the `phase_boundary_command` defined in the workflow pattern.

1. **Researcher -> Planner:**
   - Researcher writes `RESEARCH.md`.
   - Researcher invokes the reset command.
   - Harness loads `RESEARCH.md` and dispatches the Planner.

2. **Planner -> Implementer:**
   - Planner writes `PLAN.md`.
   - Planner invokes the reset command.
   - Harness loads `PLAN.md` and dispatches the Implementer.

## Session Resets

A session reset is MANDATORY between each phase. This ensures:
- The context window is cleared of previous-phase noise.
- The model's attention is focused solely on the current phase's input artifact (`RESEARCH.md` or `PLAN.md`).
- Rule-skipping and "Dumb Zone" degradation are minimized.
