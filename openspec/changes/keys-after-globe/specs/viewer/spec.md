## ADDED Requirements

### Requirement: VIEW-034 The movement keys are the camera's while the pointer looks around
While the pointer is captured for looking around, the window SHALL give every key press to the camera and the commands, whether or not a text field of the editor UI has the keyboard focus (`window::key_goes_to_world`), and starting a capture SHALL take the keyboard focus from the widget that has it (`Ui::release_keyboard_focus`), so that a field the user clicked into earlier does not swallow W, A, S, D, Q and E. With the pointer free a key is the camera's unless a text field has the focus, as before. The free camera SHALL move with the movement keys after the overview was shown and left (G and G), with a key held across the overview, and after a click flight landed.

Verify: A
Status: active
Source: owner (2026-10-20, via opus-planner), VIEW-010, VIEW-011, VIEW-013

#### Scenario: A field keeps the focus, then the pointer looks around
- **WHEN** the user clicked into a text field and then starts looking around with the pointer
- **THEN** the focus is taken from the field, W moves the camera, and a key press is the camera's for as long as the pointer is captured

#### Scenario: G and G, and a landing
- **WHEN** the free camera is flown, the overview is shown and left, or a click flight on a marker lands
- **THEN** W moves the free camera afterwards
