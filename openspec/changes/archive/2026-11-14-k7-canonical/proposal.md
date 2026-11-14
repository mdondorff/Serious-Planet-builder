## Why
Inbox #114 (architect, 2026-11-09): K7, the local pass of the new chain, floods with the plain `priority_flood` in its pit statistics, its incising carve (through the carve's default rules) and its deposition sweep (`deposit_on_own_route`). GEN-159 showed that flood's FIFO lifts cells beside a lower outlet; A3 section 6.4 replaces the FIFO order for the bake, and G-SURF is read on K7's stored change.

## What Changes
- `K7Params::canonical_flood` (default on in `standard`): K7's pit statistics (`pit_stats_with`), its carve rules (canonical flood, no MFD clip) and its deposition (`deposit_on_own_route_flood`) use the canonical flood; off, K7 is bit for bit what it was.
- `deposit::deposit_on_own_route_flood(.., canonical)`; `deposit_on_own_route` is the plain case and is unchanged.
- GEN-160 added (generator-pipeline).
- `examples/k7_region`: reports how much the stored change L moves against the plain flood (cells changed, maximum, share over 0.25 m).

## Impact
- Code: `generators::bake::{k7, deposit}`, `tests/bake_k7`, `examples/k7_region`. Pass 2 (`tile.rs`) calls `deposit_on_own_route` and `carve` with their defaults: bit for bit unchanged (tests below).
- K7 has no kernel version constant and nothing is stored from it yet; the version the store keys K7 by is set when K7 gets a store (item 15).
- Not in this change: `fill_depth`, `route` without rules, pass 1 and 2.
