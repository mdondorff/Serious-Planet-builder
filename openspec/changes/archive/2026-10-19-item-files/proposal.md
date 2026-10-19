## Why
E4 step 2: the item file is the persistent truth of authored content (ADR 0066 point 6), and git is its history. This step builds the parsed item, its ULID text, the canonical TOML, the loader that refuses what it does not know, the content hash, the version check, and the item store interface with its two backends. The commands (E4 step 3) write through it.

## What Changes
- `planet_world_def::item`: `Item` and `Anchor`, `ulid_text`, `parse_ulid` (the canonical text only), `make_ulid` (48 bits of injected milliseconds, 80 bits of injected entropy), `Item::to_toml` and `from_toml` (fixed key order, shortest round-trip floats, `-0.0` as `0.0`, unknown keys and non-finite numbers refused), `Item::hash` (BLAKE3-128 under `planet-builder item hash v1` over the parsed form without the name and notes), `Item::spec` and `put_into` (the item in the definition with its hash, direction, layer, priority, reach and height bound), `Item::check_versions` with `ActiveVersions` and `VersionMismatch`.
- `planet_world_def::item_store`: `ItemStore` (`put`, `delete`, `get`, `load_all`), `MemoryStore` and `LooseFileStore` (`<root>/<body>/<ULID>.toml`, each file written whole beside its place and moved over it; names, ids and bodies checked on load).
- AUTH-001 and AUTH-002 become active with their new text; AUTH-005 (the item file) is new. AUTH-003 (pinning and freezing) and AUTH-004 (the commands, the next step) stay as they are.

## Limits
- The kind's parameters are free finite floats by name; each kind's own validation arrives with its stage (E2).
- The pinned and frozen exceptions of AUTH-002 arrive with AUTH-003.

## No golden moves
World-definition code and its tests only.
