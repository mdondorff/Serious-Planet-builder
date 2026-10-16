## Why
ADR 0037 (accepted, owner 2026-10-15) lets the first goldens of M5 classes be blessed under policy instead of by the owner's `/bless`. `golden-review` does not implement that path: it escalates every first golden and every lit view, so the four lit goldens of M5 item 2 (PR #199) wait for the owner. Opus-planner (owner policy 2026-10-15) asked for the tool to learn ADR 0037's conditions, and for the four goldens to be reviewed with it.

## What Changes
- `testkit::review`: the M5 window as a constant (`ADR_0037_M5_CLASSES`: `lit`, `sky`, `water`; the owner revisits it at the M5 gate), the class names `sky` and `water`, the verdict `AUTO-M5`, and `classify_m5_first`, which applies only to the first golden of an M5 class and requires all of ADR 0037 point 1: (a) the declared bound tests are green in this run's nextest reports, (b) the declared debug view agrees across the adapters that took part (ADR 0017's cross-adapter rule), (c) the golden is declared under `Expected golden changes:`. Anything else escalates exactly as before.
- Proposals gain an `ADR 0037 conditions:` section (`- <golden>: debug-view=<golden>; bound-tests=<test>, <test>`).
- `cargo xtask golden-review` gains `--tests <junit.xml>` (repeatable). It checks the named bound tests in the given reports and never takes the proposal's word. Adapters that delivered no candidates are printed and never count as agreeing.
- The ADR 0017 driver gate is unchanged: a candidate from another driver than the recorded one escalates, first golden or not (ADR 0037 does not relax it).
- The `golden-review` skill, the testing spec (TEST-022 added, TEST-016 cross-referenced), `docs/status.md` (the entry telling the owner to `/bless` the four lit goldens is replaced by the outcome of this review).
- The four lit goldens are reviewed with the new tool.

## Classes and bounds (ADR 0035)
No field and no shader value is added; this change is test tooling. Its inputs are decided by numbers and names (pixel counts, deltas, test outcomes), not by new floating-point behaviour.

Expected golden changes:
- lit_orbit_ev15: new (first golden): the lit globe at a fixed EV100 15, ACES and sRGB.
- lit_aerial20km_ev15: new (first golden): the lit aerial view at a fixed EV100 15.
- lit_ground1m_ev15: new (first golden): the lit ground view at 1 m at a fixed EV100 15.
- lit_orbit_ev_bands: new (first golden): the false-colour EV-band debug view of the orbit frame.

ADR 0037 conditions:
- lit_orbit_ev15: debug-view=lit_orbit_ev_bands; bound-tests=lit_terrain_radiance_has_known_answers, the_aces_fit_and_the_srgb_encode_have_known_answers, the_lit_radiance_matches_the_f64_reference_within_its_declared_bound, the_tone_map_matches_the_f64_reference_within_one_step_on_a_ramp, the_lit_frame_is_tone_mapped_from_its_scene_target_within_one_step
- lit_aerial20km_ev15: debug-view=lit_orbit_ev_bands; bound-tests=lit_terrain_radiance_has_known_answers, the_aces_fit_and_the_srgb_encode_have_known_answers, the_lit_radiance_matches_the_f64_reference_within_its_declared_bound, the_tone_map_matches_the_f64_reference_within_one_step_on_a_ramp, the_lit_frame_is_tone_mapped_from_its_scene_target_within_one_step
- lit_ground1m_ev15: debug-view=lit_orbit_ev_bands; bound-tests=lit_terrain_radiance_has_known_answers, the_aces_fit_and_the_srgb_encode_have_known_answers, the_lit_radiance_matches_the_f64_reference_within_its_declared_bound, the_tone_map_matches_the_f64_reference_within_one_step_on_a_ramp, the_lit_frame_is_tone_mapped_from_its_scene_target_within_one_step
- lit_orbit_ev_bands: debug-view=lit_orbit_ev_bands; bound-tests=the_false_colour_bands_are_whole_stops_below_saturation, the_false_colour_view_shows_the_ev_band_of_the_exposed_luminance, the_gpu_histogram_counts_what_the_cpu_reference_counts, the_aces_fit_and_the_srgb_encode_have_known_answers

## Out of scope
Changes to existing non-M5 goldens (ADR 0017 stays exactly), changes to existing lit goldens (still escalate), any change to the driver gate. The WARP driver drift is reported, not decided here (see the PR).
