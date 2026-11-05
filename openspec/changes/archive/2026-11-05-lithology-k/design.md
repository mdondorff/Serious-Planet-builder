## Decisions
- Analytic, no store: a pure function of the seed, the direction, the height and the version (SPIKE-102 3.2, F1).
- The province terms (craton, orogen core, foreland, basin, rift, old margin) come from the class source's plate mask, sampled on a lattice of the class lattice's geometry (256 cells a face edge, `u8` a term, built in memory in seconds, never stored): `mask_terms` is far too dear for the ~9e8 cells of pass 1, and provinces are hundreds of kilometres across. The noise supplies the local variation.
- H is SPIKE-81's formula as start values; the strata are an unbounded stack of bands (nominal 100 m, edges jittered by 35 %: 30 to 170 m), each with a hardness offset (up to 0.45) from H and a smooth regional variation; the stack is tilted and folded by an offset of the height (150 m on a craton to 1,200 m in an orogen), so a hard band outcrops as a ridge or cuesta; K is blended over 8 % of a band at each edge so it is continuous in the height.
- A `Column` (the province read and the noises) is computed once per direction and read at any number of heights: a carve can call it once per cell and `at(z)` per step (the carve's use is PR 4: GEN-100, GEN-101).
- The preview is a function in the generators crate (`lithology_preview`) and a `generate` option; the images are written with the existing `dump::Rgba`.

## Open
- Parameters and the soluble share: the preview round. K in the carve (GEN-100, GEN-101): explorer's PR 4. Relief changes everywhere then; the class amplitudes and the scorecard are re-tuned after.
