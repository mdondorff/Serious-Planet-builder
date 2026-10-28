## Why
The owner wants to switch looks live to compare them (hills-b): the stills' `[look]` (ground detail and the material curves) lives only in a poses file, so the editor cannot show it.

## What Changes
- The look file gains a `ground` table (detail strength and the material curves; LOOK-011, schema version 5 with an identity migration and a fixture) and the viewer gets F8 to cycle the default ground look and the file's (VIEW-060, Keys tab entry).
- A sample look file, `docs/milestones/M8/look-review-2b/sample-driver-look.look.toml`, holds the sample-driver-look curves; a test pins it to the sample poses file.

## Impact
- Spec: LOOK-001, LOOK-002, LOOK-011, VIEW-060. Code: `frame/look/{file,materials}.rs`, `look.rs`, `viewer.rs`, `keytable.rs`, `app/window.rs`. No golden moves (the test-render plans do not read the table).
