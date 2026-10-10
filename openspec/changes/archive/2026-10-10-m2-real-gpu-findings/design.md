## Context
Owner run on 2026-10-10 (gpu --real output and perf log).

## Findings recorded
- 15 of 16 tier-C tests passed on the real GPU at first, including the geometry, morph and depth oracles; after the shader fix all 16 pass.
- Real-GPU candidates equal the blessed WARP goldens for the debug views and the hello triangle; three terrain views (normals, depth, morph) differ from the WARP candidates by at most 1 in one channel (43, 352 and 60 of 12,288 pixels): float rounding, which is why goldens are per adapter.
- The perf skeleton times a hello-triangle round trip (0.43 ms median), not a frame; it only proves the harness runs on the right adapter. Run-to-run noise 0.105 (relative spread of medians).
