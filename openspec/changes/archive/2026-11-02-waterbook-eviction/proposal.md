## Why
Tracked (ADR 0056 limits): `WaterBook` is never evicted: an entry per tile loaded, for the life of the run.

## What Changes
- `WaterBook` is bounded: `WATER_BOOK_CAP` tiles (262,144). Past it the least recently used eighth (by a use counter, ties by tile id) goes; an evicted tile is computed again, to the same bits (the answer is a pure function of the tile and the field), and its minimum height is left by its job again. Entries are touched by `get`, `min_height`, `set_min_height` and `fill`.
- The number: an entry is about 100 bytes (answer, minimum, use stamp, the map's overhead), so the cap is about 26 MB. The largest resident set R measured is about 21,000 tiles (1,000 km clipmap reach), so the cap is more than ten times it and no tile of R is evicted in a normal flight; a long flight that covers the planet turns the book over instead of growing it. The RAM shares (BUDG) are not touched: 26 MB is inside the cache share's slack.
- `WaterTileCache`'s per-tile memo is bounded by the same number (it starts over past it; it is a memo of pure answers).
- ATMO-012 states the bound; scenario "The book is bounded"; tier A `water_book` (the capacity is held over a flight of 600 tiles, the tile in use stays, an evicted wet tile and its minimum are computed again to the same bits).
- The Tracked line loses the `WaterBook` clause.

## Capabilities
### Modified Capabilities
- `atmosphere-ocean`: ATMO-012
