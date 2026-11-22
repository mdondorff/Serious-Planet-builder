# Sites Tab UI & Window Wiring for Site Bookmarks and F9 Repros (`VIEW-075`)

## Why
Expose the `VIEW-073` bookmark/coordinate-jump model and `VIEW-074` F9 repro shelf interactively inside `planet editor` via a dedicated left-panel tab (`Tab::Sites`) without altering `WorldDefinition::hash()` or rendering pixels outside egui.

## What Changes
- Add `Tab::Sites` (`"sites"`, `"Sites"`) before `Tab::Keys` in `crates/editor/src/layout.rs` (keeping `Tab::Keys` last per `VIEW-057`) and update `crates/editor/tests/model.rs`.
- Add `BookmarkRow`, `ReproRow`, `SitesView`, `sites_tab`, and `Event` variants in `crates/app/src/panels.rs`.
- Add `--bookmarks FILE` (`Options::bookmarks` and `resolve_bookmarks_path`) in `crates/app/src/lib.rs`, state/event handling and jump resolution in `crates/app/src/editwin.rs`, and capture/camera wiring in `crates/app/src/window.rs`.
- Add `VIEW-075` and Tier-A tests in `crates/app/src/{panels,editwin,lib}.rs`.
