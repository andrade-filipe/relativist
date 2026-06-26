---
tier: workflow
task_class: mixed
targets_scenarios:
  - factual-qa-with-source-citation
  - structured-output-compliance
pack_name: rpi-pipeline
member_agents:
  - researcher-agent
  - planner-agent
  - implementer-agent
member_skills: []
orchestration_substrate: filesystem
---

# RPI (Research-Plan-Implement) Pipeline Pack

A high-discipline Workflow Pack that enforces the Research-Plan-Implement pipeline pattern. It is designed for complex engineering tasks where context bloat and "Dumb Zone" degradation are significant risks.

## Overview

The RPI Pipeline divides a task into three serialized phases, with mandatory context resets between them to maintain high cognitive attention.

1. **Research Phase:** Exhaustive codebase mapping and dependency analysis. Outputs `RESEARCH.md`.
2. **Plan Phase:** Translation of research into a surgical, testable implementation plan. Outputs `PLAN.md`.
3. **Implement Phase:** Precision execution of the plan followed by mandatory verification.

## Member Agents

- **researcher-agent:** `research-breadth-coherent` specialist for deep codebase exploration.
- **planner-agent:** `decision-coherent` specialist for technical design and verification strategy.
- **implementer-agent:** `decision-coherent` specialist for surgical code modification and testing.

## Orchestration Glue

The pack relies on **filesystem-based handoffs**. 
- The `researcher-agent` writes `RESEARCH.md`.
- The `planner-agent` reads `RESEARCH.md` and writes `PLAN.md`.
- The `implementer-agent` reads `PLAN.md` and executes.

A session reset is required between each agent invocation to clear the context window.

## Usage

Dispatch the pack by initializing the `researcher-agent` with the user's high-level goal. Follow the phase-serialized flow until implementation is verified.
