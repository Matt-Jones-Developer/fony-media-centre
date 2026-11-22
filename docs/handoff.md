# Handoff

Read this first in every new session. Rewrite it at the end of each session: keep it short, move
finished detail to `docs/archive/` with a one-line pointer.

_Last updated: 2026-11-22._

## Where we are

- **Phase 1, step 1 (library core): slices 1 to 5 built.** Slice 6 is the last one in step 1.
- Design: `docs/ARCHITECTURE.md` (Library core). One-way decisions: `docs/DECISIONS.md`.
- What each slice did and its results: `docs/archive/2026-11-22-library-core-slices-1-5.md`.
- Slices 1 to 5 are committed and pushed.
- Repo: https://github.com/Matt-Jones-Developer/fony-media-centre, branch `main`.

## Next session, in order

1. **Confirm the unplug test** (slice 5) if it hasn't been done yet: eject Glitchy_2TB with the app open. Within about 3 seconds the panel should show offline, and back online when it's plugged in again.
   - If it doesn't update, replace the `/Volumes` file watch in `app/src-tauri/src/watch.rs` with a check of `/Volumes` every 2 seconds.
2. **Slice 6: looser name rules.** Run `plan-ticket` first. Unmarked, untidy folders (562 files) currently land in the review queue:
   - `He-Man and the Masters of the Universe S01-S02 (1983-)`: 130 files.
   - `Looney Tunes - Golden Collection`: about 404 files.
   - `The Pink Panther Cartoon Collection/The BEST of the Pink Panther Collection`: 28 files, shown as-is (`mode: keep`). These need placing in file name order, not renaming.
   - The reference is tapehead's loose rules: `tools/tapehead/src/tapehead/parse.py` (`parse_path`). Port only what these folders need.
3. Then stop and summarise step 1 for Glitchy before step 2 (TMDB metadata and art).

## Open items

- **`library_overview` command** (ARCHITECTURE section 5) isn't built. Build it with the shelf (step 4), once its shape is known.
- **Watch progress key:** by file or by show/season/episode. Decide in the player step (DECISIONS, "Open").
- **Bundled mpv:** which libmpv build to ship. Needed for packaging (step 7).
- **Waiting on Glitchy:** quit with ⌘Q while something is playing and confirm the mpv window closes.

## How to run and check

- App: `cd app && pnpm tauri dev`. The dev screen has the **Library drives** panel.
- Tests: `cd app/src-tauri && cargo test`. Add `-- --include-ignored --nocapture` with the drive plugged in.
- Database: `~/Library/Application Support/FonyMediaCentre/library.db`. Safe to delete: the app rebuilds it on the next scan (the folder you added has to be added again).

## Gotchas

- **`cargo metadata` / "No such file or directory" from `pnpm tauri dev`:** that terminal was opened before Rust was installed. Run `source ~/.cargo/env`, or open a new tab (`~/.zshenv` loads Rust).
- **Rust naming:** the owner's "no all-caps" rule clashes with Rust's style for `const` and `static`. Regexes live in a `TidyRules` struct built once per scan, so no statics are needed.
- **mpv's Homebrew build has no `dvd://`.** It rules out Homebrew mpv for any future DVD work.
- **zsh arrays start at 1.** Prefer Python for multi-step file scripts.

## Archive

- `docs/archive/2026-11-22-phase-0c.md`: Phase 0c results, early decisions, library tidy-up (stragglers, D&D, Little Miss).
- `docs/archive/2026-11-22-library-core-slices-1-5.md`: library core slices 1 to 5.
- `docs/archive/kickoff-prompt-phase-0c.md`: the Phase 0c kickoff prompt.
