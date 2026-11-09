## Why
Inbox #106 step 2 (A3 section 10, G-STORE): the stored local change L of the K7 chain and kind 13 were measured only on the legacy chain (about 24 GB at Earth scale, target 16-24 GB). Item 11's open part and the runtime (item 15) wait for L on the new chain.

## What Changes
- `generators/examples/k7_region <margin> <threads> store`: runs K7 with the channels of H1 over every 1,024-cell tile that meets the prototype region and writes each tile's core of L (f32 metres) and its land mask to `target/k7_l/`.
- `cache::layer_tiles::{encode_change_core, decode_change_core}`: the counts stream of a core of change (0.25 m i16 counts, row deltas, brotli), the payload of a kind 13 record without its header.
- `cache/examples/l_measure`: encodes every dumped core, checks the round trip, reports bytes per land cell and the Earth-scale extrapolation.
- `k7::SurfaceGate`: the G-SURF-I report at the vertices (p95, the nodes by local step height, the error over the step height, the spatial concentration); `examples/k7_region` prints it. GEN-148's bullet on the failing gate now says the gate is at the centres.
- GEN-156 (tier D) and GEN-154 (tier A, tests in `generators/tests/bake_k7` and `cache/tests/layer_tiles`) added (generator-pipeline).

## Impact
- Code: `cache::layer_tiles` (two functions, no existing byte moves), two examples. No store format is decided.
- Not in this change: the field bake size, a reader of the records, edit overrides. G-STORE stays open.
