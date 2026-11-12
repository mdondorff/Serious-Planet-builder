## Why
No code takes a world definition to a commit that holds a channel graph: `bake_l` is called from tests only. The G-RT flight (item 15) needs a stored region whose commit has a graph, so that the viewer's river path (STRM-086, REND-062) reads real channels. The store is a G-RT frame-time fixture, not the world bake; its README says so, and it is rebuilt when the start-surface fix lands (architect, 2026-11-12).

## What Changes
- `planet_cache::region_bake`: one function from a region of one face (a rectangle of regional cells) to a commit in a folder: the regional solve from the base, H1 hydrology on the solved surface with the runoff of the climate, the lakes (`lakes_from_hydro`), the channel graph (`ChannelGraph::from_hydro`), the channels on the local grid (`channels_from_graph`, the in-memory feed), the basin spec and `GraphInput`, then `bake_l` over the land tiles of the region (GEN-177).
- The base, the climate (runoff, open-water evaporation, discharge weight) and the sizes are inputs (closures and a spec), so that the tier-A test runs on a 64-cell fixture with analytic inputs.
- `crates/cache/examples/bake_region.rs`: the command over the real inputs (`BakeInputs`: base read C1 from a lattice sampled at the nodes, climate from the orography), a region given as in `k7_region`, a README beside the store naming it a G-RT fixture.
- Tier A (TEST-125): the fixture region bakes a commit with a graph; the commit reads back (CommitLayer, RiverReader: channels in, tiles present); two runs give the same bytes; one and eight threads give the same bytes.

## Not in this change
- The run on the prototype region R3a (a heavy run in a window bulldozer gives, after GEN-176 and the pre-breach-off PR).
- Planet-scale resolvers (`h1_cell` across faces: the region stays inside one face; the cube-edge limit is architect's option C, 2026-11-12).
- Any store format change (CON-10): the writer's formats are used as they are.

## Impact
- Code: `cache::region_bake`, an example, tests. No key, record or golden moves. Requirements GEN-177 and TEST-125 added.
