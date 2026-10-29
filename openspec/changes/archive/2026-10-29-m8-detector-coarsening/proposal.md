## Why
The Tracked item "Tiles flash to a coarser level while moving" (owner, 2026-10-19/20) named a scripted approach asserting the drawn level never coarsens as the next step (planner, 2026-10-29: automated owner checks, part 1). `cover.rs` holds the rule over a hand-built lagging plan; the real managed loop was not covered.

## What Changes
1. Tier A test `coarsening_loop`: the rule through `ManagedLoop` with streamed tiles (2 a frame) and under a tight pool that evicts, each proving it exercises the case.
2. TEST-067 in the testing spec.

## Impact
- Result on main: both runs pass; the managed loop does not coarsen on this approach. The flashing the owner saw is not explained by the loop's level choice on a dive; still open: movement margin of the plan's frustum, water, stand-ins in the lit path.
- Code: `frame/tests/coarsening_loop.rs`, `frame/tests/it.rs`. Docs: Tracked item in `docs/status.md`.
