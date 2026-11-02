## Tasks
- [x] Tier A: the plan, the stop rule, a fake window, retries and failures (BUILD-034; the duty idle, BUILD-035)
- [x] `planet_perf::window`, `xtask/src/perfwindow.rs`, dispatch and usage
- [x] Smoke on the software adapter (`--no-preflight`, smoke flights): 8 attempts in 2 rounds, rotated, retried, 3 s duty idle, the table; all INVALID by design (smoke)
- [ ] After #1095 merges: rebase onto main (local commits only)
- [ ] verifier, spec-guardian, prepush; archive with a log fragment; PR; watch
- [ ] Ask the planner for the quiet slot: PR 1's 20 preflights old against new and the slow terminal; PR 2's 10 duty-idle runs against 10 with the old cool-down; PR 3's minutes per valid result
