# Change: m8-global-coupled

## Why
- M8-1 (docs/briefings/m8-plan.md; ADR 0077 point 1; owner go 2026-10-23): the river network is fixed by one global coupled run (implicit stream power, drainage area, receivers) at the bake lattice; its fields are stored beside the four existing channels.
- A fifth channel must not repeat the record, key, stored-read and "already stored" code the four channels carry (architecture review R5, `spikes/architecture-review/results/cohesion.md` section 6).

## What changes (in small PRs)
1. R5: `TileChannel` in `cache/global_store.rs` (kind, key, codec, label per channel); `channel_record`, `has_channel` and `stored_channel` are generic over it; the block, ocean and wet-floor channels use them. No behaviour change: keys, payloads and error texts are the same.
2. The coupled run in `generators` (a new module): implicit stream power, drainage area and receivers with a jittered D8, deterministic (CON-14).
3. The new channels stored through `TileChannel` (STRM-043) and a debug view of the maps (GEN-041).
4. The wiring (part c, second PR): the world loads the stored run or makes and stores it (STRM-044), the field chain shows it through the climate views (GEN-043), K cycles the lens (VIEW-053).

## Tests
- Tier A (`cache`): the existing global-store suite is the pin for R5 (keys, payloads and error texts are unchanged); the block, ocean and wet-floor wrappers run the generic functions end to end (build, resume, verify).

- Tier A (`generators`, `app`, `frame`): the lens texture and field (GEN-043), the load-or-run and the slot (STRM-044), the K key and its help (VIEW-053).

Expected golden changes: none (the lens is off at the start; no render code changes).
