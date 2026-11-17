## Why
- The owner wants one sheet per window showing every stage of the river-graph chain, kept as a tool (lead #167a, 2026-11-17).
- The sheet reads a documented layout only, so any run that writes the layout can be rendered; no dependency on spike code.

## What changes
- BUILD-060: `cargo xtask stages --chain DIR [--step S] [--view X,Y,SX,SY] [--out FILE]`; the chain layout (`chain.txt`, f32 surfaces, H1 fields, the canonical graph, the stage fields); twelve panels at a 10-degree sun on one fixed scale.
- `planet_generators::preview::put_text` and `TEXT_ROW_H` public for the sheet's labels.

## Impact
cache (`stage_sheet` module, `stage_sheet` example), generators (`preview` text public), xtask (`stages.rs`, help); tests.
