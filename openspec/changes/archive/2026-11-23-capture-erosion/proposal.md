## Why
- Planner (2026-11-04): the first sheet over the preview store (`global16384-m9c14`) shows each region at 2 km and 300 m with the erosion layer off and on, headless, because the desktop may be locked and a windowed run loses the device. The product path for headless stills is `test-render --scene capture-stills`, which did not take `--erosion`; its world opening ignored `--class-source` and `--legacy-source`.

## What Changes
- `capture-stills` draws the erosion layer when `--erosion on` is given, as the viewer does (VIEW-062 edited in viewer-erosion's delta: the options are refused outside the viewer and the stills), and its header line (the world's description) names the layer and the field source.
- The stills' and the perf scenes' world opening takes `--class-source` / `--legacy-source` (a store of both bakes).
- Dev tool `examples/tile_latlon.rs`: the centre of tiles of the layer's grid as latitude and longitude, for the poses.

## Impact
- Code: `app` (`stills`, `streamperf`, `lib`). No key, id or golden moves with `--erosion` off.
