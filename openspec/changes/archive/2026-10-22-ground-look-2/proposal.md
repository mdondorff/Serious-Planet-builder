# ground-look-2

Why: the owner could not tell rock from grass in the ground detail (2026-10-22). The detail only multiplied the climate's albedo, so every material read as the same colour.

What: with the detail on, land takes its materials' albedo (rock grey, grass green, sand pale, soil dark brown) in place of the climate's (REND-037); the rock's crack term and the 25 cm and 3 cm octave amplitudes are stronger. No new octave. Strength 0 and the debug views are unchanged, so no golden moves.

Expected golden changes: none.

Review stills: `cargo run --release -p planet-render --example look_stills` writes `target/review/ground-detail/gt_eye<h>m_<variant>.png`.
