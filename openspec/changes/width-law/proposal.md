## Why
Item 15 PR 6 (architect D6 point 2, D19/D20): one river width law for the burn-in and the drawing, `w = a Q_w^b`. K7's burn-in cut a one-cell line whatever the discharge; the drawing (PR 8) needs the same width so that the ground and the water agree by construction.

## What Changes
- `k7::WidthLaw { a, b }` (standard `a = 7.2`, `b = 0.5`, metres for `Q_w` in m3/s), `Channel::q` (the routed discharge at each point, empty = one cell), `burn_channels_wide(z, w, origin, channels, Some((law, cell_m)))`: where `w / cell` exceeds one cell the burn-in lowers every cell whose centre lies within `w / 2` of the point of the line to the line's level there (never raised): a trench of `max(1 cell, w)`. Without a law, without discharges or narrower than a cell it is the one-cell line, bit for bit (`burn_channels` is unchanged and the K7 stage manifest does not move).
- `K7Params::width` (standard law): a bake parameter. K7's stored key must carry it: `LTileSpec` (PR 2b, `layer-v3`) takes the law's two numbers into its key bytes when that PR and this one meet.
- `k7_region` passes `Q_w` per channel point; `region_scale` prints the width distribution of the channel cells (p50, p95, max in metres and local cells, the share wider than one local cell) for R3a and R3b.
- GEN-165 added (generator-pipeline); TEST-108 continues.

## Source of the coefficient
`a = 7.2`, `b = 0.5` is the pair the architect named from Moody and Troutman (2002), Earth Surface Processes and Landforms 27(12), 1251-1266. The paper is paywalled and could not be fetched (Wiley and USGS answered 403): the pair is confirmed only through a later paper that quotes it as the bankfull width law `w = 7.2 Q^(0.5 +- 0.02)` for a discharge `Q`, without units or the discharge type. Not verified against the paper itself: the units (metres and m3/s assumed), whether `Q` is bankfull or mean annual, and the equation or page. No paper was stored.

## Impact
- Code: `generators::bake::k7`, `examples/k7_region`, `examples/region_scale`, tests `bake_k7` (2 new). No stored bit changes today: no channel carries discharges in any stored layer yet.
