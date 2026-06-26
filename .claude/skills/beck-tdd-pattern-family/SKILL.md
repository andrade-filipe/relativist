---
name: beck-tdd-pattern-family
description: >
  Use when the user is writing tests alongside production code and needs a
  disciplined cadence for how each test moves the code from red to green to
  refactored. The Skill bundles Beck's five core TDD families — the red-green-
  refactor cycle, implementation strategies (Fake It Till You Make It,
  Triangulation, Obvious Implementation), rhythm discipline (Baby Steps, One
  Step Test, Child Test), test organization (Test List, Isolated Test, Assert
  First), and the test-list externalized-working-memory move — into one
  selection rubric: for the current test, pick which strategy the situation
  demands. Trigger when the user says "I'm doing TDD on X", "write a failing
  test first", "fake it for now", "triangulate this", "the bar won't go green",
  "next test from the list", or proposes test-first work on greenfield, a new
  feature on tested code, or a regression. Anti-triggers: ill-formed problem
  whose specification is unclear (compose with design-first-discipline
  upstream); legacy code with no tests (compose with feathers-characterization-
  tests-first first); a deliberate "what's the right design?" thinking session
  (TDD verifies a chosen design — it does not surface the right problem).
license: CC-BY-4.0
source_author: Kent Beck
source_url: https://www.pearson.com/en-us/subject-catalog/p/test-driven-development-by-example/P200000009421
---

> **Attribution:** Kent Beck, *Test-Driven Development: By Example*, Addison-Wesley Professional, 2002. This Skill is an original framework distillation of the book's pattern catalog (Money + xUnit worked examples + Part III TDD patterns); the named patterns and the red-green-refactor cycle vocabulary are Beck's.

# Beck: TDD Pattern Family

Beck's TDD discipline is **a five-family vocabulary** governed by one selection rubric: for the test in front of you right now, which pattern fits this exact bar state? The Skill is family-grouped — all five families ship together because they tightly co-cite within Beck's book and split-installation would multiply co-dependency without conceptual benefit.

## The atomic unit: red-green-refactor

Every TDD step is one closed loop of three phases. The next test does not start until this loop closes.

1. **Red.** Write a small test that names the next required behavior. It fails because the behavior does not exist yet. The failure must be the *specific* failure you expected — if the test fails for a reason you did not predict, your model of the code is wrong; investigate before writing production code.
2. **Green.** Write the *minimum* production code that turns the bar green. "Minimum" includes ugly: hard-code literals, copy-paste, leave dead branches. The goal is the green bar, not the right code.
3. **Refactor.** With the bar green and the safety net live, improve the design without changing behavior. All tests stay green. Named refactorings from Fowler's catalog go here; no new behavior, no new tests.

The cycle is the atomic unit. If you cannot close it in minutes, the step was too big — see `## Rhythm discipline`.

## Implementation strategies: how to move red to green

Three legitimate moves get the bar green. Pick by current uncertainty about the right code.

- **Obvious Implementation.** The right code is small and you already know it. Write it directly. `return a + b` for an `add` method. Skip the ceremony. *Risk:* wrong call here pushes you back to red without the safety cushion of a smaller step. When in doubt, prefer Fake It.
- **Fake It Till You Make It.** The right code is not yet obvious. Return a hard-coded literal lifted from the test's expected value. `return Money.dollar(10)` to pass `$5 * 2 == $10`. The green bar is back — design pressure remains, but the safety net is live. Fake It is transitory; the next test forces it out.
- **Triangulation.** You just did Fake It (or you over-fit a real implementation to one example). Write a *second* test from a different angle that the current code cannot pass without generalization. `$5 * 3 == $15` forces `amount * multiplier`. Triangulation is the discipline that converts a fake into a general formula — without it, Fake It ships hard-coded values.

The three moves are a triangle. Fake It is the safe default when uncertain; Triangulation is the obligation that follows it; Obvious Implementation is the speed move when uncertainty is genuinely zero.

## Rhythm discipline: when the bar won't go green

When ten minutes pass red, the step was too big. The cure is to shrink, not to remove the safety net.

- **Baby Steps.** Size each red-green-refactor cycle so it closes in minutes. Step size is dynamic — large in familiar territory, small (one line of test + one line of production) in unfamiliar code, a tricky algorithm, or a flaky environment. The heuristic: *if you cannot see the green bar, the step was too big.*
- **One Step Test.** From the test list, pick the test that (a) you are confident you can make pass, (b) will teach you something you don't already know, (c) moves the system one observable behavior closer to done. **Not** the hardest test, the most thorough test, or the test that "covers" the most code. Monotonically increasing knowledge.
- **Child Test.** A test is too big to make green directly. Spawn a *smaller* test inside the same intent — the child test will go green; once it passes, the parent test is closer to passable. Recursive shrinking.

The rhythm patterns are the recovery vocabulary when the discipline strains. Reaching for the debugger before reaching for Baby Steps is the anti-pattern.

## Test organization: making tests trustworthy

Tests that are not isolated, not readable, or not driven by an explicit list degrade into noise. Three organizational patterns make the safety net usable.

- **Test List.** At the start of any feature, write down the tests you know you will need. Append-only working memory. Mid-cycle ideas (edge cases, error paths, related behaviors) get added to the list rather than derailing the current loop. The list shrinks as tests go green. This is the **externalized-working-memory** move; reading the list is reading the feature's scope.
- **Isolated Test.** Each test sets up its own fixtures, exercises behavior, asserts, tears down. No test depends on the order or state of any other. A failure points unambiguously at one behavior. The test suite is parallelizable. Non-isolated tests give false signals during refactor — they break for reasons unrelated to the change.
- **Assert First.** Write the assertion *first*, then work backward to the setup. Forces the test to declare the observable outcome before the mechanics get tangled in setup ceremony. The assertion IS the test's purpose; everything else is plumbing for it.

These three patterns are the table stakes that make the green bar trustworthy. Skipping them produces a test suite that looks like coverage but provides no safety net.

## The selection rubric: which pattern fits this test?

Pick the next pattern by reading the current bar state, the current uncertainty, and the current step size.

- **Bar is green and a refactor is visible** → refactor step; reach into Fowler's catalog (composes with `fowler-refactor-mechanics-family`). No new test.
- **Bar is red and the right code is obvious** → Obvious Implementation. Write the code.
- **Bar is red and the right code is not yet obvious** → Fake It; return the literal. Then queue a Triangulation test.
- **Bar is red and you already faked it once** → Triangulation; write the second test from a different angle.
- **Bar is red for ten minutes** → step was too big. Baby Steps (shrink the next move) or Child Test (spawn a smaller test inside the current intent).
- **You don't know which test to write next** → consult the Test List; apply One Step Test (highest-knowledge / lowest-risk pick).
- **A test reads awkwardly** → Assert First on the next test; consider rename/restructure (composes with `evans-ubiquitous-language`).
- **A test depends on another test's state** → make it Isolated; per-test fixtures, no shared mutable global.

The rubric is the load-bearing claim: TDD is not "write a test, write the code." It is *pick the right pattern for this exact bar state*.

## The §3.3 stance: TDD as design feedback, not THE design tool

This Skill takes an explicit position in the framework's documented TDD-vs-design-first debate (§3.3 of the developer-fundamentals concept card). The position is **`tests-as-design-feedback-companion`** — *not* `tests-as-primary-design-tool`.

What that means concretely:

- **Tests provide design feedback** (the API friction, the mock count, the assertion awkwardness) — this is consensus across the catalog and is the load-bearing claim under `testing-as-design-feedback`.
- **Tests do not surface whether the chosen problem is the right problem.** TDD verifies a chosen design via fast feedback; it does not replace the upstream design step that decides *what to build*. Per Hickey (REF-0173 hammock-driven-development), Dijkstra (REF-0151 "testing shows presence not absence of bugs"), Ousterhout (REF-0137 comments-first), and McConnell (REF-0146 PPP pseudocode-first), there is a legitimate design step *before* red-green-refactor that this Skill does not absorb.
- **The alternative is real.** `design-first-discipline` (the alternative-pair Technique) covers the other side of the debate explicitly. A buyer choosing `design-first-discipline` is choosing not to make TDD their primary design tool — and that is a defensible position the framework respects.

The §3.3 anti-pattern is *silently picking sides*. The Skill names its position so the buyer is not misled. If the problem in front of you is ill-formed (the spec is unclear, the domain is unfamiliar, the destination is uncertain), reach for the design-first step *first*, then TDD inside the chosen design. The red-green-refactor cycle does not surface whether you are iterating on the wrong problem.

The `tdd-as-primary-thinking-tool-for-ill-formed-problems` anti-pattern is the §3.3 failure mode this Skill explicitly guards against.

## Worked example

> **Task:** Add multi-currency addition to a `Money` value object that currently supports same-currency addition.

**Test List** (written before any code):

```
- $5 + $5 == $10  (same currency, regression — already passing)
- $5 + 10 CHF == ? (mixed currency: needs an exchange rate context)
- exchange rate $1 = 2 CHF; $5 + 10 CHF == $10
- exchange rate identity ($1 = $1); identity addition unchanged
- rate missing → explicit failure mode
```

**Cycle 1.** Pick `$5 + 10 CHF` with rate `$1 = 2 CHF` (One Step Test — known confidence, teaches the rate-application path).

- *Red.* Write: `assertEquals(Money.dollar(10), $5.plus(10 CHF, bank with rate))`. Fails — `plus` does not accept cross-currency.
- *Green.* Fake It: in `plus`, if the other currency differs, hard-code `return Money.dollar(10)`. Green bar back. The hard-code is visible in the diff; it will not survive Triangulation.
- *Refactor.* None — code is too ugly to refactor; ride the next cycle.

**Cycle 2.** Triangulation — write `$10 + 20 CHF` with the same rate → expect `$20`.

- *Red.* The hard-coded `Money.dollar(10)` cannot pass this; fails as expected.
- *Green.* Replace the literal with the actual formula: `this.amount + (other.amount / rate)`. Both tests pass.
- *Refactor.* Extract the rate-lookup into the bank object (named refactoring: Extract Method, from Fowler's catalog). Bar stays green.

**Cycle 3.** Reach for the rate-missing test (Test List item 5). Bar would need an exception path — pause and ask: *is this a TDD problem or a design problem?* The failure-mode design (exception vs Result type vs default rate) is upstream of TDD. Compose with `design-first-discipline` here: pick the failure-design protocol, *then* drive it via red-green-refactor.

The example shows the rubric in operation: One Step Test picked the first test; Fake It got the bar green; Triangulation forced generalization; a named refactor (Extract Method) ran on green; the §3.3 stance fired when a design question outgrew TDD's scope.

## Anti-patterns

- **Skipping the refactor step.** Red-green-skip-refactor ships once the bar is green; design quality decays. The refactor step is where the design-feedback signal converts into structural improvement. Compose with `fowler-refactor-mechanics-family` so the refactor step has named moves to execute, not unfocused "clean up."
- **Step too big.** Ten minutes red with no green in sight. Cure: shrink with Child Test + Baby Steps. Reaching for the debugger before shrinking is the failure.
- **Non-isolated tests.** Test B passes only because Test A ran first. Defeats the safety net — a failure no longer points at a specific behavior. Per-test fixtures, no shared mutable global state.
- **Triangulation skipped, Fake It shipped.** Hard-coded value goes into production. Fake It is transitory; without Triangulation, the discipline collapses into "write a test that passes a hard-code."
- **TDD as the primary thinking tool for ill-formed problems.** The §3.3 anti-pattern. TDD verifies a chosen design at fast feedback; it does not surface whether the problem is the right problem. Compose with `design-first-discipline` upstream when the spec is unclear.
- **Test-after-development.** Writing tests after the production code is complete. Loses the design-pressure benefit — tests confirm what was built rather than influencing what is built. If post-hoc tests are unavoidable (legacy rescue), compose with `feathers-characterization-tests-first` for the rescue protocol, then TDD inside the rescued island.

## Limitations

- **Selection rubric requires judgment.** "Pick the right pattern for this bar state" reduces dogma but cannot eliminate the call. The rubric narrows the choice space; it does not pick for you.
- **Refactor step depends on a refactoring vocabulary.** Without Fowler's catalog or equivalent named moves, the refactor step degenerates into ad-hoc tweaking. Compose with `fowler-refactor-mechanics-family`.
- **§3.3 position is opinionated.** This Skill takes `tests-as-design-feedback-companion` (TDD as one channel, design-first as upstream). A buyer who genuinely holds `tests-as-primary-design-tool` may find the §3.3 framing too cautious; defend the choice if you select the more aggressive position.
- **Legacy code needs a different entry path.** TDD presupposes the seam exists. Untested legacy code requires characterization tests first (compose with `feathers-characterization-tests-first`) before red-green-refactor becomes available.
- **TDD does not surface the right problem.** This is the §3.3 caveat made operational — TDD is verification of a chosen design at high feedback rate, not problem-discovery. Use the design-first composition for problem-discovery.

---

> Provenance + framework classification: see `composition.yaml` (sidecar).
> Compliance badges: see `badges-draft.yaml` (architect sign-off pending).
