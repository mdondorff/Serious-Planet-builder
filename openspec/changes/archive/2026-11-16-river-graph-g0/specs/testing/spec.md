## ADDED Requirements
### Requirement: TEST-140 The river graph's validators and the fixture's expected graph
Tier-A tests SHALL check each validator of GEN-189 with an injected failure, the canonical form's round trip, the diff, the rasterisation, the budget validator, and the G0 fixture's expected graph (TEST-139). The fixture's graph is built by hand in the contract's types; it SHALL validate, round-trip, give the same bytes on any thread count, and run under both partition layouts.

Verify: A
Status: active
Source: Lead 2026-11-16 (crusader #155), the river-graph plan's G0 contracts (agreed, owner 2026-11-16) sections 8 and 11
Detail: injected, each found:
- a cycle, a duplicate id, two nodes on one anchor, and a marker off vertex 0 on a node's anchor;
- an unresolved endpoint, a chain without an outlet, a confluence with one inflow, a node with zero or two outgoing reaches;
- an edge from an outlet, into a source or across levels, and a broken or misplaced chain;
- a bed rise (allowed at an exception marker) and a bed not shared at a node;
- a discharge falling along a reach, and a confluence below its inflows or not sharing its value;
- a cell owned twice;
- a lowering and a fill over the budget, a clamp without its residual, and a residual whose feature is not flagged.
Also checked:
- the hand graph's canonical form parses back to the same bytes in any input order;
- an attribute change is one named field, and a kind change on an anchor retires the node and its reaches;
- the fixture's graph holds one H1 trunk reach, its L image `exception(rise_over_budget)` across the ridge and over two faces, the explicit confluence, tributaries cut at tile lines, two sea outlets and the protected basin at both levels;
- under each layout every tile-owned item lies in one slice, the landmass items lie in every slice with the same bytes, every slice validates open, and a tributary crosses the two-region cut at a `tile_crossing`.

#### Scenario: One injected failure, one violation
- **WHEN** a valid graph is changed to break one rule
- **THEN** the validator reports that rule's violation, and the unchanged graph reports none
