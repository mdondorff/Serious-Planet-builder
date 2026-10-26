## ADDED Requirements

### Requirement: VIEW-057 The Keys tab
The editor's left panel SHALL have a tab `Keys` (last in the default layout, named `keys` in the layout file) that lists the keys the window accepts in the editor mode, the rows of `frame::keytable::rows`, which the window SHALL give it each frame: grouped under the table's group titles, each key once with its label, the state it is in in brackets when it has one (shade view, ground detail, sun time, and the like) and its help line.

Verify: A
Status: active
Source: VIEW-055, the owner (2026-10-25)

#### Scenario: Every key
- **WHEN** the Keys tab is selected with the rows of the editor mode
- **THEN** every row of the table is listed exactly once under its group, a key with a state shows it, and without rows the tab lists none
