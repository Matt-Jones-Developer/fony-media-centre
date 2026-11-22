# Handoff

Read this first in every new session. Rewrite it at the end of each session: keep it short, move
finished detail to `docs/archive/` with a one-line pointer.

_Last updated: 2026-11-22._

## Where we are

- **Phase 0c is done.** Phase 1 (MVP Mac app) is next.
- Project folder: `~/builds/fony-media-centre` (renamed from `media-centre` on 2026-11-22).
- Repo: https://github.com/Matt-Jones-Developer/fony-media-centre, branch `main`, first commit pushed.

## Next action

Start **Phase 1, step 1: library core** (`CLAUDE.md` → Build plan → Phase 1).

1. Run the `system-design` skill first. This is a new subsystem with a schema: scanner, SQLite, drive watching.
2. Settle the open questions below with Glitchy.
3. Then `plan-ticket` for the first slice: the scanner reading the tidied Cartoons folders and their markers into SQLite.

## Open questions for Phase 1

- **SQLite crate:** `rusqlite` (simple, synchronous) or `sqlx` (async, compile-time checked queries). CLAUDE.md asks for a choice with the reason explained.
- **Bundled mpv:** which libmpv build to ship. It's needed for packaging and for previews playing inside the on-screen TV.
- **Untidied folders:** He-Man and Looney Tunes have no tapehead marker yet. Tidy them with tapehead before the scanner work, or let the review queue catch them?

## Waiting on Glitchy

- Quit the app with ⌘Q while something is playing, and confirm the mpv window closes. The automated test covers the same cleanup code, but not a real quit.

## Phase 0c results (2026-11-22)

How to run and test: `app/README.md`.

| Check | Result |
| --- | --- |
| Plays mkv, mp4, avi off the SSD | Passes |
| Hardware decoding | `videotoolbox` for H.264 (mkv, mp4). Software for Xvid avi and DVD MPEG-2 mkv: Apple's decoder doesn't support those codecs |
| Shaders switch live | Passes (crt-lottes with Trinitron, Portable, Arcade, Clean presets) |
| Volume UUID | Passes: Glitchy_2TB = `D2946B44-EEF5-3A81-A7BE-648CD3312449` |
| No mpv left after quit | Passes in the drive test |

The drive test is `cargo test -- --include-ignored --nocapture` in `app/src-tauri`.

## Decisions this session

- **Home screen art** (recorded in `docs/spec.md` §5):
  - front case: the TMDB poster
  - spine: TMDB title logo on a colour taken from the poster
  - wall posters: show poster left, season poster right
- **No DVD support.** All VIDEO_TS rips were converted to mkv.
- **Bundle mpv with libmpv**, so previews can play inside the TV.
- **On launch, kill any mpv left behind** by a crashed session.
- **Package manager:** pnpm. **Node:** 26 LTS. **Rust:** 1.100 via rustup.

## Library work done this session

- **Stragglers:** 108 leftover episodes filed into their tidy show folders, using a hand-checked plan applied with tapehead.
  - The undo log is `Cartoons/.tapehead/undo-20261122-135409.jsonl`.
  - The South Park pilot was filed separately (`undo-20261122-140145.jsonl`).
- **Clean:** the old emptied folders were removed with `tapehead clean`.
- **Dungeons & Dragons:** the four DVD folders were converted to 27 lossless MKV episodes plus 3 extras in Specials, all checked against the on-screen titles.
  - Glitchy then removed the DVD originals.
  - Unsure: the two interview extras' names may be swapped.
- **Little Miss:** the 108-minute compilation was split into 13 MP4 episodes, matched to TMDB id 126227's order.

## Gotchas

- **Rust commands:** open a fresh terminal after installing Rust, or run `source ~/.cargo/env` first.
- **First build after the folder rename** recompiles everything; expect a few minutes.
- **mpv's Homebrew build has no `dvd://`.** It doesn't matter now, but it rules out Homebrew mpv for any future DVD work.
- **zsh arrays start at 1.** It bit a shell loop this session; prefer Python for multi-step file scripts.
