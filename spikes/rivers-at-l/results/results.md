# SPIKE-114: rivers routed at L inside a region (R3a)

Opened: 2026-11-13
Question: inside a region, do river lines routed at L (102 m) on the drained start surface meet the 1 % bar for line cells cut over 50 m without breaking agreement with H1 (814 m) at the region's edge?
Why it matters: #125 showed that most deep cuts come from H1 lines crossing L ridges (13 % of line cells cut over 50 m with the lines placed, 6.75 % straight; draining alone 10 %). The ADR on rivers at L inside regions (owner, 2026-11-13: "rivers at 100 m inside regions, with crusader's test first") waits on these numbers.
Time box: 1 day of agent work.
Success criteria: per variant (drain D = infinity, all breach; D = 20 m, lakes kept): line cells cut over 50 m at most 1 % inside the commit's tiles; no water-surface rise over 0.25 m; at the region's edge every H1 inflow and outflow matched by an L river within a stated distance, the discharge ratio per outlet named, and water sent to another valley than H1 named per outlet; wall time and peak commit within 12 GB; the size of the L geometry if stored and the time to rebuild it from `bake_base()`.
Fallback if it fails: keep H1's network and route at L only within a corridor (placement) with the lakes rule, or route at L with H1's outlets as fixed exits.
Code location: spike branch `spike/rivers-at-l` (crusader), examples in `crates/cache/examples/`; never merged.
