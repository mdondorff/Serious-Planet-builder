## MODIFIED Requirements

### Requirement: TEST-011 Count-based budgets
Draw calls, triangles, resident tiles and upload bytes SHALL be derived from the plan and asserted against budgets in tier A tests on any machine, and tier C tests SHALL check that the renderer drew exactly the plan's draw calls and triangles and uploaded exactly the bytes of the tiles in the format they were built in (the plan's upload bytes are the vertex-format reference count; a compact tile uploads its texels and record, levels 0-2 and fallback tiles their vertices, and the product never uploads more than the reference) (allocated VRAM joins them when the renderer allocates budgeted pools); the counts of each standard view (the scripted cameras at the production parameters) SHALL also be pinned per view, over the default world (the eroded field over a named bake) and over the reference field the goldens keep, so that a change that raises the nodes, triangles, resident tiles or upload bytes of any view fails with the view, the count and the pin, and a pin is raised only by a change that says why; time-based budgets SHALL be measured only on the reference machine.

Verify: A
Status: active
Source: CON-20, M4 item 7d

#### Scenario: Budget exceeded
- **WHEN** a frame plan exceeds its draw-call budget
- **THEN** the test fails and prints the count and the budget

#### Scenario: Pinned view
- **WHEN** the plan of a standard view has more nodes than its pin
- **THEN** the test fails naming the view, the count and the pin

#### Scenario: Renderer matches the plan
- **WHEN** a terrain plan is rendered
- **THEN** the draw calls, triangles and uploaded vertex bytes the renderer reports equal the counts derived from the plan

#### Scenario: Default world pins
- **WHEN** the ten standard views are planned over the default world (eroded field, 256-cell bake)
- **THEN** each view's nodes, triangles, resident tiles and upload bytes stay within its pin, and the test names the bake the pins were made over
