# look-d1-channels (I8a slice 2b, step 4d)

## What
- `docs/looks/ground-channels-d1.look.toml` moves to look schema 10 and puts curves on the two channel-block inputs (GEN-126, LOOK-008): soil and grass fall with the erosion depth (0.5 to 2 m) and with the fill depth of a kept pit (0.25 to 1 m).
- Effect: stripped ground loses its soil and meadow (scree takes it); a kept pit loses them and goes to wet ground.
- Tier A test `look_d1` covers both effects and the neutral channels.
- `look_stills` writes synthetic fill and erosion depths (a 1.5 m pit in the crater floor, 2.5 m of erosion on a hill flank); sheet `docs/dev_screenshots/2026-11-08-look-d1-fill-erosion-synthetic-sheet.jpg`.

## Not in this change
- The default look stays neutral; no golden moves, no key moves. The look file is a proposal for the owner's look pass.
- The SPIKE-100 D1 clause "rock over 28 degrees where erosion exceeds 2 m" is an OR with the slope rule. The weight is a product over inputs, so it is not expressible; rock keeps its slope rule.

## Requirements
- No new requirement: LOOK-008 and GEN-126 as merged by channel-inputs.
