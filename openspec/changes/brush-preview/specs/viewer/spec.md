## ADDED Requirements

### Requirement: VIEW-036 The brush preview
A brush stroke SHALL be previewed (REND-033) as a chain of preview stamps along its line (`preview::brush_chain`): a raise as hill stamps and a lower as the same with the sign turned, of the profile of the brush (`(1 - x^2)^2`), with a reach of 1.118 times the radius, a spacing of half the radius (wider when the stroke would need more than 14 stamps), both ends included, and the amplitude that makes their sum the strength on the line, plus a dab of half the strength and the radius at each end; a flatten as plateau stamps of the radius at the stroke's target with the strength as their bound; a stroke of one point as one stamp of the whole strength; at most 16 stamps per stroke. A smooth stroke SHALL NOT be previewed (a known limit: it moves the ground towards the mean of what is around it, which the stale tile cannot give; it shows when the exact tile arrives), nor SHALL a removed flatten (as a removed plateau is not). While the pointer paints a stroke, the editor SHALL give the loop the chain of the stroke so far every frame as a transient preview, and when the stroke ends an empty list once; the put stroke is then previewed by the delta of the footprint items (`preview_delta_on`): a brush item added is the chains of its strokes (sign +1), one removed the same with the signs turned, one that gained strokes at the end only the chains of the new strokes, and one that lost strokes from the end only those strokes' chains with the signs turned. The chain SHALL stay within a third of the strength of the stage's delta, and within a tenth on average, along and across a bent stroke.

Verify: A, C
Status: active
Source: opus-planner (2026-10-20), VIEW-035, REND-033, GEN-030

#### Scenario: The stroke under the pointer
- **WHEN** a raise stroke is dragged and then released
- **THEN** each frame of the drag gives the loop the chain of the stroke so far as a transient preview, the release gives an empty list once, and the delta of the put stroke is a chain of hills of the same kind

#### Scenario: A stroke added to an item, and taken off
- **WHEN** a brush item gains a stroke at the end, and then loses it again
- **THEN** the first delta holds only the new stroke's stamps and the second only the same stamps with the signs turned
