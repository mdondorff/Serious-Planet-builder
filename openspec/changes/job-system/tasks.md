## 1. Change
- [x] 1.1 Planner confirmed the design and the order; IDs BUDG-008, 009, 010 reserved (2026-10-31)
- [x] 1.2 Spec deltas (BUDG-004 modified; BUDG-008, 009, 010 added)

## 2. PR 1: tile pool
- [x] 2.1 Tier A: the spawn scan with an allow-list of the sites not yet moved (BUDG-004, BUDG-010; xtask/src/threadscan.rs)
- [x] 2.2 Tier A: the share ceiling and the class order on fake work (BUDG-008, BUDG-009; planet-streaming jobs.rs)
- [ ] 2.3 Move the tile pool: `GenWorkers::pool` returns the terrain share (after #951 merges; rebase onto it)
- [ ] 2.4 Verifier; counts companion (jobs by class, tier B); tell the planner when a perf measurement is due

## 3. PR 2 to 4
- [ ] 3.1 Chunk pool (if B6 leaves it); plan workers; horizon bakes and the scoped workers; the allow-list empties
