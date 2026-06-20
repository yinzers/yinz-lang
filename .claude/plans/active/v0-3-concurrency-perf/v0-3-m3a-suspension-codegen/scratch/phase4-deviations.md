# v0-3-m3a-suspension-codegen Phase 4 Deviations — captured 2026-06-04

D_count: 1 (scope)

## Scope Deviations (verbatim from executor report)

- **Scope Deviation #1** (file: `examples/pirates-roster/expected_stdout.txt`): touched outside declared scope. Rationale: the integration test `examples_basics_runs_end_to_end` does byte-exact golden-file comparison against this file; without updating it to include the new P4 demo output the test fails — it is a direct dependency of the pirates-roster demo extension that IS in scope. Diff hunks: `examples/pirates-roster/expected_stdout.txt:1-4` (appended 4 lines for the `scout total: 438` demo output).

## Approach Deviations (verbatim from executor report)

None — implementation matched plan's named approaches.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1 (scope) — expected_stdout.txt golden update
- **type**: scope
- **rationale**: byte-exact golden-file dependency of the in-scope pirates-roster demo extension; updating it is mandatory for the test to pass.
- **diff hunks**: examples/pirates-roster/expected_stdout.txt:1-4
- (trivial mechanical golden update; plan-adherence covers it — no dedicated judge needed)
