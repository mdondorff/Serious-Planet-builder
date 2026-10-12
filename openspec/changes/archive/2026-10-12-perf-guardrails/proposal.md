## Why

M3 item 6b (owner, 2026-10-12): GPU performance gets guardrails. The one count budget (10,000 draws for every view) could not see a view get 50 % heavier, and the perf harness overwrote its log without comparing it with the last one, so the ground-1m GPU median rose from about 3.0 to 4.2 ms unnoticed until a person read two logs.

## What Changes

- Tier A (TEST-011): `every_standard_view_stays_within_its_own_pinned_counts` pins the node count of each of the ten scripted views at cells 32, tau 6 px, 1440p (52, 424, 645, 1071, 2561, 4425, 8687, 2612, 3573, 3546); triangles, resident tiles and upload bytes follow from it per tile (2,304 triangles, 1,221 x 44 bytes) and are checked with the existing `CountBudget`. A failure lists every view over its pin with the count and the budget.
- Perf harness (TEST-012): `planet_perf::compare` reads two session logs and reports every median (frame, GPU, CPU, plan; median over the three runs) that rose by more than 15 % and 0.1 ms, and every changed count. `test-render --perf --scene terrain` runs it against `perf/logs/terrain.last-valid.json` before it writes the log, and keeps a valid session as the new baseline. Report only: nothing fails. An invalid session is neither compared nor kept; another frame size is not compared.
- The ground-1m investigation is a learning entry (`docs/learnings.md`): no node, draw or triangle count changed between the two runs, so the cause is per-vertex or per-fragment cost; it cannot be isolated without a GPU A/B, which only the architect runs.
- Not run: `cargo xtask perf` (the architect's).
