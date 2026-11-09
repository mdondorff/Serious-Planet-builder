## ADDED Requirements
### Requirement: GEN-153 The memo caches of the edit path have a budget and eviction never changes a result
The memo caches of the edit path (`PartCache`, `RoundCache`) SHALL take an optional budget in bytes, a parameter with no machine count in the rule. Over it, after each round and at the end of a solve, a cache SHALL evict previous results (at the end only), tile floods and catchment rounds, each least recently used first (ties by key), and never data the running solve used; a count of what changed SHALL say when its previous result is gone. A solve after any eviction, an edit and its undo, and a replay SHALL equal the fresh build bit for bit (G-EQ). No budget SHALL evict nothing and `without_memory` SHALL keep nothing.

Verify: A
Status: active
Source: Owner 2026-11-09 (A3 6.6: cache eviction required), architect order inbox #99, GEN-143, GEN-146

#### Scenario: Eviction
- **WHEN** a four-round solve, an edit, its undo and replays run under budgets of a third of the unlimited size, of nothing and of one byte under it
- **THEN** every result equals the fresh build, the cache holds at most the budget after each solve, something was evicted, and an unlimited cache evicts nothing
