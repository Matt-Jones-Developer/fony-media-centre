# Decisions

Choices that are expensive to undo. Newest first. Each entry: what, why, what was rejected.

## 2026-11-22: Library core (approved)

Settled with Glitchy:

- **`rusqlite`, with SQLite compiled in.** Synchronous and simple; the queries are tiny, so async buys nothing. Rejected: `sqlx` (needs an async runtime and a database at build time).
- **Cartoons only in Phase 1.** Other folders come later as `library_roots` rows.
- **Unmarked folders go through the review queue.** He-Man and Looney Tunes are not tidied first.
- **`notify` for file and drive events.** Approved.
- **`serde_norway` for YAML.** The maintained continuation of `serde_yaml`. A marker it can't read falls back to "unmarked".

Design:

- **Paths stored relative to the drive, drives found by volume UUID.** A drive renamed or mounted elsewhere still matches. Rejected: absolute paths (break when the mount name changes).
- **Plain integer ids.** The library is local and personal; nothing outside the Mac guesses them. Rejected: UUIDs (no benefit here, harder to read when debugging).
- **Only `db.rs` writes to SQLite.** One owner for every table.
- **Files gone from an online drive are deleted; offline drives are never touched.** Keeps the shelf honest without losing titles on an unplugged drive.
- **The database folder is built by hand** (`data_dir()/FonyMediaCentre`), because Tauri's `app_data_dir()` would name it after the bundle id.
- **No tapes, overrides or watch progress tables yet.** Added by migration in their own steps.
- **Open: watch progress key.** Keyed by file, it is lost when tapehead renames a file. Keyed by show, season and episode, it survives. Decide in the player step.
