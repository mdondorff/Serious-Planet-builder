## Why
Option B item B4-2 (design PR #910, planner order 2026-10-30): the owner's pass condition for level boundaries is "no visible pop in the descent" (owner test 2026-10-27, Decisions). The gate reads heights at capture sites; nothing judges a moving descent.

## What Changes
1. `planet_frame::popcheck`: a pure detector over a trace of per-frame drawn heights of fixed probes. It judges the screen-space jump of every probe whose level changed and says PASS, FAIL or NOT EXERCISED.
2. A tier-A test that runs a scripted descent (3000 km to 2 m, 2 % a frame) through the morph model of `LodParams` over a synthetic height pyramid, with a planted failure (no morph) that the detector must catch.
3. The GPU and real-store run of the same detector is a later step (it waits for the owner's test to be done).

## Impact
- Spec: `testing` gains TEST-071.
- Code: `crates/frame/src/popcheck.rs`, `crates/frame/tests/level_pop.rs`. No change to what is drawn.
