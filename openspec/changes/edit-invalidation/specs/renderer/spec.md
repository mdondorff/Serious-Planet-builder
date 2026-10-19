## ADDED Requirements

### Requirement: REND-032 Stale while revalidating
A node of the drawn plan whose tile is resident with a mesh key other than the plan's (an edit changed what the tile reads and its refresh, STRM-026, is on the way; the same holds for a loop that keeps its tiles across any change of the field) SHALL be drawn from the resident buffer as it is, not replaced by an ancestor: the drawn plan names the resident key and the resident origin, so the renderer's key check (a plan that expects a key other than the buffer's fails with the tile id) is met, the node counts as substituted and the plan is of kind `Stale`, never `Exact`. This is an exception to the ancestor substitution of STRM-016, which applies to tiles that are not resident. An edited place SHALL therefore never go coarser or blank while it reloads, and when the refresh has replaced the tile the next frame SHALL be the exact plan of the new field again.

Verify: A
Status: active
Source: SPIKE-13 variant B and D (evicting the edited tiles drew 490 nodes as ancestors for the whole interval), M6 plan row E1

#### Scenario: The edited place stays as fine as it was
- **WHEN** an edit makes the plan expect new keys for resident tiles and their refreshes have not finished
- **THEN** every node of the drawn plan is the tile the plan named, with the resident key, no node is replaced by an ancestor or blank, and the plan is `Stale`

#### Scenario: Exact again
- **WHEN** the refreshes have replaced the tiles
- **THEN** the drawn plan equals the synchronous plan of the new field
