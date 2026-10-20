## ADDED Requirements

### Requirement: VIEW-036 The brush preview
A brush stroke SHALL be previewed (REND-033) as a chain of preview stamps along its line (`preview::brush_chain`). A raise SHALL be hill stamps and a lower the same with the sign turned, of the profile of the brush (`(1 - x^2)^2`), with a reach of 1.118 times the radius, both ends of the line included, a spacing of half the radius (wider when the stroke would need more than 14 stamps), and the amplitude that makes their sum the strength on the line, plus a dab of the radius at each end of the amount the chain falls short of the strength there (the stage's distance to the polyline is to the end point). A flatten SHALL be plateau stamps at the stroke's target with a reach of 0.8 of the radius and, as their bound, the strength shared over the plateaus that cover a point of the line, so that a row of them moves the ground by the strength and not by the strength times their number. A stroke of one point SHALL be one stamp of the whole strength. A stroke SHALL have at most 16 stamps. A smooth stroke SHALL NOT be previewed (a known limit: it moves the ground towards the mean of what is around it, which the stale tile cannot give; it shows when the exact tile arrives), nor a removed flatten (as a removed plateau is not). The chain SHALL stay within a third of the strength of the stage's delta, and within a tenth on average, along and across a bent stroke and along the longest stroke the tool records (64 points a quarter of the radius apart, 16 radii); a flatten SHALL stay within the strength.

While the pointer paints a stroke, the editor SHALL give the loop the chain of the stroke so far every frame as a transient preview, and when the stroke ends an empty list once; the put stroke is then previewed by the delta of the footprint items (`preview_delta_on`): a brush item added is the chains of its strokes (sign +1), one removed the same with the signs turned, one that gained strokes at the end only the chains of the new strokes, and one that lost strokes from the end only those strokes' chains with the signs turned. The stamps of a brush item SHALL keep the order of its strokes (a plateau reads the running height). The delta SHALL have at most 16 stamps (`MAX_DELTA_STAMPS`), kept in whole units, nearest the camera first, a unit being a plain stamp's entries or one stroke of a brush item (so that one large brush item is kept stroke by stroke, not dropped whole), and the transient preview at most 16 (`BRUSH_CHAIN_STAMPS`), so that both fit the 32 of REND-033 whole.

Verify: A, C
Status: active
Source: opus-planner (2026-10-20), VIEW-035, REND-033, GEN-030

#### Scenario: The stroke under the pointer
- **WHEN** a raise stroke is dragged and then released
- **THEN** each frame of the drag gives the loop the chain of the stroke so far as a transient preview, the release gives an empty list once, and the delta of the put stroke is a chain of hills of the same kind

#### Scenario: A stroke added to an item, and taken off
- **WHEN** a brush item gains a stroke at the end, and then loses it again
- **THEN** the first delta holds only the new stroke's stamps and the second only the same stamps with the signs turned

#### Scenario: A flatten and a lower in one item
- **WHEN** a brush item holds a flatten and then a lower
- **THEN** the delta lists the flatten's plateaus before the lower's hills
