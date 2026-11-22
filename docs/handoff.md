# Handoff

Read this first in every new session. Rewrite it at the end of each session: keep it short, move
finished detail to `docs/archive/` with a one-line pointer.

_Last updated: 2026-11-22._

## Where we are

- **Phase 1, step 1 (library core): done.** Committed and pushed.
- **Phase 1, step 2 (TMDB metadata and art): done, all 5 slices**, committed (`a400831`, `4c01e5d`, `be16405`, `2e950c8`, `0ea6263`). All pushed.
- **Label style setting (Graf / Regular): built and committed.** Graf = Sedgwick Ave Display (default), Regular = Kalam. Both bundled in `app/src/assets/fonts/` (Kalam regular and bold). Movie spines have no caption; sizes were raised for both styles at Glitchy's request. Saved in the `settings` table (migration 004) via `label_style` / `set_label_style`; `app/src/settings/useLabelStyle.ts` sets `data-label-style` on the page root; toggle in the Art panel header.
- Design: `docs/ARCHITECTURE.md` "Metadata and art". Decisions: `docs/DECISIONS.md`.
- Results on the drive today:
  - 27 of 30 shows matched (23 by marker id, 4 by exact search). Unmatched on purpose: Little Miss (two TMDB entries, same name and year), Looney Tunes (no year), The Pink Panther (1964, nothing found).
  - All 4 series movies matched (M.A.S.K. The Movie has no TMDB logo, so its spine is handwritten).
  - Art cached in `~/Library/Application Support/FonyMediaCentre/art/` (posters w780, logos w500). Every matched show and movie has a spine colour.
- Code: `app/src-tauri/src/metadata/` (`tmdb.rs` the only TMDB client, `matching.rs`, `art.rs`, `mod.rs`), `library/db/metadata_store.rs`, migrations 002 and 003. React tape parts: `app/src/tape/` (`Spine`, `Sleeve`, `WallPosters`, `seasonLabel`), type `app/src/types/art.ts`. Dev screen: TMDB line with unmatched names, **Refresh TMDB**, and the **Art** panel (click a show, then a spine).
- Step 1 slice detail: `docs/archive/2026-11-22-library-core-slices-1-5.md`.
- Repo: https://github.com/Matt-Jones-Developer/fony-media-centre, branch `main`.

## Next session, in order

1. **Bundle the remaining fonts** the same way (VT323, Alfa Slab One, Roboto Mono bold, Inter) into `app/src/assets/fonts/`.
2. **Step 3: home screen.** Run `plan-ticket` against `docs/mockups/Main.dc.html` and `Boot.dc.html`; load `ux-laws` first. Needs `library_overview` (ARCHITECTURE section 5), which can absorb `series_art`.

## Open items

- **`library_overview` command** (ARCHITECTURE section 5) isn't built. Build it with the shelf (step 4), once its shape is known.
- **Watch progress key:** by file or by show/season/episode. Decide in the player step (DECISIONS, "Open").
- **Bundled mpv:** which libmpv build to ship. Needed for packaging (step 7).
- **Waiting on Glitchy:** quit with ⌘Q while something is playing and confirm the mpv window closes.
- **Fonts:** Kalam and Sedgwick Ave Display are bundled. VT323, Alfa Slab One, Roboto Mono and Inter are not yet (needed for the home screen).
- **Season numbering:** Dexter's Lab and Pinky and the Brain use local season numbers, so a season poster may belong to a different TMDB season. Fix in the editor.
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
