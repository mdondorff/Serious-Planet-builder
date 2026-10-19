## MODIFIED Requirements

### Requirement: AUTH-004 Commands and undo
Edits SHALL be commands over the WorldDefinition (`planet_editor::commands::Editor`): put (a new item whose ULID the command makes from an injected clock and entropy, authored against the active versions and the base hash of the definition now), change (a copy of the item edited, the id and the body kept; a change that leaves the item as it was is not a command) and delete. Every command SHALL write its item files through the item store, undo and redo too, and git is the history: the command log is only the in-memory stack of the session (not persisted, `UNDO_LIMIT` = 10,000 commands, the oldest dropped), cleared of its redo half by a new command. Undo and redo SHALL restore exactly the item before or after the command, so the definition hash is exactly the previous (next) one, for at least 1,000 operations. A drag (`begin_edit`, `edit` steps, `end_edit`) SHALL be one command: its steps change the definition at once and no file, its end writes the file once and records the one command (none when the item is back as it was), `cancel_edit` restores the item and records nothing, and no other command, undo or redo runs while a drag is open. A command that fails (an invalid item, a body the index refuses, an unknown id, a store that cannot write or remove) SHALL change nothing: not the definition, not the files, not the stack. Each command SHALL return the places to refresh, the anchor and reach of the item before and after it, for the footprint find of the tiles. Two additions made on two branches SHALL merge in git without a conflict (one file per item) and load as one definition, whatever the merge order.

Verify: A
Status: active
Source: ADR 0007, ADR 0066 points 6 and 7

#### Scenario: Undo all
- **WHEN** 1,000 random commands (puts, changes, deletes and drags) are applied, then undone one by one, then redone one by one, over memory and over loose files
- **THEN** after each undo the definition hash equals the one before that command, after the 1,000th the hash equals the starting hash and no item or file is left, each redo reaches the hash the command left, and the files always equal the items

#### Scenario: A drag
- **WHEN** an item is dragged in 50 steps and the drag is ended, cancelled, or ends where it began
- **THEN** the steps change the definition and no file, the end writes one file and records one command that one undo takes back whole, a cancel records nothing, and a drag with no net change records nothing and writes nothing

#### Scenario: Failures change nothing
- **WHEN** a put is invalid or refused by the index, a change is invalid, an id is unknown, or the store fails on a put, a delete, an undo or the end of a drag
- **THEN** the definition, the files and the stack are as they were, a failed undo can be tried again and a failed end keeps the drag open

#### Scenario: Merging in git
- **WHEN** two people each add an item, change a different item and one deletes a third on two branches of one repository, and the branches are merged either way
- **THEN** the merge has no conflict and the merged files load to one definition with both additions, both changes and the deletion, with the same hash whichever way it was merged

#### Scenario: Ids
- **WHEN** ids are made from a clock that stands still, goes back, moves on, and from entropy that is all ones
- **THEN** they increase strictly (the same millisecond adds one to the entropy, an overflow takes the next millisecond), the same sources give the same ids and other entropy gives other ids
