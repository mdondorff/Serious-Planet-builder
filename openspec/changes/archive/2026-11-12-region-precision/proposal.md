## Why
`region_bake` (GEN-177) took its epsilons from literals (1e-3 in the route and the flood) and left the carve's f32 flag at its default, which a world of mountains of 40 km and more breaks: the rounds' narrowing guard refuses an epsilon under the f32 spacing at the summit (GEN-175; architect, 2026-11-12).

## What Changes
- `RegionSpec.height_bounds`: the world's base heights plus the profile's largest uplift; `RegionSpec::precision()` is `WorldPrecision::new(bounds, profile epsilon, EPSILON_MAX_M)`. The carve takes `precision.apply` (epsilon and f32 flag), H1's route and flood take `epsilon_w`; the base and the solved surface are checked against `R` (a height beyond ±R is refused naming the cell: an Err, the rounds' own narrowing assert included). `solve_region` is the regional solve as a function of its own. The bounds enter the keys' identity.
- A world whose `epsilon_w` is not the profile's is refused before any work at the commit: the commit spec records no flood epsilon, so recording one is a format question for the architect (CON-10), not decided here.
- Tier A: Earth's span keeps the profile's epsilon and f32 rounds; a 60 km span (bounds -20 to 60 km, a ridge of about 45 km) solves under an epsilon of 3.9 mm and passes the narrowing guard; a surface above the bounds is refused with the cell; a commit of the 60 km world is refused up front.

## Impact
- Code: `cache::region_bake`, the example, tests. Earth's heights are the ones they were (epsilon and f32 unchanged); the keys change because the bounds are keyed (old stores need not stay compatible). No format change. Open for the architect: the bounds belong to the world's definition (GEN-175), not to a region's spec.
