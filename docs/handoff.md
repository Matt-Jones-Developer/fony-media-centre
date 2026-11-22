# Handoff

Read this first in every new session. Rewrite it at the end of each session: keep it short, move
finished detail to `docs/archive/` with a one-line pointer.

_Last updated: 2026-11-22._

## Where we are

- **Phase 1, step 1 (library core): done.** Committed and pushed.
- **Step 2 (TMDB metadata and art): slices 1 and 2 of 5 built.** Slice 1 committed (`a400831`, not pushed); slice 2 not committed. Design: `docs/ARCHITECTURE.md` "Metadata and art". Decisions: `docs/DECISIONS.md`.
- Slice 1 result: all 23 shows with marker ids fetched (poster, logo and overview for every one), 135 TMDB seasons, all with posters. No wrong matches.
- Slice 2 result: all 180 wanted images downloaded to `art/` (157 posters at w780, 23 logos at w500), 30 MB, no partial files. Dev screen has an **Art** panel served through the asset protocol.
- Code: `app/src-tauri/src/metadata/` (TMDB client, background fetch), `library/db/metadata_store.rs`, migration `002_metadata.sql`. Dev screen has a TMDB line and **Refresh TMDB**.
- Step 1 slice detail: `docs/archive/2026-11-22-library-core-slices-1-5.md`.
- Repo: https://github.com/Matt-Jones-Developer/fony-media-centre, branch `main`.

## Next session, in order

1. **Glitchy checks slice 2:** the Art panel on the dev screen shows posters and logos, and clicking a show shows its season posters. If images show "broken", the asset protocol scope (`$DATA/FonyMediaCentre/art/**` in `tauri.conf.json`) is the first suspect. Then commit.
2. Then slices 3 to 5: spine colour and tape components (adds the `image` crate), exact search for the 7 shows without ids, series movies.

## Open items

- **`library_overview` command** (ARCHITECTURE section 5) isn't built. Build it with the shelf (step 4), once its shape is known.
- **Watch progress key:** by file or by show/season/episode. Decide in the player step (DECISIONS, "Open").
- **Bundled mpv:** which libmpv build to ship. Needed for packaging (step 7).
- **Waiting on Glitchy:** quit with ⌘Q while something is playing and confirm the mpv window closes.
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
