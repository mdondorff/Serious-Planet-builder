## Decisions
- **Hypsometry first.** SPIKE-93's largest misses were the base elevation: the coast ramp reached 1000 m close to the shore, so 80 % of land was over 500 m. A top of 620 m over a land depth of 1.2 brings seeds 1-5 to a median land elevation of about 340 m.
- **Gate form over seeds.** Earth is one sample, so the bands apply to the median of five seeds with a per-seed tolerance (the planner's form, 2026-11-02).
- **Hills by their own field.** Belts alone gave hills under 10 % of land; a low-frequency field gated by the smooth depth (the rough `inland` changes several times over a lattice cell) adds hills away from the belts.
- **The coast gate.** It read the rough full continent value and switched at land depth 0 (a gate step of 0 to 0.6 over one cell: the largest `k` steps, 6.0). It now ramps over the smooth depth and the true depth; the belt stays exactly 0 in the sea. A soft switch below the coast gave a largest step of about 5.0 at the price of belts 0.01 m into the sea; not taken.
- **More plate draws are not the lever beyond 64.** With 256 draws seed 2's land above 2 km rose to 27.7 % (the retry scores land at a convergent margin, not the relief).

## Results
Seeds 1-5, medians, with seed 3 beside (preview grid, before the bake): see the PR body.

## Open
- Belt land: median 10.7 %, seed 4 at 6.0 % (convergent-only arcs, a later round).
- The step limit of `k`: largest step 5.1-5.8, a geometric consequence of belts narrower than 200 km each side; the owner decides the form.
