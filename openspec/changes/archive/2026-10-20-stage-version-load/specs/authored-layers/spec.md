## MODIFIED Requirements

### Requirement: AUTH-002 Recorded versions
Every authored item SHALL record the generator version, the stage version and the hash of the base definition (`WorldDefinition::base_hash`) it was authored against, and checking an item against other active versions (`Item::check_versions`) SHALL fail with a message that names every difference (item value and active value) and the options: pin the definition to the item's generator version, freeze the region, or re-author the item. (Pinning and freezing themselves are AUTH-003.) An item authored against an OLDER stage version than the active one is not a difference: it loads, and the editor's load (`Editor::load_report`) SHALL re-key it under the active stage version (its recorded stage version becomes the active one, so its item hash changes and the stamp it makes does not), write it back to its file and name it in the report; an item of a NEWER stage version than the build's is a difference and is refused. A stage-version bump therefore never makes authored work unloadable.

Verify: A
Status: active
Source: CON-09, ADR 0007, opus-planner (2026-10-20)

#### Scenario: Version mismatch
- **WHEN** an item authored against generator version 4, stage version 2 and one base hash is checked under generator version 5, under another base hash, under stage version 1, and under all of them
- **THEN** each check fails naming the item, the recorded and the active values and the three options, the stage version 1 check naming it as newer, and the last names three differences; the item's own versions pass

#### Scenario: An older stage loads
- **WHEN** an item authored against stage version 2 is checked under stage version 3, and the files of two such items are loaded by an editor of stage version 3
- **THEN** the check passes, the editor loads both, re-keyed under version 3 and written back, names them in the report, the item hash changes and the stamp's content hash does not, and a second load restages nothing; an editor of stage version 1 refuses the re-keyed files naming the newer stage
