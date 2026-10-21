# M7 planning

Status: scope answered by the owner (2026-10-21); exit criteria and the open questions below still to settle. No M7 code before the M6 GO, except the robustness item ADR 0074.

## Owner's answers (2026-10-21)
| # | Question | Answer |
|---|---|---|
| Q1 | What is M7? | Not Cities: Cities move to a later milestone. M7 adds **ground detail at the lowest levels** (today the near ground is "a smear of colours"; a first-person walk must read as ground): detail textures and their maps (albedo, normal, roughness and specular, and the like), possibly instancing. Together with the authoring items below. |
| Q2 | Generator editor depth | Sliders over the generator's parameters; TOML for anything more. A node graph is later. |
| Q3 | Re-bake time | It may vary. An idea: a "global edit" mode that applies global edits only down to some level of detail, then "back to normal" on demand with generation time. It must be visible which tiles are updated and which are still stale. Only if the architecture supports it in general, not for this one use case. |
| Q4 | Version pinning | Explicit upgrade, with a diff. |
| Q5 | Freeze | Important; it must blend visually (the current hard clip is too harsh). |
| Q6 | Geo-anchored objects | Only if something in M7 depends on them. |
| Q7 | Coder setup | Keep the current model. |
| Q8 | Region keep-inside | Not as a hard clip. Regions fit better as soft influence on procedural generation (for example a "park" region inside a generated city that the generator adapts to closely but not normatively). Goes with Cities. |

## Proposed M7 scope: "Ground detail and world authoring 2"
| Step | Content | Notes |
|---|---|---|
| R | ADR 0074: a tile too steep to morph is not refined; the plan degrades locally | robustness; the view no longer freezes near steep ground |
| G1 | Ground detail spike: detail textures and maps at the lowest levels, driven by the surface (slope, altitude, climate, the region's surface type), tiling without visible repetition, cost at 1440p | decides the approach and the asset question (Q9) |
| G2 | Ground detail build | the region surface types become visible materials |
| A1 | Generator editor: sliders over the definition's parameters, a coarse preview, the full bake in the background | |
| A2 | A stale-tile view: which tiles are exact for the current definition and which are stale | general (streaming, edits, re-bake); it carries Q3's "global edit" mode if that stays cheap |
| A3 | Version pinning with a diff (AUTH-003) | |
| A4 | Freeze with a blended edge, and a feathered region mask | the feathered edge is "later" in ADR 0069 |
| A5 | Edit health: a problems list of every flagged edit (too steep to draw fully, tiles capped or refused near it, very many stacked strokes), with fly-to and actions to simplify (merge a stack of strokes into one, reduce its strength, delete) | owner, 2026-10-21: the owner may attempt what the engine does not support; it must not crash, must not cost much performance, and must be findable and fixable |
| (B) | Editor guard on stacked strokes | done in M6 (#499) |

## Owner's answers, second round (2026-10-21)
| # | Question | Answer |
|---|---|---|
| Q9 | Where do ground textures come from? | Both: CC0 sets and procedural variation. The owner also wants to explore external data as a base (for example an Earth elevation dataset), towards global authoring; the main focus stays on great procedural generation and editing its product. |
| Q10 | Instancing | Later, but soon: a performance risk expected, so it is to be de-risked early. |
| Q11 | Ground detail target | Eye height about 1.8 m, and crouching: the near clipping plane must work at a few centimetres. |

## Added steps
| Step | Content | Notes |
|---|---|---|
| G0 | Near plane at a few centimetres: a crouched camera (eye about 0.5 m, near plane about 5 cm) over the finest tiles, with no z-fighting, no clipping into the ground, and the depth precision measured | part of the G exit criterion; reversed-Z is in place since M2 |
| I1 | Instancing spike: the cost of scattered instances (rocks, grass clumps) over the GPU-driven instance path of M2, at walking height, at 1440p | de-risks the performance before instancing is built (M8) |
| E1 | External-data spike: an Earth elevation dataset (licence, resolution, size) as the base field under the procedural detail | exploration only, not an M7 commitment |

## Proposed exit criteria (to confirm after G1)
- At 1.8 m and at 0.5 m eye height, rock, sand and grass are distinguishable at 1 to 10 m, within a ms budget set after G1.
- The near plane of a few centimetres shows no z-fighting or ground clipping over the finest tiles.
- Version pinning with a diff, freeze with a blended edge, the generator sliders with a coarse preview, the stale-tile view and the problems list all work as the owner reviews them.

## Open questions
| # | Question | Options | Recommendation |
|---|---|---|---|
| Q12 | M6 gate | the owner's retest of the P1 fixes, the look review, the GO | Retest when convenient. M6 stays open meanwhile; M7 design and the ADR 0074 build run alongside. |
