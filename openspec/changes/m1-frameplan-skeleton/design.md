## Context

ADR 0009 (FramePlan boundary, tiers), ADR 0001 (frames), report §10, §16.

## Decisions

- **Plan decides, renderer executes.** Pixel rectangles, colours (face and tile-id views), the height range and the camera-relative origins are all in the plan; the shader receives a rectangle, a resolution, a view index, a colour and a range and does nothing else. `execute` returns count stats so budgets can be asserted on any adapter.
- **Integer texel selection.** The height view selects texels with integer arithmetic (`px * (res + 1) / width`), identical on GPU and CPU; only the grey ramp uses float maths, hence the tier-B tolerance of 1 grey level for the height view and exactness for flat views.
- **Plan text uses hexadecimal bit patterns** for every float, so snapshots are exact and diffable; bundles embed the plan and its hash, and a tampered or edited bundle is rejected with both hashes.
- **Replay** refuses a generator-version or implementation-id mismatch, rebuilds the plan from the recorded request, compares it number by number, then renders; a GPU frame on the same adapter must be pixel-identical (tested on the software adapter).
- **Editor mode without a window:** `editor --offscreen` runs the same code path as `test-render` (tested to produce identical PNGs). The real window and surface arrive in M2.
- **Height view contrast** is low for coarse tiles because the test generator is smooth at that scale; a per-tile range would be a plan decision for later.

## Risks

- Goldens are per adapter; four candidates now wait for the owner. Tier-C tests still assert adapter-independent metrics (no holes, exact face colour, determinism, grey-level count).
- Shader code is validated by the tier-B validation test and by executing it on WARP only; real-GPU and lavapipe behaviour is unverified until the owner runs `cargo xtask test gpu --real` / CI runs.
