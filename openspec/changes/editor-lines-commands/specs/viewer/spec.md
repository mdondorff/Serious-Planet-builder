## ADDED Requirements

### Requirement: VIEW-028 Lines and regions in the command layer
The command layer SHALL apply a group of steps (put a new item, replace the content of an item with its id, body and versions kept, delete an item) as one command: all steps happen or none does (a step that fails takes back those before it, the files included), one undo takes all of them back to the exact previous definition hash, one redo does them again, and two steps on one item are refused. A group whose steps change nothing SHALL NOT be a command. A new item SHALL carry its points, group, part and mask layer (ADR 0069). A line (`ridge` or `channel` with `width_m` in its parameters) SHALL be put through a point list as one item when it fits the reach of one (VIEW-027), else as the linked parts of the split (one group, consecutive parts, each anchored at its first point with a reach that covers its points and its width), as one command; its points SHALL be changed by making the parts again, changing the items of the line in place, putting the extra and deleting the surplus ones, as one command, a line that becomes one part losing its group; and it SHALL be deleted whole as one command. A region SHALL be put or re-pointed as one item, refused with the reason (the 40 km limit, the crossing edges) when its outline is not valid (VIEW-027); a refusal changes nothing.

Verify: A
Status: active
Source: opus-planner (2026-10-19), ADR 0069, AUTH-004, VIEW-027

#### Scenario: A split line
- **WHEN** a line of 150 km is put and then undone and redone
- **THEN** it is four or more parts in one group, one undo removes all and restores the starting hash, one redo restores the end hash

#### Scenario: Re-splitting
- **WHEN** a point of the line moves, the line is shortened to two points, and then lengthened
- **THEN** each is one command; the parts keep their ids where they can, the surplus parts are deleted or the extra ones put, and one undo restores the exact previous hash

#### Scenario: A failure halfway
- **WHEN** the store fails on the third part of a split line, or a step is invalid, or two steps name one item
- **THEN** the definition, the files and the undo stack are as they were
