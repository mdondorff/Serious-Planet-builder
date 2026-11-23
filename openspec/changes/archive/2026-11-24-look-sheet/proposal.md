## Why
- Owner, 2026-11-23: every switch run comes with its look sheet, combined with the stage manifest, the cockpit only where the link is obvious (lead #183).
- The look questions of G3 (the chainsaw, the straight rivers, the corner links; SPIKE-142, SPIKE-143) are judged on crops at one L cell a pixel under a low sun, and SPIKE-143's sheets were Python scripts over full dumps that a row deletes.

## What changes
- `planet_cache::look_sheet` (BUILD-067): a crop store (a chain run's pre-registered crops, each a small chain directory of BUILD-060's layout, KB a stage) and its sheets: SPIKE-143's layout, BUILD-060's fixed-sun shade at each azimuth given, the defaults row beside the switch row and their difference, `look.txt`, and BUILD-048's per-sheet cap.
- `cargo xtask look-sheet RUN [--against DEFAULTS] [--az A,B] [--out DIR]` (the `look_sheet` example of planet-cache); output `target/review/look/<run>/`.
- `stage_sheet::shade_at`: BUILD-060's shade at another azimuth (`shade` is `shade_at` at 315 degrees, bit for bit).
- The cockpit (BUILD-064) indexes `target/review/look/`.
- The chain hook `--probe-crops` is the spike branch's (rg_chain); it writes the store through `CropWriter`.

## Impact
cache (look_sheet, stage_sheet), xtask (look-sheet, cockpit's scan roots). No product path, no golden, no key changes. BUILD-046's binned probe of the generators' fixture stays as it is: it has no RG stages.
