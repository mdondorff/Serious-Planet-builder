## Why
E4 (`edit-commands`) writes item files named by a ULID, a 128-bit id (ADR 0066 point 6). The index kept `u64` ids and `u64` content hashes (E1 step 1 wrote 16 bytes in the keys, zero-extended), so the switch to 128 bits gates every item file. It is its own small change because the shared item types are on the path of E1 step 2 and later (opus-planner, 2026-10-19: the id switch lands first; every pinned known answer stays unchanged below 2^64).

## What Changes
- `planet_world_def::ItemId` and `ItemHash` (both `u128`) in `item_id.rs`; `ItemSpec`, `put_item`, `remove_item`, `items_in_cell`, `item_ids`, the cell index, the footprint and cell entries and the read records use them. Source of call sites is unchanged (integer literals infer `u128`).
- The 64-bit-word hashes (definition, cell, footprint) and the cache's query-result hash write the low words in place and the high ones, when there are any, in a tail whose layout cannot be taken for rows; the footprint item hash writes a wide content hash as 16 bytes. Narrow values hash exactly as before.
- WDEF-009.

## Not in this change
The ULID text form, the item file and the store (E4 step 2), the commands (E4 step 3).

## No golden moves
Index and hash code only; every pinned known answer of the world definition, summary and cache tests passes unchanged.
