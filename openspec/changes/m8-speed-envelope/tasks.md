# Tasks: m8-speed-envelope

- [x] 1. TEST-046: `coarse_share` in `FrameLag` and `detail_lag`; test first.
- [x] 2. TEST-047: the search, the worst window, the flight cameras, the log text (tier A, fake trial); tests first.
- [x] 3. The scene on the streaming rig (`envelopeperf.rs`), wired as `--scene envelope`; the software-adapter smoke test.
- [x] 4. `cargo xtask perf envelope` (arguments, help text, test).
- [ ] 5. TODO(stand_in_share): when explorer's M8-2 step 3d exposes the chunk stand-in share in the per-frame log, add it as a second share beside the coarse share (the log field `stand_in_share` is `null` until then; the search then judges the larger of the two).
- [ ] 6. Verifier, CI on both OSes, merge. Do not archive until the architect's window has recorded the curve.
