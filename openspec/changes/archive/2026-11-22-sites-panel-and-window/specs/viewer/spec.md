## ADDED Requirements

### Requirement: VIEW-075 The Sites tab and window wiring for bookmarks and repro captures
The editor layout SHALL include `Tab::Sites` (`sites`) right before `Tab::Keys` so `Keys` remains last. The Sites tab (`planet::panels`) SHALL display the live `CameraReadout` and `--site LAT,LON` string, a `LAT,LON[,ALT]` jump box, a bookmark shelf (`Save view`, inline rename, `Jump`, `Delete`), and an F9 Repro Shelf (`Capture F9`, `Refresh`, compatibility badge, `Jump`, `Bookmark`, replay CLI command). `EditRuntime` and `window` SHALL load/save `BookmarkShelf` atomically outside `WorldDefinition` (`--bookmarks FILE`, `PLANET_BOOKMARKS_FILE`, else `<items>/bookmarks.toml` or `target/review/bookmarks.toml`; in-memory under `--smoke`), switch to `CameraMode::Free` on jumps, and refresh repro entries after captures.

Verify: A
Status: active
Source: companion backlog 1C (2026-11-22), VIEW-057, VIEW-073, VIEW-074

#### Scenario: Sites panel interaction and window wiring
- **WHEN** the Sites tab is opened in the headless panel rig, coordinates or bookmarks are submitted, or repro actions run in `EditRuntime`
- **THEN** jump, save, rename, delete, capture, refresh, and repro-bookmark events fire without modifying `WorldDefinition::hash()` and resolve to free-camera jumps above ground
