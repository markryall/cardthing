# Changelog

All notable changes to cardthing. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## Unreleased

### Added

- Markdown snapshot of the board for browsing a repo on GitHub without cloning
  it. Set `snapshot = ".cards/README.md"` in `.cards.toml` and every card change
  rewrites it: one section per column with a count, every card linked to its
  TOML file with its owner, priority, due date, tags, checklist progress and
  open blockers. Output is deterministic, so it only changes when the board does.
- `cardthing snapshot` regenerates the snapshot on demand.
- `cardthing init` turns the snapshot on for new boards.

### Changed

- The wayfinder tracker doc (`docs/agents/issue-tracker.md`) says how to tell
  charting a new map from working through an existing one.

## 0.1.4 - 2026-11-02

### Fixed

- `append` and `edit` help text: `append` was showing `edit`'s description.

## 0.1.3 - 2026-11-02

### Added

- `edit --claim <who>` takes ownership of a card atomically, failing if
  someone else already owns it.
- `append` adds text to a card's description, optionally at the end of a
  markdown section (`--section`), under a lock, so concurrent sessions never
  lose each other's text. `-` reads the text from stdin.

## 0.1.2 - 2026-11-02

### Added

- Card dependencies: `--blocked-by` on `add`, `--add-blocked-by` and
  `--remove-blocked-by` on `edit`, rejecting unknown cards and cycles. Blocked
  cards show a lock in `list` and on the web board.
- `list --ready` lists cards that can be picked up now: not done, unowned, not
  needing a human, and not blocked.
- Workers skip blocked cards.
- Edit a card's dependencies in the web board's card modal.
- Toggle hiding blocked cards on the web board.
- Time machine: a read-only web board at any commit that touched `.cards`.
- Wayfinder tracker doc (`docs/agents/issue-tracker.md`) and a wayfinder column,
  for using the wayfinder skill with cardthing.
- crates.io metadata, and `cargo install cardthing` documented.

## 0.1.1 - 2026-10-27

### Added

- First release on crates.io.

### Changed

- Dual-licensed under MIT or Apache-2.0.

## 0.1.0 - 2026-07-14

First release.

### Added

- Cards as TOML files in `.cards/`, with status, owner, tags, description,
  checklists, priority and due dates.
- CLI: `init`, `add`, `edit`, `list`, `show`, `rm`, `find`, `stats` and
  `checklist`, plus an interactive shell.
- Web board (`cardthing serve`) with drag and drop, live updates when cards
  change on disk, keyboard shortcuts, card order persisted within columns, and
  column management (add, rename, reorder, delete).
- Configurable board title and columns in `.cards.toml`.
- Agent workers (`cardthing work`) that watch a column and hand cards to an
  agent: graceful shutdown, filesystem-watch wakeups, board-order pickup,
  per-worker jj workspaces, per-field CLI overrides, stale claim lock stealing
  and truncated agent output.
- `edit --clear-owner` to release a crashed worker's claim.
