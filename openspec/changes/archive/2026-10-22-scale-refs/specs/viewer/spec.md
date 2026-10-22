## ADDED Requirements

### Requirement: VIEW-050 F7 drops scale references
F7 SHALL drop plain boxes of known size on the ground in front of the camera, and take them away when pressed again (`Command::ToggleScaleRefs`, off at the start). `planet_frame::scalerefs::drop_references` decides them from the camera, its heading and the ground height: a chair 0.9 m high, a house 8 m wide, a boat 10 m long, a tower 291 m high and a block of 10 x 10 boxes 100 m apart (1 km across). Each stands ahead of the camera on the lowest ground under its footprint, none overlaps another, and the same camera gives the same drop. The window title names the references while they are dropped; the keys help names F7.

Verify: A
Status: active
Source: owner (2026-10-22, via opus-planner): scale references to judge the ground detail by, VIEW-046, REND-036

#### Scenario: The sizes and the places
- **WHEN** a drop is made at a camera with a flat ground, and at one with a ground that rises ahead
- **THEN** the boxes have the named sizes, the block has 100 boxes spanning 950 m, each box stands on the lowest ground under its footprint ahead of the camera and none overlaps another, and a camera looking straight down still gets the drop

#### Scenario: The key
- **WHEN** F7 is pressed twice in a viewer
- **THEN** the references are asked for, then not, and the help text and the editor's help name F7
