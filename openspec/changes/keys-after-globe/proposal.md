## Why
The owner (2026-10-20, via opus-planner) found that after G (globe view), or another switch into camera control, the mouse look works but the movement keys do nothing until Esc and a new grab.

## What Changes
- The viewer is not the cause (tier A: the free camera moves with W after G and G, with a key held across the overview, and after a click flight landed). The window gate is: a key press is dropped while egui reports that a text field has the keyboard, and a field the user clicked into (the search box, a name, a value) keeps that focus while the pointer is captured, because egui gets no pointer events then and nothing else takes the focus.
- While the pointer is captured for looking around, every key is the camera's and the commands' (`key_goes_to_world`), and starting a capture takes the keyboard focus from the field (`Ui::release_keyboard_focus`).
- VIEW-034; tier-A tests: the viewer cycle, the key gate, the focus of a text field and its release.

## Classes and bounds (ADR 0035)
No field, no shader value and no golden is touched.

## Out of scope
Whether the real window now behaves (no display here; the owner checks).
