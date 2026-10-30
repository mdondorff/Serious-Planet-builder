## 1. Change
- [ ] 1.1 Planner confirms the design, the order and the requirement IDs
- [ ] 1.2 Spec deltas (BUDG-003, BUDG-004, the pool API)

## 2. PR 1: tile pool
- [ ] 2.1 Tier A: the spawn scan with an allow-list of the sites not yet moved (BUDG-004)
- [ ] 2.2 Tier A: the share ceiling and the class order on fake work (no thread outside, ceiling holds, a frame-critical job passes a queued idle one)
- [ ] 2.3 `JobSystem` and `Share` in `planet-streaming`; `GenWorkers::pool` returns the terrain share's spawner
- [ ] 2.4 Verifier; counts companion (jobs by class, tier B); tell the planner when a perf measurement is due

## 3. PR 2 to 4
- [ ] 3.1 Chunk pool (if B6 leaves it); plan workers; horizon bakes and the scoped workers; the allow-list empties
