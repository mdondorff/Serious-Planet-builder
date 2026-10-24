# Tasks: m8-speed-envelope

- [x] 1. TEST-046: `coarse_share` in `FrameLag` and `detail_lag`; test first.
- [x] 2. TEST-047: the search, the worst window, the flight cameras, the log text (tier A, fake trial); tests first.
- [x] 3. The scene on the streaming rig (`envelopeperf.rs`), wired as `--scene envelope`; the software-adapter smoke test.
- [x] 4. `cargo xtask perf envelope` (arguments, help text, test).
- [x] 5. The chunk stand-in share (STRM-039) wired into the scene and the log; the world of the scene has no chunk field yet (`chunk_view_of` returns none), so `stand_in_share` is `null` with a reason until the app builds a chunk manager for a world and `chunk_view_of` returns its `ManagerView`.
- [ ] 6. Verifier, CI on both OSes, merge. Do not archive until the architect's window has recorded the curve.
