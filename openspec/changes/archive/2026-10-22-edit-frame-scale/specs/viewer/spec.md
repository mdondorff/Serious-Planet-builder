## ADDED Requirements

### Requirement: VIEW-051 The shown edits are bounded
The editor SHALL show at most `MAX_SHOWN` = 400 edits at once, (SPIKE-29). Of the edits VIEW-015 shows it keeps those whose reach disc can be seen (`relevance::in_view`: the cone of the disc, widened by `HEIGHT_MARGIN_M` = 4 km for the ground's height, meets the cone of the camera's view) and, when more than 400 remain, the 400 with the largest reach on the screen (ties by the lower id), and the selected edit besides. The problems list (VIEW-047) is not limited. The footprint find is kept while the definition revision and the tiles stay, and the ground heights of the outlines while the field's item revision stays.

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29 fix 3: separate the shown set from the item count, then bound it), VIEW-015

#### Scenario: The cap
- **WHEN** 800 hills lie under a camera 400 m up looking down, one of those left out is selected, and a world of 30 hills is asked the same
- **THEN** 400 are shown, none left out is larger on the screen than one shown, the same camera shows the same edits, the selected one is added, and the 30 are those the plain find gives

#### Scenario: The view
- **WHEN** a camera 12 km from the hills looks at them, and one looks away
- **THEN** the first shows some, the second only the selected edit, and a disc beside the view is not in view

#### Scenario: Reuse
- **WHEN** the camera moves with the world unchanged, and then an edit is made
- **THEN** the session's answer equals the plain function's each time, and the new edit is shown at the next call
