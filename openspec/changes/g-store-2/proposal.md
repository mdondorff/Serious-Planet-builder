## Why
Item 14 of the terrain prototype briefing: G-STORE (A3 section 10) lists the field bake on the new chain, the temporary and replacement space and the edit overrides as unmeasured (ledger row G-STORE, partial). `change g-store` measured the new chain's own items (STRM-078).

## What Changes
- `cache/examples/store_space`: the field bake on the new chain (four channels of the production sample on the tiles of the landmass, the production block and a narrow-channel variant, by land share, Earth GB), the space of a replacement version and of `pack::compact` on the real packs of `store_measure` (median edit, largest sampled edit, full re-bake; the directory's peak sampled during the compaction), and the canonical TOML of authored brush items.
- STRM-079 added (streaming-cache).

## Impact
- Code: `crates/cache/examples/store_space.rs` only. No bit of the default bake, no store format, no pack code changes.
- Not in this change: a store format, a pack-wise compaction (reported as a finding if the peak calls for it), the kind 13 field on the new chain (L is `l-store`), a measurement at 12,288 cells.
