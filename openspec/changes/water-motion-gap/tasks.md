## Tasks: water-motion-gap
Stacked on `change/remove-vertex-render-path` (#304); rebase onto main after it merges; the PR opens after that.

## 1. Capture
- [x] 1.1 `TerrainBundle`: optional frame section (`drawn <kind> substituted <n> blank <n>`, version 3 only when present); tier-A round trip with a stand-in node, lit and water parts, corruption refused by the plan hash, version 2 text unchanged.
- [x] 1.2 `Command::CaptureFrame` (F9) in the viewer table and help; tier-A key test.
- [x] 1.3 Window: F9 writes `target/review/frame-<n>.txt` for the next drawn frame; the title names the path (or the error) for a few seconds.
- [x] 1.4 Replay of a frame bundle (`cargo xtask repro`, `test-render --repro`): world rebuilt, tiles of the drawn nodes regenerated, lit frame with water drawn to PNG, drawn nodes reported beside the request's own plan. Tier-B round trip from a world frame; tier-C replay test.

## 2. Cause and fix
- [ ] 2.1 Reproduce the owner's frame from its bundle (needs the owner's F9 bundle or the command line of the run).
- [ ] 2.2 Failing test at the cheapest tier (water coverage over moving, rotating frames, or the per-vertex sign at morphing vertices).
- [ ] 2.3 Root-cause fix; spec delta on ATMO-004 or ATMO-020; golden-review if pixels move.

## 3. Close
- [ ] 3.1 `numerics-reviewer` (water shader, if touched), `spec-guardian`, `verifier`; CI green on Windows and Linux; archive.
