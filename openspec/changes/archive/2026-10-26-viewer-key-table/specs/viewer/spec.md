## ADDED Requirements

### Requirement: VIEW-055 One key table
`planet_frame::keytable::KEYS` SHALL be the only list of the keys the window takes: each entry has the `Debug` names of its `KeyCode`s (none for a chord or the mouse), its label, its action (a viewer command, a key of the editor mode, a held key or other), its group (editing, camera and movement, view and debug), the mode that accepts it (either, editor off, editor on), its help text and, where it has one, the state shown beside it. `command_for`, `edit_key_for` and `held_for` SHALL answer from it, the window's `key_command`, `edit_key` and `set_held` SHALL call them, and the usage text (`KEYS_HELP`, `EDIT_KEYS_HELP`), the F1 list and `rows` (the rows of the panel, with their state) SHALL be made from it. Every viewer command and every key of the editor mode SHALL have an entry, no code SHALL be listed twice for the same kind of action, a key the window handles SHALL be in the table and a key the table lists SHALL be handled, and a mode SHALL list exactly the entries it accepts.

Verify: A
Status: active
Source: opus-planner (2026-10-25), owner: the keys visible in the app at all times and always current; VIEW-003, VIEW-022, VIEW-042, VIEW-054

#### Scenario: Handled and listed
- **WHEN** every key a person can press that the window might bind is looked up in the handling and in the table
- **THEN** a key is handled exactly when it is listed, a command's keys are the ones the lookup finds, and a misspelt name in the table is caught

#### Scenario: The modes
- **WHEN** the entries of the editor off and the editor on are listed
- **THEN** the viewer lists Esc (quit) and no editing key, the editor lists the tools, Ctrl+Z, F1 or ? and Esc (step back), each help text and row come from the same entries, and a row with a state shows it (vsync on, the shade view, the sun)
