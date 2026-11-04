## Why
- Inbox #56 item 1b (planner, owner 2026-11-04): the integrated preview must go through the viewer, so the viewer needs a switch that draws the erosion layer (m9-bake PR 6, #1153) over a stored world.
- Planner's decisions (2026-11-04): the switch is a viewer option, not a definition key (paths are machine-local); the layer sits on the field-bake lattice read C1 at `k` 1, without the relief channels (measured: the legacy generator's lattice is 2 km rms from the surface the layer was carved on, the relief channels do not close it, a lattice of the bake's own base costs 3.4 m rms at 611 m cells).

## What Changes
- Viewer options (VIEW-062): `--erosion on|off` (off by default), `--erosion-store DIR` (the bake's output folder or its `tiles/`; the world's `--store` when not given), `--erosion-face-res N` and `--erosion-res N` (the bake's `--face-res` and `--planet-res`, 12,288 and 98,304 by default), `--erosion-unchecked`. The run refuses unless the store says it was baked from the class source (no store writes that marker yet, so `--erosion-unchecked` is the way until explorer's bake does), and names what is on in the log and the title.
- `planet-cache`: `layer_tiles::bake_specs` (one derivation of the layer's keys from a bake shape, for the command and the viewer), `StoreLayer::open` (finds the tiles folder, checks that a stored tile decodes under these keys).
- `planet-generators`: `with_layer` and `with_crease_band` work without the relief channels (`k` 1, `m` unread; own implementation ids `cpu-erosion-v1+band`, `+band+layer`, and the C1 pair); `ErosionSwitch`, `build_field_erosion`, `chain::field_chain_erosion`.
- `WorldTerrain::with_erosion`: the field chain builds with the switch, the terrain's `id()` and `describe()` carry it.

## Impact
- Requirements: VIEW-062 added; GEN-109 modified (no relief channels needed), and GEN-112 and GEN-114 (m9-bake's delta) follow.
- Code: `app` (options, `worldfield`, `window`), `cache` (`layer_tiles`), `generators` (`erosion`, `height_fields`, `chain`); dev tools `examples/base_gap.rs`, `examples/base_lattice_gap.rs`.
- With the switch off nothing changes: no golden, key or id moves. On a store that is not marked it refuses.
