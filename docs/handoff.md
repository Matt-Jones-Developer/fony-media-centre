# Handoff

Read this first in every new session. Rewrite it at the end of each session: keep it short, move
finished detail to `docs/archive/` with a one-line pointer.

_Last updated: 2026-11-22._

## Where we are

- **Phase 1, step 1 (library core): done.** Committed and pushed.
- **Step 2 (TMDB metadata and art): slices 1 to 3 of 5 built.** Slices 1 and 2 committed (`a400831`, `4c01e5d`, not pushed). Slice 3 not committed.
- Slice 1: all 23 shows with marker ids fetched (poster, logo, overview), 135 TMDB seasons. No wrong matches.
- Slice 2: all 180 images in `art/` (posters w780, logos w500), 30 MB. Dev **Art** panel via the asset protocol.
- Slice 3: spine colour picked from each poster (`art::spine_colour`, all 23 done). Tape components in `app/src/tape/` (`Spine`, `Sleeve`, `WallPosters`), shared type in `app/src/types/art.ts`. Previewed in the Art panel: click a show, then a spine.
- Code: `app/src-tauri/src/metadata/` (TMDB client, background fetch), `library/db/metadata_store.rs`, migration `002_metadata.sql`. Dev screen has a TMDB line and **Refresh TMDB**.
- Step 1 slice detail: `docs/archive/2026-11-22-library-core-slices-1-5.md`.
- Repo: https://github.com/Matt-Jones-Developer/fony-media-centre, branch `main`.

## Next session, in order

1. **Glitchy checks slice 3** in the Art panel (spines, case, wall posters), then commit.
2. **Slice 4:** exact name and year search for the 7 shows without ids. Run `plan-ticket`.
3. **Slice 5:** series movies.

## Open items

- **`library_overview` command** (ARCHITECTURE section 5) isn't built. Build it with the shelf (step 4), once its shape is known.
- **Watch progress key:** by file or by show/season/episode. Decide in the player step (DECISIONS, "Open").
- **Bundled mpv:** which libmpv build to ship. Needed for packaging (step 7).
- **Waiting on Glitchy:** quit with ⌘Q while something is playing and confirm the mpv window closes.
- **Fonts aren't bundled:** no font is loaded yet (VT323, Kalam, Alfa Slab One, Roboto Mono). Handwritten labels fall back to macOS's Bradley Hand via `--font-hand`. Bundle them locally (offline app) before the home screen (step 3).
- **TMDB key for the packaged app:** dev reads `.env.local`. Decide where it lives in step 7.

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
