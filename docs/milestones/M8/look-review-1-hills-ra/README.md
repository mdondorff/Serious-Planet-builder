# Look review 1, second site after R-a: the same poses with the corridor anchored to the drawn ground

Same site, poses and command as `docs/milestones/M8/look-review-1-hills/` (`poses.toml` there), re-rendered with the anchor law of `openspec/changes/m8-anet-chunk/design.md` section 17 (GEN-057). Not goldens.

| Item | Before (`look-review-1-hills`) | After (this directory) |
|---|---|---|
| Corridor anchors | the coupled run's height (1.2 to 2.7 km under the base) | base height minus 3 m (A / 1e8 m2)^0.5, at most 100 m, running maximum from downstream over 8 segments |
| Unclamped change, steepest site chunk (`chunk_incision`) | min -2659 m, 15,318 cells at the 300 m clamp | min -79 m, 0 cells at the clamp (8,786 cells cut over 20 m) |
| Unclamped change, second site chunk | min -1322 m, 16,863 cells at the clamp | min -130 m, 0 cells at the clamp (14,842 cut over 20 m) |
| Channel (`sections.txt`, 21 sections) | 292 to 302 m under the median | 32 to 90 m under the median |
| Cut width at 20 m under the median | 96 to 792 m | 280 to 528 m |
| Snap | 1186 m from the anchor cell, bearing 145 | 446 m from the anchor cell, bearing 195 (the chunk field changed) |
| Capped plan nodes (`stills.txt`) | 121 to 129 of about 1750 | 126 to 131 of about 1750 |

Reading (coder, not a review): the canyon is a gorge 30 to 90 m deep with a rim and a rough plain around it instead of a 300 m slot; the wall spikes of the near views are far fewer (`30m-midday.png`). The capped-node count did not fall: the tiles the plan caps are not only the slot's walls (the base field's own steep ground and the cut walls at 8 m both exceed what the morph interval allows, not separated here). Cut depth does not rise cleanly with drainage area along the bed (the quantiles of `chunk_incision`): the anchors follow the law, the bed between them follows the ground and the breach.
